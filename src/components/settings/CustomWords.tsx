import React, { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { Input } from "../ui/Input";
import { Button } from "../ui/Button";
import { SettingContainer } from "../ui/SettingContainer";
import { Upload, Download, Search, X } from "lucide-react";

// Increased limits for enhanced custom dictionary
const MAX_WORD_LENGTH = 100;
const MAX_WORD_COUNT = 1000;

interface CustomWordsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const CustomWords: React.FC<CustomWordsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const [newWord, setNewWord] = useState("");
    const [searchFilter, setSearchFilter] = useState("");
    const [showBulkImport, setShowBulkImport] = useState(false);
    const [bulkImportText, setBulkImportText] = useState("");
    const customWords = getSetting("custom_words") || [];

    // Filter words based on search
    const filteredWords = useMemo(() => {
      if (!searchFilter.trim()) return customWords;
      const filter = searchFilter.toLowerCase();
      return customWords.filter((word) => word.toLowerCase().includes(filter));
    }, [customWords, searchFilter]);

    const sanitizeWord = (word: string): string => {
      return word.trim().replace(/[<>"'&]/g, "");
    };

    const handleAddWord = () => {
      const trimmedWord = newWord.trim();
      const sanitizedWord = sanitizeWord(trimmedWord);
      if (
        sanitizedWord &&
        !sanitizedWord.includes(" ") &&
        sanitizedWord.length <= MAX_WORD_LENGTH &&
        customWords.length < MAX_WORD_COUNT &&
        !customWords.includes(sanitizedWord)
      ) {
        updateSetting("custom_words", [...customWords, sanitizedWord]);
        setNewWord("");
      }
    };

    const handleRemoveWord = (wordToRemove: string) => {
      updateSetting(
        "custom_words",
        customWords.filter((word) => word !== wordToRemove),
      );
    };

    const handleKeyPress = (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleAddWord();
      }
    };

    // Bulk import handler
    const handleBulkImport = () => {
      // Parse words from comma, semicolon, or newline separated input
      const words = bulkImportText
        .split(/[,;\n]+/)
        .map((word) => sanitizeWord(word))
        .filter(
          (word) =>
            word &&
            !word.includes(" ") &&
            word.length <= MAX_WORD_LENGTH &&
            !customWords.includes(word),
        );

      // Deduplicate within imported words
      const uniqueNewWords = [...new Set(words)];

      // Limit to max count
      const availableSlots = MAX_WORD_COUNT - customWords.length;
      const wordsToAdd = uniqueNewWords.slice(0, availableSlots);

      if (wordsToAdd.length > 0) {
        updateSetting("custom_words", [...customWords, ...wordsToAdd]);
      }

      setBulkImportText("");
      setShowBulkImport(false);
    };

    // Export handler
    const handleExport = () => {
      const blob = new Blob([customWords.join("\n")], { type: "text/plain" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = "custom-words.txt";
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    };

    return (
      <>
        <SettingContainer
          title={t("settings.advanced.customWords.title")}
          description={t("settings.advanced.customWords.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-2">
              <Input
                type="text"
                className="max-w-40"
                value={newWord}
                onChange={(e) => setNewWord(e.target.value)}
                onKeyDown={handleKeyPress}
                placeholder={t("settings.advanced.customWords.placeholder")}
                variant="compact"
                disabled={
                  isUpdating("custom_words") ||
                  customWords.length >= MAX_WORD_COUNT
                }
              />
              <Button
                onClick={handleAddWord}
                disabled={
                  !newWord.trim() ||
                  newWord.includes(" ") ||
                  newWord.trim().length > MAX_WORD_LENGTH ||
                  customWords.length >= MAX_WORD_COUNT ||
                  isUpdating("custom_words")
                }
                variant="primary"
                size="md"
              >
                {t("settings.advanced.customWords.add")}
              </Button>
              <Button
                onClick={() => setShowBulkImport(true)}
                disabled={
                  isUpdating("custom_words") ||
                  customWords.length >= MAX_WORD_COUNT
                }
                variant="secondary"
                size="md"
                title={t("settings.advanced.customWords.bulkImport.title")}
              >
                <Upload className="w-4 h-4" />
              </Button>
              {customWords.length > 0 && (
                <Button
                  onClick={handleExport}
                  variant="secondary"
                  size="md"
                  title={t("settings.advanced.customWords.export")}
                >
                  <Download className="w-4 h-4" />
                </Button>
              )}
            </div>
            <span className="text-xs text-text/50">
              {t("settings.advanced.customWords.count", {
                count: customWords.length,
                max: MAX_WORD_COUNT,
              })}
            </span>
          </div>
        </SettingContainer>

        {/* Bulk Import Modal */}
        {showBulkImport && (
          <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
            <div className="bg-background rounded-lg p-6 max-w-md w-full mx-4 shadow-xl">
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-lg font-medium">
                  {t("settings.advanced.customWords.bulkImport.title")}
                </h3>
                <button
                  onClick={() => setShowBulkImport(false)}
                  className="text-text/50 hover:text-text"
                >
                  <X className="w-5 h-5" />
                </button>
              </div>
              <p className="text-sm text-text/60 mb-4">
                {t("settings.advanced.customWords.bulkImport.description")}
              </p>
              <textarea
                value={bulkImportText}
                onChange={(e) => setBulkImportText(e.target.value)}
                className="w-full h-40 p-3 border border-mid-gray/30 rounded-lg bg-background text-sm resize-none focus:outline-none focus:border-logo-primary"
                placeholder={t(
                  "settings.advanced.customWords.bulkImport.placeholder",
                )}
              />
              <div className="flex justify-end gap-2 mt-4">
                <Button
                  variant="secondary"
                  onClick={() => setShowBulkImport(false)}
                >
                  {t("common.cancel")}
                </Button>
                <Button
                  variant="primary"
                  onClick={handleBulkImport}
                  disabled={!bulkImportText.trim()}
                >
                  {t("settings.advanced.customWords.bulkImport.import")}
                </Button>
              </div>
            </div>
          </div>
        )}

        {customWords.length > 0 && (
          <div
            className={`px-4 p-2 ${grouped ? "" : "rounded-lg border border-mid-gray/20"}`}
          >
            {/* Search filter - shown when more than 10 words */}
            {customWords.length > 10 && (
              <div className="mb-3 relative">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text/40" />
                <Input
                  type="text"
                  value={searchFilter}
                  onChange={(e) => setSearchFilter(e.target.value)}
                  placeholder={t("settings.advanced.customWords.search")}
                  className="pl-9 w-full max-w-xs"
                  variant="compact"
                />
              </div>
            )}
            <div className="flex flex-wrap gap-1">
              {filteredWords.map((word) => (
                <Button
                  key={word}
                  onClick={() => handleRemoveWord(word)}
                  disabled={isUpdating("custom_words")}
                  variant="secondary"
                  size="sm"
                  className="inline-flex items-center gap-1 cursor-pointer"
                  aria-label={t("settings.advanced.customWords.remove", {
                    word,
                  })}
                >
                  <span>{word}</span>
                  <svg
                    className="w-3 h-3"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M6 18L18 6M6 6l12 12"
                    />
                  </svg>
                </Button>
              ))}
              {searchFilter && filteredWords.length === 0 && (
                <p className="text-sm text-text/50">
                  {t("settings.advanced.customWords.noResults")}
                </p>
              )}
            </div>
          </div>
        )}
      </>
    );
  },
);
