import type { JSX, SVGProps } from "react";
import { useUiStore, type TabId } from "./store/uiStore";
import ScriptsPage from "./pages/ScriptsPage";
import WorkflowTabs from "./pages/WorkflowTabs";
import TasksPage from "./pages/TasksPage";
import SettingsPage from "./pages/SettingsPage";
import AccountsPage from "./pages/AccountsPage";
import CredentialsPage from "./pages/CredentialsPage";
import DialogHost from "./components/DialogHost";
import UpdateBanner from "./components/UpdateBanner";
import SidebarAccountFooter from "./components/SidebarAccountFooter";
import { TopBar } from "./ui/TopBar";

// Icon duong net don gian (inline SVG, khong them thu vien ngoai) — moi icon mot ham nho, dung
// chung stroke="currentColor" de doi mau theo trang thai active/hover cua nut cha.
function IconProps(props: SVGProps<SVGSVGElement>) {
  return {
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.6,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    ...props,
  };
}

function IconWorkflow(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...IconProps(props)}>
      <circle cx="5" cy="6" r="2.2" />
      <circle cx="5" cy="18" r="2.2" />
      <circle cx="13" cy="12" r="2.2" />
      <circle cx="20" cy="6" r="2.2" />
      <path d="M7 7l4 3M7 17l4-3M15 11l3-3.5" />
    </svg>
  );
}

function IconScript(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round" {...props}>
      <path d="M6 3h9l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Z" />
      <path d="M14 3v4a1 1 0 0 0 1 1h4" />
      <path d="M8 13h8M8 17h5" />
    </svg>
  );
}

function IconTasks(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...IconProps(props)}>
      <rect x="3.5" y="4" width="17" height="17" rx="2" />
      <path d="M7.5 11.5l2 2 4-4.5M7.5 17h5" />
    </svg>
  );
}

function IconSettings(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...IconProps(props)}>
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 13.5a7.97 7.97 0 0 0 0-3l2-1.4-2-3.4-2.3.9a8 8 0 0 0-2.6-1.5L14.1 2h-4.2l-.4 2.1a8 8 0 0 0-2.6 1.5l-2.3-.9-2 3.4 2 1.4a7.97 7.97 0 0 0 0 3l-2 1.4 2 3.4 2.3-.9c.77.65 1.65 1.17 2.6 1.5l.4 2.1h4.2l.4-2.1a8 8 0 0 0 2.6-1.5l2.3.9 2-3.4-2-1.4Z" />
    </svg>
  );
}

function IconUser(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...IconProps(props)}>
      <circle cx="12" cy="8" r="3.5" />
      <path d="M4.5 20.5c1.2-4 4-6 7.5-6s6.3 2 7.5 6" />
    </svg>
  );
}

function IconKey(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...IconProps(props)}>
      <circle cx="8" cy="15" r="3.5" />
      <path d="M10.5 12.5 18 5m0 0 2 2m-2-2-2.5 2.5 1.7 1.7" />
    </svg>
  );
}

/** Mui ten trai — nut thu gon/mo rong sidebar (xoay 180 do khi da thu gon). */
function IconChevronLeft(props: SVGProps<SVGSVGElement>) {
  return (
    <svg {...IconProps(props)}>
      <path d="M14.5 6.5 9 12l5.5 5.5" />
    </svg>
  );
}

// Thu tu + nhom theo ban ban giao (`references/lvc-ui-design/screens/Sidebar.dc.html`): nhom
// "Vận hành" gom Workflow · Kịch bản · Tác vụ · Tài khoản AI · API key; nhom "Khác" chi con Cài đặt
// (truoc day "API key" nam o nhom Khác — ban thiet ke chuyen len Vận hành vi no la kho tai khoan
// dung hang ngay, cung nhom voi "Tài khoản AI").
const TABS: ReadonlyArray<{
  id: TabId;
  label: string;
  icon: (p: SVGProps<SVGSVGElement>) => JSX.Element;
  group: "ops" | "other";
}> = [
  { id: "workflow", label: "Workflow", icon: IconWorkflow, group: "ops" },
  { id: "scripts", label: "Kịch bản", icon: IconScript, group: "ops" },
  { id: "tasks", label: "Tác vụ", icon: IconTasks, group: "ops" },
  { id: "accounts", label: "Tài khoản AI", icon: IconUser, group: "ops" },
  { id: "credentials", label: "API key", icon: IconKey, group: "ops" },
  { id: "settings", label: "Cài đặt", icon: IconSettings, group: "other" },
];

const GROUP_LABEL: Record<"ops" | "other", string> = {
  ops: "Vận hành",
  other: "Khác",
};

const PAGES: Record<TabId, () => JSX.Element> = {
  workflow: WorkflowTabs,
  scripts: ScriptsPage,
  tasks: TasksPage,
  settings: SettingsPage,
  accounts: AccountsPage,
  credentials: CredentialsPage,
};

export default function App() {
  const activeTab = useUiStore((s) => s.activeTab);
  const setActiveTab = useUiStore((s) => s.setActiveTab);
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  const ActivePage = PAGES[activeTab];

  const opsTabs = TABS.filter((t) => t.group === "ops");
  const otherTabs = TABS.filter((t) => t.group === "other");

  return (
    <div className="lvc-frame flex h-screen w-screen gap-2.5 p-2.5 text-fg">
      <aside
        className={
          "lvc-sidebar relative flex shrink-0 flex-col gap-[18px] rounded-[20px] px-3 py-4 text-sidebar-fg shadow-panel transition-[width] duration-200 " +
          (sidebarCollapsed ? "w-[68px]" : "w-[200px]")
        }
      >
        {/* Nut thu gon: nut tron NOI o mep phai thanh ben (ban thiet ke), khong phai muc trong nav. */}
        <button
          type="button"
          onClick={toggleSidebar}
          title={sidebarCollapsed ? "Mở rộng thanh bên" : "Thu gọn thanh bên"}
          aria-label={sidebarCollapsed ? "Mở rộng thanh bên" : "Thu gọn thanh bên"}
          className="absolute right-[-12px] top-[22px] z-10 grid h-6 w-6 place-items-center rounded-full border border-sidebar-line bg-sidebar-2 text-sidebar-muted shadow-card hover:text-sidebar-fg"
        >
          <IconChevronLeft
            className={"h-3 w-3 transition-transform " + (sidebarCollapsed ? "rotate-180" : "")}
          />
        </button>

        <div className={"flex items-center gap-2.5 pb-1 " + (sidebarCollapsed ? "justify-center" : "px-1")}>
          {/* Logo THAT cua app (`app/public/icon.png`) — cung anh dung lam icon cua so/taskbar,
              nen thanh ben va thanh tac vu khong bao gio lech nhau. */}
          <img
            src="/icon.png"
            alt="LVC Shop"
            className="h-8 w-8 shrink-0 rounded-md object-cover"
          />
          {sidebarCollapsed ? null : (
            <div className="flex min-w-0 flex-col leading-[1.25]">
              <span className="truncate text-[13px] font-bold text-sidebar-fg">LVC Shop</span>
              <span className="truncate text-[10.5px] text-sidebar-muted">Studio video bán hàng</span>
            </div>
          )}
        </div>

        <nav className="flex flex-1 flex-col gap-[18px] overflow-y-auto overflow-x-hidden">
          {([
            ["ops", opsTabs],
            ["other", otherTabs],
          ] as const).map(([group, tabs]) => (
            <div key={group} className="flex flex-col gap-0.5">
              {sidebarCollapsed ? (
                <span className="mx-2 mb-1.5 border-t border-sidebar-line" />
              ) : (
                <span className="px-2.5 pb-1.5 text-[10px] font-semibold uppercase tracking-[.12em] text-sidebar-muted">
                  {GROUP_LABEL[group]}
                </span>
              )}
              {tabs.map((tab) => {
                const Icon = tab.icon;
                const active = activeTab === tab.id;
                return (
                  <button
                    key={tab.id}
                    type="button"
                    onClick={() => setActiveTab(tab.id)}
                    aria-current={active ? "page" : undefined}
                    title={sidebarCollapsed ? tab.label : undefined}
                    className={
                      "flex min-h-10 items-center rounded-md text-left text-[13.5px] " +
                      (sidebarCollapsed ? "justify-center px-0" : "gap-2.5 px-2.5") +
                      " " +
                      (active
                        ? "bg-sidebar-active font-semibold text-sidebar-active-fg"
                        : "text-sidebar-muted hover:bg-sidebar-hover hover:text-sidebar-fg")
                    }
                  >
                    <Icon className="h-[17px] w-[17px] shrink-0" />
                    {sidebarCollapsed ? null : tab.label}
                  </button>
                );
              })}
            </div>
          ))}
        </nav>

        {sidebarCollapsed ? null : <SidebarAccountFooter />}
      </aside>
      <div className="flex min-w-0 flex-1 flex-col overflow-hidden rounded-[20px] bg-bg shadow-panel">
        <TopBar />
        <main className="relative min-h-0 flex-1 overflow-auto">
        {/* Workflow (P-tabs) phai luon giu MOUNTED — chuyen qua tab dieu huong khac roi quay lai
            KHONG duoc mat cac tab canvas/run dang theo doi (`WorkflowTabs` tu quan ly state qua
            `useState` noi bo, se mat het neu component nay bi unmount). An/hien qua CSS, khong
            render co dieu kien nhu cac trang khac. */}
        <div
          className="h-full"
          style={{ display: activeTab === "workflow" ? "block" : "none" }}
        >
          <WorkflowTabs />
        </div>
        {activeTab === "workflow" ? null : <ActivePage />}
      </main>
      </div>
      {/* Dialog trong-app (thay window.confirm/prompt/alert) — mount mot lan cho ca app. */}
      <DialogHost />
      {/* Banner auto-update (P8) — tu check ngam luc mo app, mount mot lan. */}
      <UpdateBanner />
    </div>
  );
}
