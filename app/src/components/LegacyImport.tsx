// "Nhập dữ liệu từ bản Python" (Task 13 brief) — chon thu muc bang Python cu (`pick_directory`,
// da co trong `src/ipc/commands.ts`) -> xac nhan -> `legacy_import` -> hien bao cao.
//
// `legacy_import` (lenh Tauri + kieu `LegacyImportReport`) DA CO tren nhanh Rust
// (`crates/lvc-app/src/commands/accounts.rs`, `crates/lvc-engine/src/accounts.rs`) nhung CHUA
// duoc boc trong `src/ipc/commands.ts` tinh den LUC BAT DAU viet file nay (Task 14 — man Tai
// khoan — dang so huu file do trong dot nay, xem brief Task 13). Theo dung huong dan brief: goi
// thang `invoke("legacy_import", { pythonDir })` qua shim `src/ipc/tauri.ts` (KHONG dung
// `@tauri-apps/api/core` truc tiep), tu dinh nghia zod schema o day khop CHINH XAC
// `LegacyImportReport` (crates/lvc-engine/src/accounts.rs, doc source that vi fixture
// `app/src/ipc/fixtures/import_reports.json` cua Task 10 KHONG khop struct hien tai — xem Concerns
// trong report Task 13):
//   pub struct LegacyImportReport {
//       pub tasks: usize, pub accounts: usize, pub usage: usize, pub settings: usize,
//       pub skipped: Vec<String>, pub profiles_copied: usize, pub profiles_skipped: Vec<String>,
//   }
// (`#[serde(rename_all = "camelCase")]` -> wire: tasks/accounts/usage/settings/skipped/
// profilesCopied/profilesSkipped).
//
// CAP NHAT (cuoi phien lam Task 13): Task 14 vua them `legacyImport()`/`LegacyImportReportSchema`
// vao `src/ipc/commands.ts`/`src/ipc/types.ts` GIUA LUC dang lam task nay — kieu hoan toan khop
// ban tay o day (da doi chieu tung truong). CO CHU DICH KHONG doi sang dung ban do vi thoi diem do
// `commands.ts`/`types.ts` van dang duoc agent kia sua tiep (working tree KHONG worktree rieng —
// `pnpm typecheck` dang do vi mot file khac cua ho, `format/account.ts`), va brief Task 13 noi ro
// giu ban wrapper rieng roi bao cao de controller gop — xem "Concerns" trong task-13-report.md.
// Viec don gian hoa (xoa ham/schema o day, doi sang `import { legacyImport } from "../ipc/commands"`)
// an toan de lam sau, khi nhanh Task 14 on dinh.
import { useState } from "react";
import { z } from "zod";

import { invoke } from "../ipc/tauri";
import { pickDirectory } from "../ipc/commands";
import { errorText } from "../ipc/error_text";

const LegacyImportReportSchema = z.object({
  tasks: z.number(),
  accounts: z.number(),
  usage: z.number(),
  settings: z.number(),
  skipped: z.array(z.string()),
  profilesCopied: z.number(),
  profilesSkipped: z.array(z.string()),
});
type LegacyImportReport = z.infer<typeof LegacyImportReportSchema>;

/** Boc tay `legacy_import` — xem doc dau file. */
async function legacyImport(pythonDir: string): Promise<LegacyImportReport> {
  const res = await invoke<unknown>("legacy_import", { pythonDir });
  return LegacyImportReportSchema.parse(res);
}

export default function LegacyImport() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<LegacyImportReport | null>(null);

  async function handleClick(): Promise<void> {
    setError(null);
    setReport(null);
    const dir = await pickDirectory().catch((e: unknown) => {
      setError(errorText(e));
      return null;
    });
    if (dir === null) {
      return;
    }
    const confirmed = window.confirm(
      `Nhập dữ liệu từ bản Python cũ tại:\n"${dir}"\n\n` +
        "Sẽ đọc DB task/tài khoản/lượt dùng/cài đặt của bản cũ và ghi vào dữ liệu hiện tại " +
        "(hồ sơ trình duyệt tài khoản đã có sẵn ở đây sẽ KHÔNG bị ghi đè). Tiếp tục?",
    );
    if (!confirmed) {
      return;
    }
    setBusy(true);
    try {
      const r = await legacyImport(dir);
      setReport(r);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="flex flex-col gap-2 border-t border-slate-200 pt-4">
      <p className="text-sm font-medium">Nhập dữ liệu từ bản Python</p>
      <p className="text-xs text-slate-500">
        Chọn thư mục cài đặt bản Python cũ để nhập task/tài khoản/lượt dùng/cài đặt vào ứng dụng này.
      </p>
      <div>
        <button
          type="button"
          disabled={busy}
          className="rounded border border-slate-300 px-3 py-1.5 text-sm hover:bg-slate-50 disabled:opacity-50"
          onClick={() => void handleClick()}
        >
          {busy ? "Đang nhập…" : "Nhập dữ liệu từ bản Python"}
        </button>
      </div>

      {error ? <p className="text-xs text-red-600">{error}</p> : null}

      {report ? (
        <div className="rounded border border-emerald-200 bg-emerald-50 p-2 text-xs text-emerald-800">
          <p>
            Đã nhập {report.tasks} task, {report.accounts} tài khoản, {report.usage} lượt dùng,{" "}
            {report.settings} cài đặt.
          </p>
          <p>
            Đã sao chép {report.profilesCopied} hồ sơ trình duyệt
            {report.profilesSkipped.length > 0
              ? ` (bỏ qua ${report.profilesSkipped.length}: ${report.profilesSkipped.join(", ")})`
              : ""}
            .
          </p>
          {report.skipped.length > 0 ? <p>Bỏ qua file cũ: {report.skipped.join(", ")}.</p> : null}
        </div>
      ) : null}
    </section>
  );
}
