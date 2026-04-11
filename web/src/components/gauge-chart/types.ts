/**
 * @file types.ts
 * @module gauge-chart/types
 *
 * Shared TypeScript types used across the GaugeChart component and its helpers.
 *
 * Three layers are defined here:
 *  1. **Domain types** – plain data shapes (`ArcDatum`, `Margin`).
 *  2. **D3 aliases** – concrete D3 Selection / generator types with the generic
 *     parameters pinned to the values the chart actually uses.
 *  3. **React ref wrappers** – `RefObject<T>` aliases passed between the
 *     component and its pure-function helpers so every file shares the same
 *     ref shapes without repeating the generic noise.
 *  4. **Component props** – the public API of `<GaugeChart />`, replacing the
 *     legacy `PropTypes` declarations.
 * 
 * @copyright (c) 2026 Ronan LE MEILLAT, SCTG Development
 * @license SCTG Development Non-Commercial License v1.0
 */

import type { RefObject, CSSProperties, ReactElement } from "react";
import type { Selection, Arc, Pie, PieArcDatum } from "d3";

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/**
 * Data record for a single arc segment of the gauge.
 *
 * @property value - Relative weight of the segment. All arc values are summed
 *   by the D3 pie generator to compute each segment's angular size. When
 *   `arcsLength` is not provided every segment uses `value = 1`, making them
 *   equal.
 * @property color - CSS color string applied as the fill of the arc path.
 */
export interface ArcDatum {
  value: number;
  color: string;
}

/**
 * SVG layout margins in pixels, measured from each edge of the container div.
 *
 * Margins are recomputed on every render via {@link updateDimensions} as a
 * percentage of the container dimensions (`marginInPercent`).
 */
export interface Margin {
  /** Top margin in pixels. */
  top: number;
  /** Right margin in pixels. */
  right: number;
  /** Bottom margin in pixels. */
  bottom: number;
  /** Left margin in pixels (also adjusted by {@link centerGraph} to keep the arc centred). */
  left: number;
}

// ---------------------------------------------------------------------------
// D3 selection / generator aliases
// (parent / datum type params use `any` where D3 inference is too narrow)
// ---------------------------------------------------------------------------

/**
 * D3 selection wrapping the root `<svg>` element of the chart.
 */
export type SvgSelection = Selection<SVGSVGElement, unknown, null, undefined>;

/**
 * D3 selection wrapping a `<g>` element.
 *
 * Parent and datum generic params are `any` because D3 selections gain
 * different parent types when created via `.append()` chains, and keeping
 * a single alias avoids verbose per-call casting.
 */
export type GSelection = Selection<SVGGElement, unknown, any, any>;

/**
 * D3 selection wrapping the host `<div>` that contains the chart SVG.
 */
export type ContainerSelection = Selection<HTMLDivElement, unknown, null, undefined>;

/**
 * D3 arc path generator configured to produce paths from `PieArcDatum<ArcDatum>`.
 *
 * The `any` context param is intentional: D3's `Arc` is invoked as a plain
 * function (not as a method), so the `this` context is irrelevant.
 */
export type ArcGenerator = Arc<any, PieArcDatum<ArcDatum>>;

/**
 * D3 pie layout generator that converts `ArcDatum[]` into `PieArcDatum[]`
 * (i.e., annotates each datum with `startAngle` / `endAngle`).
 */
export type PieGenerator = Pie<any, ArcDatum>;

// ---------------------------------------------------------------------------
// React ref wrappers passed between the component and helper functions
// ---------------------------------------------------------------------------

/** Ref holding the root `<svg>` D3 selection. */
export type SvgRef = RefObject<SvgSelection>;

/** Ref holding any `<g>` D3 selection (margins group, doughnut group, needle group…). */
export type GRef = RefObject<GSelection>;

/** Ref holding the D3 selection of the host container `<div>`. */
export type ContainerRef = RefObject<ContainerSelection>;

/** Ref holding the D3 arc path generator. */
export type ArcRef = RefObject<ArcGenerator>;

/** Ref holding the D3 pie layout generator. */
export type PieRef = RefObject<PieGenerator>;

/** Ref holding a mutable `number` value (width, height, outerRadius…). */
export type NumberRef = RefObject<number>;

/** Ref holding the current {@link Margin} object. */
export type MarginRef = RefObject<Margin>;

/** Ref holding the computed colour palette for each arc level. */
export type ColorArrayRef = RefObject<string[]>;

/** Ref holding the arc data array passed to the D3 pie generator. */
export type ArcDataRef = RefObject<ArcDatum[]>;

// ---------------------------------------------------------------------------
// Component props (replaces PropTypes)
// ---------------------------------------------------------------------------

/**
 * Public props accepted by the `<GaugeChart />` component.
 *
 * All properties are optional; sensible defaults are applied inside the
 * component via `defaultProps`.
 */
export interface GaugeChartProps {
  /** HTML `id` attribute applied to the outer `<div>` wrapper. */
  id?: string;

  /** CSS class name(s) applied to the outer `<div>` wrapper. */
  className?: string;

  /**
   * Inline styles applied to the outer `<div>` wrapper.
   * @defaultValue `{ width: "100%" }`
   */
  style?: CSSProperties;

  /**
   * Fraction of the container width/height used as margin on each side.
   * @defaultValue `0.05` (5 %)
   */
  marginInPercent?: number;

  /**
   * Corner radius of each arc segment in pixels.
   * @defaultValue `6`
   */
  cornerRadius?: number;

  /**
   * Number of evenly-spaced colour levels when `arcsLength` is not provided.
   * @defaultValue `3`
   */
  nrOfLevels?: number;

  /**
   * Current gauge value expressed as a fraction between `0` and `1`.
   * @defaultValue `0.4`
   */
  percent?: number;

  /**
   * Padding between adjacent arc segments, in radians.
   * @defaultValue `0.05`
   */
  arcPadding?: number;

  /**
   * Width of the arc band expressed as a fraction of the outer radius.
   * @defaultValue `0.2` (20 % of the radius)
   */
  arcWidth?: number;

  /**
   * Custom arc lengths as an array of relative weights, one per level.
   * When provided, its length overrides `nrOfLevels`.
   */
  arcsLength?: number[];

  /**
   * Colour stops for the arc gradient interpolation.
   * The first and last entries are used as the range of a linear HSL scale
   * unless the number of colours exactly matches `nrOfLevels`.
   * @defaultValue `["#00FF00", "#FF0000"]`
   */
  colors?: string[];

  /**
   * Fill colour of the percentage text label.
   * @defaultValue `"#fff"`
   */
  textColor?: string;

  /**
   * Fill colour of the needle triangle.
   * @defaultValue `"#464A4F"`
   */
  needleColor?: string;

  /**
   * Fill colour of the circular base of the needle.
   * @defaultValue `"#464A4F"`
   */
  needleBaseColor?: string;

  /**
   * When `true`, the built-in percentage text label is not rendered.
   * @defaultValue `false`
   */
  hideText?: boolean;

  /**
   * When `true`, the needle animates from its previous to its new position
   * whenever `percent` changes.
   * @defaultValue `true`
   */
  animate?: boolean;

  /**
   * Custom formatter for the text label. Receives the percentage value
   * multiplied by 100 (e.g. `42.5` for `percent = 0.425`) and must return a
   * display string. When `null` the default `"<value>%"` format is used.
   */
  formatTextValue?: ((value: number) => string) | null;

  /**
   * Explicit CSS `font-size` for the text label (e.g. `"16px"`, `"1rem"`).
   * When `null` the size is computed automatically from the gauge width.
   */
  fontSize?: string | null;

  /**
   * Duration of the needle entrance animation in milliseconds.
   * @defaultValue `3000`
   */
  animateDuration?: number;

  /**
   * Delay before the needle animation starts, in milliseconds.
   * @defaultValue `500`
   */
  animDelay?: number;

  /**
   * Optional React element rendered inside the gauge (e.g. a custom label).
   * When provided the built-in text label is suppressed regardless of
   * `hideText`.
   */
  textComponent?: ReactElement;

  /**
   * CSS class name applied to the container `<div>` wrapping `textComponent`.
   */
  textComponentContainerClassName?: string;

  /**
   * Length of the needle as a fraction of the outer radius.
   * @defaultValue `0.55`
   */
  needleScale?: number;

  /**
   * Optional React element rendered in place of the D3-drawn needle.
   * When provided the SVG needle path is not drawn.
   */
  customNeedleComponent?: ReactElement | null;

  /**
   * CSS class name applied to the container `<div>` wrapping
   * `customNeedleComponent`.
   */
  customNeedleComponentClassName?: string;

  /**
   * Additional inline styles merged onto the container `<div>` wrapping
   * `customNeedleComponent`.
   */
  customNeedleStyle?: CSSProperties;
}
