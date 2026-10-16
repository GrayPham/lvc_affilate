// Hộp thoại "Cập nhật workflow" — dựng theo `references/lvc-ui-design/screens/UpdateWorkflow.dc.html`.
//
// Chỉ hiện khi backend thực sự tìm ra bản mới (`workflowUpdateList`), nên mọi con số ở đây là thật:
// phiên bản cũ/mới lấy từ `workflow_origin` vs cache kho, "đã sửa trên máy" lấy từ so sánh đồ thị.
//
// Khối hỏi "Giữ cả hai / Ghi đè" CHỈ xuất hiện khi `locallyModified` — chưa sửa gì thì không có gì
// để mất, hỏi thêm một câu vô nghĩa chỉ làm chậm người dùng.
import { useState } from "react";

import { workflowApplyUpdate } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import { Button, Dialog, DialogFooter } from "../ui/primitives";
import type { UpdateMode, WorkflowUpdate } from "../ipc/types";

/** "5 ngày trước" / "3 giờ trước" / "vừa xong" từ mốc giây Unix. `0` = không biết → rỗng. */
export function timeAgo(unixSeconds: number, nowMs = Date.now()): string {
  if (unixSeconds <= 0) {
    return "";
  }
  const diffSec = Math.round(nowMs / 1000 - unixSeconds);
  if (diffSec < 60) {
    return "vừa xong";
  }
  const units: ReadonlyArray<readonly [number, string]> = [
    [60, "phút"],
    [3600, "giờ"],
    [86400, "ngày"],
    [2592000, "tháng"],
  ];
  // Tìm đơn vị LỚN NHẤT vẫn cho ra số >= 1.
  let chosen = units[0]!;
  for (const u of units) {
    if (diffSec >= u[0]) {
      chosen = u;
    }
  }
  if (diffSec >= 31536000) {
    return `${Math.floor(diffSec / 31536000)} năm trước`;
  }
  return `${Math.floor(diffSec / chosen[0])} ${chosen[1]} trước`;
}

/** Màu nhãn loại thay đổi: thêm = xanh, bỏ = hồng, còn lại (sửa) = tím. */
function kindClass(kind: string): string {
  const k = kind.toLowerCase();
  if (k.startsWith("thêm") || k.startsWith("them")) {
    return "bg-ok/15 text-ok";
  }
  if (k.startsWith("bỏ") || k.startsWith("bo") || k.startsWith("xoá") || k.startsWith("xoa")) {
    return "bg-rose/15 text-rose";
  }
  return "bg-tag-script/15 text-tag-script";
}

interface UpdateWorkflowDialogProps {
  update: WorkflowUpdate;
  onClose: () => void;
  /** Gọi sau khi áp xong, kèm id workflow kết quả (id cũ khi ghi đè, id mới khi giữ cả hai). */
  onApplied?: (workflowId: string, mode: UpdateMode) => void;
}

export default function UpdateWorkflowDialog({
  update,
  onClose,
  onApplied,
}: UpdateWorkflowDialogProps) {
  // Mặc định "Giữ cả hai": không bao giờ tự ý làm mất phần người dùng đã sửa.
  const [mode, setMode] = useState<UpdateMode>("keep_both");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleApply(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      const resultId = await workflowApplyUpdate(
        update.workflowId,
        // Chưa sửa gì trên máy thì không có gì phải giữ — ghi đè thẳng.
        update.locallyModified ? mode : "overwrite",
      );
      onApplied?.(resultId, mode);
      onClose();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  }

  const ago = timeAgo(update.updatedAt);
  const meta = [update.author, ago].filter((s) => s !== "").join(" · ");

  return (
    <Dialog
      title={`Cập nhật “${update.workflowName}”`}
      width={580}
      onClose={onClose}
      footer={
        <DialogFooter>
          <Button onClick={onClose} disabled={busy}>
            Để sau
          </Button>
          <Button variant="primary" disabled={busy} onClick={() => void handleApply()}>
            {busy ? "Đang tải…" : `Tải bản ${update.newVersion}`}
          </Button>
        </DialogFooter>
      }
    >
      <p className="text-[12.5px] text-ink3">
        <span className="font-mono">{update.currentVersion}</span>
        {" → "}
        <span className="font-mono text-accent">{update.newVersion}</span>
        {meta === "" ? null : ` · ${meta}`}
      </p>

      {update.changelog.length > 0 ? (
        <section className="flex flex-col gap-1.5">
          <h3 className="text-[11px] font-semibold tracking-[0.1em] text-ink3 uppercase">
            Thay đổi
          </h3>
          <ul className="flex flex-col gap-1.5">
            {update.changelog.map((c, i) => (
              <li
                key={`${c.kind}-${i}`}
                className="flex items-start gap-2.5 text-[13px] leading-[1.5]"
              >
                <span
                  className={
                    "min-w-[52px] flex-none rounded-sm px-1.5 py-0.5 text-center text-[11px] font-bold " +
                    kindClass(c.kind)
                  }
                >
                  {c.kind}
                </span>
                <span className="text-muted">{c.text}</span>
              </li>
            ))}
          </ul>
        </section>
      ) : (
        <p className="text-[12.5px] text-ink3">Tác giả không ghi danh sách thay đổi.</p>
      )}

      {update.locallyModified ? (
        <div className="flex gap-2.5 rounded-lg border border-primary/40 bg-primary/[0.07] p-3">
          <svg
            width="17"
            height="17"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinejoin="round"
            aria-hidden="true"
            className="mt-px flex-none text-accent"
          >
            <path d="M12 3 2 20h20L12 3Z" />
            <path d="M12 10v4M12 17v.5" strokeLinecap="round" />
          </svg>
          <div className="flex flex-1 flex-col gap-2">
            <span className="text-[13px] font-semibold text-fg">
              Bạn đã sửa bản trên máy
            </span>
            <div role="radiogroup" aria-label="Cách cập nhật" className="flex flex-col gap-1.5">
              {(
                [
                  [
                    "keep_both",
                    "Giữ cả hai",
                    `bản mới thêm vào “Của tôi” với tên “(${update.newVersion})”`,
                  ],
                  ["overwrite", "Ghi đè", "bỏ phần đã sửa trên máy"],
                ] as ReadonlyArray<readonly [UpdateMode, string, string]>
              ).map(([value, label, desc]) => (
                <label
                  key={value}
                  className={
                    "flex min-h-10 items-center gap-2.5 rounded-[9px] border bg-panel px-3 text-[13px] " +
                    (mode === value ? "border-primary/60" : "border-line2")
                  }
                >
                  <input
                    type="radio"
                    name="upMode"
                    className="h-4 w-4 accent-accent"
                    checked={mode === value}
                    disabled={busy}
                    onChange={() => setMode(value)}
                  />
                  <span className="text-fg">
                    {label} <span className="text-ink3">— {desc}</span>
                  </span>
                </label>
              ))}
            </div>
          </div>
        </div>
      ) : null}

      <p className="text-xs leading-[1.5] text-ink3">
        Workflow trong hàng chờ đang dùng bản {update.currentVersion} vẫn chạy bản cũ cho tới khi
        bạn thêm lại.
      </p>

      {error ? <p className="text-[12.5px] text-rose">{error}</p> : null}
    </Dialog>
  );
}
