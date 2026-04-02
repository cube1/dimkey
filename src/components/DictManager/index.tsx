import { useState, useEffect } from "react";
import { Dialog, DialogPanel, DialogTitle, Transition, TransitionChild } from "@headlessui/react";
import { useTranslation } from "react-i18next";
import { useConfigStore } from "../../stores/configStore";
import type { DictEntry, SensitiveType } from "../../types";
import {
  SENSITIVE_TYPE_CONFIG,
  getSensitiveTypeKey,
  getSensitiveTypeInfo,
} from "../../types";

/** 所有可选的敏感类型键名 */
const SENSITIVE_TYPE_KEYS = [
  "Phone",
  "IdCard",
  "BankCard",
  "Email",
  "IpAddress",
  "LandlinePhone",
  "LicensePlate",
  "CreditCode",
  "PersonName",
  "OrgName",
  "Address",
  "Title",
  "Custom",
] as const;

interface DictManagerProps {
  open: boolean;
  onClose: () => void;
}

export function DictManager({ open, onClose }: DictManagerProps) {
  const { t } = useTranslation();
  const dictEntries = useConfigStore((s) => s.dictEntries);
  const loadDict = useConfigStore((s) => s.loadDict);
  const saveDict = useConfigStore((s) => s.saveDict);
  const addDictEntry = useConfigStore((s) => s.addDictEntry);
  const removeDictEntry = useConfigStore((s) => s.removeDictEntry);

  // 添加表单状态
  const [text, setText] = useState("");
  const [typeKey, setTypeKey] = useState<string>("Phone");
  const [matchMode, setMatchMode] = useState<"Exact" | "Fuzzy">("Exact");

  // 打开时加载词典
  useEffect(() => {
    if (open) {
      loadDict();
    }
  }, [open, loadDict]);

  /** 构建 SensitiveType 值 */
  const buildSensitiveType = (key: string, entryText: string): SensitiveType => {
    if (key === "Custom") {
      return { Custom: entryText };
    }
    return key as SensitiveType;
  };

  /** 添加词条 */
  const handleAdd = async () => {
    const trimmed = text.trim();
    if (!trimmed) return;

    const entry: DictEntry = {
      text: trimmed,
      sensitive_type: buildSensitiveType(typeKey, trimmed),
      match_mode: matchMode,
    };

    addDictEntry(entry);
    await saveDict();

    // 重置表单
    setText("");
  };

  /** 删除词条 */
  const handleRemove = async (index: number) => {
    removeDictEntry(index);
    await saveDict();
  };

  return (
    <Transition appear show={open}>
      <Dialog as="div" className="relative z-50" onClose={onClose}>
        {/* 背景遮罩 */}
        <TransitionChild
          enter="ease-out duration-300"
          enterFrom="opacity-0"
          enterTo="opacity-100"
          leave="ease-in duration-200"
          leaveFrom="opacity-100"
          leaveTo="opacity-0"
        >
          <div className="fixed inset-0 bg-black/25" />
        </TransitionChild>

        {/* 抽屉面板 */}
        <div className="fixed inset-0 overflow-hidden">
          <div className="absolute inset-0 overflow-hidden">
            <div className="fixed inset-y-0 right-0 flex max-w-full pl-10">
              <TransitionChild
                enter="transform transition ease-in-out duration-300"
                enterFrom="translate-x-full"
                enterTo="translate-x-0"
                leave="transform transition ease-in-out duration-200"
                leaveFrom="translate-x-0"
                leaveTo="translate-x-full"
              >
                <DialogPanel className="w-[400px] bg-white shadow-xl flex flex-col h-full">
                  {/* 标题栏 */}
                  <div className="flex items-center justify-between px-5 py-4 border-b border-gray-200">
                    <DialogTitle className="text-lg font-semibold text-gray-800">
                      {t("dict.title")}
                    </DialogTitle>
                    <button
                      onClick={onClose}
                      className="p-1 text-gray-400 hover:text-gray-600 rounded-md hover:bg-gray-100 transition-colors"
                    >
                      <svg
                        className="w-5 h-5"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M6 18L18 6M6 6l12 12"
                        />
                      </svg>
                    </button>
                  </div>

                  {/* 词条列表 */}
                  <div className="flex-1 overflow-auto px-5 py-4">
                    {dictEntries.length === 0 ? (
                      <div className="flex flex-col items-center justify-center h-full text-gray-400">
                        <svg
                          className="w-12 h-12 text-gray-300 mb-3"
                          fill="none"
                          stroke="currentColor"
                          viewBox="0 0 24 24"
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth={1.5}
                            d="M12 6v6m0 0v6m0-6h6m-6 0H6"
                          />
                        </svg>
                        <p className="text-sm">{t("dict.empty")}</p>
                      </div>
                    ) : (
                      <div className="space-y-2">
                        {dictEntries.map((entry, index) => {
                          const info = getSensitiveTypeInfo(entry.sensitive_type);
                          const key = getSensitiveTypeKey(entry.sensitive_type);
                          return (
                            <div
                              key={index}
                              className="flex items-center gap-2 px-3 py-2 bg-gray-50 rounded-lg group"
                            >
                              {/* 文本 */}
                              <span className="text-sm text-gray-800 truncate flex-1 min-w-0">
                                {entry.text}
                              </span>

                              {/* 类型颜色标签 */}
                              <span
                                className={`shrink-0 px-2 py-0.5 text-xs rounded ${info.bgClass} ${info.textClass}`}
                              >
                                {info.label}
                                {key === "Custom" &&
                                  typeof entry.sensitive_type === "object" &&
                                  entry.sensitive_type.Custom !== entry.text && (
                                    <>({entry.sensitive_type.Custom})</>
                                  )}
                              </span>

                              {/* 匹配模式 badge */}
                              <span
                                className={`shrink-0 px-1.5 py-0.5 text-xs rounded font-medium ${
                                  entry.match_mode === "Exact"
                                    ? "bg-blue-50 text-blue-600"
                                    : "bg-amber-50 text-amber-600"
                                }`}
                              >
                                {entry.match_mode === "Exact" ? t("common.exact") : t("common.fuzzy")}
                              </span>

                              {/* 删除按钮 */}
                              <button
                                onClick={() => handleRemove(index)}
                                className="shrink-0 p-1 text-gray-300 hover:text-red-500 rounded transition-colors opacity-0 group-hover:opacity-100"
                              >
                                <svg
                                  className="w-4 h-4"
                                  fill="none"
                                  stroke="currentColor"
                                  viewBox="0 0 24 24"
                                >
                                  <path
                                    strokeLinecap="round"
                                    strokeLinejoin="round"
                                    strokeWidth={2}
                                    d="M6 18L18 6M6 6l12 12"
                                  />
                                </svg>
                              </button>
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>

                  {/* 底部添加表单 */}
                  <div className="border-t border-gray-200 px-5 py-4 space-y-3">
                    {/* 文本输入 */}
                    <input
                      type="text"
                      value={text}
                      onChange={(e) => setText(e.target.value)}
                      placeholder={t("strategyPanel.inputSensitiveText")}
                      className="w-full px-3 py-2 text-sm border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      onKeyDown={(e) => {
                        if (e.key === "Enter") {
                          e.preventDefault();
                          handleAdd();
                        }
                      }}
                    />

                    {/* 类型选择 + 匹配模式 */}
                    <div className="flex items-center gap-2">
                      {/* 敏感类型下拉 */}
                      <select
                        value={typeKey}
                        onChange={(e) => setTypeKey(e.target.value)}
                        className="flex-1 px-3 py-2 text-sm border border-gray-300 rounded-lg bg-white focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                      >
                        {SENSITIVE_TYPE_KEYS.map((k) => (
                          <option key={k} value={k}>
                            {SENSITIVE_TYPE_CONFIG[k].label}
                          </option>
                        ))}
                      </select>

                      {/* 匹配模式切换按钮组 */}
                      <div className="flex rounded-lg border border-gray-300 overflow-hidden shrink-0">
                        <button
                          onClick={() => setMatchMode("Exact")}
                          className={`px-3 py-2 text-xs font-medium transition-colors ${
                            matchMode === "Exact"
                              ? "bg-blue-500 text-white"
                              : "bg-white text-gray-600 hover:bg-gray-50"
                          }`}
                        >
                          {t("common.exact")}
                        </button>
                        <button
                          onClick={() => setMatchMode("Fuzzy")}
                          className={`px-3 py-2 text-xs font-medium transition-colors border-l border-gray-300 ${
                            matchMode === "Fuzzy"
                              ? "bg-blue-500 text-white"
                              : "bg-white text-gray-600 hover:bg-gray-50"
                          }`}
                        >
                          {t("common.fuzzy")}
                        </button>
                      </div>
                    </div>

                    {/* 添加按钮 */}
                    <button
                      onClick={handleAdd}
                      disabled={!text.trim()}
                      className="w-full py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      {t("common.add")}
                    </button>
                  </div>
                </DialogPanel>
              </TransitionChild>
            </div>
          </div>
        </div>
      </Dialog>
    </Transition>
  );
}
