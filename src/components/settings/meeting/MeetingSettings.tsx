import React from "react";
import { useTranslation } from "react-i18next";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { SettingContainer } from "../../ui/SettingContainer";
import { useSettings } from "../../../hooks/useSettings";
import { MeetingHistory } from "./MeetingHistory";
import { PaperFlowShortcut } from "../PaperFlowShortcut";
import { SystemAudioInfo } from "../SystemAudioInfo";
import { DiarizationSettings } from "../DiarizationSettings";

// Feature preview card for the disabled state
const FeatureCard: React.FC<{
  title: string;
  description: string;
  delay: number;
}> = ({ title, description, delay }) => (
  <div
    className="p-3 rounded-lg bg-gradient-to-br from-logo-primary/5 to-transparent border border-logo-primary/10 hover:border-logo-primary/30 hover:from-logo-primary/10 transition-all duration-300"
    style={{
      animation: `fadeSlideIn 0.4s ease-out ${delay}ms both`,
    }}
  >
    <h4 className="text-sm font-medium text-text">{title}</h4>
    <p className="text-xs text-mid-gray mt-1">{description}</p>
  </div>
);

// Custom duration selector
const DurationSelector: React.FC<{
  value: number;
  onChange: (e: React.ChangeEvent<HTMLSelectElement>) => void;
  disabled: boolean;
  t: (key: string) => string;
}> = ({ value, onChange, disabled, t }) => {
  const options = [
    { value: 15, label: `15 ${t("common.seconds")}` },
    { value: 30, label: `30 ${t("common.seconds")}` },
    { value: 60, label: `1 ${t("common.minute")}` },
    { value: 120, label: `2 ${t("common.minutes")}` },
    { value: 300, label: `5 ${t("common.minutes")}` },
  ];

  return (
    <select
      value={value}
      onChange={onChange}
      disabled={disabled}
      className={`px-3 py-1.5 text-sm font-medium bg-gradient-to-r from-logo-primary/5 to-logo-primary/10 border border-logo-primary/30 rounded-lg min-w-[130px] ${
        disabled
          ? "opacity-50 cursor-not-allowed"
          : "hover:from-logo-primary/10 hover:to-logo-primary/20 cursor-pointer hover:border-logo-primary/50 focus:outline-none focus:ring-2 focus:ring-logo-primary/30"
      } transition-all duration-200`}
    >
      {options.map((opt) => (
        <option key={opt.value} value={opt.value}>
          {opt.label}
        </option>
      ))}
    </select>
  );
};

// LLM requirement notice
const LLMNotice: React.FC<{ message: string }> = ({ message }) => (
  <div className="mx-4 mb-3 px-3 py-2 rounded-lg bg-gradient-to-r from-amber-500/10 to-orange-500/10 border border-amber-500/20">
    <p className="text-xs text-amber-600 dark:text-amber-400">{message}</p>
  </div>
);

export const MeetingSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const meetingEnabled = getSetting("meeting_mode_enabled") ?? false;
  const autoSummarize = getSetting("meeting_auto_summarize") ?? false;
  const extractActionItems =
    getSetting("meeting_extract_action_items") ?? false;
  const chunkDuration = getSetting("meeting_chunk_duration_seconds") ?? 30;

  const handleChunkDurationChange = async (
    e: React.ChangeEvent<HTMLSelectElement>,
  ) => {
    const value = parseInt(e.target.value, 10);
    await updateSetting("meeting_chunk_duration_seconds", value);
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      {/* CSS for animations */}
      <style>{`
        @keyframes fadeSlideIn {
          from {
            opacity: 0;
            transform: translateY(8px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }
      `}</style>

      {/* Main Meeting Toggle */}
      <SettingsGroup title={t("settings.meeting.title")}>
        <ToggleSwitch
          checked={meetingEnabled}
          onChange={(enabled) => updateSetting("meeting_mode_enabled", enabled)}
          isUpdating={isUpdating("meeting_mode_enabled")}
          label={t("settings.meeting.enable.label")}
          description={t("settings.meeting.enable.description")}
          descriptionMode="tooltip"
          grouped={true}
        />

        {meetingEnabled && (
          <SettingContainer
            title={t("settings.meeting.chunkDuration.title")}
            description={t("settings.meeting.chunkDuration.description")}
            descriptionMode="tooltip"
            grouped={true}
          >
            <DurationSelector
              value={chunkDuration}
              onChange={handleChunkDurationChange}
              disabled={isUpdating("meeting_chunk_duration_seconds")}
              t={t}
            />
          </SettingContainer>
        )}
      </SettingsGroup>

      {/* Feature preview when disabled */}
      {!meetingEnabled && (
        <div className="space-y-3 px-1">
          <p className="text-xs text-mid-gray px-3">
            {t("settings.meeting.enable.description")}
          </p>
          <div className="grid grid-cols-2 gap-2">
            <FeatureCard
              title={t("settings.meeting.autoSummarize.label")}
              description={t("settings.meeting.autoSummarize.description")}
              delay={0}
            />
            <FeatureCard
              title={t("settings.meeting.extractActionItems.label")}
              description={t("settings.meeting.extractActionItems.description")}
              delay={100}
            />
            <FeatureCard
              title={t("settings.diarization.enable.label")}
              description={t("settings.diarization.enable.description")}
              delay={200}
            />
            <FeatureCard
              title={t("settings.sound.systemAudio.title")}
              description={t("settings.sound.systemAudio.description")}
              delay={300}
            />
          </div>
        </div>
      )}

      {/* Post Processing Section */}
      {meetingEnabled && (
        <SettingsGroup title={t("settings.meeting.postProcessing.title")}>
          <ToggleSwitch
            checked={autoSummarize}
            onChange={(enabled) =>
              updateSetting("meeting_auto_summarize", enabled)
            }
            isUpdating={isUpdating("meeting_auto_summarize")}
            label={t("settings.meeting.autoSummarize.label")}
            description={t("settings.meeting.autoSummarize.description")}
            descriptionMode="tooltip"
            grouped={true}
          />

          <ToggleSwitch
            checked={extractActionItems}
            onChange={(enabled) =>
              updateSetting("meeting_extract_action_items", enabled)
            }
            isUpdating={isUpdating("meeting_extract_action_items")}
            label={t("settings.meeting.extractActionItems.label")}
            description={t("settings.meeting.extractActionItems.description")}
            descriptionMode="tooltip"
            grouped={true}
          />

          {(autoSummarize || extractActionItems) && (
            <LLMNotice message={t("settings.meeting.llmRequired")} />
          )}
        </SettingsGroup>
      )}

      {/* Shortcut Section */}
      {meetingEnabled && (
        <SettingsGroup title={t("settings.meeting.shortcut.title")}>
          <PaperFlowShortcut shortcutId="meeting" grouped={true} />
        </SettingsGroup>
      )}

      {/* System Audio Section */}
      {meetingEnabled && (
        <SettingsGroup title={t("settings.sound.systemAudio.title")}>
          <SystemAudioInfo grouped={true} />
        </SettingsGroup>
      )}

      {/* Diarization Section */}
      {meetingEnabled && (
        <SettingsGroup title={t("settings.diarization.title")}>
          <DiarizationSettings grouped={true} />
        </SettingsGroup>
      )}

      {/* Meeting History */}
      {meetingEnabled && <MeetingHistory />}
    </div>
  );
};
