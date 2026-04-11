/**
 * @file index.tsx
 * @module gauge-chart
 *
 * Main entry point for the GaugeChart React component.
 *
 * `GaugeChart` renders a responsive D3-powered half-circle gauge. The SVG
 * element always fills 100 % of its parent container width by default, and the
 * arc radius is computed from whichever container dimension is the bottleneck
 * (width or height / 2).
 *
 * ### Architecture
 * React state is intentionally avoided for D3 internals. Instead, all D3
 * objects (selections, generators, computed scalars) live in `RefObject`
 * containers that are mutated imperatively by the helper functions in
 * `render-chart.ts`, `draw-needle.ts`, and `utils.ts`. React lifecycle hooks
 * orchestrate *when* those helpers are called:
 *
 * | Hook | Trigger | Action |
 * |---|---|---|
 * | `useLayoutEffect` | mount / any prop change | full teardown + re-init |
 * | `useDeepCompareEffect` | arc-related props (deep) | incremental redraw |
 * | `useEffect` | window resize | layout-only redraw (no animation) |
 *
 * ### Usage
 * ```tsx
 * <GaugeChart
 *   percent={0.72}
 *   nrOfLevels={20}
 *   colors={["#5BE12C", "#F5CD19", "#EA4228"]}
 *   formatTextValue={(v) => `${v.toFixed(1)} ppm`}
 * />
 * ```
 *
 * ### External dependencies
 * | Package | Version | Symbols used |
 * |---|---|---|
 * | `react` | 19.2.5 | `useCallback`, `useEffect`, `useLayoutEffect`, `useMemo`, `useRef`, `RefObject` |
 * | `d3` | 7.9.0 | `arc`, `pie`, `select` |
 * | `lodash` | 4.18.1 | `isEqual` (via `custom-hooks.ts`) |
 * 
 * @copyright (c) 2026 Ronan LE MEILLAT, SCTG Development
 * @license SCTG Development Non-Commercial License v1.0
 */

import {
  useCallback,
  useEffect,
  useRef,
  useMemo,
  useLayoutEffect,
} from "react";
import { arc, pie, select } from "d3";
import type { RefObject } from "react";

import { setArcData } from "./utils";
import { renderChart } from "./render-chart";
import { drawNeedle } from "./draw-needle";
import useDeepCompareEffect from "./custom-hooks";
import type {
  GaugeChartProps,
  ArcDatum,
  Margin,
  SvgRef,
  GRef,
  ContainerRef,
  ArcRef,
  PieRef,
} from "./types";

// ---------------------------------------------------------------------------
// Module-level constants
// ---------------------------------------------------------------------------

/** Start angle of the half-circle arc (left extremity, negative X-axis). */
const startAngle = -Math.PI / 2;

/** End angle of the half-circle arc (right extremity, positive X-axis). */
const endAngle = Math.PI / 2;

/** Default inline style applied to the outer wrapper `<div>`. */
const defaultStyle: React.CSSProperties = { width: "100%" };

/**
 * Subset of {@link GaugeChartProps} keys whose change triggers a **needle
 * animation** rather than an instant repositioning.
 *
 * When none of these keys changed between renders the chart is redrawn without
 * animation (treated as a layout-only update).
 */
const animateNeedleProps: (keyof GaugeChartProps)[] = [
  "marginInPercent",
  "arcPadding",
  "percent",
  "nrOfLevels",
  "animDelay",
];

/**
 * Default values for the subset of props that have sensible fallbacks.
 *
 * These are spread into `initialProps` via `useMemo` so that helper functions
 * can safely use non-null assertions on these fields.
 */
const defaultProps: Required<
  Pick<
    GaugeChartProps,
    | "style"
    | "marginInPercent"
    | "cornerRadius"
    | "nrOfLevels"
    | "percent"
    | "arcPadding"
    | "arcWidth"
    | "colors"
    | "textColor"
    | "needleColor"
    | "needleBaseColor"
    | "hideText"
    | "animate"
    | "animDelay"
    | "animateDuration"
    | "needleScale"
  >
> = {
  style: defaultStyle,
  marginInPercent: 0.05,
  cornerRadius: 6,
  nrOfLevels: 3,
  percent: 0.4,
  arcPadding: 0.05,
  arcWidth: 0.2,
  colors: ["#00FF00", "#FF0000"],
  textColor: "#fff",
  needleColor: "#464A4F",
  needleBaseColor: "#464A4F",
  hideText: false,
  animate: true,
  animDelay: 500,
  animateDuration: 3000,
  needleScale: 0.55,
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/**
 * Responsive D3 half-circle gauge component.
 *
 * @param initialProps - Component props. All fields are optional; see
 *   {@link GaugeChartProps} for the full list with descriptions and defaults.
 * @returns A `<div>` containing an auto-sized `<svg>` gauge.
 */
const GaugeChart = (initialProps: GaugeChartProps) => {
  /**
   * Merged props: `defaultProps` provides fallback values; `initialProps`
   * overrides them. Wrapped in `useMemo` so helpers always receive a stable,
   * fully-resolved props object.
   */
  const props = useMemo<GaugeChartProps>(
    () => ({ ...defaultProps, ...initialProps }),
    [initialProps]
  );

  // -------------------------------------------------------------------------
  // D3 object refs
  // Initialised with empty-object casts because D3 populates them during the
  // first `initChart` call (inside `useLayoutEffect`). Using casts avoids
  // null-check boilerplate in every helper while remaining safe in practice.
  // -------------------------------------------------------------------------

  /** Root `<svg>` D3 selection. */
  const svg = useRef({}) as SvgRef;
  /** Root margin `<g>` element (offset by the margin transform). */
  const g = useRef({}) as GRef;
  /** `<g class="doughnut">` that contains the arc `<path>` elements. */
  const doughnut = useRef({}) as GRef;
  /** `<g class="needle">` that contains the needle triangle and pivot circle. */
  const needle = useRef({}) as GRef;
  /** D3 selection of the host `<div>` container. */
  const container = useRef({}) as ContainerRef;

  // -------------------------------------------------------------------------
  // Numeric layout refs (pixels, updated on every render pass)
  // -------------------------------------------------------------------------

  /** Current usable width of the chart canvas in pixels. */
  const width = useRef(0) as RefObject<number>;
  /** Current derived height of the chart canvas in pixels. */
  const height = useRef(0) as RefObject<number>;
  /** Current outer radius of the arc in pixels. */
  const outerRadius = useRef(0) as RefObject<number>;
  /** Current margin values (top / right / bottom / left) in pixels. */
  const margin = useRef<Margin>({
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
  }) as RefObject<Margin>;
  /** Number of arc segments currently displayed. */
  const nbArcsToDisplay = useRef(0) as RefObject<number>;

  // -------------------------------------------------------------------------
  // Data refs
  // -------------------------------------------------------------------------

  /** Resolved colour palette — one CSS colour string per arc level. */
  const colorArray = useRef<string[]>([]) as RefObject<string[]>;
  /** Arc datum array fed to the D3 pie generator. */
  const arcData = useRef<ArcDatum[]>([]) as RefObject<ArcDatum[]>;

  // -------------------------------------------------------------------------
  // D3 generator refs
  // -------------------------------------------------------------------------

  /**
   * D3 arc path generator.
   *
   * Typed as `ArcRef` (`Arc<any, PieArcDatum<ArcDatum>>`). The double cast
   * through `unknown` is required because `arc()` infers `DefaultArcObject`
   * for the datum type param, which does not structurally overlap with
   * `PieArcDatum<ArcDatum>` at the type level even though the latter satisfies
   * the former at runtime.
   */
  const arcChart = useRef(arc()) as unknown as ArcRef;

  /** D3 pie layout generator — converts `ArcDatum[]` into angular descriptors. */
  const pieChart = useRef(pie<ArcDatum>()) as PieRef;

  // -------------------------------------------------------------------------
  // Other refs
  // -------------------------------------------------------------------------

  /**
   * Snapshot of the props from the previous render cycle, used to:
   * - Determine whether a needle animation should play (compare `percent`).
   * - Supply `prevPercent` to the `drawNeedle` tween interpolator.
   */
  const prevProps = useRef<GaugeChartProps>(props);

  /** Ref attached to the host `<div>` so D3 can `select()` it. */
  const selectedRef = useRef<HTMLDivElement | null>(null);

  // -------------------------------------------------------------------------
  // initChart
  // -------------------------------------------------------------------------

  /**
   * Initialises or updates the chart depending on whether a prior SVG exists.
   *
   * **Full init** (`update` is falsy):
   * Removes any existing `<svg>`, creates a fresh DOM skeleton (svg → g →
   * doughnut → needle), configures the pie generator, then delegates to
   * `renderChart` and `drawNeedle`.
   *
   * **Incremental update** (`update = true`):
   * Skips DOM teardown and jumps directly to `renderChart` + `drawNeedle`.
   * The `resize` flag is forwarded to `drawNeedle` to suppress animation on
   * layout-only redraws.
   *
   * @param update - `true` to update an existing chart; `undefined` / falsy
   *   for a full initialisation.
   * @param resize - `true` when the redraw originates from a window resize
   *   (suppresses needle animation). Defaults to `false`.
   * @param prev - Props snapshot from the previous render, used to derive the
   *   animation start position. `undefined` on first render.
   */
  const initChart = useCallback(
    (
      update?: boolean,
      resize = false,
      prev?: GaugeChartProps
    ): void => {
      if (update) {
        renderChart(
          resize, prev, width, margin, height, outerRadius,
          g, doughnut, arcChart, needle, pieChart, svg, props, container, arcData
        );
        if (!props.customNeedleComponent) {
          drawNeedle(resize, prev, props, width, needle, container, outerRadius, g);
        }
        return;
      }

      // --- Full initialisation: tear down any previous SVG ---
      container.current.select("svg").remove();
      svg.current = container.current.append("svg");
      g.current = svg.current.append("g"); // Margin offset group
      doughnut.current = g.current.append("g").attr("class", "doughnut");

      // Configure the pie generator (equal-weight arcs, fixed angular range)
      pieChart.current
        .value((d) => d.value)
        .startAngle(startAngle)
        .endAngle(endAngle)
        .sort(null); // Preserve insertion order

      needle.current = g.current.append("g").attr("class", "needle");

      renderChart(
        resize, prev, width, margin, height, outerRadius,
        g, doughnut, arcChart, needle, pieChart, svg, props, container, arcData
      );
      if (!props.customNeedleComponent) {
        drawNeedle(resize, prev, props, width, needle, container, outerRadius, g);
      }
    },
    [props]
  );

  // -------------------------------------------------------------------------
  // Effects
  // -------------------------------------------------------------------------

  /**
   * **Mount / full re-init effect.**
   *
   * Runs synchronously after every DOM mutation caused by `props` or
   * `initChart` identity changes. Using `useLayoutEffect` (instead of
   * `useEffect`) prevents a flash of unsized/un-positioned SVG.
   *
   * Steps:
   * 1. Populate `arcData` from the new props.
   * 2. Bind the D3 container selection to the host `<div>`.
   * 3. Fully re-initialise the chart.
   */
  useLayoutEffect(() => {
    setArcData(props, nbArcsToDisplay, colorArray, arcData);
    container.current = select(selectedRef.current!);
    initChart();
  }, [props, initChart]);

  /**
   * **Incremental arc update effect.**
   *
   * Uses deep equality comparison so that array props (`colors`, `arcsLength`)
   * do not cause spurious re-renders when a parent re-creates them with
   * identical values.
   *
   * Decides whether to animate the needle by checking if any of the
   * `animateNeedleProps` keys changed since the last render.
   */
  useDeepCompareEffect(() => {
    if (
      props.nrOfLevels ||
      prevProps.current.arcsLength?.every((a) => props.arcsLength?.includes(a)) ||
      prevProps.current.colors?.every((a) => props.colors?.includes(a))
    ) {
      setArcData(props, nbArcsToDisplay, colorArray, arcData);
    }
    // Skip animation when only layout props changed (not value props)
    const resize = !animateNeedleProps.some(
      (key) => prevProps.current[key] !== props[key]
    );
    initChart(true, resize, prevProps.current);
    prevProps.current = props;
  }, [
    props.nrOfLevels,
    props.arcsLength,
    props.colors,
    props.percent,
    props.needleColor,
    props.needleBaseColor,
  ]);

  /**
   * **Window resize effect.**
   *
   * Attaches a `resize` event listener that redraws the chart without
   * animation whenever the window dimensions change. The listener is removed
   * on cleanup to prevent memory leaks.
   */
  useEffect(() => {
    const handleResize = (): void => {
      renderChart(
        true, prevProps.current, width, margin, height, outerRadius,
        g, doughnut, arcChart, needle, pieChart, svg, props, container, arcData
      );
      if (!props.customNeedleComponent) {
        drawNeedle(true, prevProps.current, props, width, needle, container, outerRadius, g);
      }
    };

    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props]);

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  const {
    id,
    style,
    className,
    textComponent,
    textComponentContainerClassName,
    customNeedleComponent,
    customNeedleStyle,
    customNeedleComponentClassName,
  } = props;

  return (
    <div id={id} className={className} style={style}>
      {/*
       * Host div: D3 selects this element to append the <svg>.
       * The textComponent overlay is positioned at 50 % vertical to sit at
       * the visual midpoint of the half-circle.
       */}
      <div ref={selectedRef}>
        <div
          className={textComponentContainerClassName}
          style={{ position: "relative", top: "50%" }}
        >
          {textComponent}
        </div>
      </div>

      {/* Optional custom needle rendered as a React element (not SVG) */}
      {customNeedleComponent && (
        <div
          className={customNeedleComponentClassName}
          style={{ position: "relative", ...customNeedleStyle }}
        >
          {customNeedleComponent}
        </div>
      )}
    </div>
  );
};

export default GaugeChart;
