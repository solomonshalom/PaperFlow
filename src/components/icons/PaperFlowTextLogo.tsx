/* eslint-disable i18next/no-literal-string */
// This is a brand logo component - the brand name should not be translated
import React from "react";

interface PaperFlowTextLogoProps {
  width?: number;
  height?: number;
  className?: string;
  variant?: "brand" | "text";
}

const PaperFlowTextLogo: React.FC<PaperFlowTextLogoProps> = ({
  width,
  height,
  className,
  variant = "brand",
}) => {
  // "brand" uses pink logo-primary color, "text" uses currentColor (white/black based on theme)
  const fillClass = variant === "brand" ? "logo-primary" : "logo-text";

  return (
    <svg
      width={width}
      height={height}
      className={className}
      viewBox="0 0 1000 300"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <text
        x="500"
        y="165"
        dominantBaseline="middle"
        textAnchor="middle"
        fontFamily="CutiePatootie, sans-serif"
        fontWeight="400"
        fontSize="240"
        className="logo-stroke"
        stroke="currentColor"
        strokeWidth="10"
        fill="none"
        paintOrder="stroke"
      >
        PaperFlow
      </text>
      <text
        x="500"
        y="165"
        dominantBaseline="middle"
        textAnchor="middle"
        fontFamily="CutiePatootie, sans-serif"
        fontWeight="400"
        fontSize="240"
        className={fillClass}
      >
        PaperFlow
      </text>
    </svg>
  );
};

export default PaperFlowTextLogo;
