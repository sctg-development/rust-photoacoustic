declare module "react-gauge-chart" {
    import { CSSProperties, FC } from "react";

    interface GaugeChartProps {
        id?: string;
        className?: string;
        style?: CSSProperties;
        marginInPercent?: number;
        cornerRadius?: number;
        nrOfLevels?: number;
        percent?: number;
        arcPadding?: number;
        arcWidth?: number;
        colors?: string[];
        textColor?: string;
        needleColor?: string;
        needleBaseColor?: string;
        hideText?: boolean;
        animate?: boolean;
        animateDuration?: number;
        animateDelay?: number;
        formatTextValue?: (value: number) => string;
    }

    const GaugeChart: FC<GaugeChartProps>;
    export default GaugeChart;
}
