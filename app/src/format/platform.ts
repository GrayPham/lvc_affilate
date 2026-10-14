// Nhan dien NEN TANG trong bang tai khoan (chu cai + mau + dong phu) — dung theo ban ban giao
// `references/lvc-ui-design/screens/Main.dc.html` (bang `P` trong `renderVals`). THUAN, test duoc.
import type { CredentialProvider } from "../ipc/types";

interface PlatformLook {
  /** Ten hien thi tren bang. */
  name: string;
  /** Chu cai trong o vuong mau. */
  letter: string;
  /** Mau chu + nen (nen = mau nay o 15% alpha). */
  color: string;
  /** Dong phu duoi ten nen tang. */
  sub: string;
}

const COOKIE_SUB = "Đăng nhập cookie";

/** Khoa = `CredentialProvider` cua backend + `"dola"` (kho tai khoan Dola rieng). */
export const PLATFORM_LOOK: Record<string, PlatformLook> = {
  dola: { name: "Dola", letter: "D", color: "#A78BFA", sub: "Tài khoản Dola" },
  facebook: { name: "Facebook", letter: "f", color: "#60A5FA", sub: COOKIE_SUB },
  google: { name: "Google", letter: "G", color: "#38BDF8", sub: COOKIE_SUB },
  apple: { name: "Apple", letter: "A", color: "#A1A1AA", sub: COOKIE_SUB },
  museai: { name: "MuseAI", letter: "M", color: "#E879F9", sub: COOKIE_SUB },
  gemini: { name: "Gemini", letter: "G", color: "#4F8DF7", sub: COOKIE_SUB },
  chatgpt_web: { name: "ChatGPT", letter: "C", color: "#10A37F", sub: COOKIE_SUB },
  openai: { name: "OpenAI", letter: "O", color: "#10A37F", sub: "API key" },
  seedance: { name: "Seedance", letter: "S", color: "#F472B6", sub: "API key" },
  grok: { name: "Grok", letter: "X", color: "#A1A1AA", sub: "API key" },
  elevenlabs: { name: "ElevenLabs", letter: "E", color: "#A1A1AA", sub: "API key" },
  minimax: { name: "MiniMax", letter: "M", color: "#4D6BFE", sub: "API key" },
};

/** Nen tang la chua biet -> o xam, chu cai dau ten. */
export function platformLook(provider: CredentialProvider | "dola"): PlatformLook {
  return (
    PLATFORM_LOOK[provider] ?? {
      name: provider,
      letter: provider.slice(0, 1).toUpperCase(),
      color: "#8A8882",
      sub: COOKIE_SUB,
    }
  );
}

/** Nen o vuong = mau nen tang o 15% (hex `26`), giong ban thiet ke. */
export function platformBadgeBg(color: string): string {
  return `${color}26`;
}
