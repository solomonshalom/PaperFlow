import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { SettingContainer } from "../ui/SettingContainer";
import { Input } from "../ui/Input";
import { useSettings } from "../../hooks/useSettings";

interface GroqApiKeyInputProps {
  grouped?: boolean;
}

export const GroqApiKeyInput: React.FC<GroqApiKeyInputProps> = React.memo(
  ({ grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const [showKey, setShowKey] = useState(false);

    const apiKey = getSetting("groq_transcription_api_key") || "";

    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      updateSetting("groq_transcription_api_key", e.target.value);
    };

    return (
      <SettingContainer
        title={t("settings.cloudTranscription.groqApiKey.label")}
        description={t("settings.cloudTranscription.groqApiKey.description")}
        descriptionMode="inline"
        grouped={grouped}
      >
        <div className="flex items-center gap-2">
          <Input
            type={showKey ? "text" : "password"}
            className="flex-1 max-w-xs"
            value={apiKey}
            onChange={handleChange}
            placeholder={t(
              "settings.cloudTranscription.groqApiKey.placeholder",
            )}
            disabled={isUpdating("groq_transcription_api_key")}
            variant="compact"
          />
          <button
            type="button"
            onClick={() => setShowKey(!showKey)}
            className="text-sm text-mid-gray hover:text-foreground"
          >
            {showKey ? "Hide" : "Show"}
          </button>
        </div>
      </SettingContainer>
    );
  },
);
