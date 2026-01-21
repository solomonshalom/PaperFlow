import React, { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  FileAudio,
  Upload,
  Trash2,
  Play,
  Square,
  CheckCircle2,
  XCircle,
  Clock,
  Loader2,
  Copy,
  Check,
  ChevronDown,
} from "lucide-react";
import { Button } from "../../ui/Button";
import { commands, type FileTranscriptionJob } from "@/bindings";
import { ExportDropdown } from "./ExportDropdown";
import { WatchFolderSettings } from "./WatchFolderSettings";

interface FileTranscriptionEventPayload {
  job_id: string;
  status: string;
  progress: number;
  transcription: string | null;
  error: string | null;
}

const StatusIcon: React.FC<{ status: string }> = ({ status }) => {
  switch (status) {
    case "queued":
      return <Clock className="w-4 h-4 text-text/50" />;
    case "processing":
      return <Loader2 className="w-4 h-4 text-logo-primary animate-spin" />;
    case "completed":
      return <CheckCircle2 className="w-4 h-4 text-green-500" />;
    case "failed":
      return <XCircle className="w-4 h-4 text-red-500" />;
    case "cancelled":
      return <XCircle className="w-4 h-4 text-yellow-500" />;
    default:
      return <Clock className="w-4 h-4 text-text/50" />;
  }
};

const formatFileSize = (bytes: number): string => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
};

export const FileTranscriptionSettings: React.FC = () => {
  const { t } = useTranslation();
  const [jobs, setJobs] = useState<FileTranscriptionJob[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [supportedExtensions, setSupportedExtensions] = useState<string[]>([]);
  const [copiedJobId, setCopiedJobId] = useState<string | null>(null);

  // Load initial state
  useEffect(() => {
    const loadInitialState = async () => {
      try {
        const [jobsResult, extensionsResult, processingResult] =
          await Promise.all([
            commands.getFileTranscriptionJobs(),
            commands.getSupportedFileExtensions(),
            commands.isFileTranscriptionProcessing(),
          ]);

        setJobs(jobsResult);
        setSupportedExtensions(extensionsResult);
        setIsProcessing(processingResult);
      } catch (error) {
        console.error("Failed to load initial state:", error);
      }
    };

    loadInitialState();
  }, []);

  // Listen for job updates
  useEffect(() => {
    const setupListener = async () => {
      const unlisten = await listen<FileTranscriptionEventPayload>(
        "file-transcription-update",
        async (event) => {
          const payload = event.payload;

          setJobs((prevJobs) => {
            const existingIndex = prevJobs.findIndex(
              (j) => j.id === payload.job_id,
            );

            // If job not found, we'll reload from backend
            if (existingIndex === -1) {
              // Schedule reload outside of setState to avoid issues
              setTimeout(() => {
                commands.getFileTranscriptionJobs().then((result) => {
                  setJobs(result);
                });
              }, 0);
              return prevJobs;
            }

            const updatedJobs = [...prevJobs];
            updatedJobs[existingIndex] = {
              ...updatedJobs[existingIndex],
              status: payload.status as FileTranscriptionJob["status"],
              progress: payload.progress,
              transcription: payload.transcription,
              error: payload.error,
            };
            return updatedJobs;
          });

          // Update processing state
          if (
            payload.status === "completed" ||
            payload.status === "failed" ||
            payload.status === "cancelled"
          ) {
            const result = await commands.isFileTranscriptionProcessing();
            setIsProcessing(result);
          }
        },
      );

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
  }, []);

  // Listen for file drop events from Tauri
  useEffect(() => {
    const setupDropListener = async () => {
      const unlisten = await listen<{ paths: string[] }>(
        "tauri://drag-drop",
        async (event) => {
          const paths = event.payload.paths || [];
          const filteredPaths = paths.filter((path) => {
            const ext = path.split(".").pop()?.toLowerCase() || "";
            return supportedExtensions.includes(ext);
          });

          if (filteredPaths.length > 0) {
            const result =
              await commands.queueFilesForTranscription(filteredPaths);
            if (result.status === "ok") {
              setJobs((prev) => [...prev, ...result.data]);
            }
          }
          setIsDragging(false);
        },
      );

      return unlisten;
    };

    const unlistenPromise = setupDropListener();

    return () => {
      unlistenPromise.then((unlisten) => {
        if (unlisten) {
          unlisten();
        }
      });
    };
  }, [supportedExtensions]);

  const handleFileSelect = useCallback(async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: "Audio/Video Files",
            extensions: supportedExtensions,
          },
        ],
      });

      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected];
        if (paths.length > 0) {
          const result = await commands.queueFilesForTranscription(paths);
          if (result.status === "ok") {
            setJobs((prev) => [...prev, ...result.data]);
          } else {
            console.error("Failed to queue files:", result.error);
          }
        }
      }
    } catch (error) {
      console.error("Failed to open file dialog:", error);
    }
  }, [supportedExtensions]);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
  }, []);

  const processAll = useCallback(async () => {
    setIsProcessing(true);
    try {
      const result = await commands.processAllFiles();
      if (result.status !== "ok") {
        console.error("Failed to start processing:", result.error);
        setIsProcessing(false);
      }
    } catch (error) {
      console.error("Failed to process files:", error);
      setIsProcessing(false);
    }
  }, []);

  const cancelProcessing = useCallback(async () => {
    try {
      await commands.cancelFileTranscription();
    } catch (error) {
      console.error("Failed to cancel processing:", error);
    }
  }, []);

  const removeJob = useCallback(async (jobId: string) => {
    try {
      await commands.removeFileTranscriptionJob(jobId);
      setJobs((prev) => prev.filter((j) => j.id !== jobId));
    } catch (error) {
      console.error("Failed to remove job:", error);
    }
  }, []);

  const clearCompleted = useCallback(async () => {
    try {
      await commands.clearCompletedFileJobs();
      setJobs((prev) =>
        prev.filter(
          (j) =>
            j.status !== "completed" &&
            j.status !== "failed" &&
            j.status !== "cancelled",
        ),
      );
    } catch (error) {
      console.error("Failed to clear completed jobs:", error);
    }
  }, []);

  const copyTranscription = useCallback(async (jobId: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedJobId(jobId);
      setTimeout(() => setCopiedJobId(null), 2000);
    } catch (error) {
      console.error("Failed to copy to clipboard:", error);
    }
  }, []);

  const hasQueuedJobs = jobs.some((j) => j.status === "queued");
  const hasCompletedJobs = jobs.some(
    (j) =>
      j.status === "completed" ||
      j.status === "failed" ||
      j.status === "cancelled",
  );

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      {/* Drop Zone Section */}
      <div className="space-y-2">
        <div className="px-4">
          <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
            {t("settings.files.title")}
          </h2>
          <p className="text-xs text-text/50 mt-1">
            {t("settings.files.description")}
          </p>
        </div>

        <div
          className={`
            border rounded-lg transition-all duration-150 cursor-pointer
            ${
              isDragging
                ? "border-logo-primary bg-logo-primary/10"
                : "border-mid-gray/30 hover:border-mid-gray/50 bg-mid-gray/5"
            }
          `}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          onClick={handleFileSelect}
        >
          <div className="flex flex-col items-center py-10 px-6">
            <div
              className={`
                p-3 rounded-full mb-4 transition-colors duration-150
                ${isDragging ? "bg-logo-primary/20" : "bg-mid-gray/10"}
              `}
            >
              <Upload
                className={`w-6 h-6 ${isDragging ? "text-logo-primary" : "text-text/40"}`}
              />
            </div>
            <p className="text-sm font-medium text-text/80">
              {t("settings.files.dropzone.title")}
            </p>
            <p className="text-xs text-text/50 mt-1">
              {t("settings.files.dropzone.subtitle")}
            </p>
            <p className="text-xs text-text/30 mt-4">
              {supportedExtensions.length > 6
                ? supportedExtensions.slice(0, 6).join(", ") + "..."
                : supportedExtensions.join(", ")}
            </p>
          </div>
        </div>
      </div>

      {/* Queue Section */}
      {jobs.length > 0 && (
        <div className="space-y-2">
          <div className="px-4 flex items-center justify-between">
            <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
              {t("settings.files.queue.title")} ({jobs.length})
            </h2>
            <div className="flex items-center gap-2">
              {hasCompletedJobs && (
                <button
                  onClick={clearCompleted}
                  className="text-xs text-text/50 hover:text-text transition-colors"
                >
                  {t("settings.files.queue.clearCompleted")}
                </button>
              )}
              {isProcessing ? (
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={cancelProcessing}
                  className="flex items-center gap-1.5"
                >
                  <Square className="w-3 h-3" />
                  {t("settings.files.queue.stop")}
                </Button>
              ) : (
                hasQueuedJobs && (
                  <Button
                    variant="primary"
                    size="sm"
                    onClick={processAll}
                    className="flex items-center gap-1.5"
                  >
                    <Play className="w-3 h-3" />
                    {t("settings.files.queue.processAll")}
                  </Button>
                )
              )}
            </div>
          </div>

          {/* Job List */}
          <div className="border border-mid-gray/20 rounded-lg overflow-hidden">
            <div className="divide-y divide-mid-gray/20">
              {jobs.map((job) => (
                <JobItem
                  key={job.id}
                  job={job}
                  onRemove={() => removeJob(job.id)}
                  onCopy={copyTranscription}
                  copiedJobId={copiedJobId}
                />
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Watch Folder Settings */}
      <WatchFolderSettings />
    </div>
  );
};

interface JobItemProps {
  job: FileTranscriptionJob;
  onRemove: () => void;
  onCopy: (jobId: string, text: string) => void;
  copiedJobId: string | null;
}

const JobItem: React.FC<JobItemProps> = ({
  job,
  onRemove,
  onCopy,
  copiedJobId,
}) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  const canExpand = job.status === "completed" && job.transcription;
  const isCopied = copiedJobId === job.id;

  return (
    <div className="px-4 py-3">
      <div className="flex items-center gap-3">
        <StatusIcon status={job.status} />

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <FileAudio className="w-4 h-4 text-text/40 shrink-0" />
            <p
              className="text-sm font-medium text-text/90 truncate"
              title={job.file_name}
            >
              {job.file_name}
            </p>
          </div>
          <div className="flex items-center gap-2 mt-0.5">
            <span className="text-xs text-text/40">
              {formatFileSize(Number(job.file_size))}
            </span>
            {job.status === "processing" && (
              <span className="text-xs text-logo-primary">
                {Math.round(job.progress * 100)}%
              </span>
            )}
            {job.status === "failed" && job.error && (
              <span className="text-xs text-red-400 truncate" title={job.error}>
                {job.error}
              </span>
            )}
          </div>
        </div>

        <div className="flex items-center gap-0.5">
          {canExpand && (
            <>
              <button
                onClick={() => onCopy(job.id, job.transcription!)}
                className="p-1.5 text-text/40 hover:text-logo-primary transition-colors rounded hover:bg-logo-primary/10"
                title={t("settings.files.job.copy")}
              >
                {isCopied ? (
                  <Check className="w-4 h-4" />
                ) : (
                  <Copy className="w-4 h-4" />
                )}
              </button>
              <ExportDropdown
                text={job.transcription!}
                title={job.file_name}
                sourceFile={job.file_path}
              />
              <button
                onClick={() => setExpanded(!expanded)}
                className="p-1.5 text-text/40 hover:text-logo-primary transition-colors rounded hover:bg-logo-primary/10"
                title={
                  expanded
                    ? t("settings.files.job.collapse")
                    : t("settings.files.job.expand")
                }
              >
                <ChevronDown
                  className={`w-4 h-4 transition-transform duration-150 ${expanded ? "rotate-180" : ""}`}
                />
              </button>
            </>
          )}
          {job.status !== "processing" && (
            <button
              onClick={onRemove}
              className="p-1.5 text-text/40 hover:text-red-400 transition-colors rounded hover:bg-red-500/10"
              title={t("settings.files.job.remove")}
            >
              <Trash2 className="w-4 h-4" />
            </button>
          )}
        </div>
      </div>

      {/* Progress bar for processing jobs */}
      {job.status === "processing" && (
        <div className="mt-3 h-1 bg-mid-gray/20 rounded-full overflow-hidden">
          <div
            className="h-full bg-logo-primary transition-all duration-300"
            style={{ width: `${job.progress * 100}%` }}
          />
        </div>
      )}

      {/* Expanded transcription */}
      {expanded && job.transcription && (
        <div className="mt-3 p-3 bg-mid-gray/10 rounded-lg border border-mid-gray/20">
          <p className="text-sm text-text/70 whitespace-pre-wrap select-text cursor-text leading-relaxed">
            {job.transcription}
          </p>
        </div>
      )}
    </div>
  );
};
