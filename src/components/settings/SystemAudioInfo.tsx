import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { SettingsGroup } from "../ui/SettingsGroup";

interface SystemAudioInfoData {
  available: boolean;
  native_available: boolean;
  requires_setup: boolean;
  setup_instructions: string;
  devices: string[];
  native_info: string;
}

interface SystemAudioInfoProps {
  grouped?: boolean;
}

export const SystemAudioInfo: React.FC<SystemAudioInfoProps> = React.memo(
  ({ grouped = false }) => {
    const { t } = useTranslation();
    const [info, setInfo] = useState<SystemAudioInfoData | null>(null);
    const [isLoading, setIsLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [showInstructions, setShowInstructions] = useState(false);

    useEffect(() => {
      const loadInfo = async () => {
        setIsLoading(true);
        setError(null);
        try {
          const result = await commands.getSystemAudioInfo();
          setInfo(result);
        } catch (err) {
          console.error("Failed to load system audio info:", err);
          setError(
            err instanceof Error
              ? err.message
              : "Failed to load system audio info",
          );
        } finally {
          setIsLoading(false);
        }
      };
      loadInfo();
    }, []);

    // Show loading state
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
        <SettingsGroup title={t("settings.sound.systemAudio.title")}>
          {loadingContent}
        </SettingsGroup>
      );
    }

    // Show error state
    if (error || !info) {
      const errorContent = (
        <div className="px-4 py-3">
          <div className="flex items-center gap-2">
            <span className="inline-block w-1.5 h-1.5 bg-red-400 rounded-full" />
            <span className="text-sm text-red-500 dark:text-red-400">
              {error || t("common.error")}
            </span>
          </div>
        </div>
      );

      if (grouped) {
        return errorContent;
      }
      return (
        <SettingsGroup title={t("settings.sound.systemAudio.title")}>
          {errorContent}
        </SettingsGroup>
      );
    }

    const content = (
      <div className="divide-y divide-mid-gray/10">
        {/* Native capture status - subtle inline display */}
        {info.native_available && (
          <div className="px-4 py-3">
            <div className="flex items-center gap-3">
              <span className="inline-block w-1.5 h-1.5 bg-logo-primary rounded-full shrink-0" />
              <div className="min-w-0">
                <span className="text-sm font-medium text-text">
                  {t("settings.sound.systemAudio.nativeAvailable")}
                </span>
                <p className="text-xs text-mid-gray mt-0.5 leading-relaxed">
                  {info.native_info}
                </p>
              </div>
            </div>
          </div>
        )}

        {/* Overall availability status */}
        <div className="px-4 py-3">
          <div className="flex items-center gap-3">
            <span
              className={`inline-block w-1.5 h-1.5 rounded-full shrink-0 ${
                info.available
                  ? "bg-emerald-500"
                  : "bg-amber-400 dark:bg-amber-500"
              }`}
            />
            <span
              className={`text-sm ${
                info.available
                  ? "text-emerald-600 dark:text-emerald-400"
                  : "text-amber-600 dark:text-amber-400"
              }`}
            >
              {info.available
                ? t("settings.sound.systemAudio.available")
                : t("settings.sound.systemAudio.notAvailable")}
            </span>
          </div>
        </div>

        {/* Virtual device list */}
        {info.devices.length > 0 && (
          <div className="px-4 py-3">
            <p className="text-xs text-mid-gray mb-2">
              {t("settings.sound.systemAudio.detectedDevices")}
            </p>
            <ul className="space-y-1.5">
              {info.devices.map((device) => (
                <li
                  key={device}
                  className="flex items-center gap-2 text-sm text-text"
                >
                  <span className="text-logo-primary text-xs">&#8226;</span>
                  <span className="truncate">{device}</span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {/* Setup instructions for non-native capture */}
        {info.requires_setup && !info.native_available && (
          <div className="px-4 py-3">
            <button
              onClick={() => setShowInstructions(!showInstructions)}
              className="text-sm text-logo-primary hover:text-background-ui transition-colors duration-200 flex items-center gap-1.5"
            >
              {t("settings.sound.systemAudio.setupRequired")}
              <svg
                className={`w-3 h-3 transition-transform duration-200 ${showInstructions ? "rotate-180" : ""}`}
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M19 9l-7 7-7-7"
                />
              </svg>
            </button>

            {showInstructions && (
              <div className="mt-3 p-3 bg-mid-gray/5 dark:bg-mid-gray/10 rounded-md border border-mid-gray/10">
                <pre className="text-xs text-mid-gray whitespace-pre-wrap font-sans leading-relaxed">
                  {info.setup_instructions}
                </pre>
              </div>
            )}
          </div>
        )}

        {/* Description */}
        <div className="px-4 py-3">
          <p className="text-xs text-mid-gray leading-relaxed">
            {t("settings.sound.systemAudio.description")}
          </p>
        </div>
      </div>
    );

    if (grouped) {
      return content;
    }

    return (
      <SettingsGroup title={t("settings.sound.systemAudio.title")}>
        {content}
      </SettingsGroup>
    );
  },
);
