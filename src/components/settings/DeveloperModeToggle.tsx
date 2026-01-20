import React from "react";
import { useTranslation } from "react-i18next";
import { SettingContainer } from "../ui/SettingContainer";
import { Dropdown } from "../ui/Dropdown";
import { useSettings } from "../../hooks/useSettings";

interface DeveloperModeToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const DeveloperModeToggle: React.FC<DeveloperModeToggleProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const developerMode = getSetting("developer_mode") ?? "off";

    const modeOptions = [
      { value: "off", label: t("settings.developer.modeOptions.off") },
      { value: "auto", label: t("settings.developer.modeOptions.auto") },
      { value: "always", label: t("settings.developer.modeOptions.always") },
    ];

    return (
      <SettingContainer
        title={t("settings.developer.mode.label")}
        description={t("settings.developer.mode.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <Dropdown
          options={modeOptions}
          selectedValue={developerMode}
          onSelect={(v) => updateSetting("developer_mode", v as any)}
          disabled={isUpdating("developer_mode")}
        />
      </SettingContainer>
    );
  });
