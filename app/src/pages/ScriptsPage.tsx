// Màn "Kho kịch bản" — dựng theo `references/lvc-ui-design/screens/Library.dc.html` +
// `LibraryFree.dc.html`.
//
// "Kịch bản" ở đây LÀ WORKFLOW (quyết định của chủ dự án 2026-10-14): một thứ người dùng dựng và
// lưu chỉ có MỘT loại duy nhất. Trang này là kho để xem/quản lý, ba nguồn:
//
//   Của tôi   — workflow đã lưu (`workflow_list`); bấm là mở lên canvas.
//   Công khai — mẫu dựng sẵn trong app (`template_list("public")`).
//   Miễn phí  — workflow cộng đồng trên GitHub (`template_list("remote")`).
//
// Mở lên canvas KHÔNG gọi thẳng graph store được: store đó nằm sau `GraphStoreProvider` của từng
// tab workflow, trang này ở ngoài. Nên đặt một yêu cầu vào `useOpenWorkflow`, `WorkflowTabs` nhận
// và nạp vào tab đang xem (xem `store/openWorkflow.ts`).
import { useCallback, useEffect, useMemo, useState } from "react";

import { templateGet, templateList, workflowGet, workflowList } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import { useOpenWorkflow } from "../store/openWorkflow";
import { useUiStore } from "../store/uiStore";
import WorkflowCard, { coverFor } from "../components/WorkflowCard";
import { Button, SearchInput } from "../ui/primitives";
import type { TemplateMeta, TemplateScope, WorkflowMeta } from "../ipc/types";

const TABS: ReadonlyArray<readonly [TemplateScope, string]> = [
  ["private", "Của tôi"],
  ["public", "Công khai"],
  ["remote", "Miễn phí"],
];

const SUBTITLE: Record<TemplateScope, string> = {
  private: "Workflow bạn đã lưu — bấm để mở lại trên canvas.",
  public: "Workflow mẫu dựng sẵn trong ứng dụng — dùng ngay, không cần mạng.",
  remote: "Workflow cộng đồng publish lên GitHub — tải về để dùng và sửa trong “Của tôi”.",
};

const SEARCH_HINT: Record<TemplateScope, string> = {
  private: "Tìm theo tên workflow…",
  public: "Tìm theo tên hoặc mô tả…",
  remote: "Tìm theo tên hoặc mô tả…",
};

/** Tiêu đề nhóm + gạch ngang, đúng kiểu bản thiết kế. */
function GroupHeading({ children }: { children: string }) {
  return (
    <h2 className="m-0 flex items-center gap-2.5 text-[11px] font-semibold tracking-[0.1em] text-ink3 uppercase">
      {children}
      <span className="h-px flex-1 bg-line" />
    </h2>
  );
}

function CardGrid({ children }: { children: React.ReactNode }) {
  return (
    <div className="grid gap-3.5 [grid-template-columns:repeat(auto-fill,minmax(220px,1fr))]">
      {children}
    </div>
  );
}

export default function ScriptsPage() {
  const [scope, setScope] = useState<TemplateScope>("private");
  const [query, setQuery] = useState("");
  const [mine, setMine] = useState<WorkflowMeta[] | null>(null);
  const [templates, setTemplates] = useState<TemplateMeta[] | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const requestOpen = useOpenWorkflow((s) => s.request);
  const setActiveTab = useUiStore((s) => s.setActiveTab);

  const load = useCallback(async (): Promise<void> => {
    setError(null);
    if (scope === "private") {
      setMine(await workflowList());
      return;
    }
    setTemplates(await templateList(scope));
  }, [scope]);

  useEffect(() => {
    let cancelled = false;
    // Đổi tab thì xoá danh sách cũ, nếu không người dùng thấy thẻ của tab TRƯỚC trong một nhịp.
    if (scope === "private") {
      setMine(null);
    } else {
      setTemplates(null);
    }
    load().catch((e: unknown) => {
      if (!cancelled) {
        setError(errorText(e));
      }
    });
    return () => {
      cancelled = true;
    };
  }, [scope, load]);

  /** Nạp đồ thị lên canvas rồi chuyển sang tab Workflow. */
  function openOnCanvas(req: Parameters<typeof requestOpen>[0]): void {
    requestOpen(req);
    setActiveTab("workflow");
  }

  async function handleOpenMine(meta: WorkflowMeta): Promise<void> {
    setBusyId(meta.id);
    setError(null);
    try {
      const doc = await workflowGet(meta.id);
      openOnCanvas({
        graph: doc.graph,
        workflowId: doc.meta.id,
        workflowName: doc.meta.name,
        saved: true,
      });
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusyId(null);
    }
  }

  async function handleUseTemplate(meta: TemplateMeta): Promise<void> {
    setBusyId(meta.id);
    setError(null);
    try {
      const doc = await templateGet(meta.id);
      // Dùng mẫu = canvas MỚI mang đồ thị đó, CHƯA phải một workflow đã lưu — người dùng tự đặt
      // tên khi bấm Lưu. Không mượn id của mẫu làm id workflow.
      openOnCanvas({
        graph: doc.graph,
        workflowId: "",
        workflowName: doc.meta.name,
        saved: false,
      });
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusyId(null);
    }
  }

  const q = query.trim().toLowerCase();
  const visibleMine = useMemo(
    () => (mine ?? []).filter((m) => q === "" || m.name.toLowerCase().includes(q)),
    [mine, q],
  );
  const visibleTemplates = useMemo(
    () =>
      (templates ?? []).filter(
        (t) =>
          q === "" ||
          t.name.toLowerCase().includes(q) ||
          t.description.toLowerCase().includes(q),
      ),
    [templates, q],
  );

  /** Gom mẫu theo nhóm, giữ thứ tự xuất hiện — khớp cách bản thiết kế trình bày. */
  const groups = useMemo(() => {
    const order: string[] = [];
    const by = new Map<string, TemplateMeta[]>();
    for (const t of visibleTemplates) {
      const list = by.get(t.category);
      if (list) {
        list.push(t);
      } else {
        order.push(t.category);
        by.set(t.category, [t]);
      }
    }
    return order.map((name) => ({ name, rows: by.get(name) ?? [] }));
  }, [visibleTemplates]);

  const loading = scope === "private" ? mine === null : templates === null;
  const count = scope === "private" ? visibleMine.length : visibleTemplates.length;

  return (
    <div className="flex h-full min-w-0 flex-col overflow-hidden">
      <header className="flex flex-col gap-3.5 border-b border-line px-6 pt-5 pb-4">
        <div>
          <h1 className="text-[20px] font-bold tracking-[-0.01em] text-fg">Kho kịch bản</h1>
          <p className="mt-1 text-[13px] text-ink3">{SUBTITLE[scope]}</p>
        </div>

        <div
          role="tablist"
          aria-label="Nguồn kịch bản"
          className="inline-flex gap-0.5 self-start rounded-[11px] border border-line2 bg-fill p-[3px]"
        >
          {TABS.map(([s, label]) => {
            const on = s === scope;
            return (
              <button
                key={s}
                type="button"
                role="tab"
                aria-selected={on}
                onClick={() => setScope(s)}
                className={
                  "inline-flex min-h-9 items-center rounded-lg px-4 text-[13px] " +
                  (on ? "bg-primary font-bold text-primary-fg" : "text-muted hover:text-fg")
                }
              >
                {label}
              </button>
            );
          })}
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <SearchInput
            value={query}
            onChange={setQuery}
            label="Tìm workflow"
            placeholder={SEARCH_HINT[scope]}
          />
          <span className="text-[13px] text-ink3">
            {loading ? "Đang tải…" : `${count} workflow`}
          </span>
        </div>
      </header>

      <div className="flex flex-1 flex-col gap-6 overflow-auto px-6 pt-5 pb-7">
        {error ? <p className="text-[12.5px] text-rose">{error}</p> : null}

        {loading ? null : scope === "private" ? (
          visibleMine.length === 0 ? (
            <p className="text-[13px] leading-[1.6] text-ink3">
              {mine?.length === 0
                ? "Chưa lưu workflow nào. Dựng trên canvas ở tab Workflow rồi bấm Lưu (Ctrl+S) để cất vào đây."
                : "Không có workflow nào khớp."}
            </p>
          ) : (
            <CardGrid>
              {visibleMine.map((meta) => (
                <WorkflowCard
                  key={meta.id}
                  title={meta.name}
                  description={`Sửa lần cuối ${new Date(meta.updatedAt * 1000).toLocaleString("vi-VN")}`}
                  cover={coverFor(meta.id)}
                  nodeCount={meta.nodeCount}
                  action={
                    <Button
                      variant="primary"
                      disabled={busyId !== null}
                      onClick={() => void handleOpenMine(meta)}
                    >
                      {busyId === meta.id ? "Đang mở…" : "Mở trên canvas"}
                    </Button>
                  }
                />
              ))}
            </CardGrid>
          )
        ) : groups.length === 0 ? (
          <p className="text-[13px] text-ink3">
            {scope === "remote"
              ? "Chưa tải được kho cộng đồng, hoặc chưa có workflow nào được publish."
              : "Không có workflow nào khớp."}
          </p>
        ) : (
          groups.map((g) => (
            <section key={g.name} className="flex flex-col gap-3">
              <GroupHeading>{g.name}</GroupHeading>
              <CardGrid>
                {g.rows.map((meta) => (
                  <WorkflowCard
                    key={meta.id}
                    title={meta.name}
                    description={meta.description}
                    cover={coverFor(meta.id, meta.thumbnail)}
                    nodeCount={meta.nodeCount}
                    topLeftBadge={scope === "remote" ? meta.category : undefined}
                    footer={
                      scope === "remote" && meta.version !== "" ? (
                        <span className="font-mono text-[11px] text-accent">
                          {meta.version}
                        </span>
                      ) : undefined
                    }
                    action={
                      <Button
                        variant="primary"
                        disabled={busyId !== null}
                        onClick={() => void handleUseTemplate(meta)}
                      >
                        {busyId === meta.id ? "Đang mở…" : "Dùng workflow"}
                      </Button>
                    }
                  />
                ))}
              </CardGrid>
            </section>
          ))
        )}
      </div>
    </div>
  );
}
