import React from "react";
import { useTranslation } from "react-i18next";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { SettingContainer } from "../../ui/SettingContainer";
import { useSettings } from "../../../hooks/useSettings";
import { MeetingHistory } from "./MeetingHistory";
import { HandyShortcut } from "../HandyShortcut";

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
          <>
            <SettingContainer
              title={t("settings.meeting.chunkDuration.title")}
              description={t("settings.meeting.chunkDuration.description")}
              descriptionMode="tooltip"
              grouped={true}
            >
              <select
                value={chunkDuration}
                onChange={handleChunkDurationChange}
                disabled={isUpdating("meeting_chunk_duration_seconds")}
                className={`px-2 py-1 text-sm font-semibold bg-mid-gray/10 border border-mid-gray/80 rounded min-w-[120px] ${
                  isUpdating("meeting_chunk_duration_seconds")
                    ? "opacity-50 cursor-not-allowed"
                    : "hover:bg-logo-primary/10 cursor-pointer hover:border-logo-primary"
                }`}
              >
                <option value={15}>15 {t("common.seconds")}</option>
                <option value={30}>30 {t("common.seconds")}</option>
                <option value={60}>1 {t("common.minute")}</option>
                <option value={120}>2 {t("common.minutes")}</option>
                <option value={300}>5 {t("common.minutes")}</option>
              </select>
            </SettingContainer>
          </>
        )}
      </SettingsGroup>

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
            <div className="px-4 py-2 text-xs text-mid-gray">
              {t("settings.meeting.llmRequired")}
            </div>
          )}
        </SettingsGroup>
      )}

      {meetingEnabled && (
        <SettingsGroup title={t("settings.meeting.shortcut.title")}>
          <HandyShortcut shortcutId="meeting" grouped={true} />
        </SettingsGroup>
      )}

      {meetingEnabled && <MeetingHistory />}
    </div>
  );
};
