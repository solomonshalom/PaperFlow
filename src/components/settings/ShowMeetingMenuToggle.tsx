import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface ShowMeetingMenuToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ShowMeetingMenuToggle: React.FC<ShowMeetingMenuToggleProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const showMeetingMenu = getSetting("show_meeting_menu") ?? false;

    return (
      <ToggleSwitch
        checked={showMeetingMenu}
        onChange={(enabled) => updateSetting("show_meeting_menu", enabled)}
        isUpdating={isUpdating("show_meeting_menu")}
        label={t("settings.advanced.showMeetingMenu.label")}
        description={t("settings.advanced.showMeetingMenu.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  });
