import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface SnippetsToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const SnippetsToggle: React.FC<SnippetsToggleProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("snippets_enabled") || false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(enabled) => updateSetting("snippets_enabled", enabled)}
        isUpdating={isUpdating("snippets_enabled")}
        label={t("settings.snippets.enable.label")}
        description={t("settings.snippets.enable.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
