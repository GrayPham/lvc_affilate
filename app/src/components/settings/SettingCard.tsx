// Khung dùng chung cho mọi nhóm cài đặt — thẻ có tiêu đề + mô tả, bên trong là các hàng
// "nhãn + mô tả ngắn | điều khiển", đúng bản bàn giao `references/lvc-ui-design/screens/Settings.dc.html`.
//
// Tách ra file riêng vì cả 5 nhóm (Chung/Lưu trữ/Chạy tác vụ/Trình duyệt/Cập nhật) dùng y hệt;
// sửa khoảng cách một chỗ là cả màn đổi theo.
import type { ReactNode } from "react";

export function SettingCard({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="rounded-xl border border-line bg-panel">
      <header className="flex flex-wrap items-center gap-2 border-b border-line px-4 py-[13px]">
        <h2 className="flex-1 text-sm font-bold text-fg">{title}</h2>
        {description ? <span className="text-xs text-ink3">{description}</span> : null}
      </header>
      {children}
    </section>
  );
}

/** Một hàng cài đặt. `last` bỏ đường kẻ dưới (hàng cuối của thẻ). */
export function SettingRow({
  label,
  hint,
  htmlFor,
  last = false,
  children,
}: {
  label: string;
  hint?: string;
  /** Có giá trị thì nhãn thành `<label for>` — bấm nhãn là con trỏ vào đúng ô. */
  htmlFor?: string;
  last?: boolean;
  children: ReactNode;
}) {
  const Label = htmlFor ? "label" : "span";
  return (
    <div
      className={
        "flex flex-wrap gap-4 px-4 py-3.5 " + (last ? "" : "border-b border-line")
      }
    >
      <div className="flex-[1_1_200px] max-w-[240px]">
        <Label
          {...(htmlFor ? { htmlFor } : {})}
          className="block text-[13px] font-semibold text-fg"
        >
          {label}
        </Label>
        {hint ? <span className="mt-0.5 block text-xs leading-[1.5] text-ink3">{hint}</span> : null}
      </div>
      <div className="flex min-w-0 flex-[999_1_300px] flex-col gap-1.5">{children}</div>
    </div>
  );
}

/** Dòng chú thích nhỏ dưới một điều khiển (ví dụ, trạng thái, xem trước). */
export function RowNote({ children, mono = false }: { children: ReactNode; mono?: boolean }) {
  return (
    <span className={"text-xs text-ink3" + (mono ? " font-mono" : "")}>{children}</span>
  );
}
