/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 *
 * Audio Streaming Types
 *
 * TypeScript type definitions for audio streaming functionality.
 * These types correspond to the Rust structures used in the backend API.
 */

/**
 * Information about an available audio stream
 *
 * Corresponds to the Rust `AudioStreamInfo` structure from the backend API.
 * Used by the `/api/stream/audio/get-all-streams` endpoint.
 */
export interface AudioStreamInfo {
  /** Stream identifier (e.g., "realtime_source", "streaming_output") */
  id: string;
  /** Stream URL endpoint for consuming audio data */
  stream_url: string;
  /** Statistics URL endpoint for stream metrics */
  stats_url: string;
}

/**
 * Information about a streaming node
 *
 * Corresponds to the Rust `StreamingNodeInfo` structure from the backend API.
 * Used by the `/api/stream/nodes` endpoint.
 */
export interface StreamingNodeInfo {
  /** Node string ID from configuration (preferred for permanent links) */
  id: string;
  /** Node UUID (for backward compatibility and internal operations) */
  uuid: string;
  /** Human-readable node name (if available) */
  name?: string;
  /** Whether the node is currently streaming */
  is_active: boolean;
  /** Current subscriber count for this node's stream */
  subscriber_count: number;
}

/**
 * Audio frame data structure for regular JSON format
 *
 * Corresponds to the Rust `AudioFrameResponse` structure.
 * Used by the standard audio streaming endpoints.
 */
export interface AudioFrameResponse {
  /** Channel A audio data */
  channel_a: number[];
  /** Channel B audio data */
  channel_b: number[];
  /** Sample rate of the audio data */
  sample_rate: number;
  /** Timestamp when the frame was captured */
  timestamp: number;
  /** Sequential frame number */
  frame_number: number;
  /** Duration of this frame in milliseconds */
  duration_ms: number;
}

/**
 * Audio frame data structure for fast binary format
 *
 * Corresponds to the Rust `AudioFastFrameResponse` structure.
 * Uses base64-encoded binary data for improved bandwidth efficiency.
 */
export interface AudioFastFrameResponse {
  /** Base64 encoded audio data for channel A */
  channel_a: string;
  /** Base64 encoded audio data for channel B */
  channel_b: string;
  /** Number of samples per channel */
  channels_length: number;
  /** Data type (e.g. "f32") */
  channels_raw_type: string;
  /** Size of each element in bytes (e.g. 4 for f32) */
  channels_element_size: number;
  /** Sample rate of the audio data */
  sample_rate: number;
  /** Timestamp when the frame was captured */
  timestamp: number;
  /** Sequential frame number */
  frame_number: number;
  /** Duration of this frame in milliseconds */
  duration_ms: number;
}

/**
 * Spectral analysis data for real-time visualization
 *
 * Corresponds to the Rust `SpectralDataResponse` structure.
 * Contains frequency domain representation of audio data.
 */
export interface SpectralDataResponse {
  /** Frequency bins (Hz) */
  frequencies: number[];
  /** Magnitude spectrum for channel A */
  magnitude_a: number[];
  /** Magnitude spectrum for channel B */
  magnitude_b: number[];
  /** Phase spectrum for channel A (optional) */
  phase_a?: number[];
  /** Phase spectrum for channel B (optional) */
  phase_b?: number[];
  /** Frame metadata */
  frame_number: number;
  /** Timestamp */
  timestamp: number;
  /** Sample rate */
  sample_rate: number;
}

/**
 * Stream statistics information
 *
 * Contains metrics about stream performance and status.
 */
export interface StreamStats {
  /** Total number of frames processed */
  total_frames: number;
  /** Total number of dropped frames */
  dropped_frames: number;
  /** Number of active subscribers */
  active_subscribers: number;
  /** Average frames per second */
  fps: number;
  /** Last update timestamp */
  last_update: number;
  /** Frames processed since last FPS calculation */
  frames_since_last_update: number;
  /** Sample rate of the audio stream in Hz */
  sample_rate: number;
  /** Whether the stream has dual channels (true) or is mono (false) */
  dual_channel: boolean;
}

/**
 * Server-sent event types for audio streaming
 */
export interface AudioStreamEvent {
  /** Event type */
  type: "data" | "heartbeat" | "error";
  /** Event data (AudioFrameResponse or AudioFastFrameResponse for data events) */
  data?: AudioFrameResponse | AudioFastFrameResponse | SpectralDataResponse;
  /** Error message for error events */
  message?: string;
}
