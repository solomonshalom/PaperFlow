import { listen } from "@tauri-apps/api/event";
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  MicrophoneIcon,
  TranscriptionIcon,
  CancelIcon,
} from "../components/icons";
import "./RecordingOverlay.css";
import { commands } from "@/bindings";
import { syncLanguageFromSettings } from "@/i18n";

type OverlayState = "recording" | "transcribing" | "meeting";

interface LivePreviewEvent {
  text: string;
  is_final: boolean;
}

interface LivePreviewErrorEvent {
  error_type: string;
  message: string;
  is_fatal: boolean;
}

const RecordingOverlay: React.FC = () => {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("recording");
  const [levels, setLevels] = useState<number[]>(Array(16).fill(0));
  const [previewText, setPreviewText] = useState<string>("");
  const [previewError, setPreviewError] = useState<boolean>(false);
  const smoothedLevelsRef = useRef<number[]>(Array(16).fill(0));
  const previewScrollRef = useRef<HTMLDivElement>(null);

  // Scroll to end when preview text updates
  useEffect(() => {
    if (previewScrollRef.current && previewText) {
      previewScrollRef.current.scrollLeft =
        previewScrollRef.current.scrollWidth;
    }
  }, [previewText]);

  useEffect(() => {
    // Store unlisten functions for cleanup
    let unlistenShow: (() => void) | null = null;
    let unlistenHide: (() => void) | null = null;
    let unlistenPreview: (() => void) | null = null;
    let unlistenPreviewError: (() => void) | null = null;
    let unlistenLevel: (() => void) | null = null;
    let isMounted = true;

    const setupEventListeners = async () => {
      try {
        // Listen for show-overlay event from Rust
        unlistenShow = await listen("show-overlay", async (event) => {
          if (!isMounted) return;
          // Sync language from settings each time overlay is shown
          await syncLanguageFromSettings();
          const overlayState = event.payload as OverlayState;
          setState(overlayState);
          setPreviewText(""); // Clear preview text when showing new overlay
          setPreviewError(false); // Clear error state
          setIsVisible(true);
        });

        // Listen for hide-overlay event from Rust
        unlistenHide = await listen("hide-overlay", () => {
          if (!isMounted) return;
          setIsVisible(false);
          setPreviewText(""); // Clear preview text when hiding
          setPreviewError(false); // Clear error state
        });

        // Listen for live preview updates
        unlistenPreview = await listen<LivePreviewEvent>(
          "live-preview-update",
          (event) => {
            if (!isMounted) return;
            // Only update if we have valid text
            if (event.payload.text) {
              setPreviewText(event.payload.text);
              setPreviewError(false); // Clear error on successful update
            }
          },
        );

        // Listen for live preview errors
        unlistenPreviewError = await listen<LivePreviewErrorEvent>(
          "live-preview-error",
          (event) => {
            if (!isMounted) return;
            console.warn("Live preview error:", event.payload);
            if (event.payload.is_fatal) {
              // Fatal error - clear preview text and show bars instead
              setPreviewText("");
              setPreviewError(true);
            }
          },
        );

        // Listen for mic-level updates
        unlistenLevel = await listen<number[]>("mic-level", (event) => {
          if (!isMounted) return;
          const newLevels = event.payload as number[];

          // Apply smoothing to reduce jitter
          const smoothed = smoothedLevelsRef.current.map((prev, i) => {
            const target = newLevels[i] || 0;
            return prev * 0.7 + target * 0.3; // Smooth transition
          });

          smoothedLevelsRef.current = smoothed;
          setLevels(smoothed.slice(0, 9));
        });
      } catch (error) {
        console.error("Failed to setup overlay event listeners:", error);
      }
    };

    setupEventListeners();

    // Cleanup function
    return () => {
      isMounted = false;
      unlistenShow?.();
      unlistenHide?.();
      unlistenPreview?.();
      unlistenPreviewError?.();
      unlistenLevel?.();
    };
  }, []);

  const getIcon = () => {
    if (state === "recording" || state === "meeting") {
      return <MicrophoneIcon />;
    } else {
      return <TranscriptionIcon />;
    }
  };

  // Check if we're in an active recording state (regular or meeting)
  const isRecordingState = state === "recording" || state === "meeting";

  return (
    <div className={`recording-overlay ${isVisible ? "fade-in" : ""}`}>
      <div className="overlay-left">{getIcon()}</div>

      <div className="overlay-middle">
        {isRecordingState && previewText && (
          <div className="preview-text">
            <div ref={previewScrollRef} className="preview-text-scroll">
              <span className="preview-text-content">{previewText}</span>
            </div>
            <span className="blinking-cursor">|</span>
          </div>
        )}
        {isRecordingState && !previewText && (
          <div
            className={`bars-container ${previewError ? "preview-error" : ""}`}
          >
            {levels.map((v, i) => (
              <div
                key={i}
                className="bar"
                style={{
                  height: `${Math.min(20, 4 + Math.pow(v, 0.7) * 16)}px`, // Cap at 20px max height
                  transition: "height 60ms ease-out, opacity 120ms ease-out",
                  opacity: Math.max(0.2, v * 1.7), // Minimum opacity for visibility
                }}
              />
            ))}
          </div>
        )}
        {state === "transcribing" && (
          <div className="transcribing-text">{t("overlay.transcribing")}</div>
        )}
      </div>

      <div className="overlay-right">
        {isRecordingState && (
          <div
            className="cancel-button"
            onClick={() => {
              commands.cancelOperation();
            }}
          >
            <CancelIcon />
          </div>
        )}
      </div>
    </div>
  );
};

export default RecordingOverlay;
