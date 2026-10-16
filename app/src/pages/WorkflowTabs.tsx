// Nhieu tab Workflow kieu Chrome (P-tabs) — moi tab la MOT canvas doc lap (store `graph`/`run`
// RIENG, xem `store/graph.tsx`/`store/run.tsx`), CHAY DONG THOI duoc (backend da ho tro san nhieu
// run cung luc qua `ctx.runs` khoa theo `runId`, khong can sua Rust — xem `store/runRegistry.ts`
// ve cach dinh tuyen su kien). Tab KHONG active van giu MOUNTED (an qua CSS `display:none`, khong
// unmount) de tiep tuc nhan su kien tien trinh ngam — unmount se chay cleanup cua `RunBar.tsx`
// (go dang ky run khoi `runRegistry`), lam tab do THOI theo doi su kien song (van xem lai duoc qua
// trang "Tác vụ" doc DB, chi mat phan "song" ngay tren canvas).
import { useEffect, useRef, useState } from "react";
import { useStore } from "zustand";

import Workflow from "./Workflow";
import {
  createGraphStore,
  loadSession,
  saveSession,
  snapshotCanvas,
  newEphemeralWorkflowId,
  GraphStoreProvider,
  type GraphStoreApi,
} from "../store/graph";
import type { PersistedCanvas } from "../store/graph";
import { createRunStore, RunStoreProvider, type RunStoreApi } from "../store/run";
import { unregisterRunOwner } from "../store/runRegistry";
import { useOpenWorkflow } from "../store/openWorkflow";
import { runCancel } from "../ipc/commands";
import { confirmDialog } from "../store/dialog";

interface Tab {
  id: string;
  graphStore: GraphStoreApi;
  runStore: RunStoreApi;
}

function genTabId(): string {
  const hasRandomUuid = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function";
  return hasRandomUuid ? crypto.randomUUID() : `tab-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

/** Tab "chinh" — duy nhat hydrat tu `localStorage` (xem doc `createGraphStore`) va duy nhat tu
 * luu lai vao do (`persist: true`), giong hanh vi canvas DUY NHAT truoc khi co P-tabs. */
/** Dung mot tab tu canvas da luu. Danh tinh truyen vao LUC TAO store (khong `setWorkflowMeta`
 * sau) de khong co mot nhip nao canvas mang dung noi dung cu nhung lai mang ten "chưa đặt tên" —
 * bam Luu trong nhip do se tao them MOT workflow moi thay vi ghi de ban dang mo. */
function tabFrom(canvas: PersistedCanvas): Tab {
  const graphStore = createGraphStore({
    workflowId: canvas.workflowId,
    workflowName: canvas.workflowName === "" ? "Workflow chưa đặt tên" : canvas.workflowName,
    workflowSaved: canvas.workflowSaved,
  });
  graphStore.getState().loadGraph(canvas.graph);
  return { id: genTabId(), graphStore, runStore: createRunStore() };
}

function createBlankTab(): Tab {
  const graphStore = createGraphStore({
    workflowId: newEphemeralWorkflowId(),
    workflowName: "Workflow chưa đặt tên",
  });
  return { id: genTabId(), graphStore, runStore: createRunStore() };
}

/** Moi tab cua phien truoc; chua co phien nao -> mot tab trong. */
function restoreTabs(): { tabs: Tab[]; activeIndex: number } {
  const session = loadSession();
  if (session === null || session.tabs.length === 0) {
    return { tabs: [createBlankTab()], activeIndex: 0 };
  }
  return { tabs: session.tabs.map(tabFrom), activeIndex: session.activeIndex };
}

function TabLabel({ graphStore }: { graphStore: GraphStoreApi }) {
  const name = useStore(graphStore, (s) => s.workflowName);
  return <span className="max-w-[11rem] truncate">{name}</span>;
}

/** Tong `done`/`total` qua MOI node cua run (vd node fan-out 3 video, 2 xong -> done=2,total=3) —
 * cong don lai thanh tien do tong the ca run. Tra SO NGUYEN THUAN (khong phai object) de dung lam
 * ket qua selector `useStore` — object/array moi MOI LAN goi se lam `useSyncExternalStore` nghi
 * store "doi" o moi render (so sanh `Object.is` that bai) -> loop vo han "Maximum update depth
 * exceeded" (da gap that khi dung ham tra `{done,total}` truoc do — xem lich su sua loi). */
function sumDone(nodes: Record<string, { done: number; total: number }>): number {
  return Object.values(nodes).reduce((s, n) => s + n.done, 0);
}
function sumTotal(nodes: Record<string, { done: number; total: number }>): number {
  return Object.values(nodes).reduce((s, n) => s + n.total, 0);
}

/** Cham trang thai run cua tab (hien ca khi tab KHONG active — "biet tab nen van con chay") —
 * tooltip kem % tien do khi co du lieu (review nguoi dung: muon thay tien do ngay tren tab). */
function TabRunDot({ runStore }: { runStore: RunStoreApi }) {
  const running = useStore(runStore, (s) => s.runId !== null && s.status === null);
  const done = useStore(runStore, (s) => sumDone(s.nodes));
  const total = useStore(runStore, (s) => sumTotal(s.nodes));
  if (!running) {
    return null;
  }
  const title = total > 0 ? `Đang chạy — ${done}/${total} (${Math.round((done / total) * 100)}%)` : "Đang chạy";
  return <span className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-amber-400" title={title} />;
}

/** Thanh tien do mong o day tab — chieu rong = % tien do tong the cua run (kieu Chrome tai trang).
 * Chua co so lieu (`total === 0`, vua bat dau/node khong fan-out) -> thanh nhap nhay co dinh thay
 * vi 0% gay hieu lam "chua chay gi". */
function TabRunBar({ runStore }: { runStore: RunStoreApi }) {
  const running = useStore(runStore, (s) => s.runId !== null && s.status === null);
  const done = useStore(runStore, (s) => sumDone(s.nodes));
  const total = useStore(runStore, (s) => sumTotal(s.nodes));
  if (!running) {
    return null;
  }
  const pct = total > 0 ? Math.min(100, Math.round((done / total) * 100)) : null;
  return (
    <div className="absolute inset-x-0 bottom-0 h-0.5 overflow-hidden rounded-b bg-fill">
      <div
        className={"h-full bg-amber-400 " + (pct === null ? "w-1/3 animate-pulse" : "")}
        style={pct === null ? undefined : { width: `${pct}%`, transition: "width 300ms ease" }}
      />
    </div>
  );
}

export default function WorkflowTabs() {
  const [restored] = useState(restoreTabs);
  const [tabs, setTabs] = useState<Tab[]>(restored.tabs);
  const [activeId, setActiveId] = useState<string>(
    () => restored.tabs[restored.activeIndex]?.id ?? restored.tabs[0]!.id,
  );
  // Giu tabs/activeId MOI NHAT cho cac handler async (dong tab) ma khong phai liet ke lam dep phu
  // thuoc cua moi callback — doc tai THOI DIEM goi, khong phai luc dong closure.
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;

  // Nhan yeu cau "mo do thi nay" tu mot TRANG KHAC (vd Kho kịch bản) — nap vao tab DANG XEM.
  // Day la noi duy nhat biet tab nao dang xem, nen cung la noi duy nhat lam duoc viec nay.
  const pendingOpen = useOpenWorkflow((s) => s.pending);
  const consumeOpen = useOpenWorkflow((s) => s.consume);
  useEffect(() => {
    if (pendingOpen === null) {
      return;
    }
    const tab = tabsRef.current.find((t) => t.id === activeId);
    if (!tab) {
      return;
    }
    const g = tab.graphStore.getState();
    g.loadGraph(pendingOpen.graph);
    g.setWorkflowMeta(
      // Dung mau -> chua co id that, cap mot id tam de tab van co danh tinh rieng.
      pendingOpen.workflowId === "" ? newEphemeralWorkflowId() : pendingOpen.workflowId,
      pendingOpen.workflowName,
      pendingOpen.saved,
    );
    consumeOpen();
  }, [pendingOpen, activeId, consumeOpen]);

  // Luu CA phien (moi tab + tab dang xem) moi khi bat ky canvas nao doi. Gop 500ms de keo mot node
  // khong sinh hang tram luot ghi. Day la NGUON GHI DUY NHAT cua khoa phien — store khong tu ghi.
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const flush = (): void => {
      if (timer) {
        clearTimeout(timer);
      }
      timer = setTimeout(() => {
        const list = tabsRef.current;
        saveSession({
          tabs: list.map((t) => snapshotCanvas(t.graphStore.getState())),
          activeIndex: Math.max(
            0,
            list.findIndex((t) => t.id === activeId),
          ),
        });
      }, 500);
    };
    const unsubs = tabs.map((t) => t.graphStore.subscribe(flush));
    // Ghi mot lan ngay khi tab/tab-dang-xem doi (them tab, dong tab, chuyen tab) — khong cho toi
    // khi nguoi dung dong vao canvas moi nho.
    flush();
    return () => {
      if (timer) {
        clearTimeout(timer);
      }
      for (const u of unsubs) {
        u();
      }
    };
  }, [tabs, activeId]);

  function handleNewTab(): void {
    const tab = createBlankTab();
    setTabs((t) => [...t, tab]);
    setActiveId(tab.id);
  }

  async function handleCloseTab(id: string): Promise<void> {
    const current = tabsRef.current;
    if (current.length <= 1) {
      // Luon giu it nhat 1 tab (giong hanh vi canvas duy nhat truoc P-tabs) — khong cho dong tab
      // cuoi cung, tranh trang "khong co workflow nao" chua thiet ke UI rieng.
      return;
    }
    const tab = current.find((t) => t.id === id);
    if (!tab) {
      return;
    }
    const runState = tab.runStore.getState();
    const running = runState.runId !== null && runState.status === null;
    if (running && runState.runId) {
      // Quyet dinh nguoi dung da chon truoc (P-tabs): hoi truoc, cho phep Huỷ lan chay HOAC de
      // chay ngam (engine van chay tiep o server, xem lai qua trang "Tác vụ") — ca hai lua chon
      // deu DONG tab (khong co nhanh thu 3 "khong dong gi ca", giong Chrome khong hoi lai khi
      // dong tab co tai xuong dang chay, chi khac nhau o viec co huy tac vu hay khong).
      const cancelRun = await confirmDialog({
        title: "Đóng tab đang chạy",
        message: "Tab này còn một lần chạy chưa xong. Huỷ lần chạy đó trước khi đóng, hay cứ đóng và để chạy ngầm (xem tiếp ở tab \"Tác vụ\")?",
        okText: "Huỷ lần chạy rồi đóng",
        cancelText: "Đóng, để chạy ngầm",
        danger: true,
      });
      if (cancelRun) {
        try {
          await runCancel(runState.runId);
        } catch {
          // Mien la dong tab duoc — loi huy (vd run da xong giua chung) khong chan thao tac dong.
        }
      }
    }
    unregisterRunOwner(tab.runStore.getState().runId ?? "");
    setTabs((t) => t.filter((x) => x.id !== id));
    setActiveId((prevActive) => {
      if (prevActive !== id) {
        return prevActive;
      }
      const idx = current.findIndex((t) => t.id === id);
      return (current[idx - 1] ?? current[idx + 1])?.id ?? current[0]!.id;
    });
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-1 overflow-x-auto border-b border-line bg-stage px-2 pt-2">
        {tabs.map((tab) => (
          <div
            key={tab.id}
            role="button"
            tabIndex={0}
            onClick={() => setActiveId(tab.id)}
            className={
              "group relative flex max-w-[16rem] shrink-0 cursor-pointer items-center gap-1.5 rounded-t-lg border border-b-0 px-3 py-2 text-xs " +
              (tab.id === activeId
                ? "border-line bg-stage text-fg"
                : "border-transparent text-ink3 hover:bg-fill hover:text-muted")
            }
          >
            <TabRunDot runStore={tab.runStore} />
            <TabLabel graphStore={tab.graphStore} />
            <TabRunBar runStore={tab.runStore} />
            {tabs.length > 1 ? (
              <button
                type="button"
                title="Đóng tab"
                aria-label="Đóng tab"
                className="ml-1 flex h-4 w-4 shrink-0 items-center justify-center rounded text-ink3 opacity-0 hover:bg-fill2 hover:text-fg group-hover:opacity-100"
                onClick={(e) => {
                  e.stopPropagation();
                  void handleCloseTab(tab.id);
                }}
              >
                ×
              </button>
            ) : null}
          </div>
        ))}
        <button
          type="button"
          title="Mở tab workflow mới"
          aria-label="Mở tab workflow mới"
          className="ml-1 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-ink3 hover:bg-fill2 hover:text-fg"
          onClick={handleNewTab}
        >
          +
        </button>
      </div>
      <div className="relative flex-1 overflow-hidden">
        {tabs.map((tab) => (
          <div
            key={tab.id}
            className="absolute inset-0"
            style={{ display: tab.id === activeId ? "block" : "none" }}
          >
            <GraphStoreProvider store={tab.graphStore}>
              <RunStoreProvider store={tab.runStore}>
                <Workflow />
              </RunStoreProvider>
            </GraphStoreProvider>
          </div>
        ))}
      </div>
    </div>
  );
}
