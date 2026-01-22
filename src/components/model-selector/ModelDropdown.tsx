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
  const [showApiKeyWarning, setShowApiKeyWarning] = useState<string | null>(null);

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
      }
    );

    const completeUnlisten = listen<string>("coreml-download-complete", (event) => {
      const modelId = event.payload;
      setCoremlDownloadProgress((prev) => {
        const newMap = new Map(prev);
        newMap.delete(modelId);
        return newMap;
      });
    });

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

  const handleCoreMLDownloadClick = async (e: React.MouseEvent, modelId: string) => {
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
    return isCoreMLAvailable && model.engine_type === "Whisper" && model.coreml_url;
  };

  return (
    <div className="absolute bottom-full left-0 mb-2 w-64 bg-background border border-mid-gray/20 rounded-lg shadow-lg py-2 z-50 max-h-[70vh] overflow-y-auto scrollbar-hide">
      {/* API Key Warning Modal */}
      {showApiKeyWarning && (
        <div className="px-3 py-3 bg-amber-500/10 border-b border-amber-500/20">
          <div className="flex items-start gap-2">
            <svg className="w-4 h-4 text-amber-500 flex-shrink-0 mt-0.5" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
            </svg>
            <div className="flex-1">
              <div className="text-xs font-medium text-amber-500 mb-1">
                {t("modelSelector.apiKeyRequired", "API Key Required")}
              </div>
              <div className="text-xs text-text/70 mb-2">
                {t("modelSelector.apiKeyRequiredDesc", "This cloud model requires a Groq API key. Go to Settings → Advanced to add your key.")}
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
                    <div className="text-sm flex items-center flex-wrap gap-1.5">
                      <span className="truncate max-w-[120px]">{getTranslatedModelName(model, t)}</span>
                      {/* Badge container - all badges in a row */}
                      {/* Cloud model badge - soft violet with subtle glow */}
                      {isCloudModel(model) && (
                        <span
                          className="inline-flex items-center gap-1 pl-1 pr-1.5 py-0.5 rounded-md text-[10px] font-semibold uppercase tracking-wider"
                          style={{
                            background: 'linear-gradient(135deg, rgba(139, 92, 246, 0.15) 0%, rgba(168, 85, 247, 0.12) 100%)',
                            color: '#a78bfa',
                            border: '1px solid rgba(139, 92, 246, 0.25)',
                            boxShadow: '0 0 8px rgba(139, 92, 246, 0.1), inset 0 1px 0 rgba(255, 255, 255, 0.05)',
                          }}
                        >
                          <svg className="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
                            <path d="M4.406 3.342A5.53 5.53 0 0 1 8 2c2.69 0 4.923 2 5.166 4.579C14.758 6.804 16 8.137 16 9.773 16 11.569 14.502 13 12.687 13H3.781C1.708 13 0 11.366 0 9.318c0-1.763 1.266-3.223 2.942-3.593.143-.863.698-1.723 1.464-2.383z"/>
                          </svg>
                          {t("modelSelector.cloud", "Cloud")}
                        </span>
                      )}
                      {/* ANE/CoreML accelerated badge - electric cyan with shimmer */}
                      {hasCoreMLSupport && isCoreMLDownloaded && (
                        <span
                          className="inline-flex items-center gap-1 pl-1 pr-1.5 py-0.5 rounded-md text-[10px] font-semibold uppercase tracking-wider"
                          style={{
                            background: 'linear-gradient(135deg, rgba(34, 211, 238, 0.18) 0%, rgba(59, 130, 246, 0.12) 100%)',
                            color: '#22d3ee',
                            border: '1px solid rgba(34, 211, 238, 0.3)',
                            boxShadow: '0 0 10px rgba(34, 211, 238, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.08)',
                          }}
                        >
                          <svg className="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
                            <path d="M5.52.359A.5.5 0 0 1 6 0h4a.5.5 0 0 1 .474.658L8.694 6H12.5a.5.5 0 0 1 .395.807l-7 9a.5.5 0 0 1-.873-.454L6.823 9H3.5a.5.5 0 0 1-.48-.641l2.5-8z"/>
                          </svg>
                          {t("modelSelector.coreml.accelerated")}
                        </span>
                      )}
                      {/* Active badge - inline with other badges */}
                      {currentModelId === model.id && (
                        <>
                          {isCloudModel(model) && !isGroqKeyConfigured() ? (
                            <span
                              className="inline-flex items-center gap-1 pl-1 pr-1.5 py-0.5 rounded-md text-[10px] font-semibold uppercase tracking-wider"
                              style={{
                                background: 'linear-gradient(135deg, rgba(251, 191, 36, 0.18) 0%, rgba(245, 158, 11, 0.12) 100%)',
                                color: '#fbbf24',
                                border: '1px solid rgba(251, 191, 36, 0.35)',
                                boxShadow: '0 0 8px rgba(251, 191, 36, 0.12)',
                              }}
                              title={t("modelSelector.apiKeyMissing", "API key not configured")}
                            >
                              <svg className="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
                                <path d="M8.982 1.566a1.13 1.13 0 0 0-1.96 0L.165 13.233c-.457.778.091 1.767.98 1.767h13.713c.889 0 1.438-.99.98-1.767L8.982 1.566zM8 5c.535 0 .954.462.9.995l-.35 3.507a.552.552 0 0 1-1.1 0L7.1 5.995A.905.905 0 0 1 8 5zm.002 6a1 1 0 1 1 0 2 1 1 0 0 1 0-2z"/>
                              </svg>
                              {t("modelSelector.active")}
                            </span>
                          ) : (
                            <span
                              className="inline-flex items-center gap-1 pl-1 pr-1.5 py-0.5 rounded-md text-[10px] font-semibold uppercase tracking-wider"
                              style={{
                                background: 'linear-gradient(135deg, rgba(74, 222, 128, 0.18) 0%, rgba(34, 197, 94, 0.12) 100%)',
                                color: '#4ade80',
                                border: '1px solid rgba(74, 222, 128, 0.35)',
                                boxShadow: '0 0 8px rgba(74, 222, 128, 0.12)',
                              }}
                            >
                              <svg className="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
                                <path d="M12.736 3.97a.733.733 0 0 1 1.047 0c.286.289.29.756.01 1.05L7.88 12.01a.733.733 0 0 1-1.065.02L3.217 8.384a.757.757 0 0 1 0-1.06.733.733 0 0 1 1.047 0l3.052 3.093 5.4-6.425a.247.247 0 0 1 .02-.022z"/>
                              </svg>
                              {t("modelSelector.active")}
                            </span>
                          )}
                        </>
                      )}
                    </div>
                    <div className="text-xs text-text/40 italic pr-4 mt-0.5">
                      {getTranslatedModelDescription(model, t)}
                    </div>
                    {/* CoreML download option for Whisper models */}
                    {hasCoreMLSupport && !isCoreMLDownloaded && !isCoreMLDownloading && (
                      <button
                        onClick={(e) => handleCoreMLDownloadClick(e, model.id)}
                        className="mt-1 text-[10px] text-blue-400 hover:text-blue-300 hover:underline"
                      >
                        {t("modelSelector.coreml.download")} ({formatModelSize(Number(model.coreml_size_mb || 0))})
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
            const hasCoreMLSupport = supportsCoreML(model);

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
                className={`w-full px-3 py-2 text-left hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
                  isDownloading
                    ? "opacity-50 cursor-not-allowed hover:bg-transparent"
                    : ""
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="flex-1 min-w-0">
                    <div className="text-sm flex items-center gap-2">
                      <span className="truncate">{getTranslatedModelName(model, t)}</span>
                      {/* Badge container - keeps badges aligned */}
                      <div className="flex items-center gap-1.5 flex-shrink-0">
                        {model.id === "parakeet-tdt-0.6b-v3" && isFirstRun && (
                          <span
                            className="inline-flex items-center px-1.5 py-0.5 rounded-md text-[10px] font-semibold uppercase tracking-wider"
                            style={{
                              background: 'linear-gradient(135deg, rgba(250, 162, 202, 0.2) 0%, rgba(242, 140, 187, 0.15) 100%)',
                              color: 'var(--color-logo-primary)',
                              border: '1px solid rgba(250, 162, 202, 0.35)',
                              boxShadow: '0 0 8px rgba(250, 162, 202, 0.15), inset 0 1px 0 rgba(255, 255, 255, 0.05)',
                            }}
                          >
                            {t("onboarding.recommended")}
                          </span>
                        )}
                        {/* Show ANE badge for Whisper models on macOS (dimmed when not downloaded) */}
                        {hasCoreMLSupport && (
                          <span
                            className="inline-flex items-center gap-1 pl-1 pr-1.5 py-0.5 rounded-md text-[10px] font-semibold uppercase tracking-wider opacity-60"
                            style={{
                              background: 'linear-gradient(135deg, rgba(34, 211, 238, 0.12) 0%, rgba(59, 130, 246, 0.08) 100%)',
                              color: '#67e8f9',
                              border: '1px solid rgba(34, 211, 238, 0.2)',
                            }}
                            title={t("modelSelector.coreml.available")}
                          >
                            <svg className="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
                              <path d="M5.52.359A.5.5 0 0 1 6 0h4a.5.5 0 0 1 .474.658L8.694 6H12.5a.5.5 0 0 1 .395.807l-7 9a.5.5 0 0 1-.873-.454L6.823 9H3.5a.5.5 0 0 1-.48-.641l2.5-8z"/>
                            </svg>
                            {t("modelSelector.ane", "ANE")}
                          </span>
                        )}
                      </div>
                    </div>
                    <div className="text-xs text-text/40 italic pr-4 mt-0.5">
                      {getTranslatedModelDescription(model, t)}
                    </div>
                    <div className="mt-1 text-xs text-text/50 tabular-nums">
                      {t("modelSelector.downloadSize")} ·{" "}
                      {formatModelSize(Number(model.size_mb))}
                    </div>
                  </div>
                  <div className="text-xs text-logo-primary tabular-nums">
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
