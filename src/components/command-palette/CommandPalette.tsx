import React, { useState, useEffect, useRef, useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  Search,
  Cog,
  History,
  Info,
  Sparkles,
  Zap,
  ListOrdered,
  MessageSquare,
  Code,
  Users,
  FileAudio,
  Home,
} from "lucide-react";
import { useCommandPalette } from "./CommandPaletteContext";
import { useSettings } from "../../hooks/useSettings";
import { SECTIONS_CONFIG, type SidebarSection } from "../Sidebar";
import PaperFlowHand from "../icons/PaperFlowHand";

// Flexible icon props that work with both Lucide icons and custom components
interface IconProps {
  width?: number | string;
  height?: number | string;
  size?: number | string;
  className?: string;
  [key: string]: unknown;
}

interface Command {
  id: string;
  label: string;
  keywords: string[];
  icon: React.ComponentType<IconProps>;
  section?: SidebarSection | "home";
}

// Map section IDs to icons (matching Sidebar)
const SECTION_ICONS: Record<string, React.ComponentType<IconProps>> = {
  general: PaperFlowHand,
  files: FileAudio,
  advanced: Cog,
  snippets: Zap,
  formatting: ListOrdered,
  tone: MessageSquare,
  developer: Code,
  postprocessing: Sparkles,
  meeting: Users,
  history: History,
  about: Info,
};

interface CommandPaletteProps {
  onNavigate: (section: SidebarSection | "home") => void;
}

export const CommandPalette: React.FC<CommandPaletteProps> = ({
  onNavigate,
}) => {
  const { t } = useTranslation();
  const { isOpen, closePalette } = useCommandPalette();
  const { settings } = useSettings();
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Build commands list from SECTIONS_CONFIG
  const commands = useMemo<Command[]>(() => {
    const cmds: Command[] = [
      // Home command
      {
        id: "home",
        label: t("commandPalette.home"),
        keywords: ["home", "start", "main"],
        icon: Home,
        section: "home",
      },
    ];

    // Add settings sections
    Object.entries(SECTIONS_CONFIG).forEach(([id, config]) => {
      if (config.enabled(settings)) {
        cmds.push({
          id,
          label: t(config.labelKey),
          keywords: [id, t(config.labelKey).toLowerCase()],
          icon: SECTION_ICONS[id] || Cog,
          section: id as SidebarSection,
        });
      }
    });

    return cmds;
  }, [settings, t]);

  // Filter commands based on search query
  const filteredCommands = useMemo(() => {
    if (!searchQuery.trim()) return commands;

    const query = searchQuery.toLowerCase();
    return commands.filter(
      (cmd) =>
        cmd.label.toLowerCase().includes(query) ||
        cmd.keywords.some((kw) => kw.includes(query)),
    );
  }, [commands, searchQuery]);

  // Reset state when palette opens/closes
  useEffect(() => {
    if (isOpen) {
      setSearchQuery("");
      setSelectedIndex(0);
      // Focus input after a small delay to ensure the dialog is rendered
      setTimeout(() => inputRef.current?.focus(), 10);
    }
  }, [isOpen]);

  // Reset selected index when filtered results change
  useEffect(() => {
    setSelectedIndex(0);
  }, [filteredCommands.length]);

  // Scroll selected item into view
  useEffect(() => {
    if (listRef.current && filteredCommands.length > 0) {
      const selectedElement = listRef.current.children[
        selectedIndex
      ] as HTMLElement;
      if (selectedElement) {
        selectedElement.scrollIntoView({ block: "nearest" });
      }
    }
  }, [selectedIndex, filteredCommands.length]);

  // Handle keyboard navigation
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          setSelectedIndex((prev) =>
            prev < filteredCommands.length - 1 ? prev + 1 : prev,
          );
          break;
        case "ArrowUp":
          e.preventDefault();
          setSelectedIndex((prev) => (prev > 0 ? prev - 1 : prev));
          break;
        case "Enter":
          e.preventDefault();
          if (filteredCommands[selectedIndex]) {
            executeCommand(filteredCommands[selectedIndex]);
          }
          break;
        case "Escape":
          e.preventDefault();
          closePalette();
          break;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, selectedIndex, filteredCommands, closePalette]);

  const executeCommand = (command: Command) => {
    if (command.section) {
      onNavigate(command.section);
    }
    closePalette();
  };

  if (!isOpen) return null;

  return (
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/50 z-40"
        onClick={closePalette}
        aria-hidden="true"
      />

      {/* Palette */}
      <div
        className="fixed top-1/4 left-1/2 -translate-x-1/2 w-full max-w-lg bg-background border border-mid-gray/20 rounded-lg shadow-lg overflow-hidden z-50"
        role="dialog"
        aria-modal="true"
        aria-label={t("commandPalette.label")}
      >
        {/* Search input */}
        <div className="p-3 border-b border-mid-gray/20">
          <div className="flex items-center gap-2">
            <Search size={16} className="text-mid-gray shrink-0" />
            <input
              ref={inputRef}
              type="text"
              placeholder={t("commandPalette.placeholder")}
              className="w-full bg-transparent text-sm focus:outline-none placeholder:text-mid-gray"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
            />
          </div>
        </div>

        {/* Results list */}
        <div ref={listRef} className="max-h-64 overflow-y-auto p-1">
          {filteredCommands.length === 0 ? (
            <div className="px-3 py-4 text-sm text-mid-gray text-center">
              {t("commandPalette.noResults")}
            </div>
          ) : (
            filteredCommands.map((cmd, index) => {
              const Icon = cmd.icon;
              const isSelected = index === selectedIndex;

              return (
                <div
                  key={cmd.id}
                  className={`flex items-center gap-3 px-3 py-2 rounded-md cursor-pointer transition-colors ${
                    isSelected ? "bg-logo-primary/20" : "hover:bg-mid-gray/10"
                  }`}
                  onClick={() => executeCommand(cmd)}
                  onMouseEnter={() => setSelectedIndex(index)}
                  role="option"
                  aria-selected={isSelected}
                >
                  <Icon
                    width={18}
                    height={18}
                    className="shrink-0 opacity-70"
                  />
                  <span className="text-sm">{cmd.label}</span>
                </div>
              );
            })
          )}
        </div>

        {/* Footer hint */}
        <div className="px-3 py-2 border-t border-mid-gray/20 flex items-center justify-between text-xs text-mid-gray">
          <span>{t("commandPalette.hint.navigate")}</span>
          <span>{t("commandPalette.hint.select")}</span>
        </div>
      </div>
    </>
  );
};
