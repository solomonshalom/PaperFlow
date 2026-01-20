import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Dropdown } from "../../ui/Dropdown";
import { Input } from "../../ui/Input";
import { Button } from "../../ui/Button";

export const DeveloperSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const [newTerm, setNewTerm] = useState("");

  const developerMode = getSetting("developer_mode") ?? "off";
  const preserveCodeSyntax = getSetting("preserve_code_syntax") ?? false;
  const developerDictionary = getSetting("developer_dictionary") ?? [];

  const isDeveloperEnabled = developerMode !== "off";

  const modeOptions = [
    { value: "off", label: t("settings.developer.modeOptions.off") },
    { value: "auto", label: t("settings.developer.modeOptions.auto") },
    { value: "always", label: t("settings.developer.modeOptions.always") },
  ];

  const handleAddTerm = () => {
    const term = newTerm.trim();
    if (term && !developerDictionary.includes(term)) {
      updateSetting("developer_dictionary", [...developerDictionary, term]);
      setNewTerm("");
    }
  };

  const handleRemoveTerm = (termToRemove: string) => {
    updateSetting(
      "developer_dictionary",
      developerDictionary.filter((term) => term !== termToRemove),
    );
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleAddTerm();
    }
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.developer.title")}>
        <SettingContainer
          title={t("settings.developer.mode.label")}
          description={t("settings.developer.mode.description")}
          descriptionMode="tooltip"
          grouped={true}
        >
          <Dropdown
            options={modeOptions}
            selectedValue={developerMode}
            onSelect={(v) => updateSetting("developer_mode", v as any)}
            disabled={isUpdating("developer_mode")}
          />
        </SettingContainer>

        {isDeveloperEnabled && (
          <ToggleSwitch
            checked={preserveCodeSyntax}
            onChange={(v) => updateSetting("preserve_code_syntax", v)}
            isUpdating={isUpdating("preserve_code_syntax")}
            label={t("settings.developer.preserveSyntax.label")}
            description={t("settings.developer.preserveSyntax.description")}
            descriptionMode="tooltip"
            grouped={true}
          />
        )}
      </SettingsGroup>

      {isDeveloperEnabled && (
        <SettingsGroup title={t("settings.developer.dictionary.title")}>
          <SettingContainer
            title={t("settings.developer.dictionary.title")}
            description={t("settings.developer.dictionary.description")}
            descriptionMode="inline"
            grouped={true}
          >
            <div className="flex items-center gap-2">
              <Input
                type="text"
                className="max-w-40"
                value={newTerm}
                onChange={(e) => setNewTerm(e.target.value)}
                onKeyDown={handleKeyPress}
                placeholder={t("settings.developer.dictionary.placeholder")}
                variant="compact"
                disabled={isUpdating("developer_dictionary")}
              />
              <Button
                onClick={handleAddTerm}
                variant="primary"
                size="md"
                disabled={!newTerm.trim() || isUpdating("developer_dictionary")}
              >
                {t("common.add")}
              </Button>
            </div>
          </SettingContainer>
          {developerDictionary.length > 0 && (
            <div className="px-4 py-2 flex flex-wrap gap-1">
              {developerDictionary.map((term) => (
                <Button
                  key={term}
                  onClick={() => handleRemoveTerm(term)}
                  variant="secondary"
                  size="sm"
                  className="inline-flex items-center gap-1"
                  disabled={isUpdating("developer_dictionary")}
                >
                  <span>{term}</span>
                  <svg
                    className="w-3 h-3"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M6 18L18 6M6 6l12 12"
                    />
                  </svg>
                </Button>
              ))}
            </div>
          )}
        </SettingsGroup>
      )}

      {isDeveloperEnabled && (
        <div className="text-sm text-mid-gray px-4">
          <p className="font-medium mb-2">
            {t("settings.developer.description")}
          </p>
          <ul className="list-disc list-inside space-y-1">
            <li>{t("settings.developer.examples.caseConventions")}</li>
            <li>{t("settings.developer.examples.acronyms")}</li>
            <li>{t("settings.developer.examples.frameworks")}</li>
          </ul>
        </div>
      )}
    </div>
  );
};
