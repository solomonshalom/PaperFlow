import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import { SettingsGroup } from "../ui/SettingsGroup";

interface WhisperModeProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const WhisperMode: React.FC<WhisperModeProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const whisperModeEnabled = getSetting("whisper_mode_enabled") ?? false;

    const content = (
      <>
        <ToggleSwitch
          checked={whisperModeEnabled}
          onChange={(enabled) => updateSetting("whisper_mode_enabled", enabled)}
          isUpdating={isUpdating("whisper_mode_enabled")}
          label={t("settings.advanced.whisperMode.label")}
          description={t("settings.advanced.whisperMode.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />
        {whisperModeEnabled && (
          <p className="text-xs text-amber-600 dark:text-amber-400 mt-2 ml-1">
            {t("settings.advanced.whisperMode.note")}
          </p>
        )}
      </>
    );

    if (grouped) {
      return content;
    }

    return (
      <SettingsGroup title={t("settings.advanced.whisperMode.title")}>
        {content}
      </SettingsGroup>
    );
  },
);
