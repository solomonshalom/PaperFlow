import { useEffect, useState, useRef, useCallback } from "react";
import { Toaster } from "sonner";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import AccessibilityPermissions from "./components/AccessibilityPermissions";
import Footer from "./components/footer";
import Onboarding, { AccessibilityOnboarding } from "./components/onboarding";
import { Sidebar, SidebarSection, SECTIONS_CONFIG } from "./components/Sidebar";
import { useSettings } from "./hooks/useSettings";
import { useSettingsStore } from "./stores/settingsStore";
import { commands } from "@/bindings";
import {
  CommandPaletteProvider,
  CommandPalette,
  useCommandPalette,
} from "./components/command-palette";
import { HomePage } from "./components/home";

type OnboardingStep = "accessibility" | "model" | "done";
type AppView = "home" | SidebarSection;

const renderSettingsContent = (section: SidebarSection) => {
  const ActiveComponent =
    SECTIONS_CONFIG[section]?.component || SECTIONS_CONFIG.general.component;
  return <ActiveComponent />;
};

// Inner component that uses the command palette context
function AppContent() {
  const [onboardingStep, setOnboardingStep] = useState<OnboardingStep | null>(
    null,
  );
  const [currentView, setCurrentView] = useState<AppView>("home");
  const { togglePalette } = useCommandPalette();
  const { settings, updateSetting } = useSettings();
  const refreshAudioDevices = useSettingsStore(
    (state) => state.refreshAudioDevices,
  );
  const refreshOutputDevices = useSettingsStore(
    (state) => state.refreshOutputDevices,
  );
  const hasCompletedPostOnboardingInit = useRef(false);

  useEffect(() => {
    checkOnboardingStatus();
  }, []);

  // Initialize Enigo and refresh audio devices when main app loads
  useEffect(() => {
    if (onboardingStep === "done" && !hasCompletedPostOnboardingInit.current) {
      hasCompletedPostOnboardingInit.current = true;
      commands.initializeEnigo().catch((e) => {
        console.warn("Failed to initialize Enigo:", e);
      });
      refreshAudioDevices();
      refreshOutputDevices();
    }
  }, [onboardingStep, refreshAudioDevices, refreshOutputDevices]);

  // Handle keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Check for Ctrl+K (Windows/Linux) or Cmd+K (macOS) for command palette
      const isCommandPaletteShortcut =
        event.key.toLowerCase() === "k" && (event.ctrlKey || event.metaKey);

      if (isCommandPaletteShortcut) {
        event.preventDefault();
        togglePalette();
        return;
      }

      // Check for Ctrl+Shift+D (Windows/Linux) or Cmd+Shift+D (macOS) for debug
      const isDebugShortcut =
        event.shiftKey &&
        event.key.toLowerCase() === "d" &&
        (event.ctrlKey || event.metaKey);

      if (isDebugShortcut) {
        event.preventDefault();
        const currentDebugMode = settings?.debug_mode ?? false;
        updateSetting("debug_mode", !currentDebugMode);
      }
    };

    // Add event listener when component mounts
    document.addEventListener("keydown", handleKeyDown);

    // Cleanup event listener when component unmounts
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [settings?.debug_mode, updateSetting, togglePalette]);

  const checkOnboardingStatus = async () => {
    try {
      // Check if they have any models available
      const result = await commands.hasAnyModelsAvailable();
      if (result.status === "ok") {
        // If they have models/downloads, they're done. Otherwise start permissions step.
        setOnboardingStep(result.data ? "done" : "accessibility");
      } else {
        setOnboardingStep("accessibility");
      }
    } catch (error) {
      console.error("Failed to check onboarding status:", error);
      setOnboardingStep("accessibility");
    }
  };

  const handleAccessibilityComplete = () => {
    setOnboardingStep("model");
  };

  const handleModelSelected = () => {
    // Transition to main app - user has started a download
    setOnboardingStep("done");
  };

  // Handle navigation from command palette or sidebar
  const handleNavigate = useCallback((view: AppView) => {
    setCurrentView(view);
  }, []);

  // Listen for tray menu navigation to settings
  useEffect(() => {
    const unlisten = listen("navigate-to-settings", () => {
      setCurrentView("general");
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Still checking onboarding status
  if (onboardingStep === null) {
    return null;
  }

  if (onboardingStep === "accessibility") {
    return <AccessibilityOnboarding onComplete={handleAccessibilityComplete} />;
  }

  if (onboardingStep === "model") {
    return <Onboarding onModelSelected={handleModelSelected} />;
  }

  const isHomeView = currentView === "home";
  const showSidebar = settings?.show_sidebar ?? false;

  return (
    <div className="h-screen flex flex-col select-none cursor-default">
      <Toaster
        theme="system"
        toastOptions={{
          unstyled: true,
          classNames: {
            toast:
              "bg-background border border-mid-gray/20 rounded-lg shadow-lg px-4 py-3 flex items-center gap-3 text-sm",
            title: "font-medium",
            description: "text-mid-gray",
          },
        }}
      />
      {/* Main content area that takes remaining space */}
      <div className="flex-1 flex overflow-hidden">
        {/* Only show sidebar when enabled in settings and not on home view */}
        {showSidebar && !isHomeView && (
          <Sidebar
            activeSection={currentView as SidebarSection}
            onSectionChange={handleNavigate}
          />
        )}
        {/* Scrollable content area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          <div
            className={`flex-1 overflow-y-auto ${isHomeView ? "flex flex-col" : ""}`}
          >
            {isHomeView ? (
              <HomePage />
            ) : (
              <div className="flex flex-col items-center p-4 gap-4">
                <AccessibilityPermissions />
                {renderSettingsContent(currentView as SidebarSection)}
              </div>
            )}
          </div>
        </div>
      </div>
      {/* Fixed footer at bottom */}
      <Footer />
      {/* Command palette modal */}
      <CommandPalette onNavigate={handleNavigate} />
    </div>
  );
}

// Main App component with provider
function App() {
  return (
    <CommandPaletteProvider>
      <AppContent />
    </CommandPaletteProvider>
  );
}

export default App;
