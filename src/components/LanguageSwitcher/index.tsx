import { useTranslation } from "react-i18next";

export default function LanguageSwitcher() {
  const { i18n } = useTranslation();

  const toggleLanguage = async () => {
    const next = i18n.language.startsWith("en") ? "zh" : "en";
    await i18n.changeLanguage(next);
    localStorage.setItem("dimkey-lang", next);
    // set_language invoke is handled by i18n.on("languageChanged") in i18n.ts
  };

  const isEn = i18n.language.startsWith("en");

  return (
    <button
      onClick={toggleLanguage}
      className="flex items-center gap-1 px-2 py-1 text-xs text-gray-500 hover:text-gray-700 hover:bg-gray-100 rounded transition-colors"
      title={isEn ? "切换到中文" : "Switch to English"}
    >
      <span className="text-sm">{isEn ? "中" : "EN"}</span>
    </button>
  );
}
