import React, { useState, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { save } from "@tauri-apps/plugin-dialog";
import {
  Download,
  FileText,
  FileJson,
  Subtitles,
  File,
  FileCode,
  Table,
  Globe,
} from "lucide-react";
import { commands, type ExportFormat } from "@/bindings";

interface ExportDropdownProps {
  text: string;
  title?: string;
  sourceFile?: string;
  durationMs?: number;
}

const FormatIcon: React.FC<{ format: ExportFormat }> = ({ format }) => {
  switch (format) {
    case "txt":
      return <FileText className="w-4 h-4" />;
    case "json":
      return <FileJson className="w-4 h-4" />;
    case "srt":
    case "vtt":
      return <Subtitles className="w-4 h-4" />;
    case "markdown":
      return <FileCode className="w-4 h-4" />;
    case "csv":
      return <Table className="w-4 h-4" />;
    case "html":
      return <Globe className="w-4 h-4" />;
    default:
      return <File className="w-4 h-4" />;
  }
};

export const ExportDropdown: React.FC<ExportDropdownProps> = ({
  text,
  title,
  sourceFile,
  durationMs,
}) => {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const formats: ExportFormat[] = [
    "txt",
    "srt",
    "vtt",
    "json",
    "markdown",
    "csv",
    "html",
  ];

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isOpen]);

  const handleExport = async (format: ExportFormat) => {
    setIsExporting(true);
    setIsOpen(false);

    try {
      // Get file extension
      const extension = await commands.getExportFileExtension(format);

      // Generate default filename
      const defaultName = title
        ? `${title.replace(/\.[^/.]+$/, "")}.${extension}`
        : `transcription.${extension}`;

      // Open save dialog
      const filePath = await save({
        defaultPath: defaultName,
        filters: [
          {
            name: format.toUpperCase(),
            extensions: [extension],
          },
        ],
      });

      if (filePath) {
        // Export to file
        const result = await commands.exportTranscriptToFile(
          text,
          format,
          filePath,
          title ?? null,
          sourceFile ?? null,
          durationMs ?? null,
          null, // segments - let backend generate them
        );

        if (result.status !== "ok") {
          console.error("Export failed:", result.error);
        }
      }
    } catch (error) {
      console.error("Failed to export:", error);
    } finally {
      setIsExporting(false);
    }
  };

  const formatLabels: Record<ExportFormat, string> = {
    txt: t("settings.files.export.formats.txt"),
    srt: t("settings.files.export.formats.srt"),
    vtt: t("settings.files.export.formats.vtt"),
    json: t("settings.files.export.formats.json"),
    markdown: t("settings.files.export.formats.md"),
    csv: t("settings.files.export.formats.csv"),
    html: t("settings.files.export.formats.html"),
    docx: t("settings.files.export.formats.docx"),
    pdf: t("settings.files.export.formats.pdf"),
  };

  return (
    <div className="relative" ref={dropdownRef}>
      <button
        onClick={() => setIsOpen(!isOpen)}
        disabled={isExporting}
        className="p-1.5 text-text/40 hover:text-logo-primary transition-colors rounded hover:bg-logo-primary/10 disabled:opacity-50"
        title={t("settings.files.export.title")}
      >
        {isExporting ? (
          <div className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
        ) : (
          <Download className="w-4 h-4" />
        )}
      </button>

      {isOpen && (
        <div className="absolute right-0 bottom-full mb-1 z-50 min-w-[160px] bg-background border border-mid-gray/30 rounded-lg shadow-lg overflow-hidden">
          {formats.map((format) => (
            <button
              key={format}
              onClick={() => handleExport(format)}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm text-text/80 text-left hover:bg-mid-gray/10 transition-colors"
            >
              <FormatIcon format={format} />
              <span>{formatLabels[format]}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
};
