import React, { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Copy, Check, ChevronDown, ChevronUp, Trash2 } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { commands, type MeetingHistoryEntry } from "@/bindings";
import { formatDateTime } from "@/utils/dateFormat";
import { SettingsGroup } from "../../ui/SettingsGroup";

interface MeetingEntryProps {
  entry: MeetingHistoryEntry;
  onDelete: (meetingId: string) => void;
}

const MeetingEntry: React.FC<MeetingEntryProps> = ({ entry, onDelete }) => {
  const { t, i18n } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [activeTab, setActiveTab] = useState<
    "transcript" | "summary" | "actions"
  >("transcript");
  const [copied, setCopied] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const formatDuration = (seconds: number) => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;

    if (hours > 0) {
      return `${hours}h ${minutes}m ${secs}s`;
    } else if (minutes > 0) {
      return `${minutes}m ${secs}s`;
    }
    return `${secs}s`;
  };

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (error) {
      console.error("Failed to copy to clipboard:", error);
    }
  };

  const getCurrentText = () => {
    switch (activeTab) {
      case "summary":
        return entry.summary || "";
      case "actions":
        return entry.action_items?.join("\n") || "";
      default:
        return entry.full_transcript;
    }
  };

  return (
    <div className="border border-mid-gray/20 rounded-lg overflow-hidden">
      <div
        className="flex items-center justify-between p-3 bg-mid-gray/5 cursor-pointer hover:bg-mid-gray/10 transition-colors"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex flex-col">
          <span className="text-sm font-medium">
            {formatDateTime(String(entry.started_at), i18n.language)}
          </span>
          <span className="text-xs text-mid-gray">
            {formatDuration(Number(entry.duration_seconds))} &middot;{" "}
            {entry.chunk_count} {t("settings.meeting.history.chunks")}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {entry.summary && (
            <span className="text-xs bg-logo-primary/20 text-logo-primary px-2 py-0.5 rounded">
              {t("settings.meeting.history.hasSummary")}
            </span>
          )}
          <button
            onClick={(e) => {
              e.stopPropagation();
              setShowDeleteConfirm(true);
            }}
            className="p-1 hover:bg-red-500/20 rounded transition-colors"
            title={t("common.delete")}
          >
            <Trash2 className="w-4 h-4 text-mid-gray hover:text-red-500" />
          </button>
          {expanded ? (
            <ChevronUp className="w-4 h-4" />
          ) : (
            <ChevronDown className="w-4 h-4" />
          )}
        </div>
      </div>

      {/* Delete confirmation dialog */}
      {showDeleteConfirm && (
        <div className="p-3 bg-red-500/10 border-t border-red-500/20">
          <p className="text-sm text-red-500 mb-2">
            {t("settings.meeting.history.deleteConfirm")}
          </p>
          <div className="flex gap-2">
            <button
              onClick={() => {
                onDelete(entry.meeting_id);
                setShowDeleteConfirm(false);
              }}
              className="px-3 py-1 text-sm bg-red-500 text-white rounded hover:bg-red-600 transition-colors"
            >
              {t("common.delete")}
            </button>
            <button
              onClick={() => setShowDeleteConfirm(false)}
              className="px-3 py-1 text-sm bg-mid-gray/20 rounded hover:bg-mid-gray/30 transition-colors"
            >
              {t("common.cancel")}
            </button>
          </div>
        </div>
      )}

      {expanded && (
        <div className="p-3 space-y-3">
          {/* Tabs */}
          <div className="flex gap-2 border-b border-mid-gray/20 pb-2">
            <button
              onClick={() => setActiveTab("transcript")}
              className={`text-sm px-3 py-1 rounded transition-colors ${
                activeTab === "transcript"
                  ? "bg-logo-primary/20 text-logo-primary"
                  : "text-mid-gray hover:text-foreground"
              }`}
            >
              {t("settings.meeting.history.transcript")}
            </button>
            {entry.summary && (
              <button
                onClick={() => setActiveTab("summary")}
                className={`text-sm px-3 py-1 rounded transition-colors ${
                  activeTab === "summary"
                    ? "bg-logo-primary/20 text-logo-primary"
                    : "text-mid-gray hover:text-foreground"
                }`}
              >
                {t("settings.meeting.history.summary")}
              </button>
            )}
            {entry.action_items && entry.action_items.length > 0 && (
              <button
                onClick={() => setActiveTab("actions")}
                className={`text-sm px-3 py-1 rounded transition-colors ${
                  activeTab === "actions"
                    ? "bg-logo-primary/20 text-logo-primary"
                    : "text-mid-gray hover:text-foreground"
                }`}
              >
                {t("settings.meeting.history.actionItems")}
              </button>
            )}
          </div>

          {/* Content */}
          <div className="relative">
            <div className="max-h-48 overflow-y-auto bg-mid-gray/5 rounded p-3 text-sm">
              {activeTab === "actions" && entry.action_items ? (
                <ul className="list-disc pl-4 space-y-1">
                  {entry.action_items.map((item, i) => (
                    <li key={i}>{item}</li>
                  ))}
                </ul>
              ) : (
                <p className="whitespace-pre-wrap">{getCurrentText()}</p>
              )}
            </div>

            {/* Copy button */}
            <button
              onClick={() => copyToClipboard(getCurrentText())}
              className="absolute top-2 right-2 p-1.5 bg-background/80 rounded hover:bg-mid-gray/20 transition-colors"
              title={t("common.copy")}
            >
              {copied ? (
                <Check className="w-4 h-4 text-green-500" />
              ) : (
                <Copy className="w-4 h-4" />
              )}
            </button>
          </div>
        </div>
      )}
    </div>
  );
};

export const MeetingHistory: React.FC = () => {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<MeetingHistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);

  const loadEntries = useCallback(async () => {
    try {
      const result = await commands.getMeetingHistory();
      if (result.status === "ok") {
        setEntries(result.data);
      }
    } catch (error) {
      console.error("Failed to load meeting history:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  const handleDelete = useCallback(async (meetingId: string) => {
    try {
      const result = await commands.deleteMeeting(meetingId);
      if (result.status === "error") {
        console.error("Failed to delete meeting:", result.error);
      }
      // The meeting-history-updated event will trigger a refresh
    } catch (error) {
      console.error("Failed to delete meeting:", error);
    }
  }, []);

  useEffect(() => {
    loadEntries();

    const setupListener = async () => {
      const unlisten = await listen("meeting-history-updated", () => {
        loadEntries();
      });
      return unlisten;
    };

    const unlistenPromise = setupListener();

    return () => {
      unlistenPromise.then((unlisten) => {
        if (unlisten) {
          unlisten();
        }
      });
    };
  }, [loadEntries]);

  if (loading) {
    return (
      <SettingsGroup title={t("settings.meeting.history.title")}>
        <div className="p-4 text-center text-mid-gray">
          {t("common.loading")}
        </div>
      </SettingsGroup>
    );
  }

  if (entries.length === 0) {
    return (
      <SettingsGroup title={t("settings.meeting.history.title")}>
        <div className="p-4 text-center text-mid-gray">
          {t("settings.meeting.history.empty")}
        </div>
      </SettingsGroup>
    );
  }

  return (
    <SettingsGroup title={t("settings.meeting.history.title")}>
      <div className="space-y-2 p-2">
        {entries.map((entry) => (
          <MeetingEntry
            key={entry.meeting_id}
            entry={entry}
            onDelete={handleDelete}
          />
        ))}
      </div>
    </SettingsGroup>
  );
};
