import { FileText, Search, Shield, Save, Check } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";

const STEP_I18N_KEYS = [
  { key: "parsing",       i18nKey: "processing.parseFile", icon: FileText },
  { key: "detecting",     i18nKey: "processing.scanDetect", icon: Search },
  { key: "desensitizing", i18nKey: "processing.desensitize", icon: Shield },
  { key: "saving",        i18nKey: "processing.saveRecord", icon: Save },
] as const;

function getStepIndex(step: string): number {
  return STEP_I18N_KEYS.findIndex((s) => s.key === step);
}

export function ProcessingView() {
  const { t } = useTranslation();
  const step = useWorkspaceStore((s) => s.processingStep);
  const fileName = useWorkspaceStore((s) => s.processingFileName);

  const currentIdx = getStepIndex(step);

  return (
    <div className="flex-1 flex items-center justify-center animate-fade-in" data-testid="view-processing">
      <div className="text-center max-w-md w-full px-6">
        {/* 品牌色脉冲动画环 + Spinner */}
        <div className="mx-auto w-16 h-16 relative mb-8">
          <div className="absolute inset-0 rounded-full border-2 border-primary-200 animate-pulse-soft" />
          <div className="absolute inset-1 rounded-full border-2 border-transparent border-t-primary-500 animate-spin" />
          <div className="absolute inset-0 flex items-center justify-center">
            <Shield className="w-6 h-6 text-primary-500" />
          </div>
        </div>

        {/* 文件名 */}
        {fileName && (
          <p className="text-sm text-slate-500 mb-6 truncate max-w-xs mx-auto">{fileName}</p>
        )}

        {/* 4 步横向进度指示器 */}
        <div className="flex items-center justify-between relative">
          {/* 连接线（底层） */}
          <div className="absolute top-4 left-8 right-8 h-0.5 bg-slate-200" />
          <div
            className="absolute top-4 left-8 h-0.5 bg-primary-500 transition-all duration-500 ease-out"
            style={{
              width: currentIdx <= 0
                ? "0%"
                : `${(Math.min(currentIdx, STEP_I18N_KEYS.length - 1) / (STEP_I18N_KEYS.length - 1)) * 100}%`,
              maxWidth: "calc(100% - 64px)",
            }}
          />

          {STEP_I18N_KEYS.map((s, i) => {
            const Icon = s.icon;
            const isCompleted = i < currentIdx;
            const isCurrent = i === currentIdx;

            return (
              <div key={s.key} className="relative z-10 flex flex-col items-center gap-2">
                <div
                  className={`w-8 h-8 rounded-full flex items-center justify-center transition-all duration-300 ${
                    isCompleted
                      ? "bg-primary-500 text-white"
                      : isCurrent
                        ? "bg-white border-2 border-primary-500 text-primary-500 shadow-sm"
                        : "bg-slate-100 text-slate-400 border border-slate-200"
                  }`}
                >
                  {isCompleted ? (
                    <Check className="w-4 h-4" />
                  ) : (
                    <Icon className="w-4 h-4" />
                  )}
                </div>
                <span
                  className={`text-xs font-medium transition-colors ${
                    isCompleted || isCurrent ? "text-slate-700" : "text-slate-400"
                  }`}
                >
                  {t(s.i18nKey)}
                </span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
