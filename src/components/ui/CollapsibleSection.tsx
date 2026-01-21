import React, { useState, useEffect, useCallback } from "react";
import { ChevronDown } from "lucide-react";

interface CollapsibleSectionProps {
  id: string;
  title: string;
  defaultExpanded?: boolean;
  children: React.ReactNode;
}

const STORAGE_KEY_PREFIX = "collapsible-section-";

export const CollapsibleSection: React.FC<CollapsibleSectionProps> = ({
  id,
  title,
  defaultExpanded = false,
  children,
}) => {
  const [isExpanded, setIsExpanded] = useState<boolean>(() => {
    try {
      const stored = localStorage.getItem(`${STORAGE_KEY_PREFIX}${id}`);
      return stored !== null ? stored === "true" : defaultExpanded;
    } catch {
      return defaultExpanded;
    }
  });

  const handleToggle = useCallback(() => {
    setIsExpanded((prev) => {
      const newValue = !prev;
      try {
        localStorage.setItem(`${STORAGE_KEY_PREFIX}${id}`, String(newValue));
      } catch {
        // Ignore localStorage errors
      }
      return newValue;
    });
  }, [id]);

  // Sync with localStorage on mount if defaultExpanded changes
  useEffect(() => {
    try {
      const stored = localStorage.getItem(`${STORAGE_KEY_PREFIX}${id}`);
      if (stored === null) {
        setIsExpanded(defaultExpanded);
      }
    } catch {
      // Ignore localStorage errors
    }
  }, [id, defaultExpanded]);

  return (
    <div className="border border-mid-gray/20 rounded-lg overflow-hidden">
      <button
        type="button"
        onClick={handleToggle}
        className="w-full flex items-center justify-between px-4 py-3 hover:bg-mid-gray/5 transition-colors cursor-pointer"
      >
        <span className="text-sm font-medium">{title}</span>
        <ChevronDown
          size={18}
          className={`text-mid-gray transition-transform duration-200 ${
            isExpanded ? "rotate-0" : "-rotate-90"
          }`}
        />
      </button>
      <div
        className="grid transition-[grid-template-rows] duration-200 ease-out"
        style={{
          gridTemplateRows: isExpanded ? "1fr" : "0fr",
        }}
      >
        <div className="overflow-hidden">
          <div className="border-t border-mid-gray/20">
            <div className="divide-y divide-mid-gray/20">{children}</div>
          </div>
        </div>
      </div>
    </div>
  );
};
