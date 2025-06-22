/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 *
 * Advanced Real-Time Audio Stream Hook
 *
 * This hook provides a comprehensive solution for managing authenticated real-time audio streaming
 * with WebAudio API integration. It's designed for high-performance photoacoustic applications
 * requiring precise timing, minimal latency, and robust error handling.
 *
 * **Core Capabilities:**
 * - Server-Sent Events (SSE) audio streaming with authentication
 * - Automatic frame format detection (standard vs. fast binary format)
 * - High-performance audio processing with WebAudio API integration
 * - Advanced timestamp validation and gap detection
 * - Intelligent reconnection strategies with exponential backoff
 * - Memory-efficient buffer pooling and batch processing
 * - Real-time performance monitoring and statistics
 * - Optional real-time audio playback with volume control
 *
 * **Architecture Overview:**
 * ```
 * useAudioStream Hook
 * ├── Connection Management (SSE + Auth)
 * ├── Audio Processing Pipeline
 * │   ├── Frame Decoding (Binary/JSON)
 * │   ├── Buffer Creation & Pooling
 * │   ├── WebAudio Scheduling
 * │   ├── Optional Speaker Output
 * │   └── Performance Monitoring
 * ├── Timestamp Validation System
 * ├── Audio Playback Controls
 * │   ├── Enable/Disable Playback
 * │   ├── Volume Control
 * │   └── User Interaction Compliance
 * ├── Reconnection Logic
 * └── Statistics & Monitoring
 * ```
 *
 * **Performance Optimizations:**
 * - Typed array operations for minimal garbage collection
 * - Buffer pooling to reduce memory allocations
 * - Batch processing with adaptive time budgets
 * - Efficient base64 decoding for binary frames
 * - Throttled UI updates to prevent render blocking
 * - Conditional audio output for visualization-only mode
 *
 * **Frame Format Support:**
 * - Standard JSON frames: Traditional format with array data
 * - Fast binary frames: Base64-encoded Float32 data for efficiency
 * - Automatic detection and seamless switching between formats
 *
 * **Audio Playback Features:**
 * - Optional real-time audio output to speakers/headphones
 * - User-initiated playback (complies with browser autoplay policies)
 * - Volume control from 0% to 100%
 * - Independent of visualization and analysis functions
 * - Graceful fallback when audio devices are unavailable
 *
 * **Error Handling Strategy:**
 * - Categorized error types (auth, network, parse, audio, connection)
 * - Graceful degradation with user-friendly error messages
 * - Automatic recovery mechanisms where appropriate
 * - Comprehensive logging for debugging production issues
 *
 * **Usage Example:**
 * ```typescript
 * const {
 *   isConnected,
 *   currentFrame,
 *   fps,
 *   connect,
 *   disconnect,
 *   initializeAudio,
 *   enableAudioPlayback,
 *   disableAudioPlayback,
 *   setPlaybackVolume,
 *   isAudioPlaybackEnabled,
 *   playbackVolume
 * } = useAudioStream(
 *   'wss://api.example.com/stream',
 *   '/api/stats',
 *   false, // autoConnect
 *   { enabled: true, toleranceMs: 50 } // timestamp validation
 * );
 *
 * // Enable audio playback (requires user interaction)
 * const handleEnableAudio = async () => {
 *   await enableAudioPlayback();
 * };
 *
 * // Set volume to 75%
 * setPlaybackVolume(0.75);
 * ```
 */

import { useState, useEffect, useRef, useCallback } from "react";

import { useAuth, useSecuredApi } from "@/authentication";

/**
 * Audio Frame Data Structure
 *
 * Represents a single audio frame received from the server containing stereo audio data.
 * This is the standard format used throughout the application for audio processing.
 *
 * @interface AudioFrame
 * @property {number[]} channel_a - Left audio channel samples (typically 48kHz Float32 values)
 * @property {number[]} channel_b - Right audio channel samples (typically 48kHz Float32 values)
 * @property {number} sample_rate - Audio sample rate in Hz (usually 48000)
 * @property {number} timestamp - Server-side timestamp when frame was captured (milliseconds)
 * @property {number} frame_number - Sequential frame identifier for ordering and gap detection
 * @property {number} duration_ms - Frame duration in milliseconds (typically ~10-20ms)
 *
 * @example
 * ```typescript
 * const frame: AudioFrame = {
 *   channel_a: [0.1, -0.2, 0.3, ...], // 480 samples for 10ms at 48kHz
 *   channel_b: [0.2, -0.1, 0.4, ...], // 480 samples for 10ms at 48kHz
 *   sample_rate: 48000,
 *   timestamp: 1640995200000,
 *   frame_number: 12345,
 *   duration_ms: 10.0
 * };
 * ```
 */
interface AudioFrame {
  channel_a: number[];
  channel_b: number[];
  sample_rate: number;
  timestamp: number;
  frame_number: number;
  duration_ms: number;
}

/**
 * @typedef {Object} StreamError
 * @description Structure for capturing and categorizing stream-related errors
 * @property {string} type - Type of error (connection, auth, parse, network, audio)
 * @property {string} message - Error message
 * @property {number} timestamp - Time when the error occurred
 */
interface StreamError {
  type: "connection" | "auth" | "parse" | "network" | "audio";
  message: string;
  timestamp: number;
}

/**
 * @typedef {Object} AudioStreamNode
 * @description Web Audio API nodes graph for processing audio data
 * @property {AudioContext} context - Audio context managing the audio processing
 * @property {AudioBufferSourceNode | null} sourceNode - Source node for playing audio buffers
 * @property {GainNode} gainNode - Node for controlling volume
 * @property {AnalyserNode} analyserNode - Node for frequency analysis and visualization
 * @property {AudioNode} outputNode - Final node in the processing chain
 */
interface AudioStreamNode {
  context: AudioContext;
  sourceNode: AudioBufferSourceNode | null;
  gainNode: GainNode;
  analyserNode: AnalyserNode;
  outputNode: AudioNode;
}

/**
 * @typedef {Object} TimestampValidationStats
 * @description Statistics for timestamp-based frame validation
 * @property {boolean} enabled - Whether validation is enabled
 * @property {number} totalGaps - Total number of detected gaps
 * @property {number} totalMissedFrames - Estimated total missed frames
 * @property {number} averageGapSize - Average gap size in milliseconds
 * @property {number} maxGapSize - Maximum gap size detected
 * @property {number} lastGapTimestamp - Timestamp of the last detected gap
 * @property {number} expectedFrameInterval - Expected interval between frames
 * @property {number} toleranceMs - Tolerance for gap detection
 */
export interface TimestampValidationStats {
  enabled: boolean;
  totalGaps: number;
  totalMissedFrames: number;
  averageGapSize: number;
  maxGapSize: number;
  lastGapTimestamp: number;
  expectedFrameInterval: number;
  toleranceMs: number;
}

/**
 * @typedef {Object} TimestampValidationConfig
 * @description Configuration for timestamp validation
 * @property {boolean} enabled - Enable timestamp validation
 * @property {number} toleranceMs - Gap tolerance in milliseconds (default: 50ms)
 * @property {number} minGapSizeMs - Minimum gap size to consider as missing frames (default: 20ms)
 * @property {boolean} logGaps - Whether to log detected gaps to console
 * @property {boolean} autoAdjustTolerance - Automatically adjust tolerance based on jitter
 */
export interface TimestampValidationConfig {
  enabled: boolean;
  toleranceMs?: number;
  minGapSizeMs?: number;
  logGaps?: boolean;
  autoAdjustTolerance?: boolean;
}

/**
 * @typedef {Object} UseAudioStreamReturn
 * @description Return type for the useAudioStream hook
 * @property {boolean} isConnected - Whether the stream is currently connected
 * @property {boolean} isConnecting - Whether the stream is in the process of connecting
 * @property {StreamError | null} error - Current error, if any
 * @property {AudioFrame | null} currentFrame - Most recently received audio frame
 * @property {number} frameCount - Total number of frames received
 * @property {number} droppedFrames - Number of frames missed or dropped
 * @property {number} fps - Current frames per second rate
 * @property {number} averageFrameSizeBytes - Average frame size in bytes (rolling window of 1000 frames)
 * @property {AudioContext | null} audioContext - Current Web Audio context
 * @property {AudioStreamNode | null} audioStreamNode - Audio processing graph
 * @property {boolean} isAudioReady - Whether the audio system is ready
 * @property {AudioBuffer | null} currentBuffer - Most recently created audio buffer
 * @property {number} bufferDuration - Duration of the current buffer in seconds
 * @property {number} latency - Current audio latency in seconds
 * @property {TimestampValidationStats} timestampValidation - Timestamp validation statistics
 * @property {Function} connect - Function to connect to the audio stream
 * @property {Function} disconnect - Function to disconnect from the audio stream
 * @property {Function} reconnect - Function to reconnect to the audio stream
 * @property {Function} initializeAudio - Function to initialize the audio context
 * @property {Function} resumeAudio - Function to resume a suspended audio context
 * @property {Function} suspendAudio - Function to suspend the audio context
 * @property {Function} getPerformanceStats - Function to get performance statistics
 * @property {Function} resetTimestampValidation - Function to reset timestamp validation stats
 * @property {Function} updateTimestampValidationConfig - Function to update validation configuration
 * @property {Function} enableAudioPlayback - Function to enable real-time audio playback to speakers
 * @property {Function} disableAudioPlayback - Function to disable real-time audio playback
 * @property {Function} setPlaybackVolume - Function to set the playback volume (0.0 to 1.0)
 * @property {boolean} isAudioPlaybackEnabled - Whether audio is currently being played to speakers
 * @property {number} playbackVolume - Current playback volume (0.0 to 1.0)
 * @property {boolean} isDualChannel - Whether the audio stream has dual channels
 */
interface UseAudioStreamReturn {
  // Connection state
  isConnected: boolean;
  isConnecting: boolean;
  error: StreamError | null;

  // Stream data
  currentFrame: AudioFrame | null;
  frameCount: number;
  droppedFrames: number;
  fps: number;
  averageFrameSizeBytes: number;

  // Audio reconstruction
  audioContext: AudioContext | null;
  audioStreamNode: AudioStreamNode | null;
  isAudioReady: boolean;
  currentBuffer: AudioBuffer | null;
  bufferDuration: number;
  latency: number;

  // Audio playback controls
  isAudioPlaybackEnabled: boolean;
  playbackVolume: number;

  // Audio recording controls
  isRecording: boolean;

  // Timestamp validation
  timestampValidation: TimestampValidationStats;

  // Controls
  connect: () => void;
  disconnect: () => void;
  reconnect: () => void;
  initializeAudio: () => Promise<void>;
  resumeAudio: () => Promise<void>;
  suspendAudio: () => Promise<void>;
  enableAudioPlayback: () => Promise<void>;
  disableAudioPlayback: () => void;
  setPlaybackVolume: (volume: number) => void;
  startRecording: () => Promise<void>;
  stopRecording: () => void;
  getPerformanceStats: () => any;
  resetTimestampValidation: () => void;
  updateTimestampValidationConfig: (
    config: Partial<TimestampValidationConfig>,
  ) => void;
  isDualChannel: boolean;
}

/**
 * @typedef {Object} AudioFastFrame
 * @description Structure representing a fast audio frame with base64-encoded binary data
 * @property {string} channel_a - Base64-encoded binary data for channel A
 * @property {string} channel_b - Base64-encoded binary data for channel B
 * @property {number} channels_length - Number of samples per channel
 * @property {string} channels_raw_type - Data type (e.g., "f32")
 * @property {number} channels_element_size - Size of each element in bytes
 * @property {number} sample_rate - Sample rate in Hz
 * @property {number} timestamp - Server timestamp when the frame was created
 * @property {number} frame_number - Sequential frame number
 * @property {number} duration_ms - Frame duration in milliseconds
 */
interface AudioFastFrame {
  channel_a: string;
  channel_b: string;
  channels_length: number;
  channels_raw_type: string;
  channels_element_size: number;
  sample_rate: number;
  timestamp: number;
  frame_number: number;
  duration_ms: number;
}

export interface AudioStreamStatistics {
  // Total number of frames processed
  total_frames: number;
  // Total number of dropped frames
  dropped_frames: number;
  // Number of active subscribers
  active_subscribers: number;
  // Average frames per second
  fps: number;
  // Last update timestamp
  last_update: number;
  // Frames processed since last FPS calculation
  frames_since_last_update: number;
  // Sample rate of the audio stream in Hz
  sample_rate: number;
  // Whether the stream has dual channels (true) or is mono (false)
  dual_channel: boolean;
}

/**
 * Custom React hook for managing audio streaming from a server-sent events endpoint.
 * Handles connection management, authentication, audio processing, and playback.
 *
 * @param {string} [streamUrl] - Base URL for the server API
 * @param {string} [statsUrl="/api/stream/audio/stats"] - API endpoint for the audio stream
 * @param {boolean} [autoConnect=false] - Whether to automatically connect when conditions are met
 * @param {TimestampValidationConfig} [timestampValidationConfig] - Optional timestamp validation configuration
 * @returns {UseAudioStreamReturn} A collection of state and functions for managing the audio stream
 */
export const useAudioStream = (
  streamUrl?: string,
  statsUrl?: string,
  autoConnect: boolean = false,
  timestampValidationConfig?: TimestampValidationConfig,
): UseAudioStreamReturn => {
  const { getAccessToken, isAuthenticated } = useAuth();
  const { getJson } = useSecuredApi();

  // --- STATE MANAGEMENT ---

  /**
   * Connection states
   */
  const [isConnected, setIsConnected] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<StreamError | null>(null);
  const [currentFrame, setCurrentFrame] = useState<AudioFrame | null>(null);
  const [frameCount, setFrameCount] = useState(0);
  const [droppedFrames, setDroppedFrames] = useState(0);
  const [fps, setFps] = useState(0);
  const [statistics, setStatistics] = useState<AudioStreamStatistics | null>(
    null,
  );
  const [averageFrameSizeBytes, setAverageFrameSizeBytes] = useState(0);

  /**
   * Audio reconstruction states
   */
  const [audioContext, setAudioContext] = useState<AudioContext | null>(null);
  const [audioStreamNode, setAudioStreamNode] =
    useState<AudioStreamNode | null>(null);
  const [isAudioReady, setIsAudioReady] = useState(false);
  const [currentBuffer, setCurrentBuffer] = useState<AudioBuffer | null>(null);
  const [bufferDuration, setBufferDuration] = useState(0);
  const [latency, setLatency] = useState(0);

  /**
   * Audio playback control states
   */
  const [isAudioPlaybackEnabled, setIsAudioPlaybackEnabled] = useState(false);
  const [playbackVolume, setPlaybackVolumeState] = useState(0.5); // Default to 50% volume

  /**
   * Audio recording control states
   */
  const [isRecording, setIsRecording] = useState(false);

  /**
   * Audio recording references
   */
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const recordedChunksRef = useRef<Blob[]>([]);
  const recordingStreamRef = useRef<MediaStream | null>(null);

  /**
   * Timestamp validation states
   */
  const [timestampValidation, setTimestampValidation] =
    useState<TimestampValidationStats>({
      enabled: timestampValidationConfig?.enabled || false,
      totalGaps: 0,
      totalMissedFrames: 0,
      averageGapSize: 0,
      maxGapSize: 0,
      lastGapTimestamp: 0,
      expectedFrameInterval: 0,
      toleranceMs: timestampValidationConfig?.toleranceMs || 50,
    });

  // --- REFS (PERSISTENT VALUES) ---

  /**
   * References for stream handling
   */
  const readerRef = useRef<ReadableStreamDefaultReader<Uint8Array> | null>(
    null,
  );
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );
  const lastFrameTimeRef = useRef<number>(0);
  const fpsCalculationRef = useRef<{
    timestamps: number[];
    lastDisplayUpdate: number;
  }>({
    timestamps: [],
    lastDisplayUpdate: 0,
  });
  const abortControllerRef = useRef<AbortController | null>(null);

  /**
   * Frame format detection
   */
  const detectedFormatRef = useRef<"fast" | "standard" | null>(null);

  /**
   * Frame size tracking references - fixed
   */
  const frameSizesRef = useRef<number[]>([]);
  const maxFrameSizeHistoryRef = useRef<number>(100); // Reduced but working

  /**
   * Audio reconstruction references - optimized
   */
  const audioBufferQueueRef = useRef<AudioFrame[]>([]);
  const nextPlayTimeRef = useRef<number>(0);
  const sampleRateRef = useRef<number>(48000);
  const maxBufferQueueSizeRef = useRef<number>(20); // Increased to prevent frame loss

  /**
   * Performance optimization references
   */
  const fpsDisplayThrottleRef = useRef<number>(0); // Separate throttle for FPS display
  const frameProcessingBatchRef = useRef<string[]>([]);
  const batchProcessingTimeoutRef = useRef<ReturnType<
    typeof setTimeout
  > | null>(null);
  const lastProcessingTimeRef = useRef<number>(0);
  const audioBufferPoolRef = useRef<Map<string, AudioBuffer[]>>(new Map()); // Keyed by sample rate + length
  const performanceStatsRef = useRef({
    lastCpuMeasure: 0,
    processingTimes: new Float32Array(50), // Fixed size array
    processingTimeIndex: 0,
    totalProcessedFrames: 0,
    totalReceivedFrames: 0,
    averageProcessingTime: 0,
    peakProcessingTime: 0,
  });

  /**
   * Timestamp validation references
   */
  const timestampValidationConfigRef = useRef<TimestampValidationConfig>({
    enabled: timestampValidationConfig?.enabled || false,
    toleranceMs: timestampValidationConfig?.toleranceMs || 50,
    minGapSizeMs: timestampValidationConfig?.minGapSizeMs || 20,
    logGaps: timestampValidationConfig?.logGaps || false,
    autoAdjustTolerance: timestampValidationConfig?.autoAdjustTolerance || true,
  });

  const timestampValidationStatsRef = useRef({
    lastFrameTimestamp: 0,
    frameIntervals: new Float32Array(100), // Rolling window for interval calculation
    intervalIndex: 0,
    gapSizes: [] as number[],
    jitterValues: new Float32Array(50), // For auto-adjustment
    jitterIndex: 0,
  });

  // --- RECONNECTION LOGIC ---

  /**
   * Configuration for reconnection logic
   */
  const maxReconnectAttempts = 5;
  const reconnectDelay = 2000;
  const reconnectAttemptsRef = useRef(0);

  // --- AUDIO CONTEXT MANAGEMENT ---

  /**
   * Get the current sample rate from the statsUrl
   * If statsUrl is not provided, defaults to 48000 Hz.
   * This is used to ensure the audio context is created with the correct sample rate.
   * @returns {Promise<number>} The sample rate in Hz
   */
  const getStreamStatistics =
    useCallback(async (): Promise<AudioStreamStatistics | null> => {
      console.log(
        "Fetching sample rate from statsUrl:",
        statsUrl,
        "isAuthenticated:",
        isAuthenticated,
      );
      if (statsUrl && isAuthenticated) {
        try {
          const stats = (await getJson(statsUrl)) as AudioStreamStatistics;

          console.log("Fetched stats:", stats);

          return stats;
        } catch (error) {
          console.warn("Failed to fetch sample rate from stats URL:", error);

          return null;
        }
      }
      console.log(
        "No statsUrl or not authenticated, using default sample rate",
      );

      return null; // Default sample rate if statsUrl is not provided
    }, [statsUrl, isAuthenticated, getJson]);

  /**
   * Initializes the AudioContext and creates the audio processing graph.
   * Closes any existing audio context before creating a new one.
   *
   * @returns {Promise<void>}
   */
  const initializeAudio = useCallback(async () => {
    try {
      console.log(
        "initializeAudio called, current audioContext:",
        audioContext,
        "statsUrl:",
        statsUrl,
        "isAuthenticated:",
        isAuthenticated,
      );

      if (audioContext && audioContext.state !== "closed") {
        console.log("Closing existing audio context");
        await audioContext.close();
      }

      // Get sample rate first - always fetch fresh from server
      console.log("Fetching current sample rate from server");
      const currentStatistics = await getStreamStatistics();

      setStatistics(currentStatistics);
      sampleRateRef.current = currentStatistics?.sample_rate || 48000;

      console.log(
        "Creating new AudioContext with sample rate:",
        sampleRateRef.current,
      );

      const context = new AudioContext({
        sampleRate: sampleRateRef.current,
        latencyHint: "interactive",
      });

      // Create audio graph nodes
      const gainNode = context.createGain();
      const analyserNode = context.createAnalyser();

      // Configure analyser
      analyserNode.fftSize = 2048;
      analyserNode.smoothingTimeConstant = 0.8;

      // Configure gain node with current volume
      gainNode.gain.value = playbackVolume;

      // Connect the audio graph - do not connect to destination initially to avoid audio output
      gainNode.connect(analyserNode);
      // Note: analyserNode.connect(context.destination) is done conditionally in enableAudioPlayback()

      const streamNode: AudioStreamNode = {
        context,
        sourceNode: null,
        gainNode,
        analyserNode,
        outputNode: analyserNode,
      };

      console.log("Setting audio context and stream node");
      setAudioContext(context);
      setAudioStreamNode(streamNode);
      setIsAudioReady(true);
      setLatency(context.baseLatency + context.outputLatency);

      console.log("Audio context initialized successfully", {
        sampleRate: context.sampleRate,
        requestedSampleRate: sampleRateRef.current,
        latency: context.baseLatency + context.outputLatency,
        state: context.state,
      });
    } catch (err) {
      console.error("Failed to initialize audio context:", err);
      setError({
        type: "audio",
        message:
          err instanceof Error ? err.message : "Failed to initialize audio",
        timestamp: Date.now(),
      });
      setIsAudioReady(false);
    }
  }, [audioContext, getStreamStatistics]); // Remove samplerate dependency, add getSampleRate

  /**
   * Resumes the audio context if it's in a suspended state.
   *
   * @returns {Promise<void>}
   */
  const resumeAudio = useCallback(async () => {
    if (audioContext && audioContext.state === "suspended") {
      await audioContext.resume();
      console.log("Audio context resumed");
    }
  }, [audioContext]);

  /**
   * Suspends the audio context if it's in a running state.
   *
   * @returns {Promise<void>}
   */
  const suspendAudio = useCallback(async () => {
    if (audioContext && audioContext.state === "running") {
      await audioContext.suspend();
      console.log("Audio context suspended");
    }
  }, [audioContext]);

  // --- AUDIO PLAYBACK CONTROLS ---

  /**
   * Enables real-time audio playback to speakers by connecting the analyser to the audio destination.
   * Requires user interaction to comply with browser autoplay policies.
   *
   * @returns {Promise<void>}
   */
  const enableAudioPlayback = useCallback(async () => {
    if (!audioStreamNode || !audioContext) {
      console.warn("Audio system not ready for playback");

      return;
    }

    try {
      // Resume audio context if needed (required for user interaction compliance)
      if (audioContext.state === "suspended") {
        await audioContext.resume();
        console.log("Audio context resumed for playback");
      }

      // Connect analyser to destination for audio output
      if (!isAudioPlaybackEnabled) {
        audioStreamNode.analyserNode.connect(audioContext.destination);
        setIsAudioPlaybackEnabled(true);
        console.log("Audio playback enabled - connecting to speakers");
      }
    } catch (err) {
      console.error("Failed to enable audio playback:", err);
      setError({
        type: "audio",
        message:
          err instanceof Error
            ? err.message
            : "Failed to enable audio playback",
        timestamp: Date.now(),
      });
    }
  }, [audioStreamNode, audioContext, isAudioPlaybackEnabled]);

  /**
   * Disables real-time audio playback by disconnecting from the audio destination.
   * Audio processing and visualization will continue without audible output.
   */
  const disableAudioPlayback = useCallback(() => {
    if (!audioStreamNode || !audioContext) {
      console.warn("Audio system not available");

      return;
    }

    try {
      // Disconnect from destination to stop audio output
      if (isAudioPlaybackEnabled) {
        audioStreamNode.analyserNode.disconnect(audioContext.destination);
        setIsAudioPlaybackEnabled(false);
        console.log("Audio playback disabled - disconnected from speakers");
      }
    } catch (err) {
      console.error("Failed to disable audio playback:", err);
      // Set playback as disabled even if disconnect failed
      setIsAudioPlaybackEnabled(false);
    }
  }, [audioStreamNode, audioContext, isAudioPlaybackEnabled]);

  /**
   * Sets the playback volume for the audio stream.
   *
   * @param {number} volume - Volume level from 0.0 (muted) to 1.0 (full volume)
   */
  const setPlaybackVolume = useCallback(
    (volume: number) => {
      if (!audioStreamNode) {
        console.warn("Audio system not available for volume control");

        return;
      }

      // Clamp volume between 0.0 and 1.0
      const clampedVolume = Math.max(0.0, Math.min(1.0, volume));

      try {
        // Set gain node volume
        audioStreamNode.gainNode.gain.value = clampedVolume;
        setPlaybackVolumeState(clampedVolume);
        console.log(`Audio volume set to: ${Math.round(clampedVolume * 100)}%`);
      } catch (err) {
        console.error("Failed to set playback volume:", err);
      }
    },
    [audioStreamNode],
  );

  // --- AUDIO RECORDING CONTROLS ---

  /**
   * Starts recording the audio stream by capturing the output of the analyser node.
   * Creates a MediaRecorder to record the audio data to a downloadable file.
   *
   * @returns {Promise<void>}
   */
  const startRecording = useCallback(async () => {
    if (!audioStreamNode || !audioContext) {
      console.warn("Audio system not ready for recording");

      return;
    }

    if (isRecording) {
      console.warn("Recording already in progress");

      return;
    }

    try {
      // Create a MediaStreamDestination to capture audio
      const destination = audioContext.createMediaStreamDestination();

      // Connect the analyser to the destination for recording
      audioStreamNode.analyserNode.connect(destination);

      // Store the recording stream
      recordingStreamRef.current = destination.stream;

      // Create MediaRecorder
      const mediaRecorder = new MediaRecorder(destination.stream, {
        mimeType: "audio/webm;codecs=opus",
      });

      mediaRecorderRef.current = mediaRecorder;
      recordedChunksRef.current = [];

      // Set up event handlers
      mediaRecorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
          recordedChunksRef.current.push(event.data);
        }
      };

      mediaRecorder.onstop = () => {
        // Create downloadable blob
        const blob = new Blob(recordedChunksRef.current, {
          type: "audio/webm",
        });
        const url = URL.createObjectURL(blob);

        // Create download link
        const a = document.createElement("a");

        a.href = url;
        a.download = `audio-recording-${new Date().toISOString().slice(0, 19).replace(/:/g, "-")}.webm`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);

        // Clean up
        URL.revokeObjectURL(url);
        recordedChunksRef.current = [];
      };

      // Start recording
      mediaRecorder.start(1000); // Collect data every second
      setIsRecording(true);
      console.log("Audio recording started");
    } catch (err) {
      console.error("Failed to start recording:", err);
      setError({
        type: "audio",
        message:
          err instanceof Error ? err.message : "Failed to start recording",
        timestamp: Date.now(),
      });
    }
  }, [audioStreamNode, audioContext, isRecording]);

  /**
   * Stops the current audio recording and triggers download of the recorded file.
   */
  const stopRecording = useCallback(() => {
    if (!mediaRecorderRef.current || !isRecording) {
      console.warn("No recording in progress");

      return;
    }

    try {
      // Stop the MediaRecorder
      mediaRecorderRef.current.stop();

      // Disconnect from recording destination
      if (audioStreamNode && recordingStreamRef.current) {
        // Note: We don't need to explicitly disconnect as the MediaStreamDestination
        // will be garbage collected when the MediaRecorder stops
      }

      // Clean up references
      mediaRecorderRef.current = null;
      recordingStreamRef.current = null;
      setIsRecording(false);

      console.log("Audio recording stopped");
    } catch (err) {
      console.error("Failed to stop recording:", err);
      // Force reset recording state even if stop failed
      setIsRecording(false);
      mediaRecorderRef.current = null;
      recordingStreamRef.current = null;
    }
  }, [audioStreamNode, isRecording]);

  // --- OPTIMIZED AUDIO PROCESSING ---

  /**
   * Get buffer pool key for efficient lookups
   */
  const getBufferPoolKey = useCallback(
    (sampleRate: number, length: number): string => {
      return `${sampleRate}_${length}`;
    },
    [],
  );

  /**
   * Highly optimized audio buffer creation with advanced pooling
   */
  const createAudioBufferOptimized = useCallback(
    (frame: AudioFrame): AudioBuffer | null => {
      if (!audioContext) return null;

      const startTime = performance.now();

      try {
        const poolKey = getBufferPoolKey(
          frame.sample_rate,
          frame.channel_a.length,
        );
        let pool = audioBufferPoolRef.current.get(poolKey);

        if (!pool) {
          pool = [];
          audioBufferPoolRef.current.set(poolKey, pool);
        }

        // Try to reuse buffer from pool
        let buffer = pool.pop();

        if (!buffer) {
          buffer = audioContext.createBuffer(
            2,
            frame.channel_a.length,
            frame.sample_rate,
          );
        }

        // Optimized data copying using set() when possible
        const channelAData = buffer.getChannelData(0);
        const channelBData = buffer.getChannelData(1);

        if (frame.channel_a instanceof Float32Array) {
          channelAData.set(frame.channel_a);
        } else if (Array.isArray(frame.channel_a)) {
          // Batch copy optimization for arrays
          const tempArray = new Float32Array(frame.channel_a);

          channelAData.set(tempArray);
        }

        if (frame.channel_b instanceof Float32Array) {
          channelBData.set(frame.channel_b);
        } else if (Array.isArray(frame.channel_b)) {
          const tempArray = new Float32Array(frame.channel_b);

          channelBData.set(tempArray);
        }

        // Track performance with circular buffer
        const processingTime = performance.now() - startTime;
        const stats = performanceStatsRef.current;

        stats.processingTimes[stats.processingTimeIndex] = processingTime;
        stats.processingTimeIndex =
          (stats.processingTimeIndex + 1) % stats.processingTimes.length;

        if (processingTime > stats.peakProcessingTime) {
          stats.peakProcessingTime = processingTime;
        }

        return buffer;
      } catch (err) {
        console.error("Failed to create audio buffer:", err);

        return null;
      }
    },
    [audioContext, getBufferPoolKey],
  );

  /**
   * Return buffer to appropriate pool for reuse
   */
  const returnBufferToPool = useCallback(
    (buffer: AudioBuffer) => {
      const poolKey = getBufferPoolKey(buffer.sampleRate, buffer.length);
      const pool = audioBufferPoolRef.current.get(poolKey);

      if (pool && pool.length < BUFFER_POOL_SIZE) {
        pool.push(buffer);
      }
    },
    [getBufferPoolKey],
  );

  /**
   * Optimized audio buffer scheduling with minimal overhead
   */
  const scheduleAudioBufferOptimized = useCallback(
    (buffer: AudioBuffer) => {
      if (!audioStreamNode || !audioContext) return;

      try {
        const sourceNode = audioContext.createBufferSource();

        sourceNode.buffer = buffer;
        sourceNode.connect(audioStreamNode.gainNode);

        const currentTime = audioContext.currentTime;
        const scheduledTime = Math.max(currentTime, nextPlayTimeRef.current);

        sourceNode.start(scheduledTime);
        nextPlayTimeRef.current = scheduledTime + buffer.duration;

        // Update UI state less frequently to reduce overhead
        if (performanceStatsRef.current.totalProcessedFrames % 10 === 0) {
          setCurrentBuffer(buffer);
          setBufferDuration(buffer.duration);
        }

        sourceNode.onended = () => {
          sourceNode.disconnect();
          returnBufferToPool(buffer);
        };
      } catch (err) {
        console.error("Failed to schedule audio buffer:", err);
        returnBufferToPool(buffer);
      }
    },
    [audioStreamNode, audioContext, returnBufferToPool],
  );

  /**
   * Batch processing with adaptive time limits and no frame dropping
   */
  const processAudioQueueBatched = useCallback(() => {
    if (!audioContext || !isAudioReady) return;

    const startTime = performance.now();
    const queue = audioBufferQueueRef.current;
    let processed = 0;

    // Process all frames in queue with time-aware batching
    while (queue.length > 0 && processed < BATCH_SIZE) {
      const processingTime = performance.now() - startTime;

      // If we're taking too long, schedule the rest for next cycle
      if (processingTime > MAX_PROCESSING_TIME_MS && processed > 0) {
        requestAnimationFrame(() => processAudioQueueBatched());
        break;
      }

      const frame = queue.shift();

      if (frame) {
        const buffer = createAudioBufferOptimized(frame);

        if (buffer) {
          scheduleAudioBufferOptimized(buffer);
        }
        processed++;
      }
    }

    performanceStatsRef.current.totalProcessedFrames += processed;
  }, [
    audioContext,
    isAudioReady,
    createAudioBufferOptimized,
    scheduleAudioBufferOptimized,
  ]);

  /**
   * Optimized frame queuing with intelligent queue management (no dropping)
   */
  const queueAudioFrameOptimized = useCallback(
    (frame: AudioFrame) => {
      // Update sample rate if changed
      if (frame.sample_rate !== sampleRateRef.current) {
        sampleRateRef.current = frame.sample_rate;
      }

      if (!isAudioReady) return;

      // Add to queue - never drop frames, but warn if queue gets large
      audioBufferQueueRef.current.push(frame);

      if (audioBufferQueueRef.current.length > maxBufferQueueSizeRef.current) {
        console.warn(
          `Audio queue length: ${audioBufferQueueRef.current.length}, consider optimization`,
        );
        // Increase queue size dynamically instead of dropping
        maxBufferQueueSizeRef.current = Math.min(
          50,
          maxBufferQueueSizeRef.current + 5,
        );
      }

      // Intelligent processing scheduling
      const now = performance.now();

      if (now - lastProcessingTimeRef.current >= PROCESSING_THROTTLE_MS) {
        lastProcessingTimeRef.current = now;

        // Use requestIdleCallback for better performance when available
        if ("requestIdleCallback" in window) {
          requestIdleCallback(() => processAudioQueueBatched(), { timeout: 8 });
        } else {
          requestAnimationFrame(() => processAudioQueueBatched());
        }
      }
    },
    [isAudioReady, processAudioQueueBatched],
  );

  // --- OPTIMIZED DECODING ---

  /**
   * High-performance base64 decoding with typed arrays
   */
  const decodeAudioChannelOptimized = useCallback(
    (base64Data: string, length: number, elementSize: number): Float32Array => {
      try {
        // Pre-allocate result array
        const result = new Float32Array(length);

        // Use native atob for base64 decoding
        const binaryStr = atob(base64Data);
        const byteLength = binaryStr.length;
        const bytes = new Uint8Array(byteLength);

        // Optimized byte copying
        for (let i = 0; i < byteLength; i++) {
          bytes[i] = binaryStr.charCodeAt(i);
        }

        // Use DataView for efficient float32 reading
        const dataView = new DataView(bytes.buffer);

        // Batch process floats for better performance
        for (let i = 0; i < length; i++) {
          result[i] = dataView.getFloat32(i * elementSize, true);
        }

        return result;
      } catch (error) {
        console.error("Failed to decode audio channel:", error);

        return new Float32Array(length);
      }
    },
    [],
  );

  /**
   * Optimized fast frame conversion with direct typed array usage
   */
  const convertFastFrameOptimized = useCallback(
    (fastFrame: AudioFastFrame): AudioFrame => {
      const channel_a_typed = decodeAudioChannelOptimized(
        fastFrame.channel_a,
        fastFrame.channels_length,
        fastFrame.channels_element_size,
      );
      const channel_b_typed = decodeAudioChannelOptimized(
        fastFrame.channel_b,
        fastFrame.channels_length,
        fastFrame.channels_element_size,
      );

      return {
        channel_a: channel_a_typed as any, // Keep as typed array for performance
        channel_b: channel_b_typed as any,
        sample_rate: fastFrame.sample_rate,
        timestamp: fastFrame.timestamp,
        frame_number: fastFrame.frame_number,
        duration_ms: fastFrame.duration_ms,
      };
    },
    [decodeAudioChannelOptimized],
  );

  // --- THROTTLED FPS AND STATS ---

  /**
   * Fixed FPS calculation based on server timestamps, not client reception time
   */
  const updateFps = useCallback((frameTimestamp: number) => {
    const now = Date.now();
    const fpsData = fpsCalculationRef.current;

    // Always add frame timestamp for accurate FPS calculation
    fpsData.timestamps.push(frameTimestamp);

    // Keep only last 1 second of data based on server timestamps
    const oneSecondAgo = frameTimestamp - 1000;

    fpsData.timestamps = fpsData.timestamps.filter(
      (timestamp) => timestamp > oneSecondAgo,
    );

    // Only update the display every FPS_UPDATE_INTERVAL ms to reduce UI overhead
    if (now - fpsData.lastDisplayUpdate >= FPS_UPDATE_INTERVAL) {
      fpsData.lastDisplayUpdate = now;

      // Calculate FPS based on server timestamps from the last 1 second
      if (fpsData.timestamps.length > 1) {
        // More accurate FPS calculation using actual time span
        const timeSpanMs =
          fpsData.timestamps[fpsData.timestamps.length - 1] -
          fpsData.timestamps[0];

        if (timeSpanMs > 0) {
          const actualFps =
            ((fpsData.timestamps.length - 1) * 1000) / timeSpanMs;

          setFps(Math.round(actualFps * 10) / 10); // Round to 1 decimal place
        } else {
          setFps(fpsData.timestamps.length);
        }
      }
    }
  }, []);

  /**
   * Fixed frame size tracking - back to working version
   */
  const updateFrameSizeStats = useCallback((frameSize: number) => {
    const frameSizes = frameSizesRef.current;

    // Add new frame size
    frameSizes.push(frameSize);

    // Maintain rolling window
    if (frameSizes.length > maxFrameSizeHistoryRef.current) {
      frameSizes.shift();
    }

    // Calculate average every 5 frames for better responsiveness
    if (frameSizes.length % 5 === 0) {
      const sum = frameSizes.reduce((acc, size) => acc + size, 0);
      const average = Math.round(sum / frameSizes.length);

      setAverageFrameSizeBytes(average);
    }
  }, []);

  // --- PERFORMANCE MONITORING ---

  /**
   * Get comprehensive performance statistics
   */
  const getPerformanceStats = useCallback(() => {
    const stats = performanceStatsRef.current;

    // Calculate average processing time from circular buffer
    let sum = 0;
    let count = 0;

    for (let i = 0; i < stats.processingTimes.length; i++) {
      if (stats.processingTimes[i] > 0) {
        sum += stats.processingTimes[i];
        count++;
      }
    }

    const avgProcessingTime = count > 0 ? sum / count : 0;

    return {
      averageProcessingTime: Math.round(avgProcessingTime * 100) / 100,
      peakProcessingTime: Math.round(stats.peakProcessingTime * 100) / 100,
      totalProcessedFrames: stats.totalProcessedFrames,
      totalReceivedFrames: stats.totalReceivedFrames,
      queueLength: audioBufferQueueRef.current.length,
      bufferPoolSizes: Array.from(audioBufferPoolRef.current.entries()).map(
        ([key, pool]) => ({
          key,
          size: pool.length,
        }),
      ),
      processingEfficiency:
        stats.totalReceivedFrames > 0
          ? Math.round(
              (stats.totalProcessedFrames / stats.totalReceivedFrames) * 100,
            )
          : 100,
    };
  }, []);

  /**
   * Reset performance statistics
   */
  const resetPerformanceStats = useCallback(() => {
    performanceStatsRef.current = {
      lastCpuMeasure: 0,
      processingTimes: new Float32Array(50),
      processingTimeIndex: 0,
      totalProcessedFrames: 0,
      totalReceivedFrames: 0,
      averageProcessingTime: 0,
      peakProcessingTime: 0,
    };
  }, []);

  // --- BATCH PROCESSING ---

  /**
   * Optimized batch processing with adaptive scheduling
   */
  const processBatchedFrames = useCallback(() => {
    const batch = frameProcessingBatchRef.current;

    if (batch.length === 0) return;

    const startTime = performance.now();
    const framesToProcess = batch.splice(0, Math.min(batch.length, BATCH_SIZE));

    // Process with adaptive time management
    for (let i = 0; i < framesToProcess.length; i++) {
      const line = framesToProcess[i];

      // Check time budget periodically
      if (i > 0 && i % 4 === 0) {
        const elapsed = performance.now() - startTime;

        if (elapsed > MAX_PROCESSING_TIME_MS) {
          // Put remaining frames back at the front
          frameProcessingBatchRef.current.unshift(...framesToProcess.slice(i));
          // Schedule continuation
          setTimeout(() => processBatchedFrames(), 1);
          break;
        }
      }

      processServerSentEvent(line);
    }

    // Clear timeout
    if (batchProcessingTimeoutRef.current) {
      clearTimeout(batchProcessingTimeoutRef.current);
      batchProcessingTimeoutRef.current = null;
    }
  }, []);

  // --- SERVER-SENT EVENTS HANDLING ---

  /**
   * Auto-detect frame format based on first received frame
   */
  const detectFrameFormat = useCallback(
    (data: string): "fast" | "standard" | null => {
      try {
        const parsed = JSON.parse(data);

        // Check for channels_raw_type which is mandatory in AudioFastFrame
        if (parsed.channels_raw_type !== undefined) {
          return "fast";
        } else if (parsed.channel_a && parsed.channel_b && parsed.sample_rate) {
          return "standard";
        }

        return null;
      } catch (error) {
        return null;
      }
    },
    [],
  );

  /**
   * Fixed server-sent event processing with automatic format detection
   */
  const processServerSentEvent = useCallback(
    async (line: string) => {
      try {
        if (!line.startsWith("data:") && !line.startsWith("data: ")) return;

        const data = line.replace(/^data:\s*/, "");

        if (data === '{"type":"heartbeat"}' || !data.trim()) return;

        performanceStatsRef.current.totalReceivedFrames++;

        // Auto-detect format on first frame
        if (detectedFormatRef.current === null) {
          const detectedFormat = detectFrameFormat(data);

          if (detectedFormat) {
            detectedFormatRef.current = detectedFormat;
            console.log(`Auto-detected frame format: ${detectedFormat}`);
          } else {
            console.warn("Could not detect frame format, skipping frame");

            return;
          }
        }

        let frame: AudioFrame;
        let frameSize: number;

        if (detectedFormatRef.current === "fast") {
          const fastFrame: AudioFastFrame = JSON.parse(data);

          if (
            fastFrame.frame_number !== undefined &&
            fastFrame.channel_a &&
            fastFrame.channel_b &&
            fastFrame.channels_length &&
            fastFrame.channels_raw_type &&
            fastFrame.sample_rate
          ) {
            frame = convertFastFrameOptimized(fastFrame);
            frameSize = data.length;
          } else {
            return;
          }
        } else {
          frame = JSON.parse(data);
          if (
            frame.frame_number === undefined ||
            !frame.channel_a ||
            !frame.channel_b ||
            !frame.sample_rate
          ) {
            return;
          }
          frameSize = data.length;
        }

        // Perform timestamp validation
        validateFrameTimestamp(frame);

        // Update statistics with actual data
        updateFrameSizeStats(frameSize);

        // Update frame state every 5th frame to reduce overhead
        if (frameCount % 5 === 0) {
          setCurrentFrame(frame);
        }

        setFrameCount((prev) => prev + 1);
        updateFps(frame.timestamp); // Use server timestamp for FPS calculation
        queueAudioFrameOptimized(frame);

        // Simplified frame drop detection (keep existing logic)
        const lastFrameNumber = lastFrameTimeRef.current;

        if (lastFrameNumber > 0 && frame.frame_number > lastFrameNumber + 1) {
          const missed = frame.frame_number - lastFrameNumber - 1;

          setDroppedFrames((prev) => prev + missed);
        }
        lastFrameTimeRef.current = frame.frame_number;
      } catch (parseError) {
        if (Math.random() < 0.1) {
          console.error("Parse error:", parseError);
        }
      }
    },
    [
      updateFps,
      queueAudioFrameOptimized,
      convertFastFrameOptimized,
      updateFrameSizeStats,
      frameCount,
      detectFrameFormat,
    ],
  );

  /**
   * Handles stream errors and manages reconnection attempts.
   *
   * @param {any} err - The error that occurred
   */
  const handleStreamError = useCallback((err: any) => {
    console.error("Audio stream error:", err);
    setIsConnected(false);
    setIsConnecting(false);

    // Determine error type
    let errorType: StreamError["type"] = "network";
    let errorMessage = "Connection error occurred";

    if (err instanceof Error) {
      if (err.name === "AbortError") {
        errorType = "connection";
        errorMessage = "Connection was aborted";
      } else if (err.message.includes("fetch")) {
        errorType = "network";
        errorMessage = err.message;
      }
    }

    setError({
      type: errorType,
      message: errorMessage,
      timestamp: Date.now(),
    });

    // Automatic reconnection attempt with progressive delay
    if (reconnectAttemptsRef.current < maxReconnectAttempts) {
      reconnectAttemptsRef.current++;
      console.log(
        `Attempting to reconnect (${reconnectAttemptsRef.current}/${maxReconnectAttempts})`,
      );

      reconnectTimeoutRef.current = setTimeout(() => {
        connect();
      }, reconnectDelay * reconnectAttemptsRef.current);
    } else {
      console.error("Max reconnection attempts reached");
      setError({
        type: "connection",
        message: "Max reconnection attempts reached",
        timestamp: Date.now(),
      });
    }
  }, []);

  // --- TIMESTAMP VALIDATION ---

  /**
   * Reset timestamp validation statistics
   */
  const resetTimestampValidation = useCallback(() => {
    timestampValidationStatsRef.current = {
      lastFrameTimestamp: 0,
      frameIntervals: new Float32Array(100),
      intervalIndex: 0,
      gapSizes: [],
      jitterValues: new Float32Array(50),
      jitterIndex: 0,
    };

    setTimestampValidation((prev) => ({
      ...prev,
      totalGaps: 0,
      totalMissedFrames: 0,
      averageGapSize: 0,
      maxGapSize: 0,
      lastGapTimestamp: 0,
      expectedFrameInterval: 0,
    }));
  }, []);

  // --- CONNECTION MANAGEMENT ---

  /**
   * Connects to the audio stream endpoint with authentication.
   * Sets up the stream reader and event handler for incoming audio frames.
   */
  const connect = useCallback(async () => {
    if (
      !isAuthenticated ||
      !streamUrl ||
      !statsUrl ||
      isConnecting ||
      isConnected
    ) {
      console.log("Connect conditions not met:", {
        isAuthenticated,
        streamUrl,
        statsUrl,
        isConnecting,
        isConnected,
      });

      return;
    }

    try {
      if (statistics === null) {
        // Get sample rate from statsUrl if not already set
        setStatistics(await getStreamStatistics());
      }
      setIsConnecting(true);
      setError(null);

      // Reset frame counters and statistics properly
      setFrameCount(0);
      setDroppedFrames(0);
      setFps(0);
      setCurrentFrame(null);
      setAverageFrameSizeBytes(0);
      lastFrameTimeRef.current = 0;
      fpsCalculationRef.current = { timestamps: [], lastDisplayUpdate: 0 };
      fpsDisplayThrottleRef.current = 0;
      frameSizesRef.current = [];

      // Reset format detection
      detectedFormatRef.current = null;

      // Reset timestamp validation
      resetTimestampValidation();

      // Get access token
      const accessToken = await getAccessToken();

      if (!accessToken) {
        throw new Error("No access token available");
      }

      // Log stream URL
      console.log(
        `Connecting to audio stream at ${streamUrl} and stats at ${statsUrl}`,
      );

      // Close existing connection if it exists
      if (readerRef.current) {
        await readerRef.current.cancel();
        readerRef.current = null;
      }

      if (abortControllerRef.current) {
        abortControllerRef.current.abort();
      }

      // Create a new AbortController for this connection
      abortControllerRef.current = new AbortController();

      // Use fetch() with custom headers instead of EventSource
      const response = await fetch(streamUrl, {
        method: "GET",
        headers: {
          Accept: "text/event-stream",
          "Cache-Control": "no-cache",
          Authorization: `Bearer ${accessToken}`,
        },
        signal: abortControllerRef.current.signal,
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      if (!response.body) {
        throw new Error("No response body available");
      }

      // Get the reader for the stream
      const reader = response.body.getReader();

      readerRef.current = reader;
      const decoder = new TextDecoder();

      // Connection established
      console.log("Audio stream connected");
      setIsConnected(true);
      setIsConnecting(false);
      setError(null);
      reconnectAttemptsRef.current = 0;

      // Buffer for partial data
      let buffer = "";

      // Read the stream
      const readStream = async () => {
        try {
          while (true) {
            const { done, value } = await reader.read();

            if (done) {
              console.log("Stream ended");
              setIsConnected(false);
              break;
            }

            // Decode the data
            const chunk = decoder.decode(value, { stream: true });

            buffer += chunk;

            // Process complete lines
            const lines = buffer.split("\n");

            buffer = lines.pop() || ""; // Keep the partial line

            for (const line of lines) {
              const trimmedLine = line.trim();

              if (trimmedLine) {
                await processServerSentEvent(trimmedLine);
              }
            }
          }
        } catch (err) {
          if (err instanceof Error && err.name === "AbortError") {
            console.log("Stream aborted");

            return;
          }
          console.error("Stream reading error:", err);
          handleStreamError(err);
        }
      };

      // Start reading the stream
      readStream();
    } catch (err) {
      console.error("Failed to connect to audio stream:", err);
      setIsConnecting(false);
      setError({
        type: "auth",
        message: err instanceof Error ? err.message : "Failed to authenticate",
        timestamp: Date.now(),
      });
    }
  }, [
    isAuthenticated,
    streamUrl,
    statsUrl,
    getAccessToken,
    isConnecting,
    isConnected,
    processServerSentEvent,
    handleStreamError,
  ]);

  /**
   * Disconnects from the audio stream and cleans up resources.
   */
  const disconnect = useCallback(() => {
    if (readerRef.current) {
      readerRef.current.cancel();
      readerRef.current = null;
    }

    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
      abortControllerRef.current = null;
    }

    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }

    // Reset all frame-related states to ensure clean UI state
    setIsConnected(false);
    setIsConnecting(false);
    setError(null);
    setCurrentFrame(null); // This ensures the UI shows proper disconnected state
    setFrameCount(0);
    setDroppedFrames(0);
    setFps(0);
    setAverageFrameSizeBytes(0);

    // Reset internal counters and tracking
    lastFrameTimeRef.current = 0;
    fpsCalculationRef.current = { timestamps: [], lastDisplayUpdate: 0 };
    fpsDisplayThrottleRef.current = 0;
    frameSizesRef.current = [];

    // Reset format detection to ensure proper re-detection on reconnect
    detectedFormatRef.current = null;

    // Clear audio buffer queue
    audioBufferQueueRef.current = [];
    nextPlayTimeRef.current = 0;

    reconnectAttemptsRef.current = 0;
    console.log("Audio stream disconnected and state reset");
  }, []);

  /**
   * Reconnects to the audio stream by first disconnecting and then connecting again.
   * Adds a small delay between disconnection and reconnection for stability.
   */
  const reconnect = useCallback(() => {
    disconnect();
    setTimeout(() => {
      connect();
    }, 500);
  }, [disconnect, connect]);

  /**
   * Cleans up audio resources and resets audio-related state.
   */
  const cleanupAudio = useCallback(() => {
    console.log("cleanupAudio called");

    // Disable playback first to properly disconnect from destination
    if (isAudioPlaybackEnabled) {
      disableAudioPlayback();
    }

    if (audioStreamNode?.sourceNode) {
      audioStreamNode.sourceNode.disconnect();
    }
    if (audioContext && audioContext.state !== "closed") {
      console.log("Closing audio context in cleanup");
      audioContext.close();
    }

    // Clear all optimized data structures
    audioBufferPoolRef.current.clear();
    frameProcessingBatchRef.current = [];
    resetPerformanceStats();

    if (batchProcessingTimeoutRef.current) {
      clearTimeout(batchProcessingTimeoutRef.current);
      batchProcessingTimeoutRef.current = null;
    }

    setAudioContext(null);
    setAudioStreamNode(null);
    setIsAudioReady(false);
    setCurrentBuffer(null);
    setIsAudioPlaybackEnabled(false);
    audioBufferQueueRef.current = [];
    nextPlayTimeRef.current = 0;
  }, [
    audioStreamNode,
    audioContext,
    resetPerformanceStats,
    isAudioPlaybackEnabled,
    disableAudioPlayback,
  ]);

  // --- PERFORMANCE OPTIMIZATION CONSTANTS ---

  /**
   * Performance configuration - optimized for no frame dropping
   */
  const PROCESSING_THROTTLE_MS = 8; // ~120fps processing capability
  const BATCH_SIZE = 8; // Larger batches for efficiency
  const FPS_UPDATE_INTERVAL = 200; // Display FPS every 200ms (separate from calculation)
  const BUFFER_POOL_SIZE = 5; // Buffer pool per configuration
  const MAX_PROCESSING_TIME_MS = 12; // Generous time per cycle

  /**
   * Calculate expected frame interval based on recent frames
   */
  const calculateExpectedInterval = useCallback((): number => {
    const intervals = timestampValidationStatsRef.current.frameIntervals;
    let sum = 0;
    let count = 0;

    for (let i = 0; i < intervals.length; i++) {
      if (intervals[i] > 0) {
        sum += intervals[i];
        count++;
      }
    }

    return count > 0 ? sum / count : 0;
  }, []);

  /**
   * Auto-adjust tolerance based on observed jitter
   */
  const autoAdjustTolerance = useCallback(() => {
    if (!timestampValidationConfigRef.current.autoAdjustTolerance) return;

    const jitterValues = timestampValidationStatsRef.current.jitterValues;
    let maxJitter = 0;

    for (let i = 0; i < jitterValues.length; i++) {
      if (jitterValues[i] > maxJitter) {
        maxJitter = jitterValues[i];
      }
    }

    if (maxJitter > 0) {
      // Set tolerance to 3x the maximum observed jitter, with reasonable bounds
      const newTolerance = Math.min(Math.max(maxJitter * 3, 20), 200);

      timestampValidationConfigRef.current.toleranceMs = newTolerance;

      setTimestampValidation((prev) => ({
        ...prev,
        toleranceMs: newTolerance,
      }));
    }
  }, []);

  /**
   * Validate frame timestamps and detect gaps
   */
  const validateFrameTimestamp = useCallback(
    (frame: AudioFrame) => {
      const config = timestampValidationConfigRef.current;

      if (!config.enabled) return;

      const stats = timestampValidationStatsRef.current;
      const currentTimestamp = frame.timestamp;

      if (stats.lastFrameTimestamp > 0) {
        const interval = currentTimestamp - stats.lastFrameTimestamp;

        // Store interval for expected interval calculation
        stats.frameIntervals[stats.intervalIndex] = interval;
        stats.intervalIndex =
          (stats.intervalIndex + 1) % stats.frameIntervals.length;

        const expectedInterval = calculateExpectedInterval();

        if (expectedInterval > 0) {
          const jitter = Math.abs(interval - expectedInterval);

          // Store jitter for auto-adjustment
          stats.jitterValues[stats.jitterIndex] = jitter;
          stats.jitterIndex =
            (stats.jitterIndex + 1) % stats.jitterValues.length;

          // Detect gaps
          if (
            interval > expectedInterval + (config.toleranceMs || 10) &&
            interval > (config?.minGapSizeMs || 50)
          ) {
            const gapSize = interval - expectedInterval;
            const estimatedMissedFrames = Math.round(
              gapSize / expectedInterval,
            );

            stats.gapSizes.push(gapSize);

            // Update statistics
            setTimestampValidation((prev) => {
              const newTotalGaps = prev.totalGaps + 1;
              const newTotalMissedFrames =
                prev.totalMissedFrames + estimatedMissedFrames;
              const averageGapSize =
                stats.gapSizes.reduce((a, b) => a + b, 0) /
                stats.gapSizes.length;

              return {
                ...prev,
                totalGaps: newTotalGaps,
                totalMissedFrames: newTotalMissedFrames,
                averageGapSize,
                maxGapSize: Math.max(prev.maxGapSize, gapSize),
                lastGapTimestamp: currentTimestamp,
                expectedFrameInterval: expectedInterval,
              };
            });

            // Update dropped frames count
            setDroppedFrames((prev) => prev + estimatedMissedFrames);

            if (config.logGaps) {
              console.warn(`Frame gap detected:`, {
                gapSize: Math.round(gapSize),
                expectedInterval: Math.round(expectedInterval),
                actualInterval: Math.round(interval),
                estimatedMissedFrames,
                frameNumber: frame.frame_number,
                timestamp: currentTimestamp,
              });
            }
          }

          // Periodically auto-adjust tolerance
          if (stats.intervalIndex % 50 === 0) {
            autoAdjustTolerance();
          }

          // Update expected interval in state periodically
          if (stats.intervalIndex % 10 === 0) {
            setTimestampValidation((prev) => ({
              ...prev,
              expectedFrameInterval: expectedInterval,
            }));
          }
        }
      }

      stats.lastFrameTimestamp = currentTimestamp;
    },
    [calculateExpectedInterval, autoAdjustTolerance],
  );

  /**
   * Update timestamp validation configuration
   */
  const updateTimestampValidationConfig = useCallback(
    (config: Partial<TimestampValidationConfig>) => {
      timestampValidationConfigRef.current = {
        ...timestampValidationConfigRef.current,
        ...config,
      };

      setTimestampValidation((prev) => ({
        ...prev,
        enabled: config.enabled !== undefined ? config.enabled : prev.enabled,
        toleranceMs:
          config.toleranceMs !== undefined
            ? config.toleranceMs
            : prev.toleranceMs,
      }));

      if (config.enabled === false) {
        resetTimestampValidation();
      }
    },
    [resetTimestampValidation],
  );

  // --- EFFECTS (SIDE EFFECTS) ---

  /**
   * Effect for cleaning up resources when the component is unmounted.
   */
  useEffect(() => {
    return () => {
      console.log("Component cleanup effect triggered");
      disconnect();
      cleanupAudio();
    };
  }, []);

  /**
   * Effect for auto-connecting to the stream when conditions are met.
   */
  useEffect(() => {
    if (
      autoConnect &&
      isAuthenticated &&
      streamUrl &&
      statsUrl &&
      !isConnected &&
      !isConnecting
    ) {
      console.log("Auto-connecting to stream");
      connect();
    }
  }, [
    autoConnect,
    isAuthenticated,
    streamUrl,
    statsUrl,
    isConnected,
    isConnecting,
    connect,
    statistics,
  ]);

  // --- RETURN API ---

  return {
    isConnected,
    isConnecting,
    error,
    currentFrame,
    frameCount,
    droppedFrames,
    fps,
    averageFrameSizeBytes,
    audioContext,
    audioStreamNode,
    isAudioReady,
    currentBuffer,
    bufferDuration,
    latency,
    isAudioPlaybackEnabled,
    playbackVolume,
    isRecording,
    timestampValidation,
    connect,
    disconnect,
    reconnect,
    initializeAudio,
    resumeAudio,
    suspendAudio,
    enableAudioPlayback,
    disableAudioPlayback,
    setPlaybackVolume,
    startRecording,
    stopRecording,
    getPerformanceStats,
    resetTimestampValidation,
    updateTimestampValidationConfig,
    isDualChannel: statistics?.dual_channel || false,
  };
};
