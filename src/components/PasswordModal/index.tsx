import { useState, useEffect, useCallback } from "react";
import { Lock } from "lucide-react";
import { useTranslation } from "react-i18next";

interface PasswordModalProps {
  visible: boolean;
  fileType: string;
  attemptsLeft: number;
  errorMessage: string | null;
  onSubmit: (password: string) => void;
  onCancel: () => void;
}

export function PasswordModal({
  visible,
  fileType: _fileType,
  attemptsLeft,
  errorMessage,
  onSubmit,
  onCancel,
}: PasswordModalProps) {
  const { t } = useTranslation();
  const [password, setPassword] = useState("");
  const [submitting, setSubmitting] = useState(false);

  // visible 变化时清空密码和提交状态
  useEffect(() => {
    setPassword("");
    setSubmitting(false);
  }, [visible, attemptsLeft]);

  const handleSubmit = useCallback(() => {
    if (!password.trim() || submitting) return;
    setSubmitting(true);
    onSubmit(password);
  }, [password, submitting, onSubmit]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        handleSubmit();
      }
    },
    [handleSubmit]
  );

  if (!visible) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm">
      <div className="bg-white rounded-xl shadow-xl max-w-sm mx-4 p-6 w-full">
        {/* 图标 + 标题 */}
        <div className="flex items-center gap-3 mb-4">
          <div className="w-10 h-10 rounded-lg bg-amber-50 flex items-center justify-center">
            <Lock className="w-5 h-5 text-amber-600" />
          </div>
          <div>
            <h3 className="text-base font-semibold text-slate-800">
              {t("password.title")}
            </h3>
            <p className="text-xs text-slate-500">
              {t("password.hint")}
            </p>
          </div>
        </div>

        {/* 密码输入 */}
        <input
          type="password"
          autoFocus
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t("password.placeholder")}
          className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm
                     focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent
                     placeholder:text-slate-400"
        />

        {/* 错误提示 */}
        {errorMessage && (
          <div className="mt-2 px-3 py-2 bg-red-50 border border-red-200 rounded-lg">
            <p className="text-xs text-red-600">
              {errorMessage}
            </p>
          </div>
        )}

        {/* 按钮 */}
        <div className="flex gap-2 justify-end mt-4">
          <button
            onClick={onCancel}
            disabled={submitting}
            className="px-4 py-1.5 text-sm text-slate-500 hover:text-slate-700
                       hover:bg-slate-100 rounded-lg transition-colors disabled:opacity-50"
          >
            {t("common.cancel")}
          </button>
          <button
            onClick={handleSubmit}
            disabled={!password.trim() || submitting}
            className="px-4 py-1.5 text-sm font-medium text-white
                       bg-primary-600 hover:bg-primary-700 rounded-lg
                       transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {submitting ? t("password.decrypting") : t("password.decrypt")}
          </button>
        </div>

        {/* 安全提示 */}
        <p className="text-xs text-slate-400 text-center mt-3">
          {t("password.safetyHint")}
        </p>
      </div>
    </div>
  );
}
