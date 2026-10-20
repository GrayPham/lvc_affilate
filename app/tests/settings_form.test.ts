// `validateSettings` (src/components/settings/validate.ts) mirror hai dieu kien server CHAC CHAN
// se bac (`crates/lvc-core/src/settings/mod.rs::parse_threads`/`parse_duration`) + `toAppSettings`
// (src/store/settings.ts) chuan hoa phong thu truoc khi goi `settingsSet`.
import { describe, expect, it } from "vitest";

import { validateSettings } from "../src/components/settings/validate";
import { THREADS_MAX, toAppSettings } from "../src/store/settings";
import type { AppSettings } from "../src/ipc/types";

/** Khop `AppSettings::default()` (crates/lvc-core/src/settings/mod.rs,
 * fixtures/app_settings.json). */
function defaultSettings(overrides: Partial<AppSettings> = {}): AppSettings {
  return {
    threads: 1,
    nodeThreads: 1,
    silentMode: false,
    modeFallback: true,
    accountsDir: "",
    outputDir: "downloads",
    defaultMode: "fast",
    defaultModel: "seedance-2.0",
    defaultRatio: "9:16",
    defaultDuration: 10,
    dolaChannel: "api",
    subdirPattern: "workflow_date",
    filenameTemplate: "{workflow}_{gio}_{stt}",
    duplicatePolicy: "number",
    onError: "skip",
    restBetween: 30,
    missingAccount: "skip",
    apiKeyAutoRotate: false,
    ...overrides,
  };
}

describe("validateSettings", () => {
  it("gia tri mac dinh -> khong loi", () => {
    expect(validateSettings(defaultSettings())).toEqual([]);
  });

  it("thu muc rong (accountsDir/outputDir) -> KHONG loi (rong = de app tu chon)", () => {
    expect(validateSettings(defaultSettings({ accountsDir: "", outputDir: "" }))).toEqual([]);
  });

  it(`threads = ${THREADS_MAX} (bien tren) -> khong loi`, () => {
    expect(validateSettings(defaultSettings({ threads: THREADS_MAX }))).toEqual([]);
  });

  it("threads = 0 -> co loi", () => {
    const errors = validateSettings(defaultSettings({ threads: 0 }));
    expect(errors.length).toBe(1);
    expect(errors[0]).toMatch(/luồng/);
  });

  it(`threads > ${THREADS_MAX} -> co loi`, () => {
    const errors = validateSettings(defaultSettings({ threads: THREADS_MAX + 1 }));
    expect(errors.length).toBe(1);
  });

  it("threads khong phai so nguyen -> co loi", () => {
    const errors = validateSettings(defaultSettings({ threads: 2.5 }));
    expect(errors.length).toBe(1);
  });

  it("defaultDuration khong hop le voi mode/model (fast+seedance-2.0 khong co 30s) -> co loi", () => {
    const errors = validateSettings(defaultSettings({ defaultDuration: 30 }));
    expect(errors.length).toBe(1);
    expect(errors[0]).toMatch(/[Tt]hời lượng/);
  });

  it("defaultDuration = 30 HOP LE voi fast+seedance-2.5 (ADR-044) -> khong loi", () => {
    const errors = validateSettings(
      defaultSettings({ defaultModel: "seedance-2.5", defaultDuration: 30 }),
    );
    expect(errors).toEqual([]);
  });

  it("ca hai loi cung luc -> mang co 2 phan tu", () => {
    const errors = validateSettings(defaultSettings({ threads: 0, defaultDuration: 30 }));
    expect(errors.length).toBe(2);
  });
});

describe("toAppSettings", () => {
  it("threads vuot tran -> kep ve THREADS_MAX", () => {
    expect(toAppSettings(defaultSettings({ threads: 999 })).threads).toBe(THREADS_MAX);
  });

  it("threads = 0 -> ve 1 (khong bao gio gui 0)", () => {
    expect(toAppSettings(defaultSettings({ threads: 0 })).threads).toBe(1);
  });

  it("threads am -> ve 1", () => {
    expect(toAppSettings(defaultSettings({ threads: -5 })).threads).toBe(1);
  });

  it("threads NaN (vd input bi xoa trang) -> ve 1, khong panic/NaN lot qua", () => {
    expect(toAppSettings(defaultSettings({ threads: Number.NaN })).threads).toBe(1);
  });

  it("threads le -> lam tron", () => {
    expect(toAppSettings(defaultSettings({ threads: 4.9 })).threads).toBe(5);
  });

  it("accountsDir/outputDir duoc trim hai dau", () => {
    const out = toAppSettings(defaultSettings({ accountsDir: "  C:/acc  ", outputDir: "  " }));
    expect(out.accountsDir).toBe("C:/acc");
    expect(out.outputDir).toBe("");
  });

  it("cac truong khac giu nguyen", () => {
    const draft = defaultSettings({
      silentMode: true,
      modeFallback: false,
      defaultMode: "pro",
      defaultModel: "seedance-2.5",
      defaultRatio: "16:9",
      defaultDuration: 15,
    });
    const out = toAppSettings(draft);
    expect(out.silentMode).toBe(true);
    expect(out.modeFallback).toBe(false);
    expect(out.defaultMode).toBe("pro");
    expect(out.defaultModel).toBe("seedance-2.5");
    expect(out.defaultRatio).toBe("16:9");
    expect(out.defaultDuration).toBe(15);
  });
});
