import { SVGProps } from "react";

export type IconSvgProps = SVGProps<SVGSVGElement> & {
  size?: number;
};

// Re-export audio streaming types
export * from "./audio-streaming";
export * from "./computing";
export * from "./processing-graph";
export * from "./thermal";
