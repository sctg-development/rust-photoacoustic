/**
 * @file draw-needle.ts
 * @module gauge-chart/draw-needle
 *
 * Renders the gauge needle inside the D3 `<g class="needle">` group.
 *
 * The needle is composed of two SVG primitives:
 *  - A **triangle** (`<path>`) whose apex points toward the current percentage
 *    position on the arc, computed by {@link calculateRotation}.
 *  - A **circle** (`<circle>`) at the pivot point that visually anchors the
 *    needle base.
 *
 * When the component is not resizing and animation is enabled, the needle
 * transitions from its previous position to the new one using D3's elastic
 * easing and a custom `tween` interpolator.
 * 
 * @copyright (c) 2026 Ronan LE MEILLAT, SCTG Development
 * @license SCTG Development Non-Commercial License v1.0
 */

import { easeElastic, interpolateNumber } from "d3";
import { calculateRotation, addText } from "./utils";
import type {
  GaugeChartProps,
  GRef,
  NumberRef,
  ContainerRef,
} from "./types";

/**
 * Clears and redraws the gauge needle for the current component state.
 *
 * ### Rendering steps
 * 1. Remove all existing children from the needle `<g>`.
 * 2. Translate the group to the arc centre `(outerRadius, outerRadius)`.
 * 3. Append the needle `<path>` at the *previous* percent position (animation
 *    will then move it to `percent`).
 * 4. Append the pivot `<circle>`.
 * 5. Optionally render the text label via {@link addText}.
 * 6. If `resize` is `false` and `animate` is `true`, start a D3 transition
 *    that interpolates the path from `prevPercent` to `percent`.
 *    Otherwise, set the final path immediately (no animation).
 *
 * @param resize - When `true`, a layout resize triggered the redraw. The
 *   needle is repositioned instantly (no animation) to avoid a jarring
 *   animation on every window-resize event.
 * @param prevProps - The component props from the previous render cycle, used
 *   to read `prevProps.percent` as the animation start position. May be
 *   `null` or `undefined` on the first render (defaults to `0`).
 * @param props - The current component props.
 * @param width - Ref holding the current usable width in pixels; used to scale
 *   the needle base radius responsively.
 * @param needle - D3 selection ref for the `<g class="needle">` group.
 * @param container - D3 selection ref for the host `<div>`; used to select
 *   `.needle path` during the tween animation.
 * @param outerRadius - Ref holding the current outer radius in pixels; used to
 *   translate the needle group and to compute the needle length.
 * @param g - D3 selection ref for the main `<g>` element; passed to
 *   {@link addText} when the text label is rendered.
 */
export const drawNeedle = (
  resize: boolean,
  prevProps: GaugeChartProps | null | undefined,
  props: GaugeChartProps,
  width: NumberRef,
  needle: GRef,
  container: ContainerRef,
  outerRadius: NumberRef,
  g: GRef
): void => {
  const {
    percent = 0.4,
    needleColor = "#464A4F",
    needleBaseColor = "#464A4F",
    hideText = false,
    animate = true,
    needleScale = 0.55,
    textComponent,
    animDelay = 500,
    animateDuration = 3000,
  } = props;

  // Needle base radius scales proportionally with the container width
  const needleRadius = 15 * (width.current / 500);
  // Pivot is placed slightly above the arc centre so the base circle looks natural
  const centerPoint: [number, number] = [0, -needleRadius / 2];

  // Clear previous needle elements before redrawing
  needle.current.selectAll("*").remove();

  // Translate the needle group so (0,0) is at the arc midpoint
  needle.current.attr(
    "transform",
    `translate(${outerRadius.current}, ${outerRadius.current})`
  );

  // Start the triangle at the previous percent position so the animation
  // travels from there to the current `percent`
  const prevPercent = prevProps?.percent ?? 0;
  const initialPath = calculateRotation(
    prevPercent || percent,
    outerRadius,
    width,
    needleScale
  );
  needle.current.append("path").attr("d", initialPath).attr("fill", needleColor);

  // Pivot circle at the base of the needle
  needle.current
    .append("circle")
    .attr("cx", centerPoint[0])
    .attr("cy", centerPoint[1])
    .attr("r", needleRadius)
    .attr("fill", needleBaseColor);

  // Render the built-in text label unless suppressed or replaced by a custom component
  if (!hideText && !textComponent) {
    addText(percent, props, outerRadius, width, g);
  }

  if (!resize && animate) {
    // Animate the needle from `prevPercent` to `percent` using elastic easing
    needle.current
      .transition()
      .delay(animDelay)
      .ease(easeElastic)
      .duration(animateDuration)
      .tween("progress", () => {
        // interpolateNumber returns a function t ∈ [0,1] → value ∈ [prev, current]
        const currentPercent = interpolateNumber(prevPercent, percent);
        return (percentOfPercent: number): void => {
          const progress = currentPercent(percentOfPercent);
          container.current
            .select(".needle path")
            .attr(
              "d",
              calculateRotation(progress, outerRadius, width, needleScale)
            );
        };
      });
  } else {
    // No animation: set the final position immediately
    container.current
      .select(".needle path")
      .attr("d", calculateRotation(percent, outerRadius, width, needleScale));
  }
};
