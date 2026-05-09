import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import zh from "./locales/zh.json";
import en from "./locales/en.json";

// 语言由编译期决定，前后端对称：
//   后端：Cargo feature `lang-zh` / `lang-en`
//   前端：Vite 环境变量 `VITE_DIMKEY_LANG=zh|en`
// 打包脚本同时设置二者，运行时不可切换。
const buildLang =
  (import.meta.env.VITE_DIMKEY_LANG as string | undefined)?.toLowerCase().startsWith("en")
    ? "en"
    : "zh";

i18n.use(initReactI18next).init({
  resources: {
    zh: { translation: zh },
    en: { translation: en },
  },
  lng: buildLang,
  fallbackLng: "zh",
  interpolation: {
    escapeValue: false,
  },
});

// dev 一致性检查：前后端语言由两条独立路径决定（Vite env vs Cargo feature），
// 在 dev 模式下若只设置了一边会产生错位（UI 文案 ≠ 后端识别行为）。
// 启动后异步验证，仅在 dev 时打 warn，不影响 production 行为。
if (import.meta.env.DEV) {
  invoke<string>("get_language")
    .then((backendLang) => {
      if (backendLang !== buildLang) {
        // eslint-disable-next-line no-console
        console.warn(
          `[i18n] 语言不一致: 前端 VITE_DIMKEY_LANG=${buildLang}, 后端 Cargo feature=${backendLang}。` +
            ` dev 时请同时设置两者，例如: VITE_DIMKEY_LANG=en cargo tauri dev --no-default-features --features lang-en`,
        );
      }
    })
    .catch(() => {});
}

export default i18n;
