/**
 * @file custom-hooks.ts
 * @module gauge-chart/custom-hooks
 *
 * Provides `useDeepCompareEffect`, a drop-in replacement for React's built-in
 * `useEffect` that compares dependency arrays **by value** (deep equality via
 * `lodash/isEqual`) instead of by reference.
 *
 * This is necessary for the GaugeChart component because several props are
 * arrays (`colors`, `arcsLength`). A parent that recreates those arrays on
 * every render would cause an infinite re-render loop with plain `useEffect`,
 * but `useDeepCompareEffect` skips the effect when the contents are unchanged.
 * 
 * @copyright (c) 2026 Ronan LE MEILLAT, SCTG Development
 * @license SCTG Development Non-Commercial License v1.0
 */

import isEqual from "lodash/isEqual";
import { useEffect, useRef } from "react";
import type { DependencyList, EffectCallback } from "react";

/**
 * Returns `true` when `toCompare` and `reference` are deeply equal according
 * to `lodash/isEqual`.
 *
 * Accepts `null` for `reference` because the internal ref starts as `null`
 * before the first comparison.
 *
 * @param toCompare - The new dependency list to evaluate.
 * @param reference - The previously stored dependency list (or `null` on the
 *   first call).
 * @returns `true` if both arrays are structurally identical.
 */
const isDeepEquals = (
  toCompare: DependencyList | null,
  reference: DependencyList | null
): boolean => isEqual(toCompare, reference);

/**
 * Stabilises a dependency list reference across renders.
 *
 * The ref is updated **only** when the new `dependencies` differ from the
 * stored snapshot (deep comparison). Returning the same object reference when
 * contents are unchanged prevents React from re-running effects that depend on
 * this value.
 *
 * @param dependencies - The dependency list to stabilise.
 * @returns A stable `DependencyList` reference, or `null` before the first call.
 */
const useDeepCompareMemo = (
  dependencies: DependencyList
): DependencyList | null => {
  const ref = useRef<DependencyList | null>(null);
  if (!isDeepEquals(dependencies, ref.current)) {
    ref.current = dependencies;
  }
  return ref.current;
};

/**
 * A `useEffect` variant that performs **deep equality** checks on the
 * dependency list instead of the default reference equality.
 *
 * Internally, it wraps the dependency list with {@link useDeepCompareMemo} so
 * that React only schedules the effect when the *contents* of the list change,
 * not merely its object identity.
 *
 * @param callback - The effect callback, identical in contract to the first
 *   argument of `useEffect`. It may return a cleanup function.
 * @param dependencies - An array of values the effect depends on. Elements are
 *   compared deeply using `lodash/isEqual`.
 *
 * @example
 * ```tsx
 * useDeepCompareEffect(() => {
 *   redrawArcs(props);
 * }, [props.colors, props.arcsLength]);
 * ```
 */
const useDeepCompareEffect = (
  callback: EffectCallback,
  dependencies: DependencyList
): void => {
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(callback, [useDeepCompareMemo(dependencies), callback]);
};

export default useDeepCompareEffect;
