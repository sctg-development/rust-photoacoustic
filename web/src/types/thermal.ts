/**
 * TypeScript types for thermal regulation API endpoints
 * Generated from Rust structs in thermal.rs
 */

/**
 * PID controller components for analysis
 */
export interface PidComponents {
  /** Proportional term value */
  proportional: number;
  /** Integral term value */
  integral: number;
  /** Derivative term value */
  derivative: number;
  /** Error value (setpoint - process_variable) */
  error: number;
}

/**
 * Single data point in thermal regulation history
 */
export interface ThermalDataPoint {
  /** Timestamp in Unix seconds */
  timestamp: number;
  /** Temperature reading in degrees Celsius */
  temperature_celsius: number;
  /** Control output percentage (-100.0 to +100.0) */
  control_output_percent: number;
  /** PID setpoint temperature in degrees Celsius */
  setpoint_celsius: number;
  /** Individual PID components for debugging */
  pid_components: PidComponents;
}

/**
 * Current temperature information for a thermal regulator
 */
export interface CurrentTemperatureInfo {
  /** Current temperature reading in degrees Celsius */
  temperature_celsius: number;
  /** Timestamp of the temperature reading (Unix seconds) */
  timestamp: number;
  /** Current setpoint temperature in degrees Celsius */
  setpoint_celsius: number;
  /** Current output power of the regulator */
  control_output_percent: number;
  /** Current status of the regulator */
  status: string;
}

/**
 * Pagination information for thermal data responses
 */
export interface PaginationInfo {
  /** Current page number (1-indexed) */
  page: number;
  /** Number of items per page */
  limit: number;
  /** Total number of items across all pages */
  total_items: number;
  /** Total number of pages available */
  total_pages: number;
  /** Whether there is a next page available */
  has_next: boolean;
  /** Whether there is a previous page available */
  has_previous: boolean;
}

/**
 * Summary of applied filters
 */
export interface FilterSummary {
  /** Step size in seconds between data points */
  step_seconds: number;
  /** List of regulator IDs included in the response */
  included_regulators: string[];
  /** Start timestamp (Unix seconds) for data range */
  from_timestamp: number | null;
  /** End timestamp (Unix seconds) for data range */
  to_timestamp: number | null;
}

/**
 * Paginated thermal data response
 */
export interface PaginatedThermalResponse {
  /** Filtered thermal regulation data */
  data: Record<string, ThermalDataPoint[]>;
  /** Pagination metadata */
  pagination: PaginationInfo;
  /** Applied filters summary */
  filters: FilterSummary;
}

// API Response Types

/**
 * Response type for GET /api/thermal/regulators
 * Returns a list of all thermal regulator identifiers
 */
export type ThermalRegulatorsResponse = string[];

/**
 * Response type for GET /api/thermal/temperatures
 * Returns the most recent temperature reading for each thermal regulator
 */
export type LastTemperaturesResponse = Record<string, CurrentTemperatureInfo>;

/**
 * Response type for GET /api/thermal
 * Returns historical thermal regulation data with filtering and pagination
 */
export type ThermalDataResponse = PaginatedThermalResponse;

// API Query Parameters

/**
 * Query parameters for GET /api/thermal
 */
export interface ThermalDataQuery {
  /** Time interval in seconds between returned data points (default: 60) */
  steps?: number;
  /** Array of regulator IDs to include in response */
  regulators?: string[];
  /** Start timestamp for data range (Unix seconds or ISO 8601) */
  from?: string;
  /** End timestamp for data range (Unix seconds or ISO 8601) */
  to?: string;
  /** Page number for pagination (1-indexed, default: 1) */
  page?: number;
  /** Maximum number of data points per regulator per page (default: 1000, max: 10000) */
  limit?: number;
}

/**
 * Regulator status values
 */
export type RegulatorStatus =
  | "Uninitialized"
  | "Initializing"
  | "Running"
  | "Stopped"
  | string; // For "Error: message" format

/**
 * Helper type for temperature data with human-readable timestamp
 */
export interface ThermalDataPointWithDate extends Omit<
  ThermalDataPoint,
  "timestamp"
> {
  timestamp: number;
  date: Date;
}

/**
 * Helper type for current temperature info with human-readable timestamp
 */
export interface CurrentTemperatureInfoWithDate extends Omit<
  CurrentTemperatureInfo,
  "timestamp"
> {
  timestamp: number;
  date: Date;
}

/**
 * Utility functions for working with thermal data
 */
export const ThermalUtils = {
  /**
   * Convert Unix timestamp to Date object
   */
  timestampToDate: (timestamp: number): Date => new Date(timestamp * 1000),

  /**
   * Convert ThermalDataPoint to include Date object
   */
  addDateToDataPoint: (point: ThermalDataPoint): ThermalDataPointWithDate => ({
    ...point,
    date: new Date(point.timestamp * 1000),
  }),

  /**
   * Convert CurrentTemperatureInfo to include Date object
   */
  addDateToTemperatureInfo: (
    info: CurrentTemperatureInfo,
  ): CurrentTemperatureInfoWithDate => ({
    ...info,
    date: new Date(info.timestamp * 1000),
  }),

  /**
   * Check if regulator status indicates an error state
   */
  isErrorStatus: (status: string): boolean => status.startsWith("Error:"),

  /**
   * Check if regulator status indicates running state
   */
  isRunningStatus: (status: string): boolean => status === "Running",

  /**
   * Format temperature with appropriate precision
   */
  formatTemperature: (celsius: number, precision: number = 1): string =>
    `${celsius.toFixed(precision)}°C`,

  /**
   * Format control output percentage
   */
  formatControlOutput: (percent: number, precision: number = 1): string =>
    `${percent.toFixed(precision)}%`,

  /**
   * Get color class for regulator status
   */
  getStatusColor: (status: string): string => {
    if (status === "Running") return "text-green-600";
    if (status.startsWith("Error:")) return "text-red-600";
    if (status === "Initializing") return "text-yellow-600";
    if (status === "Stopped") return "text-gray-600";

    return "text-gray-400";
  },
};
