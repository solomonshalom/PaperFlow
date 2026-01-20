import React, { useState, useRef, useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";
import { LANGUAGES } from "../../../lib/constants/languages";

interface MultiLanguageSelectorProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

// Filter out "auto" from languages list for primary/secondary selection
const SELECTABLE_LANGUAGES = LANGUAGES.filter((lang) => lang.value !== "auto");

export const MultiLanguageSelector: React.FC<MultiLanguageSelectorProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const multilingualEnabled = getSetting("multilingual_mode_enabled") ?? false;
  const primaryLanguage = getSetting("primary_language") ?? null;
  const secondaryLanguage = getSetting("secondary_language") ?? null;

  const [primaryOpen, setPrimaryOpen] = useState(false);
  const [secondaryOpen, setSecondaryOpen] = useState(false);
  const [primarySearch, setPrimarySearch] = useState("");
  const [secondarySearch, setSecondarySearch] = useState("");

  const primaryRef = useRef<HTMLDivElement>(null);
  const secondaryRef = useRef<HTMLDivElement>(null);
  const primarySearchRef = useRef<HTMLInputElement>(null);
  const secondarySearchRef = useRef<HTMLInputElement>(null);

  // Close dropdowns when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        primaryRef.current &&
        !primaryRef.current.contains(event.target as Node)
      ) {
        setPrimaryOpen(false);
        setPrimarySearch("");
      }
      if (
        secondaryRef.current &&
        !secondaryRef.current.contains(event.target as Node)
      ) {
        setSecondaryOpen(false);
        setSecondarySearch("");
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // Focus search input when dropdown opens
  useEffect(() => {
    if (primaryOpen && primarySearchRef.current) {
      primarySearchRef.current.focus();
    }
  }, [primaryOpen]);

  useEffect(() => {
    if (secondaryOpen && secondarySearchRef.current) {
      secondarySearchRef.current.focus();
    }
  }, [secondaryOpen]);

  const filteredPrimaryLanguages = useMemo(
    () =>
      SELECTABLE_LANGUAGES.filter((lang) =>
        lang.label.toLowerCase().includes(primarySearch.toLowerCase()),
      ),
    [primarySearch],
  );

  const filteredSecondaryLanguages = useMemo(
    () =>
      SELECTABLE_LANGUAGES.filter(
        (lang) =>
          lang.label.toLowerCase().includes(secondarySearch.toLowerCase()) &&
          lang.value !== primaryLanguage,
      ),
    [secondarySearch, primaryLanguage],
  );

  const getPrimaryLanguageName = () => {
    if (!primaryLanguage) return t("settings.multilingual.selectLanguage");
    return (
      SELECTABLE_LANGUAGES.find((lang) => lang.value === primaryLanguage)
        ?.label || primaryLanguage
    );
  };

  const getSecondaryLanguageName = () => {
    if (!secondaryLanguage) return t("settings.multilingual.selectLanguage");
    return (
      SELECTABLE_LANGUAGES.find((lang) => lang.value === secondaryLanguage)
        ?.label || secondaryLanguage
    );
  };

  const handleToggleMultilingual = async (enabled: boolean) => {
    await updateSetting("multilingual_mode_enabled", enabled);
  };

  const handlePrimarySelect = async (value: string) => {
    await updateSetting("primary_language", value);
    setPrimaryOpen(false);
    setPrimarySearch("");
    // Clear secondary if it's the same as primary
    if (secondaryLanguage === value) {
      await updateSetting("secondary_language", null);
    }
  };

  const handleSecondarySelect = async (value: string) => {
    await updateSetting("secondary_language", value);
    setSecondaryOpen(false);
    setSecondarySearch("");
  };

  const handlePrimaryKeyDown = (
    event: React.KeyboardEvent<HTMLInputElement>,
  ) => {
    if (event.key === "Enter" && filteredPrimaryLanguages.length > 0) {
      handlePrimarySelect(filteredPrimaryLanguages[0].value);
    } else if (event.key === "Escape") {
      setPrimaryOpen(false);
      setPrimarySearch("");
    }
  };

  const handleSecondaryKeyDown = (
    event: React.KeyboardEvent<HTMLInputElement>,
  ) => {
    if (event.key === "Enter" && filteredSecondaryLanguages.length > 0) {
      handleSecondarySelect(filteredSecondaryLanguages[0].value);
    } else if (event.key === "Escape") {
      setSecondaryOpen(false);
      setSecondarySearch("");
    }
  };

  return (
    <div className="space-y-2">
      <ToggleSwitch
        checked={multilingualEnabled}
        onChange={handleToggleMultilingual}
        isUpdating={isUpdating("multilingual_mode_enabled")}
        label={t("settings.multilingual.enable.label")}
        description={t("settings.multilingual.enable.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />

      {multilingualEnabled && (
        <div className="space-y-2 ml-4 border-l-2 border-mid-gray/20 pl-4">
          {/* Primary Language Selector */}
          <SettingContainer
            title={t("settings.multilingual.primaryLanguage.title")}
            description={t("settings.multilingual.primaryLanguage.description")}
            descriptionMode={descriptionMode}
            grouped={true}
          >
            <div className="relative" ref={primaryRef}>
              <button
                type="button"
                className={`px-2 py-1 text-sm font-semibold bg-mid-gray/10 border border-mid-gray/80 rounded min-w-[160px] text-left flex items-center justify-between transition-all duration-150 ${
                  isUpdating("primary_language")
                    ? "opacity-50 cursor-not-allowed"
                    : "hover:bg-logo-primary/10 cursor-pointer hover:border-logo-primary"
                }`}
                onClick={() =>
                  !isUpdating("primary_language") &&
                  setPrimaryOpen(!primaryOpen)
                }
                disabled={isUpdating("primary_language")}
              >
                <span className="truncate">{getPrimaryLanguageName()}</span>
                <svg
                  className={`w-4 h-4 ml-2 transition-transform duration-200 ${
                    primaryOpen ? "transform rotate-180" : ""
                  }`}
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

              {primaryOpen && !isUpdating("primary_language") && (
                <div className="absolute top-full left-0 right-0 mt-1 bg-background border border-mid-gray/80 rounded shadow-lg z-50 max-h-60 overflow-hidden">
                  <div className="p-2 border-b border-mid-gray/80">
                    <input
                      ref={primarySearchRef}
                      type="text"
                      value={primarySearch}
                      onChange={(e) => setPrimarySearch(e.target.value)}
                      onKeyDown={handlePrimaryKeyDown}
                      placeholder={t(
                        "settings.general.language.searchPlaceholder",
                      )}
                      className="w-full px-2 py-1 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded focus:outline-none focus:ring-1 focus:ring-logo-primary focus:border-logo-primary"
                    />
                  </div>
                  <div className="max-h-48 overflow-y-auto">
                    {filteredPrimaryLanguages.length === 0 ? (
                      <div className="px-2 py-2 text-sm text-mid-gray text-center">
                        {t("settings.general.language.noResults")}
                      </div>
                    ) : (
                      filteredPrimaryLanguages.map((language) => (
                        <button
                          key={language.value}
                          type="button"
                          className={`w-full px-2 py-1 text-sm text-left hover:bg-logo-primary/10 transition-colors duration-150 ${
                            primaryLanguage === language.value
                              ? "bg-logo-primary/20 text-logo-primary font-semibold"
                              : ""
                          }`}
                          onClick={() => handlePrimarySelect(language.value)}
                        >
                          {language.label}
                        </button>
                      ))
                    )}
                  </div>
                </div>
              )}
            </div>
          </SettingContainer>

          {/* Secondary Language Selector */}
          <SettingContainer
            title={t("settings.multilingual.secondaryLanguage.title")}
            description={t(
              "settings.multilingual.secondaryLanguage.description",
            )}
            descriptionMode={descriptionMode}
            grouped={true}
          >
            <div className="relative" ref={secondaryRef}>
              <button
                type="button"
                className={`px-2 py-1 text-sm font-semibold bg-mid-gray/10 border border-mid-gray/80 rounded min-w-[160px] text-left flex items-center justify-between transition-all duration-150 ${
                  isUpdating("secondary_language")
                    ? "opacity-50 cursor-not-allowed"
                    : "hover:bg-logo-primary/10 cursor-pointer hover:border-logo-primary"
                }`}
                onClick={() =>
                  !isUpdating("secondary_language") &&
                  setSecondaryOpen(!secondaryOpen)
                }
                disabled={isUpdating("secondary_language")}
              >
                <span className="truncate">{getSecondaryLanguageName()}</span>
                <svg
                  className={`w-4 h-4 ml-2 transition-transform duration-200 ${
                    secondaryOpen ? "transform rotate-180" : ""
                  }`}
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

              {secondaryOpen && !isUpdating("secondary_language") && (
                <div className="absolute top-full left-0 right-0 mt-1 bg-background border border-mid-gray/80 rounded shadow-lg z-50 max-h-60 overflow-hidden">
                  <div className="p-2 border-b border-mid-gray/80">
                    <input
                      ref={secondarySearchRef}
                      type="text"
                      value={secondarySearch}
                      onChange={(e) => setSecondarySearch(e.target.value)}
                      onKeyDown={handleSecondaryKeyDown}
                      placeholder={t(
                        "settings.general.language.searchPlaceholder",
                      )}
                      className="w-full px-2 py-1 text-sm bg-mid-gray/10 border border-mid-gray/40 rounded focus:outline-none focus:ring-1 focus:ring-logo-primary focus:border-logo-primary"
                    />
                  </div>
                  <div className="max-h-48 overflow-y-auto">
                    {filteredSecondaryLanguages.length === 0 ? (
                      <div className="px-2 py-2 text-sm text-mid-gray text-center">
                        {t("settings.general.language.noResults")}
                      </div>
                    ) : (
                      filteredSecondaryLanguages.map((language) => (
                        <button
                          key={language.value}
                          type="button"
                          className={`w-full px-2 py-1 text-sm text-left hover:bg-logo-primary/10 transition-colors duration-150 ${
                            secondaryLanguage === language.value
                              ? "bg-logo-primary/20 text-logo-primary font-semibold"
                              : ""
                          }`}
                          onClick={() => handleSecondarySelect(language.value)}
                        >
                          {language.label}
                        </button>
                      ))
                    )}
                  </div>
                </div>
              )}
            </div>
          </SettingContainer>

          {/* Unsupported Model Warning */}
          <div className="px-4 py-2 text-xs text-mid-gray">
            {t("settings.multilingual.supportedModels")}
          </div>
        </div>
      )}
    </div>
  );
};
