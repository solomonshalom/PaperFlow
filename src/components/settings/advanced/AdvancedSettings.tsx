import React from "react";
import { useTranslation } from "react-i18next";
import { type } from "@tauri-apps/plugin-os";
import { CollapsibleSection } from "../../ui/CollapsibleSection";
import { useModelStore } from "../../../stores/modelStore";
import { useSettings } from "../../../hooks/useSettings";

// App Behavior
import { StartHidden } from "../StartHidden";
import { AutostartToggle } from "../AutostartToggle";
import { UpdateChecksToggle } from "../UpdateChecksToggle";
import { DeveloperModeToggle } from "../DeveloperModeToggle";
import { ShowSidebarToggle } from "../ShowSidebarToggle";

// Recording & Input
import { AlwaysOnMicrophone } from "../AlwaysOnMicrophone";
import { ClamshellMicrophoneSelector } from "../ClamshellMicrophoneSelector";
import { MuteWhileRecording } from "../MuteWhileRecording";
import { WhisperMode } from "../WhisperMode";
import { LivePreviewSetting } from "../LivePreviewSetting";

// Output & Paste
import { ShowOverlay } from "../ShowOverlay";
import { PasteMethodSetting } from "../PasteMethod";
import { ClipboardHandlingSetting } from "../ClipboardHandling";
import { AppendTrailingSpace } from "../AppendTrailingSpace";

// Transcription Processing
import { TranslateToEnglish } from "../TranslateToEnglish";
import { WordCorrectionThreshold } from "../debug/WordCorrectionThreshold";
import { CustomWords } from "../CustomWords";
import { ModelUnloadTimeoutSetting } from "../ModelUnloadTimeout";

// Feature Toggles
import { PostProcessingToggle } from "../PostProcessingToggle";
import { SnippetsToggle } from "../SnippetsToggle";
import { FormattingToggle } from "../FormattingToggle";
import { ToneToggle } from "../ToneToggle";
import { CorrectionsToggle } from "../CorrectionsToggle";
import { ContextAwarenessToggle } from "../ContextAwarenessToggle";
import { ShowMeetingMenuToggle } from "../ShowMeetingMenuToggle";

// Storage & Diagnostics
import { HistoryLimit } from "../HistoryLimit";
import { RecordingRetentionPeriodSelector } from "../RecordingRetentionPeriod";
import { LogDirectory } from "../debug/LogDirectory";
import { LogLevelSelector } from "../debug/LogLevelSelector";
import { GroqApiKeyInput } from "../GroqApiKeyInput";
import { PaperFlowShortcut } from "../PaperFlowShortcut";

export const AdvancedSettings: React.FC = () => {
  const { t } = useTranslation();
  const { currentModel, getModelInfo } = useModelStore();
  const { getSetting } = useSettings();
  const currentModelInfo = getModelInfo(currentModel);
  const showTranslateToEnglish =
    currentModelInfo?.engine_type === "Whisper" && currentModel !== "turbo";
  const pushToTalk = getSetting("push_to_talk");
  const isLinux = type() === "linux";

  return (
    <div className="max-w-3xl w-full mx-auto space-y-4">
      <div className="px-4">
        <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
          {t("settings.advanced.title")}
        </h2>
        <p className="text-xs text-mid-gray mt-1">
          {t("settings.advanced.description")}
        </p>
      </div>

      {/* Section 1: App Behavior (default expanded) */}
      <CollapsibleSection
        id="advanced-app-behavior"
        title={t("settings.advanced.sections.appBehavior.title")}
        defaultExpanded={true}
      >
        <StartHidden descriptionMode="tooltip" grouped={true} />
        <AutostartToggle descriptionMode="tooltip" grouped={true} />
        <ShowSidebarToggle descriptionMode="tooltip" grouped={true} />
        <UpdateChecksToggle descriptionMode="tooltip" grouped={true} />
        <DeveloperModeToggle descriptionMode="tooltip" grouped={true} />
      </CollapsibleSection>

      {/* Section 2: Recording & Input */}
      <CollapsibleSection
        id="advanced-recording-input"
        title={t("settings.advanced.sections.recordingInput.title")}
      >
        <AlwaysOnMicrophone descriptionMode="tooltip" grouped={true} />
        <ClamshellMicrophoneSelector descriptionMode="tooltip" grouped={true} />
        <MuteWhileRecording descriptionMode="tooltip" grouped={true} />
        <WhisperMode descriptionMode="tooltip" grouped={true} />
        <LivePreviewSetting descriptionMode="tooltip" grouped={true} />
      </CollapsibleSection>

      {/* Section 3: Output & Paste */}
      <CollapsibleSection
        id="advanced-output-paste"
        title={t("settings.advanced.sections.outputPaste.title")}
      >
        <ShowOverlay descriptionMode="tooltip" grouped={true} />
        <PasteMethodSetting descriptionMode="tooltip" grouped={true} />
        <ClipboardHandlingSetting descriptionMode="tooltip" grouped={true} />
        <AppendTrailingSpace descriptionMode="tooltip" grouped={true} />
      </CollapsibleSection>

      {/* Section 4: Transcription Processing */}
      <CollapsibleSection
        id="advanced-transcription-processing"
        title={t("settings.advanced.sections.transcriptionProcessing.title")}
      >
        {showTranslateToEnglish && (
          <TranslateToEnglish descriptionMode="tooltip" grouped={true} />
        )}
        <WordCorrectionThreshold descriptionMode="tooltip" grouped={true} />
        <CustomWords descriptionMode="tooltip" grouped />
        <ModelUnloadTimeoutSetting descriptionMode="tooltip" grouped={true} />
      </CollapsibleSection>

      {/* Section 5: Feature Toggles */}
      <CollapsibleSection
        id="advanced-feature-toggles"
        title={t("settings.advanced.sections.featureToggles.title")}
      >
        <PostProcessingToggle descriptionMode="tooltip" grouped={true} />
        <SnippetsToggle descriptionMode="tooltip" grouped={true} />
        <FormattingToggle descriptionMode="tooltip" grouped={true} />
        <ToneToggle descriptionMode="tooltip" grouped={true} />
        <CorrectionsToggle descriptionMode="tooltip" grouped={true} />
        <ContextAwarenessToggle descriptionMode="tooltip" grouped={true} />
        <ShowMeetingMenuToggle descriptionMode="tooltip" grouped={true} />
      </CollapsibleSection>

      {/* Section 6: Storage & Diagnostics */}
      <CollapsibleSection
        id="advanced-storage-diagnostics"
        title={t("settings.advanced.sections.storageDiagnostics.title")}
      >
        <HistoryLimit descriptionMode="tooltip" grouped={true} />
        <RecordingRetentionPeriodSelector
          descriptionMode="tooltip"
          grouped={true}
        />
        <LogDirectory grouped={true} />
        <LogLevelSelector grouped={true} />
        <GroqApiKeyInput grouped={true} />
        {/* Cancel shortcut is disabled on Linux due to instability with dynamic shortcut registration */}
        {!isLinux && (
          <PaperFlowShortcut
            shortcutId="cancel"
            grouped={true}
            disabled={pushToTalk}
          />
        )}
      </CollapsibleSection>
    </div>
  );
};
