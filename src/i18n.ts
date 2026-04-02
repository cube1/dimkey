import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";
import { invoke } from "@tauri-apps/api/core";
import zh from "./locales/zh.json";
import en from "./locales/en.json";

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      zh: { translation: zh },
      en: { translation: en },
    },
    fallbackLng: "zh",
    interpolation: {
      escapeValue: false,
    },
    detection: {
      order: ["localStorage", "navigator"],
      caches: ["localStorage"],
      lookupLocalStorage: "dimkey-lang",
    },
  });

// 启动时同步语言到 Rust 后端
i18n.on("initialized", () => {
  invoke("set_language", { lang: i18n.language }).catch(() => {});
});

i18n.on("languageChanged", (lng) => {
  invoke("set_language", { lang: lng }).catch(() => {});
});

export default i18n;
