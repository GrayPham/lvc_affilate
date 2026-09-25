import type { JSX } from "react";
import { useUiStore, type TabId } from "./store/uiStore";
import Workflow from "./pages/Workflow";
import ScriptsPage from "./pages/ScriptsPage";
import TasksPage from "./pages/TasksPage";
import SettingsPage from "./pages/SettingsPage";
import AccountsPage from "./pages/AccountsPage";
import CredentialsPage from "./pages/CredentialsPage";

// P3c Task 2: tab "Kịch bản" dat NGAY SAU "Workflow". P4c: them "Tài khoản AI" (kho tai khoan AI)
// SAU "Tài khoản" (Dola) — giu tab Dola cu nguyen ven.
const TABS: ReadonlyArray<{ id: TabId; label: string }> = [
  { id: "workflow", label: "Workflow" },
  { id: "scripts", label: "Kịch bản" },
  { id: "tasks", label: "Tác vụ" },
  { id: "settings", label: "Cài đặt" },
  { id: "accounts", label: "Tài khoản" },
  { id: "credentials", label: "Tài khoản AI" },
];

const PAGES: Record<TabId, () => JSX.Element> = {
  workflow: Workflow,
  scripts: ScriptsPage,
  tasks: TasksPage,
  settings: SettingsPage,
  accounts: AccountsPage,
  credentials: CredentialsPage,
};

export default function App() {
  const activeTab = useUiStore((s) => s.activeTab);
  const setActiveTab = useUiStore((s) => s.setActiveTab);
  const ActivePage = PAGES[activeTab];

  return (
    <div className="flex h-screen w-screen flex-col bg-slate-50 text-slate-900">
      <header className="flex items-center gap-6 border-b border-slate-200 bg-white px-4 py-2">
        <h1 className="text-base font-semibold tracking-tight">LVC Seedance</h1>
        <nav className="flex gap-1">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(tab.id)}
              className={
                "rounded-md px-3 py-1.5 text-sm font-medium transition-colors " +
                (activeTab === tab.id
                  ? "bg-slate-900 text-white"
                  : "text-slate-600 hover:bg-slate-100")
              }
              aria-current={activeTab === tab.id ? "page" : undefined}
            >
              {tab.label}
            </button>
          ))}
        </nav>
      </header>
      <main className="flex-1 overflow-auto">
        <ActivePage />
      </main>
    </div>
  );
}
