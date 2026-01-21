import React from "react";
import { useTranslation } from "react-i18next";
import { MicrophoneSelector } from "../MicrophoneSelector";
import { LanguageSelector } from "../LanguageSelector";
import { MultiLanguageSelector } from "../language/MultiLanguageSelector";
import { PaperFlowShortcut } from "../PaperFlowShortcut";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { OutputDeviceSelector } from "../OutputDeviceSelector";
import { PushToTalk } from "../PushToTalk";
import { AudioFeedback } from "../AudioFeedback";
import { useSettings } from "../../../hooks/useSettings";
import { useModelStore } from "../../../stores/modelStore";
import { VolumeSlider } from "../VolumeSlider";
import { SoundPicker } from "../SoundPicker";

export const GeneralSettings: React.FC = () => {
  const { t } = useTranslation();
  const { audioFeedbackEnabled } = useSettings();
  const { currentModel, getModelInfo } = useModelStore();
  const currentModelInfo = getModelInfo(currentModel);
  const showLanguageSelector = currentModelInfo?.engine_type === "Whisper";
  // Multilingual mode is supported for Whisper and GroqCloud models
  const showMultilingualSelector =
    currentModelInfo?.engine_type === "Whisper" ||
    currentModelInfo?.engine_type === "GroqCloud";
  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.general.title")}>
        <PaperFlowShortcut shortcutId="transcribe" grouped={true} />
        {showLanguageSelector && (
          <LanguageSelector descriptionMode="tooltip" grouped={true} />
        )}
        {showMultilingualSelector && (
          <MultiLanguageSelector descriptionMode="tooltip" grouped={true} />
        )}
        <PushToTalk descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>
      <SettingsGroup title={t("settings.sound.title")}>
        <MicrophoneSelector descriptionMode="tooltip" grouped={true} />
        <AudioFeedback descriptionMode="tooltip" grouped={true} />
        {audioFeedbackEnabled && (
          <SoundPicker
            label={t("settings.debug.soundTheme.label")}
            description={t("settings.debug.soundTheme.description")}
          />
        )}
        <OutputDeviceSelector
          descriptionMode="tooltip"
          grouped={true}
          disabled={!audioFeedbackEnabled}
        />
        <VolumeSlider disabled={!audioFeedbackEnabled} />
      </SettingsGroup>
    </div>
  );
};
