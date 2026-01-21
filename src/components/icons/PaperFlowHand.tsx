import darkLogo from "../../assets/dark-logo.png";

const PaperFlowHand = ({
  width,
  height,
  className,
}: {
  width?: number | string;
  height?: number | string;
  className?: string;
}) => (
  <img
    src={darkLogo}
    alt="PaperFlow"
    width={width || 126}
    height={height || 135}
    className={className}
    style={{ objectFit: "contain" }}
  />
);

export default PaperFlowHand;
