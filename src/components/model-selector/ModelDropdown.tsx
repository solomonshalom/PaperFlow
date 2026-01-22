import React, { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import type { ModelInfo } from "@/bindings";
import { commands } from "@/bindings";
import { formatModelSize } from "../../lib/utils/format";
import {
  getTranslatedModelName,
  getTranslatedModelDescription,
} from "../../lib/utils/modelTranslation";
import { ProgressBar } from "../shared";
import { useSettings } from "../../hooks/useSettings";

interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

interface CoreMLDownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

interface ModelDropdownProps {
  models: ModelInfo[];
  currentModelId: string;
  downloadProgress: Map<string, DownloadProgress>;
  onModelSelect: (modelId: string) => void;
  onModelDownload: (modelId: string) => void;
  onModelDelete: (modelId: string) => Promise<void>;
  onError?: (error: string) => void;
}

const ModelDropdown: React.FC<ModelDropdownProps> = ({
  models,
  currentModelId,
  downloadProgress,
  onModelSelect,
  onModelDownload,
  onModelDelete,
  onError,
}) => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const [isCoreMLAvailable, setIsCoreMLAvailable] = useState(false);
  const [coremlDownloadProgress, setCoremlDownloadProgress] = useState<
    Map<string, CoreMLDownloadProgress>
  >(new Map());
  const [showApiKeyWarning, setShowApiKeyWarning] = useState<string | null>(
    null,
  );

  const availableModels = models.filter((m) => m.is_downloaded);
  const downloadableModels = models.filter((m) => !m.is_downloaded);

  // Check if a model is a cloud model requiring an API key
  const isCloudModel = useCallback((model: ModelInfo) => {
    return model.engine_type === "GroqCloud";
  }, []);

  // Check if the Groq API key is configured
  const isGroqKeyConfigured = useCallback(() => {
    const apiKey = getSetting("groq_transcription_api_key") || "";
    return apiKey.trim().length > 0;
  }, [getSetting]);
  const isFirstRun = availableModels.length === 0 && models.length > 0;

  // Check if CoreML is available (macOS only)
  useEffect(() => {
    commands.isCoremlAvailable().then(setIsCoreMLAvailable);
  }, []);

  // Listen for CoreML download progress events
  useEffect(() => {
    const progressUnlisten = listen<CoreMLDownloadProgress>(
      "coreml-download-progress",
      (event) => {
        const progress = event.payload;
        setCoremlDownloadProgress((prev) => {
          const newMap = new Map(prev);
          newMap.set(progress.model_id, progress);
          return newMap;
        });
      },
    );

    const completeUnlisten = listen<string>(
      "coreml-download-complete",
      (event) => {
        const modelId = event.payload;
        setCoremlDownloadProgress((prev) => {
          const newMap = new Map(prev);
          newMap.delete(modelId);
          return newMap;
        });
      },
    );

    return () => {
      progressUnlisten.then((fn) => fn());
      completeUnlisten.then((fn) => fn());
    };
  }, []);

  const handleDeleteClick = async (e: React.MouseEvent, modelId: string) => {
    e.preventDefault();
    e.stopPropagation();

    try {
      await onModelDelete(modelId);
    } catch (err) {
      const errorMsg = `Failed to delete model: ${err}`;
      onError?.(errorMsg);
    }
  };

  const handleModelClick = (modelId: string) => {
    if (downloadProgress.has(modelId)) {
      return; // Don't allow interaction while downloading
    }

    // Check if this is a cloud model and API key is not configured
    const model = models.find((m) => m.id === modelId);
    if (model && isCloudModel(model) && !isGroqKeyConfigured()) {
      setShowApiKeyWarning(modelId);
      return;
    }

    setShowApiKeyWarning(null);
    onModelSelect(modelId);
  };

  const handleDownloadClick = (modelId: string) => {
    if (downloadProgress.has(modelId)) {
      return; // Don't allow interaction while downloading
    }
    onModelDownload(modelId);
  };

  const handleCoreMLDownloadClick = async (
    e: React.MouseEvent,
    modelId: string,
  ) => {
    e.preventDefault();
    e.stopPropagation();

    if (coremlDownloadProgress.has(modelId)) {
      return; // Already downloading
    }

    try {
      const result = await commands.downloadCoremlModel(modelId);
      if (result.status === "error") {
        onError?.(result.error);
      }
    } catch (err) {
      onError?.(`Failed to download CoreML model: ${err}`);
    }
  };

  // Check if a model supports CoreML (Whisper models only)
  const supportsCoreML = (model: ModelInfo) => {
    return (
      isCoreMLAvailable && model.engine_type === "Whisper" && model.coreml_url
    );
  };

  return (
    <div className="absolute bottom-full left-0 mb-2 w-64 bg-background border border-mid-gray/20 rounded-lg shadow-lg py-2 z-50 max-h-[70vh] overflow-y-auto scrollbar-hide">
      {/* API Key Warning Modal */}
      {showApiKeyWarning && (
        <div className="px-3 py-3 bg-amber-500/10 border-b border-amber-500/20">
          <div className="flex items-start gap-2">
            <svg
              className="w-4 h-4 text-amber-500 flex-shrink-0 mt-0.5"
              fill="currentColor"
              viewBox="0 0 20 20"
            >
              <path
                fillRule="evenodd"
                d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z"
                clipRule="evenodd"
              />
            </svg>
            <div className="flex-1">
              <div className="text-xs font-medium text-amber-500 mb-1">
                {t("modelSelector.apiKeyRequired", "API Key Required")}
              </div>
              <div className="text-xs text-text/70 mb-2">
                {t(
                  "modelSelector.apiKeyRequiredDesc",
                  "This cloud model requires a Groq API key. Go to Settings → Advanced to add your key.",
                )}
              </div>
              <div className="flex gap-2">
                <button
                  onClick={() => setShowApiKeyWarning(null)}
                  className="text-xs px-2 py-1 bg-mid-gray/20 hover:bg-mid-gray/30 rounded transition-colors"
                >
                  {t("common.cancel", "Cancel")}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* First Run Welcome */}
      {isFirstRun && (
        <div className="px-3 py-2 bg-logo-primary/10 border-b border-logo-primary/20">
          <div className="text-xs font-medium text-logo-primary mb-1">
            {t("modelSelector.welcome")}
          </div>
          <div className="text-xs text-text/70">
            {t("modelSelector.downloadPrompt")}
          </div>
        </div>
      )}

      {/* Available Models */}
      {availableModels.length > 0 && (
        <div>
          <div className="px-3 py-1 text-xs font-medium text-text/80 border-b border-mid-gray/10">
            {t("modelSelector.availableModels")}
          </div>
          {availableModels.map((model) => {
            const hasCoreMLSupport = supportsCoreML(model);
            const isCoreMLDownloaded = model.is_coreml_downloaded;
            const isCoreMLDownloading = coremlDownloadProgress.has(model.id);
            const coremlProgress = coremlDownloadProgress.get(model.id);

            return (
              <div
                key={model.id}
                onClick={() => handleModelClick(model.id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    handleModelClick(model.id);
                  }
                }}
                tabIndex={0}
                role="button"
                className={`w-full px-3 py-2 text-left hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
                  currentModelId === model.id
                    ? "bg-logo-primary/10 text-logo-primary"
                    : ""
                }`}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="flex-1 min-w-0">
                    <div className="text-sm">
                      <span className="truncate">
                        {getTranslatedModelName(model, t)}
                      </span>
                    </div>
                    <div className="text-xs text-text/40 italic pr-4 mt-0.5">
                      {getTranslatedModelDescription(model, t)}
                    </div>
                    {/* CoreML download option for Whisper models */}
                    {hasCoreMLSupport &&
                      !isCoreMLDownloaded &&
                      !isCoreMLDownloading && (
                        <button
                          onClick={(e) =>
                            handleCoreMLDownloadClick(e, model.id)
                          }
                          className="mt-1 text-[10px] text-blue-400 hover:text-blue-300 hover:underline"
                        >
                          {t("modelSelector.coreml.download")} (
                          {formatModelSize(Number(model.coreml_size_mb || 0))})
                        </button>
                      )}
                    {/* CoreML downloading progress */}
                    {isCoreMLDownloading && coremlProgress && (
                      <div className="mt-1 text-[10px] text-blue-400">
                        {t("modelSelector.coreml.downloading", {
                          percentage: Math.round(coremlProgress.percentage),
                        })}
                      </div>
                    )}
                  </div>
                  <div className="flex items-center gap-2 flex-shrink-0">
                    {/* Cloud models don't have a delete button - they're always available */}
                    {currentModelId !== model.id && !isCloudModel(model) && (
                      <button
                        onClick={(e) => handleDeleteClick(e, model.id)}
                        className="text-red-400 hover:text-red-300 p-1 hover:bg-red-500/10 rounded transition-colors"
                        title={t("modelSelector.deleteModel", {
                          modelName: getTranslatedModelName(model, t),
                        })}
                      >
                        <svg
                          className="w-3 h-3"
                          fill="currentColor"
                          viewBox="0 0 20 20"
                        >
                          <path
                            fillRule="evenodd"
                            d="M9 2a1 1 0 00-.894.553L7.382 4H4a1 1 0 000 2v10a2 2 0 002 2h8a2 2 0 002-2V6a1 1 0 100-2h-3.382l-.724-1.447A1 1 0 0011 2H9zM7 8a1 1 0 012 0v6a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v6a1 1 0 102 0V8a1 1 0 00-1-1z"
                            clipRule="evenodd"
                          />
                        </svg>
                      </button>
                    )}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Downloadable Models */}
      {downloadableModels.length > 0 && (
        <div>
          {(availableModels.length > 0 || isFirstRun) && (
            <div className="border-t border-mid-gray/10 my-1" />
          )}
          <div className="px-3 py-1 text-xs font-medium text-text/80">
            {isFirstRun
              ? t("modelSelector.chooseModel")
              : t("modelSelector.downloadModels")}
          </div>
          {downloadableModels.map((model) => {
            const isDownloading = downloadProgress.has(model.id);
            const progress = downloadProgress.get(model.id);

            return (
              <div
                key={model.id}
                onClick={() => handleDownloadClick(model.id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    handleDownloadClick(model.id);
                  }
                }}
                tabIndex={0}
                role="button"
                aria-disabled={isDownloading}
                className={`group w-full px-3 py-2 text-left hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
                  isDownloading
                    ? "opacity-50 cursor-not-allowed hover:bg-transparent"
                    : ""
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="flex-1 min-w-0">
                    <div className="text-sm">
                      <span className="truncate">
                        {getTranslatedModelName(model, t)}
                      </span>
                    </div>
                    <div className="text-xs text-text/40 italic pr-4 mt-0.5">
                      {getTranslatedModelDescription(model, t)}
                    </div>
                    <div className="mt-1 text-xs text-text/50 tabular-nums">
                      {t("modelSelector.downloadSize")} ·{" "}
                      {formatModelSize(Number(model.size_mb))}
                    </div>
                  </div>
                  <div
                    className={`text-xs text-logo-primary tabular-nums transition-opacity ${
                      isDownloading && progress
                        ? "opacity-100"
                        : "opacity-0 group-hover:opacity-100"
                    }`}
                  >
                    {isDownloading && progress
                      ? `${Math.max(0, Math.min(100, Math.round(progress.percentage)))}%`
                      : t("modelSelector.download")}
                  </div>
                </div>

                {isDownloading && progress && (
                  <div className="mt-2">
                    <ProgressBar
                      progress={[
                        {
                          id: model.id,
                          percentage: progress.percentage,
                          label: model.name,
                        },
                      ]}
                      size="small"
                    />
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* No Models Available */}
      {availableModels.length === 0 && downloadableModels.length === 0 && (
        <div className="px-3 py-2 text-sm text-text/60">
          {t("modelSelector.noModelsAvailable")}
        </div>
      )}
    </div>
  );
};

export default ModelDropdown;
