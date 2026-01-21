import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import { SettingsGroup } from "../ui/SettingsGroup";

interface LivePreviewSettingProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const LivePreviewSetting: React.FC<LivePreviewSettingProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const livePreviewEnabled = getSetting("live_preview_enabled") ?? false;
    const livePreviewIntervalMs =
      getSetting("live_preview_interval_ms") ?? 2000;

    const content = (
      <>
        <ToggleSwitch
          checked={livePreviewEnabled}
          onChange={(enabled) => updateSetting("live_preview_enabled", enabled)}
          isUpdating={isUpdating("live_preview_enabled")}
          label={t("settings.advanced.livePreview.label")}
          description={t("settings.advanced.livePreview.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />
        {livePreviewEnabled && (
          <div className="mt-4 ml-1">
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              {t("settings.advanced.livePreview.interval.label")}
            </label>
            <div className="flex items-center gap-4">
              <input
                type="range"
                value={livePreviewIntervalMs / 1000}
                min={1}
                max={4}
                step={0.5}
                onChange={(e) =>
                  updateSetting(
                    "live_preview_interval_ms",
                    Math.round(parseFloat(e.target.value) * 1000),
                  )
                }
                disabled={isUpdating("live_preview_interval_ms")}
                className="flex-1 h-2 rounded-lg appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-logo-primary disabled:opacity-50 disabled:cursor-not-allowed"
                style={{
                  background: `linear-gradient(to right, var(--color-background-ui) ${
                    ((livePreviewIntervalMs / 1000 - 1) / (4 - 1)) * 100
                  }%, rgba(128, 128, 128, 0.2) ${
                    ((livePreviewIntervalMs / 1000 - 1) / (4 - 1)) * 100
                  }%)`,
                }}
              />
              <span className="text-sm text-gray-500 dark:text-gray-400 min-w-[3rem] text-right">
                {(livePreviewIntervalMs / 1000).toFixed(1)}
                {t("common.seconds").charAt(0)}
              </span>
            </div>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
              {t("settings.advanced.livePreview.interval.description")}
            </p>
          </div>
        )}
      </>
    );

    if (grouped) {
      return content;
    }

    return (
      <SettingsGroup title={t("settings.advanced.livePreview.title")}>
        {content}
      </SettingsGroup>
    );
  },
);
