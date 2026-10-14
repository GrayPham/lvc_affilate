// Dialog "Thêm API key" — dung theo `references/lvc-ui-design/screens/AddApiKey.dc.html`.
//
// AN TOAN BI MAT: o nhap key GHI-MOT-CHIEU — gui thang xuong IPC khi Luu, KHONG luu vao state nao
// khac, KHONG log. Sau khi luu, chuoi key bi xoa khoi bo nho form ngay.
import { useState } from "react";

import { useCredentialsStore } from "../store/credentials";
import { describeCredentialError } from "../format/credential";
import { platformBadgeBg, platformLook } from "../format/platform";
import {
  Button,
  Dialog,
  DialogFooter,
  Field,
  INPUT_CLASS,
  NodeTagPicker,
  SelectCard,
  type NodeTypeKey,
} from "../ui/primitives";
import type { CredentialProvider } from "../ipc/types";

/** Nha cung cap co API key — khop `API_PROVIDERS` + ban thiet ke. */
const PROVIDERS: ReadonlyArray<CredentialProvider> = [
  "openai",
  "gemini",
  "seedance",
  "grok",
  "elevenlabs",
  "minimax",
];

/** Goi y tag loai node theo kha nang THUC TE cua tung nha cung cap. */
const SUGGESTED: Record<string, NodeTypeKey[]> = {
  openai: ["script"],
  gemini: ["script", "image"],
  seedance: ["video", "image"],
  grok: ["image"],
  elevenlabs: [],
  minimax: ["video"],
};

export default function AddApiKeyDialog({ onClose }: { onClose: () => void }) {
  const add = useCredentialsStore((s) => s.add);
  const setNodeTypes = useCredentialsStore((s) => s.setNodeTypes);

  const [provider, setProvider] = useState<CredentialProvider>("gemini");
  const [label, setLabel] = useState("");
  const [secret, setSecret] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [reveal, setReveal] = useState(false);
  const [nodes, setNodes] = useState<ReadonlySet<NodeTypeKey>>(new Set(SUGGESTED.gemini));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function pickProvider(p: CredentialProvider): void {
    setProvider(p);
    setNodes(new Set(SUGGESTED[p] ?? []));
  }

  async function handleSave(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      const meta: Record<string, unknown> = {};
      if (baseUrl.trim() !== "") {
        meta.base_url = baseUrl.trim();
      }
      const view = await add({
        provider,
        kind: "api_key",
        label: label.trim(),
        secret,
        meta,
      });
      if (nodes.size > 0) {
        await setNodeTypes(view.id, [...nodes]);
      }
      setSecret(""); // Xoa bi mat khoi bo nho form NGAY sau khi gui.
      onClose();
    } catch (e) {
      setError(describeCredentialError(e));
    } finally {
      setBusy(false);
    }
  }

  const canSave = label.trim() !== "" && secret.trim() !== "" && !busy;

  return (
    <Dialog
      title="Thêm API key"
      width={600}
      onClose={onClose}
      footer={
        <DialogFooter>
          <Button onClick={onClose} disabled={busy}>
            Huỷ
          </Button>
          <Button variant="primary" disabled={!canSave} onClick={() => void handleSave()}>
            {busy ? "Đang lưu…" : "Lưu key"}
          </Button>
        </DialogFooter>
      }
    >
      <fieldset className="m-0 border-none p-0">
        <legend className="mb-2 p-0 text-[12.5px] font-semibold text-ink2">
          Nhà cung cấp <span className="text-rose">*</span>
        </legend>
        <div className="grid grid-cols-3 gap-2">
          {PROVIDERS.map((p) => {
            const look = platformLook(p);
            return (
              <SelectCard key={p} selected={provider === p} ring onClick={() => pickProvider(p)}>
                <span
                  className="grid h-[26px] w-[26px] shrink-0 place-items-center rounded-sm text-[12px] font-bold"
                  style={{ color: look.color, background: platformBadgeBg(look.color) }}
                >
                  {look.letter}
                </span>
                <span className="truncate text-[13px] font-semibold">{look.name}</span>
              </SelectCard>
            );
          })}
        </div>
      </fieldset>

      <div className="grid grid-cols-2 gap-3">
        <Field label="Tên key" required>
          <input
            className={INPUT_CLASS}
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder={`${provider}-01`}
          />
        </Field>
        <Field label="Endpoint tuỳ chỉnh" hint="bỏ trống = chính thức">
          <input
            className={INPUT_CLASS}
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="https://…"
          />
        </Field>
      </div>

      <Field label="API key" required>
        <div className="flex gap-1.5">
          <div className="flex min-h-10 flex-1 items-center rounded-md border border-line2 bg-fill pl-3 pr-1">
            <input
              type={reveal ? "text" : "password"}
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
              placeholder="Dán key vào đây"
              autoComplete="off"
              className="min-w-0 flex-1 border-none bg-transparent font-mono text-[12.5px] text-ink outline-none placeholder:text-ink3"
            />
            <button
              type="button"
              aria-label={reveal ? "Ẩn key" : "Hiện key"}
              onClick={() => setReveal((v) => !v)}
              className="grid h-8 w-8 place-items-center rounded-sm text-ink3 hover:text-ink"
            >
              <svg viewBox="0 0 24 24" className="h-[15px] w-[15px]" fill="none" stroke="currentColor" strokeWidth={1.8}>
                <path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12Z" />
                <circle cx="12" cy="12" r="3" />
              </svg>
            </button>
          </div>
        </div>
      </Field>
      <p className="-mt-2 text-[12px] text-ink3">
        Key chỉ lưu mã hoá trên máy này, không gửi lên GitHub khi publish workflow.
      </p>

      <fieldset className="m-0 border-none p-0">
        <legend className="mb-2 p-0 text-[12.5px] font-semibold text-ink2">
          Dùng cho node <span className="font-normal text-ink3">gợi ý theo khả năng của key</span>
        </legend>
        <NodeTagPicker value={nodes} onChange={setNodes} />
      </fieldset>

      {error ? <p className="text-[12.5px] text-rose">{error}</p> : null}
    </Dialog>
  );
}
