import React, { useEffect, useState, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { SettingsGroup } from "../ui/SettingsGroup";
import { commands } from "@/bindings";

interface DiarizationStatus {
  available: boolean;
  enabled: boolean;
  models_downloaded: boolean;
  download_progress: number | null;
  error: string | null;
}

interface DiarizationSettingsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const DiarizationSettings: React.FC<DiarizationSettingsProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const [status, setStatus] = useState<DiarizationStatus | null>(null);
    const [isLoading, setIsLoading] = useState(true);
    const [isDownloading, setIsDownloading] = useState(false);
    const [isUpdating, setIsUpdating] = useState(false);
    const [downloadError, setDownloadError] = useState<string | null>(null);
    const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);

    const loadStatus = useCallback(async () => {
      try {
        const result = await commands.getDiarizationStatus();
        setStatus(result);

        // Check if download completed (was downloading, now has progress null and either ready or error)
        if (
          isDownloading &&
          result.download_progress === null &&
          (result.available || result.error)
        ) {
          setIsDownloading(false);
          if (result.error) {
            setDownloadError(result.error);
          }
        }
      } catch (error) {
        console.error("Failed to load diarization status:", error);
      } finally {
        setIsLoading(false);
      }
    }, [isDownloading]);

    // Initial load
    useEffect(() => {
      loadStatus();
    }, [loadStatus]);

    // Polling while downloading
    useEffect(() => {
      if (isDownloading) {
        pollingRef.current = setInterval(loadStatus, 500);
      } else if (pollingRef.current) {
        clearInterval(pollingRef.current);
        pollingRef.current = null;
      }

      return () => {
        if (pollingRef.current) {
          clearInterval(pollingRef.current);
        }
      };
    }, [isDownloading, loadStatus]);

    const handleToggle = async (enabled: boolean) => {
      setIsUpdating(true);
      try {
        await commands.changeDiarizationEnabledSetting(enabled);
        await loadStatus();
      } catch (error) {
        console.error("Failed to update diarization setting:", error);
        // Reload status to get correct state
        await loadStatus();
      } finally {
        setIsUpdating(false);
      }
    };

    const handleDownload = async () => {
      setIsDownloading(true);
      setDownloadError(null);
      try {
        await commands.downloadDiarizationModels();
        // Status will be updated by polling
      } catch (error) {
        console.error("Failed to download models:", error);
        setDownloadError(
          error instanceof Error ? error.message : "Download failed",
        );
        setIsDownloading(false);
      }
    };

    // Loading state
    if (isLoading) {
      const loadingContent = (
        <div className="px-4 py-3">
          <div className="flex items-center gap-2 text-sm text-mid-gray">
            <span className="inline-block w-3 h-3 border-2 border-mid-gray/30 border-t-logo-primary rounded-full animate-spin" />
            {t("common.loading")}
          </div>
        </div>
      );

      if (grouped) {
        return loadingContent;
      }
      return (
        <SettingsGroup title={t("settings.diarization.title")}>
          {loadingContent}
        </SettingsGroup>
      );
    }

    if (!status) {
      return null;
    }

    // Show download button if:
    // - Models not downloaded AND not currently downloading
    // - OR there was an error (allow retry)
    const showDownloadButton =
      (!status.models_downloaded && !isDownloading) ||
      (downloadError && !isDownloading);

    const progressPercent = Math.round((status.download_progress ?? 0) * 100);

    const content = (
      <div className="divide-y divide-mid-gray/10">
        {/* Toggle */}
        <ToggleSwitch
          checked={status.enabled}
          onChange={handleToggle}
          isUpdating={isUpdating}
          label={t("settings.diarization.enable.label")}
          description={t("settings.diarization.enable.description")}
          descriptionMode={descriptionMode}
          grouped={true}
          disabled={!status.available}
        />

        {/* Status indicator */}
        <div className="px-4 py-3">
          <div className="flex items-center gap-3">
            {status.available ? (
              <>
                <span className="inline-block w-1.5 h-1.5 bg-emerald-500 rounded-full shrink-0" />
                <span className="text-sm text-emerald-600 dark:text-emerald-400">
                  {t("settings.diarization.status.available")}
                </span>
              </>
            ) : isDownloading || status.download_progress !== null ? (
              <>
                <span className="inline-block w-1.5 h-1.5 bg-logo-primary rounded-full animate-pulse shrink-0" />
                <span className="text-sm text-logo-primary">
                  {t("settings.diarization.status.downloading", {
                    progress: progressPercent,
                  })}
                </span>
              </>
            ) : downloadError || status.error ? (
              <>
                <span className="inline-block w-1.5 h-1.5 bg-red-400 rounded-full shrink-0" />
                <span className="text-sm text-red-500 dark:text-red-400">
                  {downloadError || status.error}
                </span>
              </>
            ) : (
              <>
                <span className="inline-block w-1.5 h-1.5 bg-amber-400 dark:bg-amber-500 rounded-full shrink-0" />
                <span className="text-sm text-amber-600 dark:text-amber-400">
                  {t("settings.diarization.status.notAvailable")}
                </span>
              </>
            )}
          </div>
        </div>

        {/* Download section */}
        {(showDownloadButton ||
          isDownloading ||
          status.download_progress !== null) && (
          <div className="px-4 py-3">
            {/* Download button */}
            {showDownloadButton && (
              <button
                onClick={handleDownload}
                className="inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-white bg-background-ui hover:bg-background-ui/90 rounded-md transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
                disabled={isDownloading}
              >
                {downloadError ? (
                  <>
                    <svg
                      className="w-3.5 h-3.5"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
                      />
                    </svg>
                    {t("common.retry") || "Retry"}
                  </>
                ) : (
                  <>
                    <svg
                      className="w-3.5 h-3.5"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"
                      />
                    </svg>
                    {t("settings.diarization.models.download")}
                  </>
                )}
              </button>
            )}

            {/* Download progress bar */}
            {(isDownloading || status.download_progress !== null) && (
              <div className="mt-3">
                <div className="flex items-center justify-between text-xs text-mid-gray mb-1.5">
                  <span>{t("settings.diarization.models.downloading")}</span>
                  <span>{progressPercent}%</span>
                </div>
                <div className="w-full h-1 bg-mid-gray/20 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-background-ui rounded-full transition-all duration-300 ease-out"
                    style={{
                      width: `${Math.min(progressPercent, 100)}%`,
                    }}
                  />
                </div>
              </div>
            )}

            {/* Model size info */}
            {showDownloadButton && (
              <p className="text-xs text-mid-gray mt-2">
                {t("settings.diarization.models.size")}
              </p>
            )}
          </div>
        )}

        {/* Info note */}
        {status.enabled && status.available && (
          <div className="px-4 py-3">
            <p className="text-xs text-mid-gray leading-relaxed">
              {t("settings.diarization.note")}
            </p>
          </div>
        )}
      </div>
    );

    if (grouped) {
      return content;
    }

    return (
      <SettingsGroup title={t("settings.diarization.title")}>
        {content}
      </SettingsGroup>
    );
  });
