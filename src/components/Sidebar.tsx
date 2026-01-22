import React from "react";
import { useTranslation } from "react-i18next";
import {
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
} from "lucide-react";
import PaperFlowHand from "./icons/PaperFlowHand";
import { useSettings } from "../hooks/useSettings";
import {
  GeneralSettings,
  AdvancedSettings,
  SnippetsSettings,
  FormattingSettings,
  ToneSettings,
  DeveloperSettings,
  HistorySettings,
  AboutSettings,
  PostProcessingSettings,
  MeetingSettings,
  FileTranscriptionSettings,
} from "./settings";

export type SidebarSection = keyof typeof SECTIONS_CONFIG;

interface IconProps {
  width?: number | string;
  height?: number | string;
  size?: number | string;
  className?: string;
  [key: string]: any;
}

interface SectionConfig {
  labelKey: string;
  icon: React.ComponentType<IconProps>;
  component: React.ComponentType;
  enabled: (settings: any) => boolean;
}

export const SECTIONS_CONFIG = {
  general: {
    labelKey: "sidebar.general",
    icon: PaperFlowHand,
    component: GeneralSettings,
    enabled: () => true,
  },
  files: {
    labelKey: "sidebar.files",
    icon: FileAudio,
    component: FileTranscriptionSettings,
    enabled: () => true,
  },
  advanced: {
    labelKey: "sidebar.advanced",
    icon: Cog,
    component: AdvancedSettings,
    enabled: () => true,
  },
  snippets: {
    labelKey: "sidebar.snippets",
    icon: Zap,
    component: SnippetsSettings,
    enabled: (settings) => settings?.snippets_enabled ?? false,
  },
  formatting: {
    labelKey: "sidebar.formatting",
    icon: ListOrdered,
    component: FormattingSettings,
    enabled: (settings) => settings?.auto_format_enabled ?? false,
  },
  tone: {
    labelKey: "sidebar.tone",
    icon: MessageSquare,
    component: ToneSettings,
    enabled: (settings) => settings?.tone_adjustment_enabled ?? false,
  },
  developer: {
    labelKey: "sidebar.developer",
    icon: Code,
    component: DeveloperSettings,
    enabled: (settings) => settings?.developer_mode !== "off",
  },
  postprocessing: {
    labelKey: "sidebar.postProcessing",
    icon: Sparkles,
    component: PostProcessingSettings,
    enabled: (settings) => settings?.post_process_enabled ?? false,
  },
  meeting: {
    labelKey: "sidebar.meeting",
    icon: Users,
    component: MeetingSettings,
    enabled: (settings) => settings?.show_meeting_menu ?? false,
  },
  history: {
    labelKey: "sidebar.history",
    icon: History,
    component: HistorySettings,
    enabled: () => true,
  },
  about: {
    labelKey: "sidebar.about",
    icon: Info,
    component: AboutSettings,
    enabled: () => true,
  },
} as const satisfies Record<string, SectionConfig>;

interface SidebarProps {
  activeSection: SidebarSection;
  onSectionChange: (section: SidebarSection) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
}) => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const availableSections = Object.entries(SECTIONS_CONFIG)
    .filter(([_, config]) => config.enabled(settings))
    .map(([id, config]) => ({ id: id as SidebarSection, ...config }));

  return (
    <div className="flex flex-col w-40 h-full border-r border-mid-gray/20 items-center px-2 pt-2">
      <div className="flex flex-col w-full items-center gap-1">
        {availableSections.map((section) => {
          const Icon = section.icon;
          const isActive = activeSection === section.id;

          return (
            <div
              key={section.id}
              className={`flex gap-2 items-center p-2 w-full rounded-lg cursor-pointer transition-colors ${
                isActive
                  ? "bg-logo-primary/80"
                  : "hover:bg-mid-gray/20 hover:opacity-100 opacity-85"
              }`}
              onClick={() => onSectionChange(section.id)}
            >
              <Icon width={24} height={24} className="shrink-0" />
              <p
                className="text-sm font-medium truncate"
                title={t(section.labelKey)}
              >
                {t(section.labelKey)}
              </p>
            </div>
          );
        })}
      </div>
    </div>
  );
};
