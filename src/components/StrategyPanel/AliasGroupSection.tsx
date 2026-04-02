import { useState } from "react";
import { Link2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import toast from "react-hot-toast";

export function AliasGroupSection() {
  const { t } = useTranslation();
  const aliasGroups = useWorkspaceStore((s) => s.aliasGroups);
  const addMemberToGroup = useWorkspaceStore((s) => s.addMemberToGroup);
  const removeMemberFromGroup = useWorkspaceStore((s) => s.removeMemberFromGroup);
  const deleteAliasGroup = useWorkspaceStore((s) => s.deleteAliasGroup);

  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [newMemberText, setNewMemberText] = useState("");

  const handleToggleExpand = (groupId: string) => {
    const next = expandedId === groupId ? null : groupId;
    setExpandedId(next);
    setNewMemberText("");
  };

  const handleAddMember = async (groupId: string) => {
    const text = newMemberText.trim();
    if (!text) return;
    try {
      await addMemberToGroup(groupId, text);
      setNewMemberText("");
      toast.success(t("strategyPanel.memberAdded"));
    } catch (e) {
      toast.error(t("strategyPanel.addFailed", { error: String(e) }));
    }
  };

  const handleRemoveMember = async (groupId: string, member: string) => {
    try {
      await removeMemberFromGroup(groupId, member);
    } catch (e) {
      toast.error(t("strategyPanel.removeFailed", { error: String(e) }));
    }
  };

  const handleDeleteGroup = async (groupId: string) => {
    try {
      await deleteAliasGroup(groupId);
      toast.success(t("strategyPanel.groupDeleted"));
    } catch (e) {
      toast.error(t("strategyPanel.addFailed", { error: String(e) }));
    }
  };

  return (
    <div className="border-b border-slate-200">
      {/* 折叠标题 */}
      <div className="px-4 py-2.5 flex items-center justify-between">
        <div className="flex items-center gap-1.5 text-xs font-semibold text-slate-500 tracking-wider">
          <Link2 className="w-3.5 h-3.5" />
          {t("strategyPanel.aliasGroup")}
          {aliasGroups.length > 0 && (
            <span className="text-[10px] font-normal text-slate-400">({aliasGroups.length})</span>
          )}
        </div>
      </div>

      {aliasGroups.length === 0 ? (
        <div className="px-4 pb-3 text-xs text-slate-400">
          {t("strategyPanel.noAliasGroup")}
        </div>
      ) : (
        <div className="px-3 pb-3 space-y-1.5">
          {aliasGroups.map((group) => {
            const expanded = expandedId === group.id;
            return (
              <div key={group.id} className="border border-slate-200 rounded-lg overflow-hidden">
                {/* 组头 */}
                <button
                  onClick={() => handleToggleExpand(group.id)}
                  className="w-full flex items-center justify-between px-3 py-2 hover:bg-slate-50 transition-colors"
                >
                  <div className="flex items-center gap-2 min-w-0">
                    <svg className={`w-3 h-3 text-slate-400 transition-transform shrink-0 ${expanded ? "rotate-90" : ""}`}
                      fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                    <span className="text-xs font-medium text-slate-700 truncate">{group.primary}</span>
                    <span className="text-[10px] text-slate-400 shrink-0">{t("strategyPanel.members", { count: group.members.length })}</span>
                  </div>
                  <button
                    onClick={(e) => { e.stopPropagation(); handleDeleteGroup(group.id); }}
                    className="text-slate-300 hover:text-red-500 transition-colors p-0.5"
                    title={t("strategyPanel.deleteGroup")}
                  >
                    <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5}
                        d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                    </svg>
                  </button>
                </button>

                {/* 展开内容 */}
                {expanded && (
                  <div className="border-t border-slate-100 px-3 py-2 space-y-1">
                    {group.members.map((member) => (
                      <div key={member} className="flex items-center justify-between group">
                        <span className={`text-xs ${member === group.primary ? "font-medium text-indigo-600" : "text-slate-600"}`}>
                          {member}
                          {member === group.primary && (
                            <span className="ml-1 text-[10px] text-indigo-400">{t("strategyPanel.primary")}</span>
                          )}
                        </span>
                        {member !== group.primary && (
                          <button
                            onClick={() => handleRemoveMember(group.id, member)}
                            className="opacity-0 group-hover:opacity-100 text-slate-300 hover:text-red-500 transition-all p-0.5"
                          >
                            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                            </svg>
                          </button>
                        )}
                      </div>
                    ))}

                    {/* 添加新成员 */}
                    <div className="flex items-center gap-1.5 mt-1.5 pt-1.5 border-t border-slate-100">
                      <input
                        value={newMemberText}
                        onChange={(e) => setNewMemberText(e.target.value)}
                        onKeyDown={(e) => e.key === "Enter" && handleAddMember(group.id)}
                        className="flex-1 text-xs px-2 py-1 border border-slate-200 rounded-md focus:outline-none focus:ring-1 focus:ring-indigo-300"
                        placeholder={t("strategyPanel.addAlias")}
                      />
                      <button
                        onClick={() => handleAddMember(group.id)}
                        disabled={!newMemberText.trim()}
                        className="text-xs px-2 py-1 text-indigo-600 hover:bg-indigo-50 rounded-md disabled:opacity-40 transition-colors"
                      >
                        {t("common.add")}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
