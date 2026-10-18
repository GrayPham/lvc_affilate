// Hộp thoại "Đang kiểm tra N tài khoản" — dựng theo
// `references/lvc-ui-design/screens/CheckProgress.dc.html`.
//
// Chạy LẦN LƯỢT từng tài khoản, không song song: mỗi lần kiểm có thể mở trình duyệt thật, chạy
// đồng loạt sẽ nghẽn (cùng một lý do hàng chờ chạy tuần tự — xem `queue.rs`). Hộp thoại tự chủ vòng
// chạy; "Dừng kiểm tra" dừng sau tài khoản đang chạy (không cắt giữa chừng một lần kiểm đang mở
// trình duyệt — cắt nửa chừng để lại profile ở trạng thái dở).
//
// Logic thuần (ETA, nhãn/màu hàng) ở `check_progress.ts`, có test riêng.
import { useEffect, useRef, useState } from "react";

import { errorText } from "../ipc/error_text";
import { Button, Dialog, DialogFooter } from "../ui/primitives";
import {
  etaText,
  progressPercent,
  progressText,
  rowView,
  type CheckPhase,
} from "./check_progress";
import type { CredentialStatus, VerifyOutcome } from "../ipc/types";

export interface CheckItem {
  key: string;
  name: string;
  platform: string;
  /** Chạy một lần kiểm; trả trạng thái, hoặc ném khi lỗi hệ thống. */
  run: () => Promise<VerifyOutcome>;
}

interface RowState {
  phase: CheckPhase;
  status?: CredentialStatus;
  detail: string;
}

interface CheckProgressDialogProps {
  items: readonly CheckItem[];
  onClose: () => void;
  /** Danh tu dem trong tieu de ("tài khoản" / "key"…). */
  noun?: string;
  /** Giai thich vi sao chay tuan tu — khac nhau theo loai (trinh duyet vs. goi mang). */
  subtitle?: string;
}

export default function CheckProgressDialog({
  items,
  onClose,
  noun = "tài khoản",
  subtitle = "Chạy lần lượt từng tài khoản để không nghẽn trình duyệt.",
}: CheckProgressDialogProps) {
  const [rows, setRows] = useState<Record<string, RowState>>(() =>
    Object.fromEntries(items.map((i) => [i.key, { phase: "waiting", detail: "Chờ tới lượt" }])),
  );
  const [doneCount, setDoneCount] = useState(0);
  const [running, setRunning] = useState(true);
  const stopRef = useRef(false);
  const startRef = useRef(0);

  useEffect(() => {
    let cancelled = false;
    stopRef.current = false;
    startRef.current = Date.now();

    (async () => {
      let done = 0;
      for (const item of items) {
        if (cancelled || stopRef.current) {
          break;
        }
        setRows((r) => ({ ...r, [item.key]: { phase: "checking", detail: "Đang kiểm tra…" } }));
        try {
          const outcome = await item.run();
          if (cancelled) {
            return;
          }
          setRows((r) => ({
            ...r,
            [item.key]: { phase: "done", status: outcome.status, detail: outcome.detail },
          }));
        } catch (e) {
          if (cancelled) {
            return;
          }
          setRows((r) => ({ ...r, [item.key]: { phase: "failed", detail: errorText(e) } }));
        }
        done += 1;
        setDoneCount(done);
      }
      if (!cancelled) {
        setRunning(false);
      }
    })();

    return () => {
      cancelled = true;
    };
    // Chạy đúng một lần cho danh sách truyền vào lúc mở — `items` cố định suốt vòng đời hộp thoại.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const total = items.length;
  const eta = running ? etaText(doneCount, total, Date.now() - startRef.current) : null;

  function stop(): void {
    stopRef.current = true;
    setRunning(false);
  }

  return (
    <Dialog
      title={`Đang kiểm tra ${total} ${noun}`}
      subtitle={subtitle}
      width={560}
      onClose={onClose}
      footer={
        <DialogFooter hint="Đóng hộp thoại thì việc kiểm tra vẫn tiếp tục.">
          {running ? (
            <Button variant="danger" onClick={stop}>
              Dừng kiểm tra
            </Button>
          ) : null}
          <Button variant="primary" onClick={onClose}>
            {running ? "Ẩn, vẫn chạy" : "Xong"}
          </Button>
        </DialogFooter>
      }
    >
      <div className="flex flex-col gap-[7px]">
        <div className="flex justify-between text-[12.5px] text-ink2">
          <span>{progressText(doneCount, total)}</span>
          {eta ? <span>{eta}</span> : null}
        </div>
        <span className="block h-1.5 overflow-hidden rounded bg-fill2">
          <span
            className="block h-full bg-accent transition-[width] duration-300"
            style={{ width: `${progressPercent(doneCount, total)}%` }}
          />
        </span>
      </div>

      <ul className="m-0 list-none overflow-hidden rounded-xl border border-line p-0">
        {items.map((item) => {
          const st = rows[item.key] ?? { phase: "waiting" as const, detail: "" };
          const v = rowView(st.phase, st.status);
          const iconBg =
            st.phase === "checking" ? "bg-accent/15" : "bg-ink3/12";
          return (
            <li
              key={item.key}
              className="flex items-center gap-3 border-b border-line px-3.5 py-3 last:border-b-0"
            >
              <span
                className={
                  "grid h-[26px] w-[26px] flex-none place-items-center rounded-full text-[13px] font-bold " +
                  iconBg +
                  " " +
                  v.colorClass
                }
              >
                {v.icon}
              </span>
              <div className="min-w-0 flex-1">
                <div className="text-[13px] font-semibold text-fg">
                  {item.name} <span className="font-normal text-ink3">· {item.platform}</span>
                </div>
                <div className="mt-0.5 truncate text-[12px] text-ink3" title={st.detail}>
                  {st.detail}
                </div>
              </div>
              <span className={"text-[12.5px] font-semibold whitespace-nowrap " + v.colorClass}>
                {v.label}
              </span>
            </li>
          );
        })}
      </ul>
    </Dialog>
  );
}
