import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Dropdown } from "../../ui/Dropdown";

export const ToneSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const toneAdjustmentEnabled = getSetting("tone_adjustment_enabled") ?? false;
  const defaultTone = getSetting("default_tone") ?? "neutral";

  const toneOptions = [
    { value: "formal", label: t("settings.tone.tones.formal") },
    { value: "casual", label: t("settings.tone.tones.casual") },
    { value: "technical", label: t("settings.tone.tones.technical") },
    { value: "neutral", label: t("settings.tone.tones.neutral") },
  ];

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.tone.title")}>
        <ToggleSwitch
          checked={toneAdjustmentEnabled}
          onChange={(v) => updateSetting("tone_adjustment_enabled", v)}
          isUpdating={isUpdating("tone_adjustment_enabled")}
          label={t("settings.tone.enable.label")}
          description={t("settings.tone.enable.description")}
          descriptionMode="tooltip"
          grouped={true}
        />

        {toneAdjustmentEnabled && (
          <SettingContainer
            title={t("settings.tone.defaultTone.label")}
            description={t("settings.tone.defaultTone.description")}
            descriptionMode="tooltip"
            grouped={true}
          >
            <Dropdown
              options={toneOptions}
              selectedValue={defaultTone}
              onSelect={(v) => updateSetting("default_tone", v as any)}
              disabled={isUpdating("default_tone")}
            />
          </SettingContainer>
        )}
      </SettingsGroup>

      {toneAdjustmentEnabled && (
        <div className="text-sm text-mid-gray px-4">
          <p className="font-medium mb-2">{t("settings.tone.description")}</p>
          <ul className="list-disc list-inside space-y-1">
            <li>
              {t("settings.tone.tones.formal")}:{" "}
              {t("settings.tone.examples.formal")}
            </li>
            <li>
              {t("settings.tone.tones.casual")}:{" "}
              {t("settings.tone.examples.casual")}
            </li>
            <li>
              {t("settings.tone.tones.technical")}:{" "}
              {t("settings.tone.examples.technical")}
            </li>
          </ul>
        </div>
      )}
    </div>
  );
};
