/* eslint-disable i18next/no-literal-string */
// This is a brand logo component - the brand name should not be translated
import React from "react";

const PaperFlowTextLogo = ({
  width,
  height,
  className,
}: {
  width?: number;
  height?: number;
  className?: string;
}) => {
  return (
    <svg
      width={width}
      height={height}
      className={className}
      viewBox="0 0 1000 300"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <defs>
        <filter id="glow" x="-20%" y="-20%" width="140%" height="140%">
          <feGaussianBlur stdDeviation="2" result="blur1" />
          <feGaussianBlur stdDeviation="4" result="blur2" />
          <feMerge>
            <feMergeNode in="blur2" />
            <feMergeNode in="blur1" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>
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
        filter="url(#glow)"
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
        className="logo-primary"
        filter="url(#glow)"
      >
        PaperFlow
      </text>
    </svg>
  );
};

export default PaperFlowTextLogo;
