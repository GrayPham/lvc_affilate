// "Nhập dữ liệu từ bản Python" (Task 13 brief) — chon thu muc bang Python cu (`pick_directory`)
// -> xac nhan -> `legacy_import` -> hien bao cao. `legacyImport`/`LegacyImportReport` dung chung
// tu `src/ipc/commands.ts`/`src/ipc/types.ts` (Task 14 da them, review Task 13 fix round 1 I1:
// xoa ban wrapper/schema tay tung co o day de tranh trung lap voi ban chinh thuc).
import { useState } from "react";
import { confirmDialog } from "../store/dialog";

import { legacyImport, pickDirectory } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import type { LegacyImportReport } from "../ipc/types";
import { BTN_NEUTRAL_SM } from "../ui/buttonStyles";

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
    const confirmed = await confirmDialog({
      title: "Nhập dữ liệu bản Python",
      message:
        `Nhập dữ liệu từ bản Python cũ tại:\n"${dir}"\n\n` +
        "Sẽ đọc DB task/tài khoản/lượt dùng/cài đặt của bản cũ và ghi vào dữ liệu hiện tại " +
        "(hồ sơ trình duyệt tài khoản đã có sẵn ở đây sẽ KHÔNG bị ghi đè). Thư mục bản Python cũ " +
        "chỉ được ĐỌC (read-only) và giữ nguyên. Tiếp tục?",
      okText: "Nhập",
    });
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
    <section className="flex flex-col gap-2 border-t border-line pt-4 text-ink">
      <p className="text-sm font-medium text-ink">Nhập dữ liệu từ bản Python</p>
      <p className="text-xs text-ink3">
        Chọn thư mục cài đặt bản Python cũ để nhập task/tài khoản/lượt dùng/cài đặt vào ứng dụng này.
      </p>
      <div>
        <button
          type="button"
          disabled={busy}
          className={BTN_NEUTRAL_SM}
          onClick={() => void handleClick()}
        >
          {busy ? "Đang nhập…" : "Nhập dữ liệu từ bản Python"}
        </button>
      </div>

      {error ? <p className="text-xs text-rose">{error}</p> : null}

      {report ? (
        <div className="rounded border border-ok/35 bg-ok/10 p-2 text-xs text-ok">
          <p>
            Đã nhập {report.tasks} task, {report.accounts} tài khoản, {report.usage} lượt dùng,{" "}
            {report.settings} cài đặt, {report.scripts} kịch bản ({report.scenes} cảnh).
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
