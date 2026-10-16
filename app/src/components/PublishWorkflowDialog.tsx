// Dialog "Publish workflow lên GitHub" — dung theo ban ban giao
// `references/lvc-ui-design/screens/PublishWorkflow.dc.html`.
//
// Mo mot PULL REQUEST vao kho cong dong, KHONG day thang vao nhanh chinh — chu repo duyet truoc.
// Token la personal access token do CHINH nguoi dung tao, luu ma hoa o tab API key
// (`provider = "github"`); dialog nay chi chon ID credential, KHONG BAO GIO cham plaintext.
import { useEffect, useMemo, useState } from "react";

import { useCredentialsStore } from "../store/credentials";
import { publishWorkflow } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import { Button, Dialog, DialogFooter, Field, INPUT_CLASS } from "../ui/primitives";
import type { Graph, PublishResult } from "../ipc/types";

/** Nhom goi y — khop `category` ma Kho mau dang dung. */
const CATEGORIES = ["Affiliate", "Bán hàng/Sản phẩm", "Storyboard", "Cơ bản"] as const;

/** Anh bia co san trong `app/public/covers/` (nguon ban ban giao). */
const COVERS = [
  "g_live",
  "g_food",
  "g_fashion",
  "g_tech",
  "review",
  "hook",
  "char",
  "testi",
  "beforeafter",
  "demo",
  "unbox",
  "story",
  "multi",
  "basic",
] as const;

interface PublishWorkflowDialogProps {
  /** Id on dinh cua muc tren kho cong dong — trung id = cap nhat ban da publish. */
  defaultId: string;
  defaultName: string;
  graph: Graph;
  onClose: () => void;
}

export default function PublishWorkflowDialog({
  defaultId,
  defaultName,
  graph,
  onClose,
}: PublishWorkflowDialogProps) {
  const rows = useCredentialsStore((s) => s.rows);
  const loadCred = useCredentialsStore((s) => s.load);

  const [id, setId] = useState(defaultId);
  const [name, setName] = useState(defaultName);
  const [description, setDescription] = useState("");
  const [category, setCategory] = useState<string>(CATEGORIES[0]);
  const [cover, setCover] = useState<string>(COVERS[0]);
  const [version, setVersion] = useState("1.0.0");
  const [author, setAuthor] = useState("");
  const [changelog, setChangelog] = useState("");
  const [credentialId, setCredentialId] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<PublishResult | null>(null);

  useEffect(() => {
    void loadCred();
  }, [loadCred]);

  /** Chi token GitHub con bat — khong liet ke API key cua nha cung cap AI. */
  const tokens = useMemo(
    () => rows.filter((r) => r.provider === "github" && r.kind === "api_key" && r.enabled),
    [rows],
  );

  useEffect(() => {
    if (credentialId === "" && tokens.length > 0) {
      setCredentialId(tokens[0]!.id);
    }
  }, [tokens, credentialId]);

  async function handlePublish(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      const out = await publishWorkflow(credentialId, {
        id: id.trim(),
        name: name.trim(),
        description: description.trim(),
        category,
        cover,
        version: version.trim(),
        author: author.trim(),
        changelog,
        graph,
      });
      setResult(out);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  const canPublish =
    credentialId !== "" && id.trim() !== "" && name.trim() !== "" && version.trim() !== "" && !busy;

  // Sau khi mo PR thanh cong -> man ket qua, khong cho bam publish lan nua.
  if (result) {
    return (
      <Dialog
        title="Đã mở Pull Request"
        width={560}
        onClose={onClose}
        footer={
          <DialogFooter>
            <Button variant="primary" onClick={onClose}>
              Xong
            </Button>
          </DialogFooter>
        }
      >
        <p className="text-[13px] leading-[1.55] text-muted">
          {result.updatedExisting
            ? "Đã cập nhật mục cùng id trong kho cộng đồng."
            : "Đã thêm một mục mới vào kho cộng đồng."}{" "}
          Workflow chỉ xuất hiện ở tab "Miễn phí" sau khi chủ kho duyệt và gộp Pull Request này.
        </p>
        <div className="flex flex-col gap-1.5 rounded-md border border-line bg-fill p-3 text-[12.5px]">
          <span className="text-ink3">Nhánh</span>
          <span className="font-mono text-fg">{result.branch}</span>
        </div>
        <a
          href={result.pullRequestUrl}
          target="_blank"
          rel="noreferrer"
          className="break-all text-[12.5px] font-semibold text-accent hover:underline"
        >
          {result.pullRequestUrl}
        </a>
      </Dialog>
    );
  }

  return (
    <Dialog
      title="Publish workflow lên GitHub"
      width={620}
      onClose={onClose}
      footer={
        <DialogFooter hint="Workflow chỉ lên kho chung sau khi chủ kho duyệt Pull Request.">
          <Button onClick={onClose} disabled={busy}>
            Huỷ
          </Button>
          <Button variant="primary" disabled={!canPublish} onClick={() => void handlePublish()}>
            {busy ? "Đang mở PR…" : `Publish ${version.trim()}`}
          </Button>
        </DialogFooter>
      }
    >
      {tokens.length === 0 ? (
        <p className="rounded-md border border-warn/40 bg-warn/10 px-3 py-2.5 text-[12.5px] leading-[1.5] text-muted">
          Chưa có token GitHub nào trong kho. Sang tab <strong className="text-fg">API key</strong>{" "}
          → "+ Thêm API key" → chọn nhà cung cấp <strong className="text-fg">GitHub</strong> rồi dán
          personal access token có quyền ghi repo.
        </p>
      ) : (
        <Field label="Token GitHub" required>
          <select
            className={INPUT_CLASS}
            value={credentialId}
            onChange={(e) => setCredentialId(e.target.value)}
          >
            {tokens.map((t) => (
              <option key={t.id} value={t.id} className="bg-panel text-fg">
                {t.label} · {t.secretHint}
              </option>
            ))}
          </select>
        </Field>
      )}

      <div className="grid grid-cols-2 gap-3">
        <Field label="Tên hiển thị" required>
          <input className={INPUT_CLASS} value={name} onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label="Id trên kho" required hint="trùng id = cập nhật bản cũ">
          <input
            className={`${INPUT_CLASS} font-mono text-[12.5px]`}
            value={id}
            onChange={(e) => setId(e.target.value)}
          />
        </Field>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <Field label="Nhóm">
          <select
            className={INPUT_CLASS}
            value={category}
            onChange={(e) => setCategory(e.target.value)}
          >
            {CATEGORIES.map((c) => (
              <option key={c} value={c} className="bg-panel text-fg">
                {c}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Phiên bản" required>
          <input
            className={`${INPUT_CLASS} font-mono text-[12.5px]`}
            value={version}
            onChange={(e) => setVersion(e.target.value)}
          />
        </Field>
      </div>

      <Field label="Mô tả ngắn">
        <textarea
          rows={2}
          className={`${INPUT_CLASS} resize-y py-2`}
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="Một câu mô tả workflow này làm gì."
        />
      </Field>

      <Field label="Tên tác giả" hint="hiện cho người tải về biết ai làm">
        <input
          className={INPUT_CLASS}
          value={author}
          onChange={(e) => setAuthor(e.target.value)}
          placeholder="Để trống nếu không muốn hiện."
        />
      </Field>

      <Field
        label="Thay đổi ở bản này"
        hint="mỗi dòng một thay đổi; người đang dùng bản cũ sẽ đọc đúng danh sách này"
      >
        <textarea
          rows={3}
          className={`${INPUT_CLASS} resize-y py-2 font-mono text-[12.5px]`}
          value={changelog}
          onChange={(e) => setChangelog(e.target.value)}
          placeholder={"Thêm: node Chèn phụ đề sau cảnh 3\nSửa: prompt cảnh 2 nhấn mạnh hơi nóng"}
        />
      </Field>

      <Field label="Ảnh bìa">
        <div className="flex flex-wrap gap-2">
          {COVERS.map((c) => (
            <button
              key={c}
              type="button"
              aria-pressed={cover === c}
              onClick={() => setCover(c)}
              className={
                "h-[42px] w-[66px] overflow-hidden rounded-sm border " +
                (cover === c ? "border-primary" : "border-line2 hover:border-ink3")
              }
            >
              <img src={`/covers/${c}.svg`} alt={c} loading="lazy" className="h-full w-full object-cover" />
            </button>
          ))}
        </div>
      </Field>

      <p className="text-[12px] leading-[1.5] text-ink3">
        Trước khi publish, kiểm tra đồ thị không còn tên tài khoản/API key hay đường dẫn riêng trên
        máy bạn — những giá trị đó sẽ công khai cho mọi người tải về.
      </p>

      {error ? <p className="text-[12.5px] text-rose">{error}</p> : null}
    </Dialog>
  );
}
