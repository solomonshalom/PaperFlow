import React, { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import {
  FolderOpen,
  Plus,
  Trash2,
  Eye,
  EyeOff,
  Settings2,
  Folder,
} from "lucide-react";
import { Button } from "../../ui/Button";
import {
  commands,
  type WatchFolderConfig,
  type WatchFolderStatus,
} from "@/bindings";

interface WatchFolderFileDetectedPayload {
  folder_id: string;
  file_path: string;
  file_name: string;
}

export const WatchFolderSettings: React.FC = () => {
  const { t } = useTranslation();
  const [folders, setFolders] = useState<WatchFolderConfig[]>([]);
  const [statuses, setStatuses] = useState<WatchFolderStatus[]>([]);
  const [expandedFolderId, setExpandedFolderId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  // Load initial state
  useEffect(() => {
    const loadInitialState = async () => {
      try {
        const [foldersResult, statusResult] = await Promise.all([
          commands.getWatchFolders(),
          commands.getWatchFolderStatus(),
        ]);
        setFolders(foldersResult);
        setStatuses(statusResult);
      } catch (error) {
        console.error("Failed to load watch folders:", error);
      } finally {
        setIsLoading(false);
      }
    };

    loadInitialState();
  }, []);

  // Listen for file detection events
  useEffect(() => {
    const setupListener = async () => {
      const unlisten = await listen<WatchFolderFileDetectedPayload>(
        "watch-folder-file-detected",
        async () => {
          // Refresh status when a file is detected
          const statusResult = await commands.getWatchFolderStatus();
          setStatuses(statusResult);
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

  const handleAddFolder = useCallback(async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      });

      if (selected && typeof selected === "string") {
        const result = await commands.addWatchFolder(selected, false);
        if (result.status === "ok") {
          setFolders((prev) => [...prev, result.data]);
          // Refresh statuses
          const statusResult = await commands.getWatchFolderStatus();
          setStatuses(statusResult);
        } else {
          console.error("Failed to add watch folder:", result.error);
          toast.error(t("settings.files.watchFolder.errors.addFailed"));
        }
      }
    } catch (error) {
      console.error("Failed to open folder dialog:", error);
      toast.error(t("settings.files.watchFolder.errors.dialogFailed"));
    }
  }, []);

  const handleRemoveFolder = useCallback(
    async (folderId: string) => {
      try {
        const result = await commands.removeWatchFolder(folderId);
        if (result.status === "ok") {
          setFolders((prev) => prev.filter((f) => f.id !== folderId));
          setStatuses((prev) => prev.filter((s) => s.folder_id !== folderId));
        } else {
          console.error("Failed to remove watch folder:", result.error);
          toast.error(t("settings.files.watchFolder.errors.removeFailed"));
        }
      } catch (error) {
        console.error("Failed to remove watch folder:", error);
        toast.error(t("settings.files.watchFolder.errors.removeFailed"));
      }
    },
    [t],
  );

  const handleToggleEnabled = useCallback(
    async (folder: WatchFolderConfig) => {
      const updatedConfig: WatchFolderConfig = {
        ...folder,
        enabled: !folder.enabled,
      };

      try {
        const result = await commands.updateWatchFolder(updatedConfig);
        if (result.status === "ok") {
          setFolders((prev) =>
            prev.map((f) => (f.id === folder.id ? updatedConfig : f)),
          );
          // Refresh statuses
          const statusResult = await commands.getWatchFolderStatus();
          setStatuses(statusResult);
        } else {
          console.error("Failed to toggle watch folder:", result.error);
          toast.error(t("settings.files.watchFolder.errors.toggleFailed"));
        }
      } catch (error) {
        console.error("Failed to toggle watch folder:", error);
        toast.error(t("settings.files.watchFolder.errors.toggleFailed"));
      }
    },
    [t],
  );

  const handleToggleRecursive = useCallback(
    async (folder: WatchFolderConfig) => {
      const updatedConfig: WatchFolderConfig = {
        ...folder,
        recursive: !folder.recursive,
      };

      try {
        const result = await commands.updateWatchFolder(updatedConfig);
        if (result.status === "ok") {
          setFolders((prev) =>
            prev.map((f) => (f.id === folder.id ? updatedConfig : f)),
          );
          // Refresh statuses since watcher was restarted
          const statusResult = await commands.getWatchFolderStatus();
          setStatuses(statusResult);
        } else {
          console.error("Failed to update watch folder:", result.error);
          toast.error(t("settings.files.watchFolder.errors.updateFailed"));
        }
      } catch (error) {
        console.error("Failed to update watch folder:", error);
        toast.error(t("settings.files.watchFolder.errors.updateFailed"));
      }
    },
    [t],
  );

  const handleToggleAutoProcess = useCallback(
    async (folder: WatchFolderConfig) => {
      const updatedConfig: WatchFolderConfig = {
        ...folder,
        auto_process: !folder.auto_process,
      };

      try {
        const result = await commands.updateWatchFolder(updatedConfig);
        if (result.status === "ok") {
          setFolders((prev) =>
            prev.map((f) => (f.id === folder.id ? updatedConfig : f)),
          );
        } else {
          console.error("Failed to update watch folder:", result.error);
          toast.error(t("settings.files.watchFolder.errors.updateFailed"));
        }
      } catch (error) {
        console.error("Failed to update watch folder:", error);
        toast.error(t("settings.files.watchFolder.errors.updateFailed"));
      }
    },
    [t],
  );

  const getStatus = (folderId: string): WatchFolderStatus | undefined => {
    return statuses.find((s) => s.folder_id === folderId);
  };

  const getFolderName = (path: string): string => {
    const parts = path.split(/[/\\]/);
    return parts[parts.length - 1] || path;
  };

  if (isLoading) {
    return (
      <div className="space-y-2 mt-6">
        <div className="px-4">
          <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
            {t("settings.files.watchFolder.title")}
          </h2>
        </div>
        <div className="px-4 py-4 text-sm text-text/50">
          {t("common.loading")}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-2 mt-6">
      <div className="px-4 flex items-center justify-between">
        <div>
          <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
            {t("settings.files.watchFolder.title")}
          </h2>
          <p className="text-xs text-text/50 mt-1">
            {t("settings.files.watchFolder.description")}
          </p>
        </div>
        <Button
          variant="primary"
          size="sm"
          onClick={handleAddFolder}
          className="flex items-center gap-1.5"
        >
          <Plus className="w-3.5 h-3.5" />
          {t("settings.files.watchFolder.add")}
        </Button>
      </div>

      {folders.length === 0 ? (
        <div className="border border-mid-gray/20 rounded-lg p-8 text-center">
          <FolderOpen className="w-10 h-10 text-text/30 mx-auto mb-3" />
          <p className="text-sm text-text/50">
            {t("settings.files.watchFolder.empty")}
          </p>
          <p className="text-xs text-text/30 mt-1">
            {t("settings.files.watchFolder.emptyHint")}
          </p>
        </div>
      ) : (
        <div className="border border-mid-gray/20 rounded-lg overflow-hidden">
          <div className="divide-y divide-mid-gray/20">
            {folders.map((folder) => {
              const status = getStatus(folder.id);
              const isExpanded = expandedFolderId === folder.id;

              return (
                <div key={folder.id} className="px-4 py-3">
                  <div className="flex items-center gap-3">
                    {/* Status indicator */}
                    <button
                      onClick={() => handleToggleEnabled(folder)}
                      className={`p-1.5 rounded transition-colors ${
                        folder.enabled
                          ? "text-green-500 hover:bg-green-500/10"
                          : "text-text/40 hover:bg-mid-gray/10"
                      }`}
                      title={
                        folder.enabled
                          ? t("settings.files.watchFolder.disable")
                          : t("settings.files.watchFolder.enable")
                      }
                    >
                      {folder.enabled ? (
                        <Eye className="w-4 h-4" />
                      ) : (
                        <EyeOff className="w-4 h-4" />
                      )}
                    </button>

                    {/* Folder info */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <Folder className="w-4 h-4 text-text/40 shrink-0" />
                        <p
                          className="text-sm font-medium text-text/90 truncate"
                          title={folder.path}
                        >
                          {getFolderName(folder.path)}
                        </p>
                      </div>
                      <div className="flex items-center gap-2 mt-0.5">
                        <span
                          className="text-xs text-text/40 truncate"
                          title={folder.path}
                        >
                          {folder.path}
                        </span>
                        {status?.is_watching && (
                          <span className="text-xs text-green-500">
                            {t("settings.files.watchFolder.watching")}
                          </span>
                        )}
                        {status?.last_error && (
                          <span
                            className="text-xs text-red-400"
                            title={status.last_error}
                          >
                            {t("settings.files.watchFolder.error")}
                          </span>
                        )}
                      </div>
                    </div>

                    {/* Actions */}
                    <div className="flex items-center gap-0.5">
                      <button
                        onClick={() =>
                          setExpandedFolderId(isExpanded ? null : folder.id)
                        }
                        className="p-1.5 text-text/40 hover:text-logo-primary transition-colors rounded hover:bg-logo-primary/10"
                        title={t("settings.files.watchFolder.settings")}
                      >
                        <Settings2 className="w-4 h-4" />
                      </button>
                      <button
                        onClick={() => handleRemoveFolder(folder.id)}
                        className="p-1.5 text-text/40 hover:text-red-400 transition-colors rounded hover:bg-red-500/10"
                        title={t("settings.files.watchFolder.remove")}
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  </div>

                  {/* Expanded settings */}
                  {isExpanded && (
                    <div className="mt-3 pt-3 border-t border-mid-gray/20">
                      <div className="space-y-3">
                        {/* Recursive toggle */}
                        <label className="flex items-center justify-between cursor-pointer">
                          <div>
                            <span className="text-sm text-text/80">
                              {t("settings.files.watchFolder.recursive")}
                            </span>
                            <p className="text-xs text-text/50">
                              {t(
                                "settings.files.watchFolder.recursiveDescription",
                              )}
                            </p>
                          </div>
                          <button
                            onClick={() => handleToggleRecursive(folder)}
                            className={`w-10 h-5 rounded-full transition-colors ${
                              folder.recursive
                                ? "bg-logo-primary"
                                : "bg-mid-gray/30"
                            }`}
                          >
                            <div
                              className={`w-4 h-4 rounded-full bg-white shadow transition-transform ${
                                folder.recursive
                                  ? "translate-x-5"
                                  : "translate-x-0.5"
                              }`}
                            />
                          </button>
                        </label>

                        {/* Auto-process toggle */}
                        <label className="flex items-center justify-between cursor-pointer">
                          <div>
                            <span className="text-sm text-text/80">
                              {t("settings.files.watchFolder.autoProcess")}
                            </span>
                            <p className="text-xs text-text/50">
                              {t(
                                "settings.files.watchFolder.autoProcessDescription",
                              )}
                            </p>
                          </div>
                          <button
                            onClick={() => handleToggleAutoProcess(folder)}
                            className={`w-10 h-5 rounded-full transition-colors ${
                              folder.auto_process
                                ? "bg-logo-primary"
                                : "bg-mid-gray/30"
                            }`}
                          >
                            <div
                              className={`w-4 h-4 rounded-full bg-white shadow transition-transform ${
                                folder.auto_process
                                  ? "translate-x-5"
                                  : "translate-x-0.5"
                              }`}
                            />
                          </button>
                        </label>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
};
