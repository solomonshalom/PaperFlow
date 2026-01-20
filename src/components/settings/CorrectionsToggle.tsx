import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface CorrectionsToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const CorrectionsToggle: React.FC<CorrectionsToggleProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("correction_detection_enabled") || false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(enabled) =>
          updateSetting("correction_detection_enabled", enabled)
        }
        isUpdating={isUpdating("correction_detection_enabled")}
        label={t("settings.corrections.enable.label")}
        description={t("settings.corrections.enable.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
