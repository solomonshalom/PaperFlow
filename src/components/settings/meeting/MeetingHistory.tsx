import React, { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Copy, Check, ChevronDown, Trash2 } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { commands, type MeetingHistoryEntry } from "@/bindings";
import { formatDateTime } from "@/utils/dateFormat";
import { SettingsGroup } from "../../ui/SettingsGroup";

interface MeetingEntryProps {
  entry: MeetingHistoryEntry;
  onDelete: (meetingId: string) => void;
  index: number;
}

// Tab button component
const TabButton: React.FC<{
  active: boolean;
  onClick: () => void;
  label: string;
  count?: number;
}> = ({ active, onClick, label, count }) => (
  <button
    onClick={onClick}
    className={`text-xs font-medium transition-colors ${
      active ? "text-text" : "text-mid-gray hover:text-text/70"
    }`}
  >
    {label}
    {count !== undefined && count > 0 && (
      <span className="ml-1 text-mid-gray">({count})</span>
    )}
  </button>
);

// Duration badge
const DurationBadge: React.FC<{ seconds: number }> = ({ seconds }) => {
  const formatDuration = (secs: number) => {
    const hours = Math.floor(secs / 3600);
    const minutes = Math.floor((secs % 3600) / 60);
    const s = secs % 60;

    if (hours > 0) return `${hours}h ${minutes}m`;
    if (minutes > 0) return `${minutes}m ${s}s`;
    return `${s}s`;
  };

  return (
    <span className="px-2 py-0.5 rounded-full bg-logo-primary/10 text-logo-primary text-xs font-medium">
      {formatDuration(seconds)}
    </span>
  );
};

const MeetingEntry: React.FC<MeetingEntryProps> = ({
  entry,
  onDelete,
  index,
}) => {
  const { t, i18n } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [activeTab, setActiveTab] = useState<
    "transcript" | "summary" | "actions"
  >("transcript");
  const [copied, setCopied] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

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

  const hasSummary = Boolean(entry.summary);
  const hasActions = entry.action_items && entry.action_items.length > 0;

  return (
    <div
      className="group border border-mid-gray/20 rounded-xl overflow-hidden bg-background hover:border-logo-primary/30 transition-all duration-300"
      style={{
        animation: `fadeSlideIn 0.3s ease-out ${index * 50}ms both`,
      }}
    >
      {/* Card Header */}
      <div
        className="flex items-center justify-between p-4 cursor-pointer hover:bg-gradient-to-r hover:from-logo-primary/5 hover:to-transparent transition-all duration-200"
        onClick={() => setExpanded(!expanded)}
      >
        <div className="flex flex-col gap-2">
          {/* Date and time */}
          <span className="text-sm font-medium">
            {formatDateTime(String(entry.started_at), i18n.language)}
          </span>

          {/* Badges row */}
          <div className="flex items-center gap-2 flex-wrap">
            <DurationBadge seconds={Number(entry.duration_seconds)} />
            <span className="px-2 py-0.5 rounded-full bg-mid-gray/10 text-mid-gray text-xs">
              {entry.chunk_count} {t("settings.meeting.history.chunks")}
            </span>
            {hasSummary && (
              <span className="px-2 py-0.5 rounded-full bg-gradient-to-r from-purple-500/10 to-pink-500/10 text-purple-600 dark:text-purple-400 text-xs font-medium border border-purple-500/20">
                {t("settings.meeting.history.hasSummary")}
              </span>
            )}
            {hasActions && (
              <span className="px-2 py-0.5 rounded-full bg-gradient-to-r from-emerald-500/10 to-teal-500/10 text-emerald-600 dark:text-emerald-400 text-xs font-medium border border-emerald-500/20">
                {entry.action_items?.length}{" "}
                {t("settings.meeting.history.actionItems")}
              </span>
            )}
          </div>
        </div>

        {/* Right side controls */}
        <div className="flex items-center gap-2">
          <button
            onClick={(e) => {
              e.stopPropagation();
              setShowDeleteConfirm(true);
            }}
            className="p-2 rounded-lg text-mid-gray/50 hover:text-red-500 hover:bg-red-500/10 transition-all duration-200 opacity-0 group-hover:opacity-100"
            title={t("common.delete")}
          >
            <Trash2 className="w-4 h-4" />
          </button>
          <div
            className={`p-2 rounded-lg bg-mid-gray/10 transition-transform duration-300 ${
              expanded ? "rotate-180" : ""
            }`}
          >
            <ChevronDown className="w-4 h-4 text-mid-gray" />
          </div>
        </div>
      </div>

      {/* Delete confirmation dialog */}
      {showDeleteConfirm && (
        <div className="p-4 bg-gradient-to-r from-red-500/10 to-orange-500/10 border-t border-red-500/20">
          <p className="text-sm font-medium text-red-600 dark:text-red-400 mb-3">
            {t("settings.meeting.history.deleteConfirm")}
          </p>
          <div className="flex gap-2">
            <button
              onClick={() => {
                onDelete(entry.meeting_id);
                setShowDeleteConfirm(false);
              }}
              className="px-4 py-1.5 text-sm font-medium bg-red-500 text-white rounded-lg hover:bg-red-600 transition-colors shadow-sm"
            >
              {t("common.delete")}
            </button>
            <button
              onClick={() => setShowDeleteConfirm(false)}
              className="px-4 py-1.5 text-sm font-medium bg-mid-gray/20 rounded-lg hover:bg-mid-gray/30 transition-colors"
            >
              {t("common.cancel")}
            </button>
          </div>
        </div>
      )}

      {/* Expanded Content */}
      {expanded && (
        <div className="border-t border-mid-gray/20 p-4 space-y-3">
          {/* Tabs */}
          <div className="flex items-center gap-4">
            <TabButton
              active={activeTab === "transcript"}
              onClick={() => setActiveTab("transcript")}
              label={t("settings.meeting.history.transcript")}
            />
            {hasSummary && (
              <TabButton
                active={activeTab === "summary"}
                onClick={() => setActiveTab("summary")}
                label={t("settings.meeting.history.summary")}
              />
            )}
            {hasActions && (
              <TabButton
                active={activeTab === "actions"}
                onClick={() => setActiveTab("actions")}
                label={t("settings.meeting.history.actionItems")}
                count={entry.action_items?.length}
              />
            )}
            <div className="flex-1" />
            {/* Copy button */}
            <button
              onClick={() => copyToClipboard(getCurrentText())}
              className={`p-1.5 rounded transition-colors ${
                copied
                  ? "text-green-500"
                  : "text-text/40 hover:text-logo-primary hover:bg-logo-primary/10"
              }`}
              title={t("common.copy")}
            >
              {copied ? (
                <Check className="w-4 h-4" />
              ) : (
                <Copy className="w-4 h-4" />
              )}
            </button>
          </div>

          {/* Content area */}
          <div className="max-h-56 overflow-y-auto">
            {activeTab === "actions" && entry.action_items ? (
              <ul className="space-y-1.5 text-sm">
                {entry.action_items.map((item, i) => (
                  <li key={i} className="flex items-start gap-2">
                    <span className="text-mid-gray shrink-0">{i + 1}.</span>
                    <span className="text-text/90">{item}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="italic text-text/90 text-sm select-text cursor-text whitespace-pre-wrap">
                {getCurrentText()}
              </p>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

// Empty state
const EmptyState: React.FC<{ message: string }> = ({ message }) => (
  <div className="px-4 py-3 text-center text-text/60">{message}</div>
);

// Loading skeleton
const LoadingSkeleton: React.FC = () => (
  <div className="space-y-3 p-3">
    {[1, 2].map((i) => (
      <div
        key={i}
        className="p-4 rounded-xl border border-mid-gray/20 animate-pulse"
      >
        <div className="h-4 w-32 rounded bg-mid-gray/20 mb-3" />
        <div className="flex gap-2">
          <div className="h-5 w-16 rounded-full bg-mid-gray/15" />
          <div className="h-5 w-20 rounded-full bg-mid-gray/15" />
        </div>
      </div>
    ))}
  </div>
);

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

  return (
    <SettingsGroup title={t("settings.meeting.history.title")}>
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

      {loading ? (
        <LoadingSkeleton />
      ) : entries.length === 0 ? (
        <EmptyState message={t("settings.meeting.history.empty")} />
      ) : (
        <div className="space-y-3 p-3">
          {entries.map((entry, index) => (
            <MeetingEntry
              key={entry.meeting_id}
              entry={entry}
              onDelete={handleDelete}
              index={index}
            />
          ))}
        </div>
      )}
    </SettingsGroup>
  );
};
