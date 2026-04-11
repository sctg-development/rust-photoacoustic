/**
 * @file utils.ts
 * @module gauge-chart/utils
 *
 * Pure utility functions shared by the GaugeChart rendering pipeline.
 *
 * Responsibilities:
 *  - **Layout** – compute the outer radius, centre the graph inside its margins,
 *    and synchronise the margin ref with the current container dimensions
 *    (`calculateRadius`, `centerGraph`, `updateDimensions`).
 *  - **Needle geometry** – build the SVG path string that positions the needle
 *    at a given percentage angle (`calculateRotation`).
 *  - **Text label** – append a D3 `<text>` element below the gauge arc
 *    (`addText`).
 *  - **Arc data** – populate the arc datum array and colour palette used by the
 *    D3 pie generator (`setArcData`).
 *
 * All functions operate on `RefObject` containers (mutating `.current`) rather
 * than returning new values, so D3 selections and dimension scalars stay in
 * sync across the component's effect chain without triggering React re-renders.
 * 
 * @copyright (c) 2026 Ronan LE MEILLAT, SCTG Development
 * @license SCTG Development Non-Commercial License v1.0
 */

import { scaleLinear, interpolateHsl } from "d3";
import type {
  ArcDatum,
  GaugeChartProps,
  GRef,
  MarginRef,
  NumberRef,
  ContainerRef,
  ColorArrayRef,
  ArcDataRef,
} from "./types";

// ---------------------------------------------------------------------------
// Layout helpers
// ---------------------------------------------------------------------------

/**
 * Recalculates the outer radius of the gauge arc based on the current
 * container dimensions and updates the `outerRadius` ref in place.
 *
 * The gauge is a **half-circle**, so the usable height is half the usable
 * width. The radius is constrained by whichever dimension is the bottleneck:
 * - If `width < 2 × height`, the arc is width-limited: `r = (width − margins) / 2`.
 * - Otherwise it is height-limited: `r = height − margins`.
 *
 * After updating `outerRadius`, {@link centerGraph} is called to reposition
 * the main `<g>` translation.
 *
 * @param width - Ref holding the current usable width in pixels.
 * @param height - Ref holding the current usable height in pixels.
 * @param outerRadius - Ref that will receive the new outer radius value.
 * @param margin - Ref holding the current margin object.
 * @param g - D3 selection ref for the main `<g>` element (passed to {@link centerGraph}).
 */
export const calculateRadius = (
  width: NumberRef,
  height: NumberRef,
  outerRadius: NumberRef,
  margin: MarginRef,
  g: GRef
): void => {
  if (width.current < 2 * height.current) {
    // Width is the limiting dimension
    outerRadius.current =
      (width.current - margin.current.left - margin.current.right) / 2;
  } else {
    // Height is the limiting dimension
    outerRadius.current =
      height.current - margin.current.top - margin.current.bottom;
  }
  centerGraph(width, g, outerRadius, margin);
};

/**
 * Adjusts the left margin so the half-circle arc is horizontally centred
 * inside the container, then applies the resulting translation to the main
 * `<g>` element.
 *
 * The formula derives the left margin from:
 * `leftMargin = containerWidth/2 − outerRadius + rightMargin`
 *
 * This accounts for the right margin so that both sides of the arc are
 * equidistant from the container edges.
 *
 * @param width - Ref holding the current usable width in pixels.
 * @param g - D3 selection ref for the main `<g>` element to transform.
 * @param outerRadius - Ref holding the current outer radius in pixels.
 * @param margin - Ref holding the margin object; `left` will be mutated.
 */
export const centerGraph = (
  width: NumberRef,
  g: GRef,
  outerRadius: NumberRef,
  margin: MarginRef
): void => {
  margin.current.left =
    width.current / 2 - outerRadius.current + margin.current.right;
  g.current.attr(
    "transform",
    `translate(${margin.current.left}, ${margin.current.top})`
  );
};

/**
 * Reads the live bounding box of the host container `<div>` and updates the
 * `margin`, `width`, and `height` refs to reflect the new dimensions.
 *
 * Called at the start of every render pass so that all subsequent calculations
 * operate on fresh layout values. The height is derived from the usable width
 * (divided by two) to guarantee the half-circle always fits without overflow.
 *
 * @param props - Current component props; `marginInPercent` is read here.
 * @param container - Ref holding the D3 selection of the host `<div>`.
 * @param margin - Ref whose `top`, `right`, `bottom`, `left` fields will be updated.
 * @param width - Ref that will receive the new usable width in pixels.
 * @param height - Ref that will receive the new derived height in pixels.
 */
export const updateDimensions = (
  props: GaugeChartProps,
  container: ContainerRef,
  margin: MarginRef,
  width: NumberRef,
  height: NumberRef
): void => {
  const marginInPercent = props.marginInPercent ?? 0.05;
  const node = container.current.node();
  if (!node) return;
  const { width: divWidth, height: divHeight } = node.getBoundingClientRect();

  margin.current.left = divWidth * marginInPercent;
  margin.current.right = divWidth * marginInPercent;
  width.current = divWidth - margin.current.left - margin.current.right;

  margin.current.top = divHeight * marginInPercent;
  margin.current.bottom = divHeight * marginInPercent;
  // Derive height from width so the arc always fits inside a half-circle
  height.current =
    width.current / 2 - margin.current.top - margin.current.bottom;
};

// ---------------------------------------------------------------------------
// Needle geometry
// ---------------------------------------------------------------------------

/**
 * Computes the SVG `d` attribute string for the needle triangle at a given
 * percentage position.
 *
 * The needle is represented as an isoceles triangle whose apex points along
 * the radius direction corresponding to `percent`. Its three vertices are:
 * - **top** – the apex, at `needleLength` from the pivot.
 * - **left** – one base corner, perpendicular to the needle direction.
 * - **right** – the other base corner, on the opposite side.
 *
 * The pivot is located at `(0, −needleRadius/2)` in the local coordinate
 * system (shifted slightly above the centre so the base circle looks natural).
 *
 * @param percent - Gauge value in `[0, 1]`. `0` points to the left of the arc
 *   and `1` to the right (`−π/2` to `+π/2` in radians).
 * @param outerRadius - Ref holding the outer radius; determines needle length.
 * @param width - Ref holding the usable width; determines needle base radius.
 * @param needleScale - Fraction of `outerRadius` used as the needle length.
 * @returns An SVG path `d` string of the form `"M … L … L …"`.
 */
export const calculateRotation = (
  percent: number,
  outerRadius: NumberRef,
  width: NumberRef,
  needleScale: number
): string => {
  const needleLength = outerRadius.current * needleScale;
  const needleRadius = 15 * (width.current / 500); // Scales with container width
  const theta = percentToRad(percent);
  const centerPoint: [number, number] = [0, -needleRadius / 2];
  const topPoint: [number, number] = [
    centerPoint[0] - needleLength * Math.cos(theta),
    centerPoint[1] - needleLength * Math.sin(theta),
  ];
  const leftPoint: [number, number] = [
    centerPoint[0] - needleRadius * Math.cos(theta - Math.PI / 2),
    centerPoint[1] - needleRadius * Math.sin(theta - Math.PI / 2),
  ];
  const rightPoint: [number, number] = [
    centerPoint[0] - needleRadius * Math.cos(theta + Math.PI / 2),
    centerPoint[1] - needleRadius * Math.sin(theta + Math.PI / 2),
  ];
  return `M ${leftPoint[0]} ${leftPoint[1]} L ${topPoint[0]} ${topPoint[1]} L ${rightPoint[0]} ${rightPoint[1]}`;
};

// ---------------------------------------------------------------------------
// Text label
// ---------------------------------------------------------------------------

/**
 * Appends a `<text>` element below the gauge arc that displays the current
 * percentage value.
 *
 * The text is positioned at `(outerRadius, outerRadius/2 + padding)` in the
 * local `<g>` coordinate system, which places it just beneath the centre of
 * the half-circle.  Font size is computed automatically to avoid overflow for
 * long formatted strings unless `fontSize` is explicitly provided.
 *
 * @param percentage - Raw gauge value in `[0, 1]`. It is converted to a
 *   display number via {@link floatingNumber} before formatting.
 * @param props - Component props; `formatTextValue`, `fontSize`, and
 *   `textColor` are consumed.
 * @param outerRadius - Ref holding the current outer radius in pixels.
 * @param width - Ref holding the current usable width; used for auto font sizing.
 * @param g - D3 selection ref for the main `<g>` element where the text group
 *   will be appended.
 */
export const addText = (
  percentage: number,
  props: GaugeChartProps,
  outerRadius: NumberRef,
  width: NumberRef,
  g: GRef
): void => {
  const { formatTextValue, fontSize, textColor = "#fff" } = props;
  const textPadding = 20;
  const text = formatTextValue
    ? formatTextValue(floatingNumber(percentage))
    : `${floatingNumber(percentage)}%`;

  g.current
    .append("g")
    .attr("class", "text-group")
    .attr(
      "transform",
      `translate(${outerRadius.current}, ${outerRadius.current / 2 + textPadding})`
    )
    .append("text")
    .text(text)
    .style(
      "font-size",
      // Shrink font proportionally when the formatted string exceeds 10 chars
      fontSize
        ? fontSize
        : `${width.current / 11 / (text.length > 10 ? text.length / 10 : 1)}px`
    )
    .style("fill", textColor)
    .style("text-anchor", "middle");
};

// ---------------------------------------------------------------------------
// Arc data
// ---------------------------------------------------------------------------

/**
 * Populates the `nbArcsToDisplay`, `colorArray`, and `arcData` refs from the
 * current component props.
 *
 * This function is called on mount and whenever arc-related props change
 * (`nrOfLevels`, `arcsLength`, `colors`). It determines:
 * 1. **How many arcs** to draw – `arcsLength.length` when provided, otherwise
 *    `nrOfLevels`.
 * 2. **Which colours** to use – the `colors` array directly when its length
 *    matches the arc count, or an HSL-interpolated palette otherwise.
 * 3. **The datum array** – one `{ value, color }` entry per arc, fed to the
 *    D3 pie generator.
 *
 * @param props - Current component props.
 * @param nbArcsToDisplay - Ref that will receive the total arc count.
 * @param colorArray - Ref that will receive the resolved colour palette.
 * @param arcData - Ref that will receive the `ArcDatum[]` array.
 */
export const setArcData = (
  props: GaugeChartProps,
  nbArcsToDisplay: NumberRef,
  colorArray: ColorArrayRef,
  arcData: ArcDataRef
): void => {
  nbArcsToDisplay.current = props.arcsLength
    ? props.arcsLength.length
    : (props.nrOfLevels ?? 3);

  const colors = props.colors ?? ["#00FF00", "#FF0000"];
  colorArray.current =
    nbArcsToDisplay.current === colors.length
      ? colors
      : getColors(colors, nbArcsToDisplay);

  arcData.current = [];
  for (let i = 0; i < nbArcsToDisplay.current; i++) {
    const arcDatum: ArcDatum = {
      value:
        props.arcsLength && props.arcsLength.length > i
          ? props.arcsLength[i]
          : 1,
      color: colorArray.current[i],
    };
    arcData.current.push(arcDatum);
  }
};

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/**
 * Generates a palette of `n` colours by linearly interpolating between the
 * first and last entries of `colors` in the HSL colour space.
 *
 * The interpolation is performed by D3's `scaleLinear` with `interpolateHsl`,
 * which produces perceptually smooth gradients (e.g. green → yellow → red).
 *
 * @param colors - Source colour array; only the first and last entries are
 *   used as the interpolation endpoints.
 * @param nbArcsToDisplay - Ref holding the target palette size `n`.
 * @returns An array of `n` CSS colour strings.
 */
const getColors = (
  colors: string[],
  nbArcsToDisplay: NumberRef
): string[] => {
  const colorScale = scaleLinear<string>()
    .domain([1, nbArcsToDisplay.current])
    .range([colors[0], colors[colors.length - 1]])
    .interpolate(interpolateHsl);

  const result: string[] = [];
  for (let i = 1; i <= nbArcsToDisplay.current; i++) {
    result.push(colorScale(i));
  }
  return result;
};

/**
 * Rounds a gauge value (in `[0, 1]`) to a display number with up to
 * `maxDigits` decimal places, expressed as a percentage.
 *
 * @example
 * floatingNumber(0.4257) // → 42.57
 * floatingNumber(0.4257, 0) // → 43
 *
 * @param value - Gauge fraction in `[0, 1]`.
 * @param maxDigits - Maximum decimal digits in the result. Defaults to `2`.
 * @returns The percentage value rounded to `maxDigits` decimal places.
 */
const floatingNumber = (value: number, maxDigits = 2): number =>
  Math.round(value * 100 * 10 ** maxDigits) / 10 ** maxDigits;

/**
 * Converts a gauge percentage fraction to a radian angle on the half-circle.
 *
 * The gauge spans from `−π/2` (left) to `+π/2` (right), mapping `percent = 0`
 * to the left end and `percent = 1` to the right end. Since the full arc
 * covers `π` radians, the conversion is simply `percent × π`.
 *
 * @param percent - Gauge value in `[0, 1]`.
 * @returns Angle in radians in `[0, π]`.
 */
const percentToRad = (percent: number): number => percent * Math.PI;
