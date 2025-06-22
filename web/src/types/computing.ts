// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

/**
 * Peak result data from a computing node
 *
 * Contains frequency, amplitude, and optional concentration data
 * from photoacoustic signal analysis.
 */
export interface PeakResultResponse {
  /** Peak frequency in Hz */
  frequency: number;

  /** Peak amplitude */
  amplitude: number;

  /** Calculated gas concentration in parts per million (optional) */
  concentration_ppm?: number;

  /** Timestamp when this result was generated */
  timestamp: string; // unix timestamp format
}

/**
 * Complete computing response from the API
 *
 * Contains peak results from multiple computing nodes along with
 * legacy fields for backward compatibility and metadata about
 * active nodes and latest results.
 */
export interface ComputingResponse {
  /** Peak results from multiple nodes, keyed by node ID */
  peak_results: Record<string, PeakResultResponse>;

  /** Legacy fields for backward compatibility */

  /** Legacy peak frequency field */
  peak_frequency?: number;

  /** Legacy peak amplitude field */
  peak_amplitude?: number;

  /** Legacy concentration field */
  concentration_ppm?: number;

  /** Polynomial coefficients used for concentration calculation */
  polynomial_coefficients: [number, number, number, number, number];

  /** Node IDs that have recent data (within last 30 seconds) */
  active_node_ids: string[];

  /** Most recent result across all nodes */
  latest_result?: PeakResultResponse;
}

/**
 * Utility functions for working with computing data
 */
export namespace ComputingUtils {
  /**
   * Check if a computing response has any active nodes
   * @param response - The computing response
   * @returns True if there are active nodes with recent data
   */
  export function hasActiveNodes(response: ComputingResponse): boolean {
    return response.active_node_ids.length > 0;
  }

  /**
   * Get the most recent peak result from all nodes
   * @param response - The computing response
   * @returns The most recent peak result or null if none available
   */
  export function getLatestResult(
    response: ComputingResponse,
  ): PeakResultResponse | null {
    return response.latest_result || null;
  }

  /**
   * Get peak results sorted by timestamp (newest first)
   * @param response - The computing response
   * @returns Array of peak results sorted by timestamp
   */
  export function getSortedResults(
    response: ComputingResponse,
  ): Array<{ nodeId: string; result: PeakResultResponse }> {
    return Object.entries(response.peak_results)
      .map(([nodeId, result]) => ({ nodeId, result }))
      .sort(
        (a, b) =>
          new Date(b.result.timestamp).getTime() -
          new Date(a.result.timestamp).getTime(),
      );
  }

  /**
   * Get peak results filtered by node IDs
   * @param response - The computing response
   * @param nodeIds - Array of node IDs to filter by
   * @returns Filtered peak results
   */
  export function getResultsByNodeIds(
    response: ComputingResponse,
    nodeIds: string[],
  ): Record<string, PeakResultResponse> {
    const filtered: Record<string, PeakResultResponse> = {};

    for (const nodeId of nodeIds) {
      if (response.peak_results[nodeId]) {
        filtered[nodeId] = response.peak_results[nodeId];
      }
    }

    return filtered;
  }

  /**
   * Check if a specific node has recent data
   * @param response - The computing response
   * @param nodeId - The node ID to check
   * @returns True if the node has recent data
   */
  export function isNodeActive(
    response: ComputingResponse,
    nodeId: string,
  ): boolean {
    return response.active_node_ids.includes(nodeId);
  }

  /**
   * Calculate the average concentration from all active nodes
   * @param response - The computing response
   * @returns Average concentration in PPM or null if no concentration data
   */
  export function getAverageConcentration(
    response: ComputingResponse,
  ): number | null {
    const concentrations = Object.values(response.peak_results)
      .map((result) => result.concentration_ppm)
      .filter((ppm): ppm is number => ppm !== undefined && ppm !== null);

    if (concentrations.length === 0) {
      return null;
    }

    return (
      concentrations.reduce((sum, ppm) => sum + ppm, 0) / concentrations.length
    );
  }

  /**
   * Format timestamp for display
   * @param timestamp - ISO timestamp string
   * @returns Formatted time string
   */
  export function formatTimestamp(timestamp: string): string {
    try {
      return new Date(timestamp).toLocaleTimeString();
    } catch {
      return "Invalid time";
    }
  }
}

/**
 * Type guards for computing data validation
 */
export namespace ComputingTypeGuards {
  /**
   * Type guard to check if an object is a valid PeakResultResponse
   */
  export function isPeakResultResponse(obj: any): obj is PeakResultResponse {
    return (
      obj &&
      typeof obj === "object" &&
      typeof obj.frequency === "number" &&
      typeof obj.amplitude === "number" &&
      typeof obj.timestamp === "string" &&
      (obj.concentration_ppm === undefined ||
        typeof obj.concentration_ppm === "number")
    );
  }

  /**
   * Type guard to check if an object is a valid ComputingResponse
   */
  export function isComputingResponse(obj: any): obj is ComputingResponse {
    return (
      obj &&
      typeof obj === "object" &&
      typeof obj.peak_results === "object" &&
      Array.isArray(obj.polynomial_coefficients) &&
      obj.polynomial_coefficients.length === 5 &&
      Array.isArray(obj.active_node_ids) &&
      (obj.peak_frequency === undefined ||
        typeof obj.peak_frequency === "number") &&
      (obj.peak_amplitude === undefined ||
        typeof obj.peak_amplitude === "number") &&
      (obj.concentration_ppm === undefined ||
        typeof obj.concentration_ppm === "number") &&
      (obj.latest_result === undefined ||
        isPeakResultResponse(obj.latest_result))
    );
  }
}
