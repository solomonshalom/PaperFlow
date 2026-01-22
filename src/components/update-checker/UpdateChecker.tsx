import React, { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useSettings } from "../../hooks/useSettings";

const GITHUB_REPO = "solomonshalom/paperflow";
const RELEASES_URL = `https://github.com/${GITHUB_REPO}/releases`;

interface GitHubRelease {
  tag_name: string;
  html_url: string;
}

interface UpdateCheckerProps {
  className?: string;
}

const UpdateChecker: React.FC<UpdateCheckerProps> = ({ className = "" }) => {
  const { t } = useTranslation();
  const [isChecking, setIsChecking] = useState(false);
  const [updateAvailable, setUpdateAvailable] = useState(false);
  const [latestVersion, setLatestVersion] = useState<string | null>(null);
  const [showUpToDate, setShowUpToDate] = useState(false);

  const { settings, isLoading } = useSettings();
  const settingsLoaded = !isLoading && settings !== null;
  const updateChecksEnabled = settings?.update_checks_enabled ?? false;

  const upToDateTimeoutRef = useRef<ReturnType<typeof setTimeout>>();
  const isManualCheckRef = useRef(false);

  useEffect(() => {
    if (!settingsLoaded) return;

    if (!updateChecksEnabled) {
      if (upToDateTimeoutRef.current) {
        clearTimeout(upToDateTimeoutRef.current);
      }
      setIsChecking(false);
      setUpdateAvailable(false);
      setShowUpToDate(false);
      return;
    }

    checkForUpdates();

    const updateUnlisten = listen("check-for-updates", () => {
      handleManualUpdateCheck();
    });

    return () => {
      if (upToDateTimeoutRef.current) {
        clearTimeout(upToDateTimeoutRef.current);
      }
      updateUnlisten.then((fn) => fn());
    };
  }, [settingsLoaded, updateChecksEnabled]);

  const parseVersion = (version: string): number[] => {
    // Remove 'v' prefix if present
    const clean = version.replace(/^v/, "");
    return clean.split(".").map((n) => parseInt(n, 10) || 0);
  };

  const isNewerVersion = (latest: string, current: string): boolean => {
    const latestParts = parseVersion(latest);
    const currentParts = parseVersion(current);

    for (
      let i = 0;
      i < Math.max(latestParts.length, currentParts.length);
      i++
    ) {
      const l = latestParts[i] || 0;
      const c = currentParts[i] || 0;
      if (l > c) return true;
      if (l < c) return false;
    }
    return false;
  };

  const checkForUpdates = async () => {
    if (!updateChecksEnabled || isChecking) return;

    try {
      setIsChecking(true);

      const response = await fetch(
        `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`,
      );

      if (!response.ok) {
        throw new Error(`GitHub API error: ${response.status}`);
      }

      const release: GitHubRelease = await response.json();
      const currentVersion = await getVersion();

      if (isNewerVersion(release.tag_name, currentVersion)) {
        setUpdateAvailable(true);
        setLatestVersion(release.tag_name.replace(/^v/, ""));
        setShowUpToDate(false);
      } else {
        setUpdateAvailable(false);
        setLatestVersion(null);

        if (isManualCheckRef.current) {
          setShowUpToDate(true);
          if (upToDateTimeoutRef.current) {
            clearTimeout(upToDateTimeoutRef.current);
          }
          upToDateTimeoutRef.current = setTimeout(() => {
            setShowUpToDate(false);
          }, 3000);
        }
      }
    } catch (error) {
      console.error("Failed to check for updates:", error);
    } finally {
      setIsChecking(false);
      isManualCheckRef.current = false;
    }
  };

  const handleManualUpdateCheck = () => {
    if (!updateChecksEnabled) return;
    isManualCheckRef.current = true;
    checkForUpdates();
  };

  const openReleasesPage = async () => {
    await openUrl(RELEASES_URL);
  };

  const getUpdateStatusText = () => {
    if (!updateChecksEnabled) {
      return t("footer.updateCheckingDisabled");
    }
    if (isChecking) return t("footer.checkingUpdates");
    if (showUpToDate) return t("footer.upToDate");
    if (updateAvailable && latestVersion) {
      return t("footer.updateAvailable", { version: latestVersion });
    }
    return t("footer.checkForUpdates");
  };

  const getUpdateStatusAction = () => {
    if (!updateChecksEnabled) return undefined;
    if (updateAvailable) return openReleasesPage;
    if (!isChecking && !updateAvailable) return handleManualUpdateCheck;
    return undefined;
  };

  const isUpdateDisabled = !updateChecksEnabled || isChecking;
  const isUpdateClickable =
    !isUpdateDisabled && (updateAvailable || (!isChecking && !showUpToDate));

  return (
    <div className={`flex items-center gap-3 ${className}`}>
      {isUpdateClickable ? (
        <button
          onClick={getUpdateStatusAction()}
          disabled={isUpdateDisabled}
          className={`transition-colors disabled:opacity-50 tabular-nums ${
            updateAvailable
              ? "text-logo-primary hover:text-logo-primary/80 font-medium"
              : "text-text/60 hover:text-text/80"
          }`}
        >
          {getUpdateStatusText()}
        </button>
      ) : (
        <span className="text-text/60 tabular-nums">
          {getUpdateStatusText()}
        </span>
      )}
    </div>
  );
};

export default UpdateChecker;
