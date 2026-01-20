import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Input } from "../../ui/Input";
import { Button } from "../../ui/Button";
import type { Snippet } from "@/bindings";

export const SnippetsSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const [newTrigger, setNewTrigger] = useState("");
  const [newExpansion, setNewExpansion] = useState("");

  const snippetsEnabled = getSetting("snippets_enabled") ?? false;
  const snippets = getSetting("snippets") ?? [];

  const handleAddSnippet = () => {
    const trigger = newTrigger.trim();
    const expansion = newExpansion.trim();

    if (trigger && expansion) {
      // Check for duplicate trigger
      if (
        snippets.some((s) => s.trigger.toLowerCase() === trigger.toLowerCase())
      ) {
        return; // Don't add duplicate
      }

      const newSnippet: Snippet = {
        id: crypto.randomUUID(),
        trigger,
        expansion,
        case_sensitive: false,
        whole_word: true,
      };
      updateSetting("snippets", [...snippets, newSnippet]);
      setNewTrigger("");
      setNewExpansion("");
    }
  };

  const handleRemoveSnippet = (id: string) => {
    updateSetting(
      "snippets",
      snippets.filter((s) => s.id !== id),
    );
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleAddSnippet();
    }
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.snippets.title")}>
        <ToggleSwitch
          checked={snippetsEnabled}
          onChange={(v) => updateSetting("snippets_enabled", v)}
          isUpdating={isUpdating("snippets_enabled")}
          label={t("settings.snippets.enable.label")}
          description={t("settings.snippets.enable.description")}
          descriptionMode="tooltip"
          grouped={true}
        />

        {snippetsEnabled && (
          <SettingContainer
            title={t("settings.snippets.add.title")}
            description={t("settings.snippets.add.description")}
            descriptionMode="inline"
            grouped={true}
          >
            <div className="flex flex-col gap-2 w-full">
              <div className="flex items-center gap-2 flex-wrap">
                <Input
                  type="text"
                  className="w-36 min-w-0"
                  value={newTrigger}
                  onChange={(e) => setNewTrigger(e.target.value)}
                  onKeyDown={handleKeyPress}
                  placeholder={t("settings.snippets.triggerPlaceholder")}
                  variant="compact"
                  disabled={isUpdating("snippets")}
                />
                <span className="text-mid-gray shrink-0">→</span>
                <Input
                  type="text"
                  className="flex-1 min-w-[140px]"
                  value={newExpansion}
                  onChange={(e) => setNewExpansion(e.target.value)}
                  onKeyDown={handleKeyPress}
                  placeholder={t("settings.snippets.expansionPlaceholder")}
                  variant="compact"
                  disabled={isUpdating("snippets")}
                />
                <Button
                  onClick={handleAddSnippet}
                  disabled={
                    !newTrigger.trim() ||
                    !newExpansion.trim() ||
                    isUpdating("snippets")
                  }
                  variant="primary"
                  size="md"
                >
                  {t("common.add")}
                </Button>
              </div>
            </div>
          </SettingContainer>
        )}
      </SettingsGroup>

      {snippetsEnabled && snippets.length > 0 && (
        <SettingsGroup title={t("settings.snippets.list.title")}>
          <div className="px-4 py-2 space-y-2">
            {snippets.map((snippet) => (
              <div
                key={snippet.id}
                className="flex items-center justify-between p-3 bg-mid-gray/10 rounded-lg group"
              >
                <div className="flex items-center gap-3 min-w-0 flex-1">
                  <span className="font-medium text-sm shrink-0 max-w-[120px] truncate">
                    {snippet.trigger}
                  </span>
                  <span className="text-mid-gray shrink-0">→</span>
                  <span className="text-sm text-mid-gray truncate">
                    {snippet.expansion}
                  </span>
                </div>
                <Button
                  onClick={() => handleRemoveSnippet(snippet.id)}
                  disabled={isUpdating("snippets")}
                  variant="secondary"
                  size="sm"
                  className="shrink-0 ml-2 opacity-60 group-hover:opacity-100 transition-opacity"
                  aria-label={t("settings.snippets.delete")}
                >
                  <svg
                    className="w-4 h-4"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M6 18L18 6M6 6l12 12"
                    />
                  </svg>
                </Button>
              </div>
            ))}
          </div>
        </SettingsGroup>
      )}

      {snippetsEnabled && snippets.length === 0 && (
        <div className="text-center text-mid-gray py-4 text-sm">
          {t("settings.snippets.list.empty")}
        </div>
      )}
    </div>
  );
};
