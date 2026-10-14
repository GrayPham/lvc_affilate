// Màn "Cài đặt" — dựng theo `references/lvc-ui-design/screens/Settings.dc.html`: tiêu đề + chỉ báo
// "Đã lưu tự động", thanh điều hướng trái, nội dung từng nhóm là các thẻ.
//
// ## Tự lưu
// Bản bàn giao không có nút Lưu — chỉ báo "Đã lưu tự động". Ở đây tự lưu sau 700ms kể từ lần sửa
// cuối (gộp nhiều lần gõ thành MỘT lần ghi) và chỉ khi `draft` thực sự khác `saved` + không có lỗi
// hiển nhiên. Backend vẫn tự chuẩn hoá và trả `warnings`, nên một lần ghi sai giá trị không bao giờ
// làm hỏng các cài đặt khác (xem `AppSettings::from_map`).
//
// ## Nhóm nào có mặt
// Bản thiết kế vẽ 6 mục điều hướng; ở đây chỉ hiện 5 — mục "Thông báo" bị bỏ vì ứng dụng CHƯA có
// hệ thống thông báo nào để cấu hình, dựng một trang trống chỉ làm người dùng tưởng là có.
import { useEffect, useMemo, useRef, useState } from "react";

import { useSettingsStore } from "../store/settings";
import { validateSettings } from "../components/settings/validate";
import GeneralSettings from "../components/settings/GeneralSettings";
import StorageSettings from "../components/settings/StorageSettings";
import RunSettings from "../components/settings/RunSettings";
import BrowserSettings from "../components/settings/BrowserSettings";
import UpdateSettings from "../components/settings/UpdateSettings";

type SectionKey = "general" | "storage" | "run" | "browser" | "update";

const SECTIONS: ReadonlyArray<readonly [SectionKey, string]> = [
  ["general", "Chung"],
  ["storage", "Lưu trữ"],
  ["run", "Chạy tác vụ"],
  ["browser", "Trình duyệt"],
  ["update", "Cập nhật"],
];

/** Mỗi lần `draft` đổi thì hẹn lưu sau `delay`ms; lần sửa mới hoãn tiếp. */
function useAutoSave(delay = 700): void {
  const draft = useSettingsStore((s) => s.draft);
  const dirty = useSettingsStore((s) => s.dirty);
  const save = useSettingsStore((s) => s.save);
  const timer = useRef<number | null>(null);

  useEffect(() => {
    if (!dirty || !draft || validateSettings(draft).length > 0) {
      return;
    }
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
    }
    timer.current = window.setTimeout(() => {
      void save();
    }, delay);
    return () => {
      if (timer.current !== null) {
        window.clearTimeout(timer.current);
      }
    };
  }, [draft, dirty, save, delay]);
}

/** Chỉ báo bên phải tiêu đề: đang lưu / đã lưu / có lỗi. */
function SaveIndicator() {
  const saving = useSettingsStore((s) => s.saving);
  const dirty = useSettingsStore((s) => s.dirty);
  const error = useSettingsStore((s) => s.error);

  if (error) {
    return (
      <span className="inline-flex items-center gap-1.5 text-[12.5px] text-rose">
        <span className="h-[7px] w-[7px] rounded-full bg-rose" />
        Lưu thất bại
      </span>
    );
  }
  if (saving || dirty) {
    return (
      <span className="inline-flex items-center gap-1.5 text-[12.5px] text-ink3">
        <span className="h-[7px] w-[7px] rounded-full bg-ink3" />
        Đang lưu…
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1.5 text-[12.5px] text-ok">
      <span className="h-[7px] w-[7px] rounded-full bg-ok" />
      Đã lưu tự động
    </span>
  );
}

export default function SettingsPage() {
  const load = useSettingsStore((s) => s.load);
  const loading = useSettingsStore((s) => s.loading);
  const draft = useSettingsStore((s) => s.draft);
  const warnings = useSettingsStore((s) => s.warnings);
  const error = useSettingsStore((s) => s.error);
  const [section, setSection] = useState<SectionKey>("general");

  useEffect(() => {
    void load();
    // `load` la action zustand (tham chieu on dinh) — chi chay dung mot lan luc mount trang.
  }, []);

  useAutoSave();

  const errors = useMemo(() => (draft ? validateSettings(draft) : []), [draft]);

  return (
    <div className="flex h-full min-w-0 flex-col">
      <header className="flex flex-wrap items-start gap-2.5 border-b border-line px-6 pt-5 pb-4">
        <div className="min-w-[220px] flex-1">
          <h1 className="text-xl font-bold tracking-[-0.01em] text-ink">Cài đặt</h1>
          <p className="mt-1 text-[13px] text-ink3">
            Áp dụng cho toàn bộ workflow và tác vụ trên máy này.
          </p>
        </div>
        {draft ? <SaveIndicator /> : null}
      </header>

      <div className="flex min-h-0 flex-1 flex-wrap">
        <nav
          aria-label="Mục cài đặt"
          className="flex max-w-[200px] flex-[1_1_170px] flex-col gap-0.5 border-r border-line px-3 py-4"
        >
          {SECTIONS.map(([key, label]) => {
            const on = key === section;
            return (
              <button
                key={key}
                type="button"
                aria-current={on ? "page" : undefined}
                onClick={() => setSection(key)}
                className={
                  "flex min-h-[38px] items-center rounded-lg px-2.5 text-left text-[13px] " +
                  (on ? "bg-fill2 font-semibold text-ink" : "text-ink2 hover:text-ink")
                }
              >
                {label}
              </button>
            );
          })}
        </nav>

        <div className="flex min-w-0 flex-[999_1_480px] flex-col gap-4 overflow-auto px-6 pt-5 pb-7">
          {loading || !draft ? (
            <p className="text-[13px] text-ink3">Đang tải cài đặt…</p>
          ) : (
            <>
              {errors.length > 0 ? (
                <ul className="flex flex-col gap-1 rounded-md border border-rose/30 bg-rose/10 px-3 py-2.5 text-[12.5px] text-rose">
                  {errors.map((e) => (
                    <li key={e}>{e}</li>
                  ))}
                </ul>
              ) : null}
              {warnings.length > 0 ? (
                <ul className="flex flex-col gap-1 rounded-md border border-warn/30 bg-warn/10 px-3 py-2.5 text-[12.5px] text-ink2">
                  {warnings.map((w) => (
                    <li key={w}>{w}</li>
                  ))}
                </ul>
              ) : null}
              {error ? <p className="text-[12.5px] text-rose">{error}</p> : null}

              {section === "general" ? <GeneralSettings /> : null}
              {section === "storage" ? <StorageSettings /> : null}
              {section === "run" ? <RunSettings /> : null}
              {section === "browser" ? <BrowserSettings /> : null}
              {section === "update" ? <UpdateSettings /> : null}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
