import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface ToneToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ToneToggle: React.FC<ToneToggleProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("tone_adjustment_enabled") || false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(enabled) =>
          updateSetting("tone_adjustment_enabled", enabled)
        }
        isUpdating={isUpdating("tone_adjustment_enabled")}
        label={t("settings.tone.enable.label")}
        description={t("settings.tone.enable.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
