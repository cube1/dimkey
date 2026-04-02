import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import toast from "react-hot-toast";

export function AliasLinkBar() {
  const { t } = useTranslation();
  const aliasLinkMode = useWorkspaceStore((s) => s.aliasLinkMode);
  const aliasLinkMembers = useWorkspaceStore((s) => s.aliasLinkMembers);
  const exitAliasLinkMode = useWorkspaceStore((s) => s.exitAliasLinkMode);
  const removeAliasLinkMember = useWorkspaceStore((s) => s.removeAliasLinkMember);
  const confirmAliasGroup = useWorkspaceStore((s) => s.confirmAliasGroup);

  if (!aliasLinkMode) return null;

  const handleConfirm = async () => {
    if (aliasLinkMembers.length < 2) {
      toast.error(t("aliasLink.minMembers"));
      return;
    }
    try {
      await confirmAliasGroup();
      toast.success(t("aliasLink.createSuccess"));
    } catch (e) {
      toast.error(t("aliasLink.createFailed", { error: String(e) }));
    }
  };

  return (
    <div className="flex items-center gap-2 px-4 py-2 bg-indigo-50 border-b border-indigo-100 text-sm shrink-0">
      <span className="text-indigo-700 font-medium shrink-0">{t("aliasLink.mode")}</span>
      <div className="flex items-center gap-1.5 flex-wrap flex-1 min-w-0">
        {aliasLinkMembers.map((m) => (
          <span
            key={m.id}
            className="inline-flex items-center gap-1 px-2 py-0.5 bg-white border border-indigo-200 rounded-full text-xs text-indigo-700"
          >
            <span className="truncate max-w-[120px]">{m.text}</span>
            <button
              onClick={() => removeAliasLinkMember(m.id)}
              className="text-indigo-400 hover:text-red-500 transition-colors"
            >
              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </span>
        ))}
        <span className="text-xs text-indigo-400">{t("aliasLink.clickToAdd")}</span>
      </div>
      <div className="flex items-center gap-1.5 shrink-0">
        <button
          onClick={exitAliasLinkMode}
          className="px-3 py-1 text-xs text-slate-500 hover:text-slate-700 hover:bg-slate-100 rounded-lg transition-colors"
        >
          {t("common.cancel")}
        </button>
        <button
          onClick={handleConfirm}
          disabled={aliasLinkMembers.length < 2}
          className="px-3 py-1 text-xs bg-indigo-500 text-white rounded-lg hover:bg-indigo-600 disabled:opacity-40 transition-colors"
        >
          {t("aliasLink.confirmLink", { count: aliasLinkMembers.length })}
        </button>
      </div>
    </div>
  );
}
