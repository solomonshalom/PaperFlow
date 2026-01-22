import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface ShowSidebarToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ShowSidebarToggle: React.FC<ShowSidebarToggleProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const showSidebar = getSetting("show_sidebar") ?? false;

    return (
      <ToggleSwitch
        checked={showSidebar}
        onChange={(enabled) => updateSetting("show_sidebar", enabled)}
        isUpdating={isUpdating("show_sidebar")}
        label={t("settings.advanced.showSidebar.label")}
        description={t("settings.advanced.showSidebar.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
