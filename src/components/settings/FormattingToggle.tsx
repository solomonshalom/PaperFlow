import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface FormattingToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const FormattingToggle: React.FC<FormattingToggleProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("auto_format_enabled") || false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(enabled) => updateSetting("auto_format_enabled", enabled)}
        isUpdating={isUpdating("auto_format_enabled")}
        label={t("settings.formatting.enable.label")}
        description={t("settings.formatting.enable.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
