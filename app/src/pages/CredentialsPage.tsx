// Man "API key" — dung theo ban ban giao `references/lvc-ui-design/screens/ApiKeys.dc.html`.
// Chi hien credential `kind === "api_key"`; tai khoan dang nhap/cookie o man "Tài khoản AI".
//
// AN TOAN BI MAT: `CredentialView` KHONG BAO GIO chua bi mat nguyen van — chi `secretHint` da che
// phia Rust. Ban thiet ke co nut "Hiện key"/"Sao chép key" nhung backend CO Y khong mo duong dua
// plaintext ra IPC (luat #3 CLAUDE.md) — nen o day chi hien chuoi da che, KHONG lam 2 nut do.
//
// CHUA CO BACKEND (de "—", khong bia so lieu):
// - Cot "Hạn mức" (% da dung trong ngay): `CredentialView` chua co bo dem su dung.
// - Cong tac "Tự xoay vòng key cùng nhà cung cấp": chua co co trong DB.
import { useEffect, useMemo, useState } from "react";

import AddApiKeyDialog from "../components/AddApiKeyDialog";
import ConfirmDialog from "../components/ConfirmDialog";
import { useCredentialsStore } from "../store/credentials";
import { platformBadgeBg, platformLook } from "../format/platform";
import {
  Button,
  Chip,
  NodeTag,
  NODE_TYPE_VI,
  SearchInput,
  Switch,
  type NodeTypeKey,
} from "../ui/primitives";
import type { CredentialStatus, CredentialView } from "../ipc/types";

/** Nhan theo trang thai — khop bang `ST` trong ApiKeys.dc.html. */
const STATUS_VI: Record<CredentialStatus, string> = {
  ok: "Hoạt động",
  quota: "Hết hạn mức",
  expired: "Key sai",
  banned: "Key sai",
  unknown: "Chưa kiểm tra",
};

const STATUS_COLOR: Record<CredentialStatus, string> = {
  ok: "var(--ok)",
  quota: "var(--nocredit)",
  expired: "var(--warn)",
  banned: "var(--warn)",
  unknown: "var(--unknown)",
};

/** Nhom trang thai cho chip loc — `expired`/`banned` gop thanh "Key sai". */
type StatusGroup = "ok" | "quota" | "bad" | "unknown";

const GROUP_ORDER: ReadonlyArray<readonly [StatusGroup, string, string]> = [
  ["ok", "Hoạt động", "var(--ok)"],
  ["quota", "Hết hạn mức", "var(--nocredit)"],
  ["bad", "Key sai", "var(--warn)"],
  ["unknown", "Chưa kiểm tra", "var(--unknown)"],
];

function statusGroup(s: CredentialStatus): StatusGroup {
  return s === "ok" || s === "quota" || s === "unknown" ? s : "bad";
}

function readNodeTypes(meta: Record<string, unknown>): NodeTypeKey[] {
  const raw = meta.node_types;
  if (!Array.isArray(raw)) {
    return [];
  }
  return raw.filter((v): v is NodeTypeKey => v === "video" || v === "image" || v === "script");
}

/** "14:02" (hom nay) · "Hôm qua" · "10-04"; `0` = "—". */
function formatLast(epochSeconds: number, now: number): string {
  if (epochSeconds <= 0) {
    return "—";
  }
  const d = new Date(epochSeconds * 1000);
  const startOfToday = new Date(now * 1000);
  startOfToday.setHours(0, 0, 0, 0);
  const diffDays = Math.floor(
    (startOfToday.getTime() - new Date(d).setHours(0, 0, 0, 0)) / 86_400_000,
  );
  if (diffDays <= 0) {
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }
  if (diffDays === 1) {
    return "Hôm qua";
  }
  return `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export default function ApiKeyManager() {
  const rows = useCredentialsStore((s) => s.rows);
  const loading = useCredentialsStore((s) => s.loading);
  const error = useCredentialsStore((s) => s.error);
  const busy = useCredentialsStore((s) => s.busy);
  const load = useCredentialsStore((s) => s.load);
  const verify = useCredentialsStore((s) => s.verify);
  const update = useCredentialsStore((s) => s.update);
  const remove = useCredentialsStore((s) => s.remove);
  const setNodeTypes = useCredentialsStore((s) => s.setNodeTypes);

  const [query, setQuery] = useState("");
  const [group, setGroup] = useState<StatusGroup | "all">("all");
  const [nodeFilter, setNodeFilter] = useState<NodeTypeKey | "all">("all");
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [showAdd, setShowAdd] = useState(false);
  const [deleting, setDeleting] = useState<CredentialView | null>(null);
  const [checkingAll, setCheckingAll] = useState(false);
  const [tagMenu, setTagMenu] = useState<string | null>(null);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!tagMenu) {
      return;
    }
    const close = (): void => setTagMenu(null);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [tagMenu]);

  const now = useMemo(() => Math.floor(Date.now() / 1000), [rows]);
  const apiRows = useMemo(() => rows.filter((r) => r.kind === "api_key"), [rows]);

  const groupCounts = useMemo(() => {
    const m = new Map<StatusGroup, number>();
    for (const r of apiRows) {
      const g = statusGroup(r.status);
      m.set(g, (m.get(g) ?? 0) + 1);
    }
    return m;
  }, [apiRows]);

  const nodeCounts = useMemo(() => {
    const m = new Map<NodeTypeKey, number>();
    for (const r of apiRows) {
      for (const k of readNodeTypes(r.meta)) {
        m.set(k, (m.get(k) ?? 0) + 1);
      }
    }
    return m;
  }, [apiRows]);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return apiRows.filter((r) => {
      if (group !== "all" && statusGroup(r.status) !== group) {
        return false;
      }
      if (nodeFilter !== "all" && !readNodeTypes(r.meta).includes(nodeFilter)) {
        return false;
      }
      if (q === "") {
        return true;
      }
      return (
        r.label.toLowerCase().includes(q) || platformLook(r.provider).name.toLowerCase().includes(q)
      );
    });
  }, [apiRows, query, group, nodeFilter]);

  const okCount = groupCounts.get("ok") ?? 0;
  const badCount = groupCounts.get("bad") ?? 0;
  const selectedRows = visible.filter((r) => selected.has(r.id));
  const allSelected = visible.length > 0 && selectedRows.length === visible.length;

  async function handleCheckAll(): Promise<void> {
    setCheckingAll(true);
    try {
      for (const r of selectedRows.length > 0 ? selectedRows : visible) {
        await verify(r.id);
      }
    } finally {
      setCheckingAll(false);
    }
  }

  const TH =
    "border-b border-line px-2 py-3 text-left text-[10.5px] font-semibold uppercase tracking-[.09em] text-ink3";
  const TD = "border-b border-line px-2 py-3.5 align-top";

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex flex-col gap-3.5 border-b border-line px-6 pb-4 pt-5">
        <div className="flex flex-wrap items-start gap-2">
          <div className="min-w-[200px] flex-1">
            <h1 className="text-[20px] font-bold tracking-[-.01em]">API key</h1>
            <p className="mt-1 text-[13px] text-ink3">
              {apiRows.length} key · {okCount} đang hoạt động
              {badCount > 0 ? ` · ${badCount} cần thay` : ""}
            </p>
          </div>
          <Button variant="primary" onClick={() => setShowAdd(true)}>
            + Thêm API key
          </Button>
          <Button disabled={checkingAll || visible.length === 0} onClick={() => void handleCheckAll()}>
            {checkingAll
              ? "Đang kiểm tra…"
              : selectedRows.length > 0
                ? `Kiểm tra ${selectedRows.length} đã chọn`
                : "Kiểm tra tất cả"}
          </Button>
        </div>

        <div className="flex flex-wrap items-center gap-1.5">
          <span className="w-[120px] shrink-0 text-[10.5px] font-semibold uppercase tracking-[.1em] text-ink3">
            Trạng thái
          </span>
          <Chip label="Tất cả" count={apiRows.length} on={group === "all"} onClick={() => setGroup("all")} />
          {GROUP_ORDER.map(([g, label, color]) => (
            <Chip
              key={g}
              label={label}
              count={groupCounts.get(g) ?? 0}
              dotColor={color}
              on={group === g}
              onClick={() => setGroup(g)}
            />
          ))}
        </div>

        <div className="flex flex-wrap items-center gap-1.5">
          <span className="w-[120px] shrink-0 text-[10.5px] font-semibold uppercase tracking-[.1em] text-ink3">
            Dùng cho node
          </span>
          <Chip
            label="Tất cả"
            count={apiRows.length}
            on={nodeFilter === "all"}
            onClick={() => setNodeFilter("all")}
          />
          {(Object.keys(NODE_TYPE_VI) as NodeTypeKey[]).map((k) => (
            <Chip
              key={k}
              label={NODE_TYPE_VI[k]}
              count={nodeCounts.get(k) ?? 0}
              dotColor={`var(--tag-${k})`}
              squareSwatch
              on={nodeFilter === k}
              onClick={() => setNodeFilter(k)}
            />
          ))}
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <SearchInput
            value={query}
            onChange={setQuery}
            label="Tìm API key"
            placeholder="Tìm theo tên key hoặc nhà cung cấp…"
          />
          {selectedRows.length > 0 ? (
            <span className="text-[12.5px] font-bold text-accent-text">
              {selectedRows.length} đã chọn
            </span>
          ) : null}
        </div>
      </header>

      <div className="flex-1 overflow-auto px-6 pb-6 pt-2">
        {error ? <p className="mb-2 text-xs text-rose">Không tải được: {error}</p> : null}

        {visible.length === 0 && !loading ? (
          <p className="rounded-lg border border-dashed border-line2 px-3 py-10 text-center text-[13px] text-ink3">
            {apiRows.length === 0
              ? 'Chưa có API key nào — bấm "+ Thêm API key" để thêm.'
              : "Không có key nào khớp bộ lọc."}
          </p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[980px] border-collapse text-[13px]">
              <thead>
                <tr>
                  <th className={`${TH} w-8`}>
                    <input
                      type="checkbox"
                      checked={allSelected}
                      onChange={() =>
                        setSelected(allSelected ? new Set() : new Set(visible.map((r) => r.id)))
                      }
                      aria-label="Chọn tất cả"
                      className="h-4 w-4 accent-[#F5A524]"
                    />
                  </th>
                  <th className={TH}>Tên</th>
                  <th className={TH}>Nhà cung cấp</th>
                  <th className={TH}>Key</th>
                  <th className={TH}>Dùng cho node</th>
                  <th className={TH}>Trạng thái</th>
                  <th className={TH}>Hạn mức</th>
                  <th className={TH}>Bật</th>
                  <th className={TH}>Gần nhất</th>
                  <th className={TH}>Thao tác</th>
                </tr>
              </thead>
              <tbody>
                {visible.map((r) => {
                  const look = platformLook(r.provider);
                  const nodes = readNodeTypes(r.meta);
                  const isSel = selected.has(r.id);
                  const rowBusy = (busy[r.id] ?? 0) > 0;
                  return (
                    <tr key={r.id} style={isSel ? { background: "rgb(245 165 36 / .05)" } : undefined}>
                      <td className={TD}>
                        <input
                          type="checkbox"
                          checked={isSel}
                          onChange={() =>
                            setSelected((prev) => {
                              const next = new Set(prev);
                              if (next.has(r.id)) {
                                next.delete(r.id);
                              } else {
                                next.add(r.id);
                              }
                              return next;
                            })
                          }
                          aria-label={`Chọn ${r.label}`}
                          className="mt-0.5 h-4 w-4 accent-[#F5A524]"
                        />
                      </td>
                      <td className={`${TD} whitespace-nowrap font-semibold text-ink`}>{r.label}</td>
                      <td className={TD}>
                        <span className="flex items-center gap-2.5 whitespace-nowrap">
                          <span
                            className="grid h-7 w-7 shrink-0 place-items-center rounded-sm text-[12px] font-bold"
                            style={{ color: look.color, background: platformBadgeBg(look.color) }}
                          >
                            {look.letter}
                          </span>
                          <span className="font-semibold text-ink">{look.name}</span>
                        </span>
                      </td>
                      <td className={TD}>
                        <span
                          className="inline-flex items-center whitespace-nowrap rounded-sm border border-line bg-fill px-2.5 py-1 font-mono text-[12px] text-ink2"
                          title="Bí mật không bao giờ rời khỏi máy — chỉ hiện dạng che"
                        >
                          {r.secretHint}
                        </span>
                      </td>
                      <td className={`${TD} relative`}>
                        <button
                          type="button"
                          className="flex max-w-[180px] flex-wrap gap-1.5 rounded-sm text-left hover:opacity-80"
                          title="Bấm để đổi tag loại node"
                          onClick={(e) => {
                            e.stopPropagation();
                            setTagMenu(tagMenu === r.id ? null : r.id);
                          }}
                        >
                          {nodes.length === 0 ? (
                            <span className="text-ink3">— đặt tag</span>
                          ) : (
                            nodes.map((k) => <NodeTag key={k} kind={k} />)
                          )}
                        </button>
                        {tagMenu === r.id ? (
                          <div
                            className="absolute left-2 top-full z-40 mt-1 w-[170px] rounded-md border border-line2 bg-panel p-1 shadow-lg"
                            onClick={(e) => e.stopPropagation()}
                          >
                            {(Object.keys(NODE_TYPE_VI) as NodeTypeKey[]).map((k) => {
                              const on = nodes.includes(k);
                              return (
                                <label
                                  key={k}
                                  className="flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-[12.5px] text-ink2 hover:bg-fill"
                                >
                                  <input
                                    type="checkbox"
                                    checked={on}
                                    className="h-4 w-4 accent-[#F5A524]"
                                    onChange={() => {
                                      const next = on
                                        ? nodes.filter((x) => x !== k)
                                        : [...nodes, k];
                                      void setNodeTypes(r.id, next);
                                    }}
                                  />
                                  {NODE_TYPE_VI[k]}
                                </label>
                              );
                            })}
                          </div>
                        ) : null}
                      </td>
                      <td className={TD}>
                        <span
                          className="inline-flex items-center gap-[7px] whitespace-nowrap font-semibold"
                          style={{ color: STATUS_COLOR[r.status] }}
                        >
                          <span
                            className="h-[7px] w-[7px] rounded-[4px]"
                            style={{ background: STATUS_COLOR[r.status] }}
                          />
                          {STATUS_VI[r.status]}
                        </span>
                      </td>
                      {/* Hạn mức: backend chưa có bộ đếm sử dụng — để "—" thay vì bịa %. */}
                      <td className={`${TD} text-ink3`}>—</td>
                      <td className={TD}>
                        <Switch
                          checked={r.enabled}
                          disabled={rowBusy}
                          onChange={(on) => void update(r.id, { enabled: on })}
                          label={`Bật ${r.label}`}
                        />
                      </td>
                      <td className={`${TD} whitespace-nowrap text-ink2`}>
                        {formatLast(r.lastUsedAt, now)}
                      </td>
                      <td className={`${TD} whitespace-nowrap`}>
                        <div className="flex gap-1.5">
                          <Button small disabled={rowBusy} onClick={() => void verify(r.id)}>
                            {rowBusy ? "…" : "Kiểm tra"}
                          </Button>
                          <Button small variant="danger" onClick={() => setDeleting(r)}>
                            Xoá
                          </Button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {showAdd ? <AddApiKeyDialog onClose={() => setShowAdd(false)} /> : null}

      {deleting ? (
        <ConfirmDialog
          title={`Xoá API key “${deleting.label}”?`}
          body="Key bị xoá khỏi máy. Key trên trang nhà cung cấp vẫn còn — muốn vô hiệu hoá hẳn thì thu hồi ở đó."
          list={[
            { a: deleting.label, b: `${platformLook(deleting.provider).name} · ${deleting.secretHint}` },
          ]}
          okLabel="Xoá key"
          onCancel={() => setDeleting(null)}
          onConfirm={() => {
            void remove(deleting.id);
            setDeleting(null);
          }}
        />
      ) : null}
    </div>
  );
}
