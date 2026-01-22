import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { type } from "@tauri-apps/plugin-os";
import PaperFlowTextLogo from "../icons/PaperFlowTextLogo";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import { PaperFlowShortcut } from "../settings/PaperFlowShortcut";

export const HomePage: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const [isMac, setIsMac] = useState(true);

  const pttEnabled = getSetting("push_to_talk") || false;

  useEffect(() => {
    const checkPlatform = async () => {
      try {
        const osType = type();
        setIsMac(osType === "macos");
      } catch {
        // Default to Mac style
        setIsMac(true);
      }
    };
    checkPlatform();
  }, []);

  const settingsHint = isMac
    ? t("home.settingsHint")
    : t("home.settingsHintWindows");

  return (
    <div className="flex-1 flex flex-col items-center justify-center px-4 py-4">
      {/* Logo */}
      <PaperFlowTextLogo width={280} className="mb-4" variant="text" />

      {/* Action cards - constrained width */}
      <div className="w-full max-w-md space-y-2">
        {/* Push To Talk */}
        <div className="rounded-lg border border-mid-gray/20 overflow-hidden">
          <ToggleSwitch
            checked={pttEnabled}
            onChange={(enabled) => updateSetting("push_to_talk", enabled)}
            isUpdating={isUpdating("push_to_talk")}
            label={t("settings.general.pushToTalk.label")}
            description={t("settings.general.pushToTalk.description")}
            descriptionMode="inline"
            grouped={true}
          />
        </div>

        {/* Transcribe Shortcut */}
        <div className="rounded-lg border border-mid-gray/20 overflow-hidden">
          <PaperFlowShortcut
            shortcutId="transcribe"
            descriptionMode="inline"
            grouped={true}
          />
        </div>
      </div>

      {/* Settings hint */}
      <p className="text-xs text-mid-gray mt-4">{settingsHint}</p>
    </div>
  );
};
