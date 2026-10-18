// Man "Tài khoản AI" — dung theo ban ban giao `references/lvc-ui-design/screens/Main.dc.html`.
//
// MOT bang thong nhat gop tai khoan Dola (`useAccountsStore`, kho rieng co lich/han muc) + tai khoan
// dang nhap/cookie nen tang khac (`useCredentialsStore`, kho dung chung P4c) — hai nguon du lieu
// THAT SU khac nhau phia backend, trang nay chi la lop HIEN THI hop nhat.
//
// CHUA CO BACKEND (hien dung gia tri rong, KHONG bia so lieu):
// - `nodeTypes` (tag Video/Hình ảnh/Tạo kịch bản tren tung tai khoan) — ban ban giao ghi ro
//   "Backend cần thêm trường này". Doc tam tu `meta.node_types`; chua co thi khong hien tag nao.
// - Chip loc "Nền tảng: Tất cả" va cot trinh duyet cho credential (Dola co `browser` that).
import { useEffect, useMemo, useState } from "react";

import AddAccountDialog from "../components/AddAccountDialog";
import AssignProxyDialog from "../components/AssignProxyDialog";
import ConfirmDialog from "../components/ConfirmDialog";
import CheckProgressDialog, { type CheckItem } from "../components/CheckProgressDialog";
import EditAccountDialog, { type EditTarget } from "../components/EditAccountDialog";
import { useAccountsStore } from "../store/accounts";
import { useCredentialsStore } from "../store/credentials";
import { accountOpenViewer, credentialOpenViewer, isIpcError } from "../ipc/commands";
import {
  ACCOUNT_STATE_ORDER,
  ACCOUNT_STATE_VI,
  accountState,
  accountStateDetail,
  proxyKindLabel,
  type AccountState,
} from "../format/accountRow";
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
import type { VerifyOutcome, VerifyReport } from "../ipc/types";

/** Mau cham theo trang thai — bien CSS doi theo theme (xem `index.css`). */
const STATE_COLOR: Record<AccountState, string> = {
  active: "var(--ok)",
  relogin: "var(--warn)",
  resting: "var(--rest)",
  limited: "var(--limit)",
  no_credit: "var(--nocredit)",
  unknown: "var(--unknown)",
};

// `account_verify` (Dola) tra TRANG THAI PHIEN, khong phai `CredentialStatus` nhu credential chung.
// Quy ve cung kieu de hop thoai tien do hien dong nhat moi nen tang.
function dolaOutcome(r: VerifyReport): VerifyOutcome {
  switch (r.state) {
    case "logged_in":
      return { status: "ok", detail: "Phiên còn đăng nhập" };
    case "logged_out":
      return { status: "expired", detail: "Phiên đã đăng xuất — cần đăng nhập lại" };
    default:
      return { status: "unknown", detail: "Chưa xác định được trạng thái phiên" };
  }
}

interface UnifiedRow {
  key: string;
  platformKey: string;
  label: string;
  /** Dong phu duoi ten (email Dola / goi y bi mat da che). */
  sublabel: string;
  nodeTypes: NodeTypeKey[];
  state: AccountState;
  stateDetail: string;
  proxy: string;
  proxyKind: string;
  browser: string;
  enabled: boolean;
  lastUsedAt: number;
  canOpenViewer: boolean;
  target: EditTarget;
}

/** Doc tag loai node tu `meta.node_types` (backend CHUA co truong nay — rong la binh thuong). */
function readNodeTypes(meta: Record<string, unknown>): NodeTypeKey[] {
  const raw = meta.node_types;
  if (!Array.isArray(raw)) {
    return [];
  }
  return raw.filter((v): v is NodeTypeKey => v === "video" || v === "image" || v === "script");
}

/** "Hôm nay 13:42" / "Hôm qua 21:16" / "3 ngày trước" / "10-04 14:32"; `0` = "—". */
function formatLastUsed(epochSeconds: number, now: number): string {
  if (epochSeconds <= 0) {
    return "—";
  }
  const d = new Date(epochSeconds * 1000);
  const hhmm = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  const startOfToday = new Date(now * 1000);
  startOfToday.setHours(0, 0, 0, 0);
  const diffDays = Math.floor(
    (startOfToday.getTime() - new Date(d).setHours(0, 0, 0, 0)) / 86_400_000,
  );
  if (diffDays <= 0) {
    return `Hôm nay ${hhmm}`;
  }
  if (diffDays === 1) {
    return `Hôm qua ${hhmm}`;
  }
  if (diffDays < 7) {
    return `${diffDays} ngày trước`;
  }
  return `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")} ${hhmm}`;
}

export default function AccountsPage() {
  const dolaRows = useAccountsStore((s) => s.rows);
  const dolaLoading = useAccountsStore((s) => s.loading);
  const dolaError = useAccountsStore((s) => s.error);
  const loadDola = useAccountsStore((s) => s.load);
  const verifyDola = useAccountsStore((s) => s.verify);
  const setSchedulingDola = useAccountsStore((s) => s.setScheduling);
  const removeDola = useAccountsStore((s) => s.remove);

  const credRows = useCredentialsStore((s) => s.rows);
  const credLoading = useCredentialsStore((s) => s.loading);
  const credError = useCredentialsStore((s) => s.error);
  const loadCred = useCredentialsStore((s) => s.load);
  const verifyCred = useCredentialsStore((s) => s.verify);
  const updateCred = useCredentialsStore((s) => s.update);
  const removeCred = useCredentialsStore((s) => s.remove);

  const [showAdd, setShowAdd] = useState(false);
  const [editing, setEditing] = useState<EditTarget | null>(null);
  const [pendingVerify, setPendingVerify] = useState<string | null>(null);
  const [pendingViewer, setPendingViewer] = useState<string | null>(null);
  const [viewerError, setViewerError] = useState<string | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ key: string; x: number; y: number } | null>(null);
  const [query, setQuery] = useState("");
  const [stateFilter, setStateFilter] = useState<AccountState | "all">("all");
  const [nodeFilter, setNodeFilter] = useState<NodeTypeKey | "all">("all");
  /** Lọc theo nền tảng (`dola`/`museai`/…). Chỉ hiện những nền tảng THỰC SỰ có tài khoản — bày
   * ra 13 nền tảng rỗng chỉ làm người dùng phải đọc nhiều hơn mà không lọc được gì. */
  const [platformFilter, setPlatformFilter] = useState<string>("all");
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [bulkBusy, setBulkBusy] = useState(false);
  /** `true` = đang hỏi trước khi xoá hàng loạt. Xoá tài khoản là KHÔNG hoàn tác được (mất cả
   * cookie/phiên đăng nhập đã nạp), nên không bao giờ xoá thẳng từ một cú bấm. */
  const [confirmBulkDelete, setConfirmBulkDelete] = useState(false);
  /** Danh sach tai khoan dang kiem trong hop thoai tien do; `null` = dong. */
  const [checkItems, setCheckItems] = useState<CheckItem[] | null>(null);
  /** `true` = dang mo hop thoai gan proxy hang loat cho cac dong da chon. */
  const [showAssignProxy, setShowAssignProxy] = useState(false);

  useEffect(() => {
    void loadDola();
    void loadCred();
  }, [loadDola, loadCred]);

  useEffect(() => {
    if (!menu) {
      return;
    }
    const close = (): void => setMenu(null);
    window.addEventListener("click", close);
    window.addEventListener("scroll", close, true);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("scroll", close, true);
    };
  }, [menu]);

  const loading = dolaLoading || credLoading;
  const now = useMemo(() => Math.floor(Date.now() / 1000), [dolaRows, credRows]);

  const rows: UnifiedRow[] = useMemo(() => {
    const fromDola: UnifiedRow[] = dolaRows.map((row) => ({
      key: `dola:${row.name}`,
      platformKey: "dola",
      label: row.name,
      sublabel: row.email,
      nodeTypes: [],
      state: accountState(row),
      stateDetail: accountStateDetail(row, now),
      proxy: row.proxy,
      proxyKind: proxyKindLabel(row.proxy, row.proxySet),
      browser: row.browser,
      enabled: row.scheduling,
      lastUsedAt: row.lastUsedAt,
      canOpenViewer: true,
      target: { source: "dola", row },
    }));
    const fromCred: UnifiedRow[] = credRows
      .filter((r) => r.kind !== "api_key")
      .map((row) => ({
        key: `cred:${row.id}`,
        platformKey: row.provider,
        label: row.label,
        sublabel: row.secretHint,
        nodeTypes: readNodeTypes(row.meta),
        state:
          row.status === "ok"
            ? "active"
            : row.status === "expired" || row.status === "banned"
              ? "relogin"
              : row.status === "quota"
                ? "no_credit"
                : "unknown",
        stateDetail: row.status === "banned" ? "Tài khoản bị khoá" : "",
        proxy: row.proxyMasked,
        proxyKind: proxyKindLabel(row.proxyMasked, row.proxyMasked.trim() !== ""),
        browser: typeof row.meta.profile_dir === "string" ? "Chrome" : "",
        enabled: row.enabled,
        lastUsedAt: row.lastUsedAt,
        canOpenViewer: typeof row.meta.profile_dir === "string",
        target: { source: "credential", row },
      }));
    return [...fromDola, ...fromCred].sort((a, b) => b.lastUsedAt - a.lastUsedAt);
  }, [dolaRows, credRows, now]);

  const stateCounts = useMemo(() => {
    const m = new Map<AccountState, number>();
    for (const r of rows) {
      m.set(r.state, (m.get(r.state) ?? 0) + 1);
    }
    return m;
  }, [rows]);

  const nodeCounts = useMemo(() => {
    const m = new Map<NodeTypeKey, number>();
    for (const r of rows) {
      for (const k of r.nodeTypes) {
        m.set(k, (m.get(k) ?? 0) + 1);
      }
    }
    return m;
  }, [rows]);

  /** `[khoá nền tảng, số tài khoản]`, nhiều nhất trước — chỉ nền tảng đang CÓ tài khoản. */
  const platformCounts = useMemo(() => {
    const m = new Map<string, number>();
    for (const r of rows) {
      m.set(r.platformKey, (m.get(r.platformKey) ?? 0) + 1);
    }
    return [...m.entries()].sort((a, b) => b[1] - a[1]);
  }, [rows]);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return rows.filter((r) => {
      if (stateFilter !== "all" && r.state !== stateFilter) {
        return false;
      }
      if (nodeFilter !== "all" && !r.nodeTypes.includes(nodeFilter)) {
        return false;
      }
      if (platformFilter !== "all" && r.platformKey !== platformFilter) {
        return false;
      }
      if (q === "") {
        return true;
      }
      return (
        r.label.toLowerCase().includes(q) ||
        r.sublabel.toLowerCase().includes(q) ||
        r.proxy.toLowerCase().includes(q) ||
        platformLook(r.platformKey as never).name.toLowerCase().includes(q)
      );
    });
  }, [rows, query, stateFilter, nodeFilter, platformFilter]);

  const activeCount = stateCounts.get("active") ?? 0;
  const selectedRows = visible.filter((r) => selected.has(r.key));
  const allVisibleSelected = visible.length > 0 && selectedRows.length === visible.length;

  function toggleSelect(key: string): void {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }

  async function handleVerify(row: UnifiedRow): Promise<void> {
    setPendingVerify(row.key);
    try {
      if (row.target.source === "dola") {
        await verifyDola(row.target.row.name);
      } else {
        await verifyCred(row.target.row.id);
      }
    } finally {
      setPendingVerify(null);
    }
  }

  async function handleOpenViewer(row: UnifiedRow): Promise<void> {
    setPendingViewer(row.key);
    setViewerError(null);
    try {
      if (row.target.source === "dola") {
        await accountOpenViewer(row.target.row.name);
      } else {
        await credentialOpenViewer(row.target.row.id);
      }
    } catch (e) {
      setViewerError(
        isIpcError(e) && e.code === "PROFILE_NOT_FOUND"
          ? `"${row.label}" chưa có profile trình duyệt — thêm lại cookie rồi thử lại.`
          : `Không mở được trình duyệt cho "${row.label}": ${isIpcError(e) ? e.message : String(e)}`,
      );
    } finally {
      setPendingViewer(null);
    }
  }

  async function handleToggle(row: UnifiedRow, on: boolean): Promise<void> {
    if (row.target.source === "dola") {
      await setSchedulingDola(row.target.row.name, on);
    } else {
      await updateCred(row.target.row.id, { enabled: on });
    }
  }

  async function removeRow(row: UnifiedRow): Promise<void> {
    if (row.target.source === "dola") {
      await removeDola(row.target.row.name);
    } else {
      await removeCred(row.target.row.id);
    }
  }

  /** Kiem tra TUAN TU — tranh ban song song N lenh mo trinh duyet cung luc. */
  function handleBulkVerify(): void {
    // Hop thoai tu chay vong lap tuan tu va hien tien do tung tai khoan (xem CheckProgressDialog).
    setCheckItems(
      selectedRows.map((row) => ({
        key: row.key,
        name: row.label,
        platform: platformLook(row.platformKey as never).name,
        run: () =>
          row.target.source === "dola"
            ? verifyDola(row.target.row.name).then(dolaOutcome)
            : verifyCred(row.target.row.id),
      })),
    );
  }

  async function handleBulkDelete(): Promise<void> {
    setConfirmBulkDelete(false);
    setBulkBusy(true);
    try {
      for (const row of selectedRows) {
        await removeRow(row);
      }
      setSelected(new Set());
    } finally {
      setBulkBusy(false);
    }
  }

  const menuRow = menu ? rows.find((r) => r.key === menu.key) : undefined;
  const TH = "border-b border-line px-2 py-3 text-left text-[10.5px] font-semibold uppercase tracking-[.09em] text-ink3";
  const TD = "border-b border-line px-2 py-3.5 align-top";

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex flex-col gap-3.5 border-b border-line px-6 pb-4 pt-5">
        <div className="flex flex-wrap items-start gap-2">
          <div className="min-w-[200px] flex-1">
            <h1 className="text-[20px] font-bold tracking-[-.01em]">Tài khoản AI</h1>
            <p className="mt-1 text-[13px] text-ink3">
              {rows.length} tài khoản · {activeCount} đang hoạt động
            </p>
          </div>
          <Button variant="primary" onClick={() => setShowAdd(true)}>
            + Thêm tài khoản
          </Button>
          <Button
            disabled={loading}
            onClick={() => {
              void loadDola();
              void loadCred();
            }}
          >
            Làm mới
          </Button>
        </div>

        <div className="flex flex-wrap items-center gap-1.5">
          <span className="w-[120px] shrink-0 text-[10.5px] font-semibold uppercase tracking-[.1em] text-ink3">
            Trạng thái
          </span>
          <Chip label="Tất cả" count={rows.length} on={stateFilter === "all"} onClick={() => setStateFilter("all")} />
          {ACCOUNT_STATE_ORDER.map((s) => (
            <Chip
              key={s}
              label={ACCOUNT_STATE_VI[s]}
              count={stateCounts.get(s) ?? 0}
              dotColor={STATE_COLOR[s]}
              on={stateFilter === s}
              onClick={() => setStateFilter(s)}
            />
          ))}
        </div>

        <div className="flex flex-wrap items-center gap-1.5">
          <span className="w-[120px] shrink-0 text-[10.5px] font-semibold uppercase tracking-[.1em] text-ink3">
            Dùng cho node
          </span>
          <Chip label="Tất cả" count={rows.length} on={nodeFilter === "all"} onClick={() => setNodeFilter("all")} />
          {(Object.keys(NODE_TYPE_VI) as NodeTypeKey[]).map((k) => (
            <Chip
              key={k}
              label={NODE_TYPE_VI[k]}
              count={nodeCounts.get(k) ?? 0}
              dotColor={`var(--tag-${k === "script" ? "script" : k})`}
              squareSwatch
              on={nodeFilter === k}
              onClick={() => setNodeFilter(k)}
            />
          ))}
        </div>

        <div className="flex flex-wrap items-center gap-1.5">
          <span className="w-[120px] shrink-0 text-[10.5px] font-semibold uppercase tracking-[.1em] text-ink3">
            Nền tảng
          </span>
          <Chip
            label="Tất cả"
            count={rows.length}
            on={platformFilter === "all"}
            onClick={() => setPlatformFilter("all")}
          />
          {platformCounts.map(([key, count]) => (
            <Chip
              key={key}
              label={platformLook(key as never).name}
              count={count}
              on={platformFilter === key}
              onClick={() => setPlatformFilter(key)}
            />
          ))}
        </div>

        <div className="flex flex-wrap items-center gap-2.5">
          <SearchInput
            value={query}
            onChange={setQuery}
            label="Tìm tài khoản"
            placeholder="Tìm theo tên, email hoặc proxy…"
          />
          {selectedRows.length > 0 ? (
            <>
              <span className="text-[12.5px] font-bold text-accent">
                {selectedRows.length} đã chọn
              </span>
              <Button disabled={bulkBusy} onClick={handleBulkVerify}>
                {bulkBusy ? "Đang kiểm tra…" : "Kiểm tra"}
              </Button>
              <Button disabled={bulkBusy} onClick={() => setShowAssignProxy(true)}>
                Gán proxy…
              </Button>
              <Button variant="danger" disabled={bulkBusy} onClick={() => setConfirmBulkDelete(true)}>
                Xoá
              </Button>
            </>
          ) : null}
        </div>
      </header>

      <div className="flex-1 overflow-auto px-6 pb-6 pt-2">
        {dolaError ? <p className="mb-2 text-xs text-rose">Không tải được tài khoản Dola: {dolaError}</p> : null}
        {credError ? <p className="mb-2 text-xs text-rose">Không tải được tài khoản khác: {credError}</p> : null}
        {viewerError ? <p className="mb-2 text-xs text-rose">{viewerError}</p> : null}

        {visible.length === 0 && !loading ? (
          <p className="rounded-lg border border-dashed border-line2 px-3 py-10 text-center text-[13px] text-ink3">
            {rows.length === 0
              ? 'Chưa có tài khoản nào — bấm "+ Thêm tài khoản" để nạp cookie hoặc đăng nhập.'
              : "Không có tài khoản nào khớp bộ lọc."}
          </p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[980px] border-collapse text-[13px]">
              <thead>
                <tr>
                  <th className={`${TH} w-8`}>
                    <input
                      type="checkbox"
                      checked={allVisibleSelected}
                      onChange={() =>
                        setSelected(allVisibleSelected ? new Set() : new Set(visible.map((r) => r.key)))
                      }
                      aria-label="Chọn tất cả"
                      className="h-4 w-4 accent-[#F5A524]"
                    />
                  </th>
                  <th className={TH}>Tài khoản</th>
                  <th className={TH}>Nền tảng</th>
                  <th className={TH}>Dùng cho node</th>
                  <th className={TH}>Trạng thái</th>
                  <th className={TH}>Lịch</th>
                  <th className={TH}>Proxy</th>
                  <th className={TH}>Gần nhất</th>
                  <th className={TH}>Thao tác</th>
                </tr>
              </thead>
              <tbody>
                {visible.map((row) => {
                  const look = platformLook(row.platformKey as never);
                  const isSel = selected.has(row.key);
                  return (
                    <tr
                      key={row.key}
                      style={isSel ? { background: "rgb(245 165 36 / .05)" } : undefined}
                      onContextMenu={(e) => {
                        e.preventDefault();
                        setMenu({ key: row.key, x: e.clientX, y: e.clientY });
                      }}
                    >
                      <td className={TD}>
                        <input
                          type="checkbox"
                          checked={isSel}
                          onChange={() => toggleSelect(row.key)}
                          aria-label={`Chọn ${row.label}`}
                          className="mt-0.5 h-4 w-4 accent-[#F5A524]"
                        />
                      </td>
                      <td className={TD}>
                        <span className="font-semibold text-fg">{row.label}</span>
                        {row.sublabel ? (
                          <span className="mt-0.5 block text-[12px] text-ink3">{row.sublabel}</span>
                        ) : null}
                      </td>
                      <td className={TD}>
                        <span className="flex items-center gap-2.5 whitespace-nowrap">
                          <span
                            className="grid h-7 w-7 shrink-0 place-items-center rounded-sm text-[12px] font-bold"
                            style={{ color: look.color, background: platformBadgeBg(look.color) }}
                          >
                            {look.letter}
                          </span>
                          <span className="flex flex-col leading-[1.3]">
                            <span className="font-semibold text-fg">{look.name}</span>
                            <span className="text-[11.5px] text-ink3">{look.sub}</span>
                          </span>
                        </span>
                      </td>
                      <td className={TD}>
                        {row.nodeTypes.length === 0 ? (
                          <span className="text-ink3">—</span>
                        ) : (
                          <span className="flex max-w-[190px] flex-wrap gap-1.5">
                            {row.nodeTypes.map((k) => (
                              <NodeTag key={k} kind={k} />
                            ))}
                          </span>
                        )}
                      </td>
                      <td className={TD}>
                        <span
                          className="inline-flex items-center gap-[7px] whitespace-nowrap font-semibold"
                          style={{ color: STATE_COLOR[row.state] }}
                        >
                          <span
                            className="h-[7px] w-[7px] rounded-[4px]"
                            style={{ background: STATE_COLOR[row.state] }}
                          />
                          {ACCOUNT_STATE_VI[row.state]}
                        </span>
                        {row.stateDetail && row.stateDetail !== "—" ? (
                          <span className="mt-[3px] block max-w-[200px] text-[12px] leading-[1.45] text-ink3">
                            {row.stateDetail}
                          </span>
                        ) : null}
                      </td>
                      <td className={TD}>
                        <Switch
                          checked={row.enabled}
                          onChange={(on) => void handleToggle(row, on)}
                          label={`Lịch chạy ${row.label}`}
                        />
                      </td>
                      <td className={`${TD} whitespace-nowrap`}>
                        <span className="rounded-sm bg-fill px-[7px] py-0.5 text-[11px] text-muted">
                          {row.proxyKind}
                        </span>
                        <span className="ml-1 font-mono text-[12px] text-muted">
                          {row.proxy === "" ? "—" : row.proxy}
                        </span>
                        {row.browser ? (
                          <span className="mt-[3px] block text-[12px] text-ink3">{row.browser}</span>
                        ) : null}
                      </td>
                      <td className={`${TD} whitespace-nowrap text-muted`}>
                        {formatLastUsed(row.lastUsedAt, now)}
                      </td>
                      <td className={`${TD} whitespace-nowrap`}>
                        <div className="flex gap-1.5">
                          <Button
                            small
                            disabled={pendingViewer === row.key || !row.canOpenViewer}
                            title={row.canOpenViewer ? undefined : "Chưa có profile trình duyệt — thêm cookie trước"}
                            onClick={() => void handleOpenViewer(row)}
                          >
                            {pendingViewer === row.key ? "…" : "Mở"}
                          </Button>
                          <Button
                            small
                            disabled={pendingVerify === row.key}
                            onClick={() => void handleVerify(row)}
                          >
                            {pendingVerify === row.key ? "…" : "Kiểm tra"}
                          </Button>
                          {confirmingDelete === row.key ? (
                            <span className="flex items-center gap-1 text-[12.5px]">
                              Xoá?
                              <Button
                                small
                                variant="danger"
                                onClick={() => {
                                  void removeRow(row);
                                  setConfirmingDelete(null);
                                }}
                              >
                                Có
                              </Button>
                              <Button small onClick={() => setConfirmingDelete(null)}>
                                Không
                              </Button>
                            </span>
                          ) : (
                            <Button
                              small
                              aria-label="Thêm thao tác: Proxy, Xoá"
                              onClick={(e) => {
                                e.stopPropagation();
                                setMenu({ key: row.key, x: e.clientX, y: e.clientY });
                              }}
                            >
                              ⋯
                            </Button>
                          )}
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

      {menu && menuRow ? (
        <div
          className="fixed z-50 min-w-[170px] rounded-md border border-line2 bg-panel py-1 text-xs shadow-lg"
          style={{ left: menu.x, top: menu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          {[
            ["Kiểm tra", () => void handleVerify(menuRow)],
            ["Mở trình duyệt", () => void handleOpenViewer(menuRow)],
            [menuRow.enabled ? "Tắt lịch chạy" : "Bật lịch chạy", () => void handleToggle(menuRow, !menuRow.enabled)],
            ["Proxy & trình duyệt", () => setEditing(menuRow.target)],
          ].map(([label, fn]) => (
            <button
              key={label as string}
              type="button"
              className="block w-full px-3 py-1.5 text-left text-muted hover:bg-fill hover:text-fg"
              onClick={() => {
                setMenu(null);
                (fn as () => void)();
              }}
            >
              {label as string}
            </button>
          ))}
          <button
            type="button"
            className="block w-full px-3 py-1.5 text-left text-rose hover:bg-rose/10"
            onClick={() => {
              setMenu(null);
              setConfirmingDelete(menuRow.key);
            }}
          >
            Xoá
          </button>
        </div>
      ) : null}

      {confirmBulkDelete ? (
        <ConfirmDialog
          title={`Xoá ${selectedRows.length} tài khoản?`}
          body="Xoá tài khoản là không hoàn tác được: cookie và phiên đăng nhập đã nạp sẽ mất, muốn dùng lại phải đăng nhập từ đầu."
          list={selectedRows.slice(0, 8).map((r) => ({ a: r.label, b: r.sublabel }))}
          note={
            selectedRows.length > 8
              ? `…và ${selectedRows.length - 8} tài khoản nữa. Workflow đang chạy bằng các tài khoản này sẽ lỗi ở node tương ứng.`
              : "Workflow đang chạy bằng các tài khoản này sẽ lỗi ở node tương ứng."
          }
          okLabel={`Xoá ${selectedRows.length} tài khoản`}
          busy={bulkBusy}
          onCancel={() => setConfirmBulkDelete(false)}
          onConfirm={() => void handleBulkDelete()}
        />
      ) : null}

      {showAdd ? <AddAccountDialog onClose={() => setShowAdd(false)} /> : null}
      {editing ? <EditAccountDialog target={editing} onClose={() => setEditing(null)} /> : null}
      {showAssignProxy ? (
        <AssignProxyDialog
          rows={selectedRows.map((r) => ({ key: r.key, label: r.label, target: r.target }))}
          onClose={() => setShowAssignProxy(false)}
        />
      ) : null}
      {checkItems ? (
        <CheckProgressDialog items={checkItems} onClose={() => setCheckItems(null)} />
      ) : null}
    </div>
  );
}
