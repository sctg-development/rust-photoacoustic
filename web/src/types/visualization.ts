/**
 * Configuration item for visualization output display
 *
 * Each item represents a specific measurement that will be displayed
 * in the visualization interface, with customizable properties.
 */
export interface VisualizationOutputItem {
  /** Unique identifier for this output configuration */
  id: string;

  /** ID of the action node providing the measurement data */
  action_node_id: string;

  /** Name of the molecule being measured (e.g., "H₂S", "CO₂", "CH₄") */
  molecule: string;

  /** Unit of measurement (e.g., "ppm", "mg/m³", "%") */
  unit: string;

  /** Display order in the interface (negative values hide the item) */
  display_order: number;

  /** Description of the measurement (e.g., "Spectral ray 3963nm") */
  description: string;

  /** Minimal concentration value to display (below this value, the item is hidden) */
  concentration_min: number;

  /** Maximum concentration value to display (above this value, the item is hidden) */
  concentration_max: number;
}

/**
 * Configuration for the visualization web server
 *
 * This structure contains all settings required for the visualization server component,
 * including network binding parameters, TLS certificate settings, and authentication
 * configuration with both HMAC and RSA key-based JWT options.
 */
export interface VisualizationConfig {
  /** The TCP port the visualization server will listen on (default: 8080) */
  port: number;

  /** The network address the server will bind to (default: "127.0.0.1") */
  address: string;

  /** The server name reported in HTTP headers and logs */
  name: string;

  /** SSL/TLS certificate in PEM format, Base64 encoded */
  cert?: string;

  /** SSL/TLS private key in PEM format, Base64 encoded */
  key?: string;

  /** Secret key for HMAC-based JWT token signing and verification */
  hmac_secret: string;

  /** RS256 private key in PEM format, Base64 encoded */
  rs256_private_key: string;

  /** RS256 public key in PEM format, Base64 encoded */
  rs256_public_key: string;

  /** Enable or disable the visualization server (default: true) */
  enabled: boolean;

  /** Session secret key for cookie-based authentication */
  session_secret: string;

  /** Enable compression for server responses (default: true) */
  enable_compression: boolean;

  /** List of output items to be displayed in the visualization interface */
  output: VisualizationOutputItem[];
}

/**
 * Core measurement data from action drivers
 *
 * This interface represents the measurement data that flows through
 * the action driver system, containing concentration values, peak
 * information, and associated metadata.
 */
export interface MeasurementData {
  /** Current concentration value in ppm */
  concentration_ppm: number;

  /** Source node ID that generated this data */
  source_node_id: string;

  /** Peak amplitude value (0.0-1.0) */
  peak_amplitude: number;

  /** Peak frequency in Hz */
  peak_frequency: number;

  /** Timestamp of the measurement (Rust SystemTime format) */
  timestamp: {
    secs_since_epoch: number;
    nanos_since_epoch: number;
  };

  /** Additional metadata for the action */
  metadata: Record<string, any>;
}

/**
 * Alert/alarm data for special action states
 *
 * This interface represents alert data used when threshold
 * conditions are met and notifications need to be displayed.
 */
export interface AlertData {
  /** Type of alert (concentration, amplitude, timeout, etc.) */
  alert_type: string;

  /** Alert severity (info, warning, critical) */
  severity: "info" | "warning" | "critical";

  /** Human-readable alert message */
  message: string;

  /** Alert-specific data */
  data: Record<string, any>;

  /** Timestamp when alert was triggered (ISO 8601 string) */
  timestamp: string;
}

/**
 * Type guard to check if an object is a valid VisualizationOutputItem
 */
export function isVisualizationOutputItem(
  obj: any,
): obj is VisualizationOutputItem {
  return (
    typeof obj === "object" &&
    obj !== null &&
    typeof obj.id === "string" &&
    typeof obj.action_node_id === "string" &&
    typeof obj.molecule === "string" &&
    typeof obj.unit === "string" &&
    typeof obj.display_order === "number" &&
    typeof obj.description === "string" &&
    typeof obj.concentration_min === "number" &&
    typeof obj.concentration_max === "number"
  );
}

/**
 * Type guard to check if an object is a valid VisualizationConfig
 */
export function isVisualizationConfig(obj: any): obj is VisualizationConfig {
  return (
    typeof obj === "object" &&
    obj !== null &&
    typeof obj.port === "number" &&
    typeof obj.address === "string" &&
    typeof obj.name === "string" &&
    (obj.cert === undefined || typeof obj.cert === "string") &&
    (obj.key === undefined || typeof obj.key === "string") &&
    typeof obj.hmac_secret === "string" &&
    typeof obj.rs256_private_key === "string" &&
    typeof obj.rs256_public_key === "string" &&
    typeof obj.enabled === "boolean" &&
    typeof obj.session_secret === "string" &&
    typeof obj.enable_compression === "boolean" &&
    Array.isArray(obj.output) &&
    obj.output.every(isVisualizationOutputItem)
  );
}

/**
 * Type guard to check if an object is a valid MeasurementData
 */
export function isMeasurementData(obj: any): obj is MeasurementData {
  return (
    typeof obj === "object" &&
    obj !== null &&
    typeof obj.concentration_ppm === "number" &&
    typeof obj.source_node_id === "string" &&
    typeof obj.peak_amplitude === "number" &&
    typeof obj.peak_frequency === "number" &&
    typeof obj.timestamp === "object" &&
    obj.timestamp !== null &&
    typeof obj.timestamp.secs_since_epoch === "number" &&
    typeof obj.timestamp.nanos_since_epoch === "number" &&
    typeof obj.metadata === "object" &&
    obj.metadata !== null
  );
}

/**
 * Type guard to check if an object is a valid AlertData
 */
export function isAlertData(obj: any): obj is AlertData {
  return (
    typeof obj === "object" &&
    obj !== null &&
    typeof obj.alert_type === "string" &&
    (obj.severity === "info" ||
      obj.severity === "warning" ||
      obj.severity === "critical") &&
    typeof obj.message === "string" &&
    typeof obj.data === "object" &&
    obj.data !== null &&
    typeof obj.timestamp === "string"
  );
}

/**
 * Default values for VisualizationOutputItem
 */
export const defaultVisualizationOutputItem: Omit<
  VisualizationOutputItem,
  "id" | "action_node_id"
> = {
  molecule: "",
  unit: "ppm",
  display_order: 0,
  description: "",
  concentration_min: 0,
  concentration_max: 1000,
};

/**
 * Default values for MeasurementData
 */
export const defaultMeasurementData: Omit<MeasurementData, "source_node_id"> = {
  concentration_ppm: 0,
  peak_amplitude: 0,
  peak_frequency: 0,
  timestamp: {
    secs_since_epoch: Math.floor(Date.now() / 1000),
    nanos_since_epoch: 0,
  },
  metadata: {},
};

/**
 * Alert severity levels
 */
export const ALERT_SEVERITIES = ["info", "warning", "critical"] as const;
export type AlertSeverity = (typeof ALERT_SEVERITIES)[number];

/**
 * Common alert types
 */
export const ALERT_TYPES = {
  CONCENTRATION_THRESHOLD: "concentration_threshold",
  AMPLITUDE_THRESHOLD: "amplitude_threshold",
  FREQUENCY_ANOMALY: "frequency_anomaly",
  SENSOR_TIMEOUT: "sensor_timeout",
  CALIBRATION_REQUIRED: "calibration_required",
  HARDWARE_ERROR: "hardware_error",
} as const;

export type AlertType = (typeof ALERT_TYPES)[keyof typeof ALERT_TYPES];

/**
 * Common molecule names and their typical units
 */
export const COMMON_MOLECULES = {
  "H₂S": { unit: "ppm", description: "Hydrogen Sulfide" },
  "CO₂": { unit: "ppm", description: "Carbon Dioxide" },
  "CH₄": { unit: "ppm", description: "Methane" },
  "NH₃": { unit: "ppm", description: "Ammonia" },
  "SO₂": { unit: "ppm", description: "Sulfur Dioxide" },
  "NO₂": { unit: "ppm", description: "Nitrogen Dioxide" },
  "O₃": { unit: "ppm", description: "Ozone" },
  "C₂H₄": { unit: "ppm", description: "Ethylene" },
} as const;

/**
 * Common units for concentration measurements
 */
export const CONCENTRATION_UNITS = [
  "ppm",
  "ppb",
  "mg/m³",
  "µg/m³",
  "%",
  "vol%",
] as const;

export type ConcentrationUnit = (typeof CONCENTRATION_UNITS)[number];
export type CommonMolecule = keyof typeof COMMON_MOLECULES;
