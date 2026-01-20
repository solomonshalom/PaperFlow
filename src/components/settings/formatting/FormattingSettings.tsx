import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { ToggleSwitch } from "../../ui/ToggleSwitch";

export const FormattingSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const autoFormatEnabled = getSetting("auto_format_enabled") ?? false;
  const autoFormatLists = getSetting("auto_format_lists") ?? false;
  const verbalCommandsEnabled = getSetting("verbal_commands_enabled") ?? false;

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.formatting.title")}>
        <ToggleSwitch
          checked={autoFormatEnabled}
          onChange={(v) => updateSetting("auto_format_enabled", v)}
          isUpdating={isUpdating("auto_format_enabled")}
          label={t("settings.formatting.enable.label")}
          description={t("settings.formatting.enable.description")}
          descriptionMode="tooltip"
          grouped={true}
        />

        {autoFormatEnabled && (
          <>
            <ToggleSwitch
              checked={autoFormatLists}
              onChange={(v) => updateSetting("auto_format_lists", v)}
              isUpdating={isUpdating("auto_format_lists")}
              label={t("settings.formatting.lists.label")}
              description={t("settings.formatting.lists.description")}
              descriptionMode="tooltip"
              grouped={true}
            />

            <ToggleSwitch
              checked={verbalCommandsEnabled}
              onChange={(v) => updateSetting("verbal_commands_enabled", v)}
              isUpdating={isUpdating("verbal_commands_enabled")}
              label={t("settings.formatting.verbalCommands.label")}
              description={t("settings.formatting.verbalCommands.description")}
              descriptionMode="tooltip"
              grouped={true}
            />
          </>
        )}
      </SettingsGroup>

      {autoFormatEnabled && (
        <div className="text-sm text-mid-gray px-4">
          <p className="font-medium mb-2">
            {t("settings.formatting.examples.title")}:
          </p>
          <ul className="list-disc list-inside space-y-1">
            {verbalCommandsEnabled && (
              <>
                <li>{t("settings.formatting.examples.newLine")}</li>
                <li>{t("settings.formatting.examples.newParagraph")}</li>
                <li>{t("settings.formatting.examples.bulletPoint")}</li>
                <li>{t("settings.formatting.examples.deleteThat")}</li>
                <li>{t("settings.formatting.examples.deleteLastLine")}</li>
                <li>{t("settings.formatting.examples.scratchLastSentence")}</li>
              </>
            )}
            {autoFormatLists && (
              <li>{t("settings.formatting.examples.numberedList")}</li>
            )}
          </ul>
        </div>
      )}
    </div>
  );
};
