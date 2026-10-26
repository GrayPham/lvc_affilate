// Bảng tra từ điển theo mã ngôn ngữ — nơi DUY NHẤT ghép ngôn ngữ vào từ điển.
import { en } from "./en";
import type { DictKey } from "./vi";
import { vi } from "./vi";
import type { Lang } from "./types";

export const DICTS: Record<Lang, Record<DictKey, string>> = { vi, en };
