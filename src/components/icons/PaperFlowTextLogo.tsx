/* eslint-disable i18next/no-literal-string */
// This is a brand logo component - the brand name should not be translated
import darkLogo from "../../assets/dark-logo.png";

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
    <img
      src={darkLogo}
      alt="PaperFlow"
      width={width}
      height={height}
      className={className}
      style={{ objectFit: "contain" }}
    />
  );
};

export default PaperFlowTextLogo;
