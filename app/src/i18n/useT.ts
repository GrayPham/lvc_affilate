// Hook dịch cho component: `const t = useT(); t("gate.exit")`. Đổi ngôn ngữ ở store là mọi
// component dùng hook này tự vẽ lại.
import { useCallback } from "react";

import { useLangStore } from "./store";
import { translate } from "./translate";
import type { Vars } from "./types";
import type { DictKey } from "./vi";

export function useT(): (key: DictKey, vars?: Vars) => string {
  const lang = useLangStore((s) => s.lang);
  return useCallback((key: DictKey, vars?: Vars) => translate(lang, key, vars), [lang]);
}
