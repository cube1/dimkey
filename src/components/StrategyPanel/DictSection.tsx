import { useState, useEffect } from "react";
import { ChevronDown, ChevronRight, X, BookOpen, Pencil, Lock } from "lucide-react";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useReDetectDict } from "../../hooks/useReDetectDict";
import { useAutoDesensitize } from "../../hooks/useAutoDesensitize";
import type { DictEntry, SensitiveType } from "../../types";
import { SENSITIVE_TYPE_CONFIG, getSensitiveTypeInfo } from "../../types";

const SENSITIVE_TYPE_KEYS = Object.keys(SENSITIVE_TYPE_CONFIG).filter((k) => k !== "Custom");

export function DictSection() {
  const { t, i18n } = useTranslation();
  const [collapsed, setCollapsed] = useState(false);
  const wsData = useWorkspaceStore((s) => s.activeWorkspaceData);
  const updateDictEntries = useWorkspaceStore((s) => s.updateDictEntries);

  const reDetectDict = useReDetectDict();
  const { desensitizeManualItems } = useAutoDesensitize();

  const [text, setText] = useState("");
  const [typeKey, setTypeKey] = useState("PersonName");
  const [matchMode, setMatchMode] = useState<"Exact" | "Fuzzy">("Exact");
  const [replacement, setReplacement] = useState("");

  // 行内编辑状态
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [editText, setEditText] = useState("");
  const [editReplacement, setEditReplacement] = useState("");
  const [builtinEntries, setBuiltinEntries] = useState<DictEntry[]>([]);

  const workspaceMode = wsData?.workspace.mode || "Desensitize";
  const isTemplateMode = workspaceMode === "TemplateReplace";

  const entries = wsData?.workspace.dict_entries || [];

  // 加载内置词典（语言变化时重新加载）
  useEffect(() => {
    invoke<DictEntry[]>("get_builtin_dict").then(setBuiltinEntries).catch(() => {});
  }, [i18n.language]);

  // 合并展示：用户词条 + 内置词条
  const allEntries = [...entries, ...builtinEntries];

  const handleAdd = async () => {
    if (!text.trim()) return;

    // 查重
    if (entries.some((e) => e.text === text.trim())) {
      toast.error(t("dict.alreadyExists"));
      return;
    }

    const sensitiveType: SensitiveType =
      typeKey === "Custom" ? { Custom: text.trim() } : (typeKey as SensitiveType);

    const currentLang = i18n.language.startsWith("en") ? "en" : "zh";
    const newEntry: DictEntry = {
      text: text.trim(),
      sensitive_type: sensitiveType,
      match_mode: matchMode,
      ...(replacement.trim() ? { replacement: replacement.trim() } : {}),
      language: currentLang,
    };

    try {
      await updateDictEntries([...entries, newEntry]);
      setText("");
      setReplacement("");
      await reDetectDict();
      // 脱敏模式下重新执行脱敏
      if (!isTemplateMode) {
        await desensitizeManualItems();
      }
    } catch {
      toast.error(t("dict.addFailed"));
    }
  };

  const handleRemove = async (index: number) => {
    const newEntries = entries.filter((_, i) => i !== index);
    try {
      await updateDictEntries(newEntries);
      await reDetectDict();
      // 脱敏模式下重新执行脱敏
      if (!isTemplateMode) {
        await desensitizeManualItems();
      }
    } catch {
      toast.error(t("dict.deleteFailed"));
    }
  };

  // 开始编辑词条
  const handleStartEdit = (index: number) => {
    const entry = entries[index];
    setEditingIndex(index);
    setEditText(entry.text);
    setEditReplacement(entry.replacement || "");
  };

  // 保存编辑
  const handleSaveEdit = async () => {
    if (editingIndex === null || !editText.trim()) return;
    const entry = entries[editingIndex];
    const updated: DictEntry = {
      ...entry,
      text: editText.trim(),
      ...(editReplacement.trim() ? { replacement: editReplacement.trim() } : { replacement: undefined }),
    };
    const newEntries = [...entries];
    newEntries[editingIndex] = updated;
    try {
      await updateDictEntries(newEntries);
      setEditingIndex(null);
      await reDetectDict();
      // 脱敏模式下重新执行脱敏
      if (!isTemplateMode) {
        await desensitizeManualItems();
      }
    } catch {
      toast.error(t("dict.editFailed"));
    }
  };

  // 取消编辑
  const handleCancelEdit = () => {
    setEditingIndex(null);
  };

  return (
    <div className="border-b border-slate-100" data-testid="panel-dict">
      <button
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center gap-2 px-4 py-2.5 text-xs font-semibold text-slate-600 hover:bg-slate-50 transition-colors"
      >
        {collapsed ? (
          <ChevronRight className="w-3.5 h-3.5 text-slate-400" />
        ) : (
          <ChevronDown className="w-3.5 h-3.5 text-slate-400" />
        )}
        <BookOpen className="w-3.5 h-3.5 text-amber-400" />
        {t("strategyPanel.dict")}
        {allEntries.length > 0 && (
          <span className="text-[11px] font-normal text-slate-400">({allEntries.length})</span>
        )}
      </button>

      {!collapsed && (
        <div className="px-4 pb-4">
          {/* 模版模式映射统计 */}
          {isTemplateMode && entries.length > 0 && (
            <div className="text-xs text-slate-500 px-1 mb-1">
              {t("strategyPanel.dictMapping", { set: entries.filter(e => e.replacement).length, total: entries.length })}
            </div>
          )}

          {/* 词条列表 */}
          {allEntries.length > 0 && (
            <div className="mb-3 space-y-1.5 max-h-40 overflow-auto">
              {allEntries.map((entry, index) => {
                const info = getSensitiveTypeInfo(entry.sensitive_type);
                const isBuiltin = entry.builtin === true;
                // 用户词条的真实索引（内置词条不在 entries 数组中）
                const userIndex = isBuiltin ? -1 : index;

                // 编辑模式（仅用户词条）
                if (!isBuiltin && editingIndex === userIndex) {
                  return (
                    <div key={entry.text} className="flex flex-col gap-1 p-1.5 bg-slate-50 rounded">
                      <input
                        value={editText}
                        onChange={(e) => setEditText(e.target.value)}
                        className="text-xs px-1.5 py-0.5 border border-slate-300 rounded w-full focus:outline-none focus:ring-1 focus:ring-primary-500/30 focus:border-primary-400"
                        placeholder={t("strategyPanel.inputOriginalText")}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            handleSaveEdit();
                          } else if (e.key === "Escape") {
                            handleCancelEdit();
                          }
                        }}
                      />
                      {isTemplateMode && (
                        <input
                          value={editReplacement}
                          onChange={(e) => setEditReplacement(e.target.value)}
                          className="text-xs px-1.5 py-0.5 border border-slate-300 rounded w-full focus:outline-none focus:ring-1 focus:ring-primary-500/30 focus:border-primary-400"
                          placeholder={t("strategyPanel.replaceToRequired")}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") {
                              e.preventDefault();
                              handleSaveEdit();
                            } else if (e.key === "Escape") {
                              handleCancelEdit();
                            }
                          }}
                        />
                      )}
                      <div className="flex gap-1 justify-end">
                        <button
                          onClick={handleSaveEdit}
                          className="text-xs px-1.5 py-0.5 bg-primary-500 text-white rounded hover:bg-primary-600 transition-colors"
                        >
                          {t("common.save")}
                        </button>
                        <button
                          onClick={handleCancelEdit}
                          className="text-xs px-1.5 py-0.5 bg-slate-200 text-slate-600 rounded hover:bg-slate-300 transition-colors"
                        >
                          {t("common.cancel")}
                        </button>
                      </div>
                    </div>
                  );
                }

                // 非编辑状态（普通显示）
                return (
                  <div
                    key={`${entry.text}-${isBuiltin ? "b" : "u"}-${index}`}
                    className={`flex items-center gap-2 px-2.5 py-1.5 rounded group ${isBuiltin ? "bg-blue-50/50" : "bg-slate-50"}`}
                  >
                    {isBuiltin && (
                      <span title={t("dict.builtin")}><Lock className="w-3 h-3 text-blue-400 shrink-0" /></span>
                    )}
                    <span className="text-xs text-slate-800 truncate flex-1 min-w-0">
                      {entry.text}
                      {entry.replacement && (
                        <span className="text-primary-600"> → {entry.replacement}</span>
                      )}
                    </span>
                    <span
                      className={`shrink-0 px-1.5 py-0.5 text-xs rounded ${info.bgClass} ${info.textClass}`}
                    >
                      {info.label}
                    </span>
                    <span className="shrink-0 text-xs text-slate-400">
                      {entry.match_mode === "Exact" ? t("common.exact") : t("common.fuzzy")}
                    </span>
                    {!isBuiltin && (
                      <>
                        <button
                          onClick={() => handleStartEdit(userIndex)}
                          className="shrink-0 p-0.5 text-slate-300 hover:text-slate-600 rounded transition-colors opacity-0 group-hover:opacity-100"
                          title={t("dict.edit")}
                        >
                          <Pencil className="w-3 h-3" />
                        </button>
                        <button
                          onClick={() => handleRemove(userIndex)}
                          className="shrink-0 p-0.5 text-slate-300 hover:text-rose-500 rounded transition-colors opacity-0 group-hover:opacity-100"
                          title={t("common.delete")}
                        >
                          <X className="w-3 h-3" />
                        </button>
                      </>
                    )}
                  </div>
                );
              })}
            </div>
          )}

          {/* 添加表单 */}
          <div className="space-y-2">
            <input
              type="text"
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder={t("strategyPanel.inputSensitiveText")}
              className="w-full px-2.5 py-1.5 text-xs border border-slate-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400"
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  handleAdd();
                }
              }}
            />

            {isTemplateMode && (
              <input
                type="text"
                value={replacement}
                onChange={(e) => setReplacement(e.target.value)}
                placeholder={t("strategyPanel.replaceTo")}
                className="w-full px-2.5 py-1.5 text-xs border border-slate-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400"
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    handleAdd();
                  }
                }}
              />
            )}

            <div className="flex items-center gap-2">
              <select
                value={typeKey}
                onChange={(e) => setTypeKey(e.target.value)}
                className="flex-1 px-2 py-1.5 text-xs border border-slate-200 rounded-lg bg-white focus:outline-none focus:ring-2 focus:ring-primary-500/20 focus:border-primary-400"
              >
                {SENSITIVE_TYPE_KEYS.map((k) => (
                  <option key={k} value={k}>
                    {SENSITIVE_TYPE_CONFIG[k].label}
                  </option>
                ))}
              </select>

              <div className="flex rounded border border-slate-200 overflow-hidden shrink-0">
                <button
                  onClick={() => setMatchMode("Exact")}
                  className={`px-2 py-1 text-xs transition-colors ${
                    matchMode === "Exact"
                      ? "bg-primary-500 text-white"
                      : "bg-white text-slate-600 hover:bg-slate-50"
                  }`}
                >
                  {t("common.exact")}
                </button>
                <button
                  onClick={() => setMatchMode("Fuzzy")}
                  className={`px-2 py-1 text-xs transition-colors border-l border-slate-200 ${
                    matchMode === "Fuzzy"
                      ? "bg-primary-500 text-white"
                      : "bg-white text-slate-600 hover:bg-slate-50"
                  }`}
                >
                  {t("common.fuzzy")}
                </button>
              </div>
            </div>

            <button
              onClick={handleAdd}
              disabled={!text.trim()}
              className="w-full py-1.5 text-xs font-medium text-white bg-primary-600 hover:bg-primary-700 rounded-lg shadow-sm shadow-primary-600/20 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {t("common.add")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
