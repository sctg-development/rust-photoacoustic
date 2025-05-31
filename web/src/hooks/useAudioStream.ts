/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 *
 * React hook for managing a real-time authenticated audio stream with WebAudio API integration.
 * Provides functionality for connecting to a server-sent events stream, processing audio frames,
 * and managing audio context lifecycle including reconnection strategies.
 */

import { useState, useEffect, useRef, useCallback } from "react";

import { useAuth } from "@/authentication";

/**
 * @typedef {Object} AudioFrame
 * @description Structure representing an audio frame received from the server
 * @property {number[]} channel_a - Array of samples for channel A
 * @property {number[]} channel_b - Array of samples for channel B
 * @property {number} sample_rate - Sample rate in Hz
 * @property {number} timestamp - Server timestamp when the frame was created
 * @property {number} frame_number - Sequential frame number
 * @property {number} duration_ms - Frame duration in milliseconds
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
 * @property {Function} connect - Function to connect to the audio stream
 * @property {Function} disconnect - Function to disconnect from the audio stream
 * @property {Function} reconnect - Function to reconnect to the audio stream
 * @property {Function} initializeAudio - Function to initialize the audio context
 * @property {Function} resumeAudio - Function to resume a suspended audio context
 * @property {Function} suspendAudio - Function to suspend the audio context
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

  // Controls
  connect: () => void;
  disconnect: () => void;
  reconnect: () => void;
  initializeAudio: () => Promise<void>;
  resumeAudio: () => Promise<void>;
  suspendAudio: () => Promise<void>;
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

/**
 * Custom React hook for managing audio streaming from a server-sent events endpoint.
 * Handles connection management, authentication, audio processing, and playback.
 *
 * @param {string} [baseUrl] - Base URL for the server API
 * @param {boolean} [autoConnect=false] - Whether to automatically connect when conditions are met
 * @param {boolean} [useFastFormat=false] - Whether to use the fast binary format
 * @returns {UseAudioStreamReturn} A collection of state and functions for managing the audio stream
 */
export const useAudioStream = (
  baseUrl?: string,
  autoConnect: boolean = false,
  useFastFormat: boolean = false,
): UseAudioStreamReturn => {
  const { getAccessToken, isAuthenticated } = useAuth();

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
  const fpsCalculationRef = useRef<number[]>([]);
  const abortControllerRef = useRef<AbortController | null>(null);

  /**
   * Frame size tracking references
   */
  const frameSizesRef = useRef<number[]>([]);
  const maxFrameSizeHistoryRef = useRef<number>(1000);

  /**
   * Audio reconstruction references
   */
  const audioBufferQueueRef = useRef<AudioFrame[]>([]);
  const nextPlayTimeRef = useRef<number>(0);
  const sampleRateRef = useRef<number>(44100); // Will be dynamically updated based on received frames
  const maxBufferQueueSizeRef = useRef<number>(10); // Maximum frames to queue

  /**
   * Configuration for reconnection logic
   */
  const maxReconnectAttempts = 5;
  const reconnectDelay = 2000; // 2 seconds
  const reconnectAttemptsRef = useRef(0);

  // --- AUDIO CONTEXT MANAGEMENT ---

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
      );

      if (audioContext && audioContext.state !== "closed") {
        console.log("Closing existing audio context");
        await audioContext.close();
      }

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

      // Connect the audio graph - do not connect to destination to avoid audio output
      gainNode.connect(analyserNode);
      // analyserNode.connect(context.destination); // Removed to prevent audio output

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
  }, []); // Empty dependency array to prevent circular references

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

  // --- AUDIO PROCESSING ---

  /**
   * Converts an audio frame to an AudioBuffer for playback.
   *
   * @param {AudioFrame} frame - The audio frame to convert
   * @returns {AudioBuffer | null} The created audio buffer or null if creation failed
   */
  const createAudioBuffer = useCallback(
    (frame: AudioFrame): AudioBuffer | null => {
      if (!audioContext) return null;

      try {
        const buffer = audioContext.createBuffer(
          2, // stereo
          frame.channel_a.length,
          frame.sample_rate,
        );

        // Fill channel data
        const channelAData = buffer.getChannelData(0);
        const channelBData = buffer.getChannelData(1);

        for (let i = 0; i < frame.channel_a.length; i++) {
          channelAData[i] = frame.channel_a[i];
          channelBData[i] = frame.channel_b[i];
        }

        return buffer;
      } catch (err) {
        console.error("Failed to create audio buffer:", err);

        return null;
      }
    },
    [audioContext],
  );

  /**
   * Schedules an audio buffer for playback through the audio graph.
   * Ensures sequential playback by scheduling based on the previous buffer's end time.
   *
   * @param {AudioBuffer} buffer - The audio buffer to play
   */
  const scheduleAudioBuffer = useCallback(
    (buffer: AudioBuffer) => {
      if (!audioStreamNode || !audioContext) return;

      try {
        // Create buffer source node
        const sourceNode = audioContext.createBufferSource();

        sourceNode.buffer = buffer;

        // Connect to audio graph
        sourceNode.connect(audioStreamNode.gainNode);

        // Calculate playback timing
        const currentTime = audioContext.currentTime;
        const scheduledTime = Math.max(currentTime, nextPlayTimeRef.current);

        // Schedule playback
        sourceNode.start(scheduledTime);

        // Update next play time
        nextPlayTimeRef.current = scheduledTime + buffer.duration;

        // Store current buffer info
        setCurrentBuffer(buffer);
        setBufferDuration(buffer.duration);

        // Cleanup after playback
        sourceNode.onended = () => {
          sourceNode.disconnect();
        };
      } catch (err) {
        console.error("Failed to schedule audio buffer:", err);
        setError({
          type: "audio",
          message: "Failed to schedule audio playback",
          timestamp: Date.now(),
        });
      }
    },
    [audioStreamNode, audioContext],
  );

  /**
   * Processes all queued audio frames and schedules them for playback.
   */
  const processAudioQueue = useCallback(() => {
    if (!audioContext || !isAudioReady) return;

    const queue = audioBufferQueueRef.current;

    if (queue.length === 0) return;

    // Process frames in queue
    while (queue.length > 0) {
      const frame = queue.shift();

      if (frame) {
        const buffer = createAudioBuffer(frame);

        if (buffer) {
          scheduleAudioBuffer(buffer);
        }
      }
    }
  }, [audioContext, isAudioReady, createAudioBuffer, scheduleAudioBuffer]);

  /**
   * Queues an audio frame for processing and updates the sample rate if needed.
   *
   * @param {AudioFrame} frame - The audio frame to queue
   */
  const queueAudioFrame = useCallback(
    (frame: AudioFrame) => {
      // Update sample rate if changed (even before audio is ready)
      if (frame.sample_rate !== sampleRateRef.current) {
        sampleRateRef.current = frame.sample_rate;
        console.log("Sample rate updated from frame:", frame.sample_rate);
      }

      if (!isAudioReady) {
        return;
      }

      // Add to queue
      audioBufferQueueRef.current.push(frame);

      // Limit queue size to prevent memory issues
      if (audioBufferQueueRef.current.length > maxBufferQueueSizeRef.current) {
        audioBufferQueueRef.current.shift();
        setDroppedFrames((prev) => prev + 1);
        console.warn("Audio buffer queue overflow, dropping frame");
      }

      // Process queue
      processAudioQueue();
    },
    [isAudioReady, processAudioQueue],
  );

  // --- FPS TRACKING ---

  /**
   * Updates the FPS (frames per second) calculation based on frame timestamps.
   * Uses a 10-second rolling window for calculation.
   */
  const updateFps = useCallback(() => {
    const now = Date.now();

    fpsCalculationRef.current.push(now);

    // Keep only timestamps from the last 10 seconds
    const tenSecondsAgo = now - 10000;

    fpsCalculationRef.current = fpsCalculationRef.current.filter(
      (time) => time > tenSecondsAgo,
    );

    // Calculate FPS based on frames from the last 10 seconds
    if (fpsCalculationRef.current.length > 1) {
      const timeSpan = (now - fpsCalculationRef.current[0]) / 1000;
      const calculatedFps = fpsCalculationRef.current.length / timeSpan;

      setFps(Math.round(calculatedFps * 10) / 10); // Round to 1 decimal
    }
  }, []);

  // --- FRAME SIZE TRACKING ---

  /**
   * Calculates the size of a frame in bytes for statistical tracking.
   * This estimates the serialized JSON size of the frame data.
   *
   * @param {AudioFrame} frame - The audio frame to measure
   * @param {string} [rawData] - Optional raw JSON string if available
   * @returns {number} Estimated frame size in bytes
   */
  const calculateFrameSize = useCallback((frame: AudioFrame, rawData?: string): number => {
    if (rawData) {
      // Use actual raw data size if available
      return new TextEncoder().encode(rawData).length;
    }

    if (useFastFormat) {
      // For fast format, estimate based on base64 data + metadata
      const base64Size = Math.ceil((frame.channel_a.length * 2 * 4) * 1.34); // Base64 overhead ~34%
      const metadataSize = 200; // Approximate metadata overhead
      return base64Size + metadataSize;
    } else {
      // For regular format, estimate JSON size
      // Each f32 in JSON is approximately 8-15 characters (including commas, spaces)
      const samplesPerChannel = frame.channel_a.length;
      const totalSamples = samplesPerChannel * 2; // Both channels
      const estimatedJsonSize = totalSamples * 12 + 200; // ~12 chars per number + metadata
      return estimatedJsonSize;
    }
  }, [useFastFormat]);

  /**
   * Updates the frame size statistics with a new frame size.
   * Maintains a rolling window of the last 1000 frame sizes.
   *
   * @param {number} frameSize - Size of the frame in bytes
   */
  const updateFrameSizeStats = useCallback((frameSize: number) => {
    const frameSizes = frameSizesRef.current;

    // Add new frame size
    frameSizes.push(frameSize);

    // Maintain rolling window of max 1000 frames
    if (frameSizes.length > maxFrameSizeHistoryRef.current) {
      frameSizes.shift();
    }

    // Calculate average
    if (frameSizes.length > 0) {
      const sum = frameSizes.reduce((acc, size) => acc + size, 0);
      const average = Math.round(sum / frameSizes.length);
      setAverageFrameSizeBytes(average);
    }
  }, []);

  // --- SERVER-SENT EVENTS HANDLING ---

  /**
   * Decodes a base64-encoded binary audio channel to float32 array.
   *
   * @param {string} base64Data - Base64-encoded binary data
   * @param {number} length - Expected number of samples
   * @param {number} elementSize - Size of each element in bytes
   * @returns {number[]} Decoded float32 array
   */
  const decodeAudioChannel = useCallback(
    (base64Data: string, length: number, elementSize: number): number[] => {
      try {
        // Decode base64 to binary
        const binaryStr = atob(base64Data);
        const bytes = new Uint8Array(binaryStr.length);

        for (let i = 0; i < binaryStr.length; i++) {
          bytes[i] = binaryStr.charCodeAt(i);
        }

        // Convert bytes to float32 array
        const floats: number[] = [];
        const dataView = new DataView(bytes.buffer);

        for (let i = 0; i < length; i++) {
          const offset = i * elementSize;
          const value = dataView.getFloat32(offset, true); // true for little-endian

          floats.push(value);
        }

        return floats;
      } catch (error) {
        console.error("Failed to decode audio channel:", error);

        return new Array(length).fill(0);
      }
    },
    [],
  );

  /**
   * Converts a fast frame format to standard AudioFrame format.
   *
   * @param {AudioFastFrame} fastFrame - The fast frame to convert
   * @returns {AudioFrame} Standard audio frame
   */
  const convertFastFrame = useCallback(
    (fastFrame: AudioFastFrame): AudioFrame => {
      const channel_a = decodeAudioChannel(
        fastFrame.channel_a,
        fastFrame.channels_length,
        fastFrame.channels_element_size,
      );
      const channel_b = decodeAudioChannel(
        fastFrame.channel_b,
        fastFrame.channels_length,
        fastFrame.channels_element_size,
      );

      return {
        channel_a,
        channel_b,
        sample_rate: fastFrame.sample_rate,
        timestamp: fastFrame.timestamp,
        frame_number: fastFrame.frame_number,
        duration_ms: fastFrame.duration_ms,
      };
    },
    [decodeAudioChannel],
  );

  /**
   * Processes a single Server-Sent Event line from the stream.
   * Parses JSON data, validates audio frames, and updates state.
   *
   * @param {string} line - The event stream line to process
   */
  const processServerSentEvent = useCallback(
    async (line: string) => {
      try {
        if (line.startsWith("data:") || line.startsWith("data: ")) {
          const data = line.replace(/^data:\s*/, "");

          if (data === '{"type":"heartbeat"}') {
            return;
          }

          if (!data.trim()) {
            return;
          }

          let frame: AudioFrame;
          let frameSize: number;

          if (useFastFormat) {
            // Parse as fast frame and convert
            const fastFrame: AudioFastFrame = JSON.parse(data);

            // Validate fast frame
            if (
              fastFrame.frame_number !== undefined &&
              fastFrame.channel_a &&
              fastFrame.channel_b &&
              fastFrame.channels_length &&
              fastFrame.sample_rate
            ) {
              frame = convertFastFrame(fastFrame);
              // Calculate frame size from raw JSON data
              frameSize = calculateFrameSize(frame, data);
            } else {
              console.warn("Fast frame validation failed:", fastFrame);
              return;
            }
          } else {
            // Parse as regular frame
            frame = JSON.parse(data);

            // Validate regular frame
            if (
              frame.frame_number === undefined ||
              !frame.channel_a ||
              !frame.channel_b ||
              !frame.sample_rate
            ) {
              console.warn("Frame validation failed:", frame);
              return;
            }

            // Calculate frame size from raw JSON data
            frameSize = calculateFrameSize(frame, data);
          }

          // Update frame size statistics
          updateFrameSizeStats(frameSize);

          // Process the frame (same logic for both formats)
          setCurrentFrame(frame);
          setFrameCount((prev) => prev + 1);
          updateFps();
          queueAudioFrame(frame);

          // Detect missed frames
          const lastFrameNumber = lastFrameTimeRef.current;

          if (lastFrameNumber > 0 && frame.frame_number > lastFrameNumber + 1) {
            const missed = frame.frame_number - lastFrameNumber - 1;

            setDroppedFrames((prev) => prev + missed);
            console.warn(`Missed ${missed} frames`);
          }
          lastFrameTimeRef.current = frame.frame_number;
        }
      } catch (parseError) {
        console.error(
          "Failed to parse audio frame:",
          parseError,
          "Line:",
          line,
        );
        setError({
          type: "parse",
          message: "Failed to parse audio frame data",
          timestamp: Date.now(),
        });
      }
    },
    [updateFps, queueAudioFrame, useFastFormat, convertFastFrame, calculateFrameSize, updateFrameSizeStats],
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

  // --- CONNECTION MANAGEMENT ---

  /**
   * Connects to the audio stream endpoint with authentication.
   * Sets up the stream reader and event handler for incoming audio frames.
   */
  const connect = useCallback(async () => {
    if (!isAuthenticated || !baseUrl || isConnecting || isConnected) {
      console.log("Connect conditions not met:", {
        isAuthenticated,
        baseUrl,
        isConnecting,
        isConnected,
      });

      return;
    }

    try {
      setIsConnecting(true);
      setError(null);

      // Reset frame counters and statistics
      setFrameCount(0);
      setDroppedFrames(0);
      setFps(0);
      setCurrentFrame(null);
      setAverageFrameSizeBytes(0);
      lastFrameTimeRef.current = 0;
      fpsCalculationRef.current = [];
      frameSizesRef.current = [];

      // Get access token
      const accessToken = await getAccessToken();

      if (!accessToken) {
        throw new Error("No access token available");
      }

      // Create stream URL - choose endpoint based on format
      const endpoint = useFastFormat ? "/stream/audio/fast" : "/stream/audio";
      const streamUrl = `${baseUrl}${endpoint}`;

      console.log(
        `Connecting to audio stream at ${streamUrl} (fast: ${useFastFormat})`,
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
    baseUrl,
    getAccessToken,
    isConnecting,
    isConnected,
    processServerSentEvent,
    handleStreamError,
    useFastFormat,
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

    setIsConnected(false);
    setIsConnecting(false);
    setError(null);
    reconnectAttemptsRef.current = 0;
    console.log("Audio stream disconnected");
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
    if (audioStreamNode?.sourceNode) {
      audioStreamNode.sourceNode.disconnect();
    }
    if (audioContext && audioContext.state !== "closed") {
      console.log("Closing audio context in cleanup");
      audioContext.close();
    }
    setAudioContext(null);
    setAudioStreamNode(null);
    setIsAudioReady(false);
    setCurrentBuffer(null);
    audioBufferQueueRef.current = [];
    nextPlayTimeRef.current = 0;
  }, []);

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
      baseUrl &&
      !isConnected &&
      !isConnecting
    ) {
      console.log("Auto-connecting to stream");
      connect();
    }
  }, [
    autoConnect,
    isAuthenticated,
    baseUrl,
    isConnected,
    isConnecting,
    connect,
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
    connect,
    disconnect,
    reconnect,
    initializeAudio,
    resumeAudio,
    suspendAudio,
  };
};
