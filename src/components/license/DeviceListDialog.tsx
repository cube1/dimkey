// src/components/license/DeviceListDialog.tsx
import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useLicenseStore, type DeviceInfo } from "../../stores/licenseStore";

interface DeviceListDialogProps {
  visible: boolean;
  onClose: () => void;
  /** ActivationDialog 触发 DEVICE_LIMIT_REACHED 时把列表预填进来，避免再发一次请求 */
  initialDevices?: DeviceInfo[];
  initialMax?: number;
}

export function DeviceListDialog({
  visible,
  onClose,
  initialDevices,
  initialMax,
}: DeviceListDialogProps) {
  const { t } = useTranslation();
  const list = useLicenseStore((s) => s.listDevices);
  const deactivate = useLicenseStore((s) => s.deactivateDevice);
  const [devices, setDevices] = useState<DeviceInfo[]>(initialDevices ?? []);
  const [max, setMax] = useState<number>(initialMax ?? 3);
  const [loading, setLoading] = useState(false);

  const reload = async () => {
    setLoading(true);
    try {
      const arr = await list();
      setDevices(arr);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (visible && !initialDevices) {
      reload();
    }
    if (visible && initialDevices) {
      setDevices(initialDevices);
      setMax(initialMax ?? 3);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visible, initialDevices, initialMax]);

  if (!visible) return null;

  const handleDeactivate = async (id: string) => {
    if (!confirm(t("license.devices.deactivate_confirm"))) return;
    await deactivate(id);
    await reload();
  };

  const fmtTime = (ts: number) => new Date(ts).toLocaleString();

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="bg-white rounded-xl shadow-xl w-[520px] max-h-[80vh] overflow-y-auto mx-4 p-6 relative"
        onClick={(e) => e.stopPropagation()}
      >
        <button
          onClick={onClose}
          className="absolute top-4 right-4 text-gray-400 hover:text-gray-600"
          aria-label="close"
        >
          <X size={20} />
        </button>
        <h2 className="text-lg font-semibold mb-5">{t("license.devices.title")}</h2>

        {loading && <p className="text-sm text-gray-500">Loading...</p>}

        <div className="space-y-2">
          {devices.map((d) => (
            <div
              key={d.device_id}
              className="border border-gray-200 rounded-lg p-3 flex justify-between items-center"
            >
              <div>
                <div className="font-medium text-sm">
                  {d.machine_label || "(unnamed)"}
                  {d.is_current && (
                    <span className="ml-2 text-xs text-blue-600">
                      {t("license.devices.this_device")}
                    </span>
                  )}
                </div>
                <div className="text-xs text-gray-500 mt-0.5">
                  {d.os} · {d.flavor} · {fmtTime(d.last_seen)}
                </div>
              </div>
              {!d.is_current && (
                <button
                  onClick={() => handleDeactivate(d.device_id)}
                  className="text-sm text-red-600 border border-red-300 px-3 py-1 rounded hover:bg-red-50"
                >
                  {t("license.devices.deactivate")}
                </button>
              )}
            </div>
          ))}
        </div>

        <p className="text-xs text-gray-500 mt-4">
          {t("license.devices.summary", { active: devices.length, max })}
        </p>
      </div>
    </div>
  );
}
