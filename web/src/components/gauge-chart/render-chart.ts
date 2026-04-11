/**
 * @file render-chart.ts
 * @module gauge-chart/render-chart
 *
 * Handles the SVG layout and arc rendering pipeline for the GaugeChart.
 *
 * `renderChart` is the central function called on every render pass — whether
 * triggered by a prop change, a window resize, or the initial mount. It is
 * intentionally stateless: all mutable state lives in `RefObject` containers
 * passed as arguments, so the function can be called from different React
 * effects without closure conflicts.
 *
 * ### Rendering pipeline
 * 1. **Dimensions** – sync margins, width, and height with the host container.
 * 2. **SVG canvas** – resize the `<svg>` element and reposition the root `<g>`.
 * 3. **Radius** – recalculate the outer radius and re-centre the arc group.
 * 4. **Arc generator** – update `outerRadius`, `innerRadius`, `cornerRadius`,
 *    and `padAngle` on the D3 arc generator.
 * 5. **Segments** – clear stale arc and text elements, then draw fresh `<path>`
 *    elements from the pie-computed data.
 * 
 * @copyright (c) 2026 Ronan LE MEILLAT, SCTG Development
 * @license SCTG Development Non-Commercial License v1.0
 */

import type { PieArcDatum } from "d3";
import { updateDimensions, calculateRadius } from "./utils";
import type {
  ArcDatum,
  GaugeChartProps,
  SvgRef,
  GRef,
  ContainerRef,
  ArcRef,
  PieRef,
  NumberRef,
  MarginRef,
  ArcDataRef,
} from "./types";

/**
 * Redraws the gauge arc segments for the current component state.
 *
 * This function is pure with respect to React state — it reads from and writes
 * to `RefObject` containers only, never touching React state setters. It is
 * therefore safe to call from `useLayoutEffect`, `useEffect`, and imperative
 * event handlers.
 *
 * @param _resize - Whether the call originates from a window resize event.
 *   Currently unused inside this function but kept in the signature for API
 *   consistency with {@link drawNeedle}, which uses it to suppress animation.
 * @param _prevProps - Props from the previous render cycle. Currently unused
 *   inside this function; retained for API consistency.
 * @param width - Ref holding the current usable width in pixels.
 * @param margin - Ref holding the current margin object (mutated by
 *   {@link updateDimensions} and {@link centerGraph}).
 * @param height - Ref holding the current derived height in pixels.
 * @param outerRadius - Ref holding the current outer radius in pixels (mutated
 *   by {@link calculateRadius}).
 * @param g - D3 selection ref for the main margins `<g>` element.
 * @param doughnut - D3 selection ref for the `<g class="doughnut">` arc group.
 * @param arcChart - Ref holding the D3 arc path generator (its parameters are
 *   updated in place).
 * @param _needle - D3 selection ref for the `<g class="needle">` group.
 *   Currently unused here; the needle is redrawn separately by
 *   {@link drawNeedle}.
 * @param pieChart - Ref holding the D3 pie layout generator; converts
 *   `ArcDatum[]` into angular `PieArcDatum[]`.
 * @param svg - Ref holding the root `<svg>` D3 selection (resized here).
 * @param props - Current component props (`arcWidth`, `cornerRadius`,
 *   `arcPadding` are consumed).
 * @param container - Ref holding the D3 selection of the host `<div>`;
 *   passed to {@link updateDimensions}.
 * @param arcData - Ref holding the `ArcDatum[]` array fed to the pie generator.
 */
export const renderChart = (
  _resize: boolean,
  _prevProps: GaugeChartProps | null | undefined,
  width: NumberRef,
  margin: MarginRef,
  height: NumberRef,
  outerRadius: NumberRef,
  g: GRef,
  doughnut: GRef,
  arcChart: ArcRef,
  _needle: GRef,
  pieChart: PieRef,
  svg: SvgRef,
  props: GaugeChartProps,
  container: ContainerRef,
  arcData: ArcDataRef
): void => {
  // 1. Sync layout dimensions with the host container's bounding box
  updateDimensions(props, container, margin, width, height);

  // 2. Resize the SVG canvas to fit the new dimensions (including margins)
  svg.current
    .attr("width", width.current + margin.current.left + margin.current.right)
    .attr(
      "height",
      height.current + margin.current.top + margin.current.bottom
    );

  // Apply the top-left margin offset to the root <g>
  g.current.attr(
    "transform",
    `translate(${margin.current.left}, ${margin.current.top})`
  );

  // 3. Recalculate the outer radius and re-centre the arc group
  calculateRadius(width, height, outerRadius, margin, g);
  doughnut.current.attr(
    "transform",
    `translate(${outerRadius.current}, ${outerRadius.current})`
  );

  // 4. Reconfigure the D3 arc generator with updated geometry
  arcChart.current
    .outerRadius(outerRadius.current)
    .innerRadius(outerRadius.current * (1 - (props.arcWidth ?? 0.2)))
    .cornerRadius(props.cornerRadius ?? 6)
    .padAngle(props.arcPadding ?? 0.05);

  // 5. Remove stale elements before drawing the new pass
  doughnut.current.selectAll(".arc").remove();
  g.current.selectAll(".text-group").remove();

  // Bind pie-computed data to <g class="arc"> elements (enter selection)
  const arcPaths = doughnut.current
    .selectAll<SVGGElement, PieArcDatum<ArcDatum>>(".arc")
    .data(pieChart.current(arcData.current))
    .enter()
    .append("g")
    .attr("class", "arc");

  // Draw one <path> per segment, filled with the datum's resolved colour
  arcPaths
    .append("path")
    .attr("d", arcChart.current)
    .style("fill", (d) => d.data.color);
};
