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
  getPerformanceStats: () => any; // Added performance monitoring
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
  const readerRef = useRef<ReadableStreamDefaultReader<Uint8Array> | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastFrameTimeRef = useRef<number>(0);
  const fpsCalculationRef = useRef<number[]>([]);
  const abortControllerRef = useRef<AbortController | null>(null);

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
  const sampleRateRef = useRef<number>(44100);
  const maxBufferQueueSizeRef = useRef<number>(20); // Increased to prevent frame loss

  /**
   * Performance optimization references
   */
  const processingThrottleRef = useRef<number>(0);
  const fpsDisplayThrottleRef = useRef<number>(0); // Separate throttle for FPS display
  const frameProcessingBatchRef = useRef<string[]>([]);
  const batchProcessingTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
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
   * Performance configuration - optimized for no frame dropping
   */
  const PROCESSING_THROTTLE_MS = 8; // ~120fps processing capability
  const BATCH_SIZE = 8; // Larger batches for efficiency
  const BATCH_TIMEOUT_MS = 4; // Faster timeout for responsiveness
  const FPS_UPDATE_INTERVAL = 200; // Display FPS every 200ms (separate from calculation)
  const BUFFER_POOL_SIZE = 5; // Buffer pool per configuration
  const MAX_PROCESSING_TIME_MS = 12; // Generous time per cycle
  const STATS_UPDATE_INTERVAL = 1000; // Update stats every second

  /**
   * Configuration for reconnection logic
   */
  const maxReconnectAttempts = 5;
  const reconnectDelay = 2000;
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

  // --- OPTIMIZED AUDIO PROCESSING ---

  /**
   * Get buffer pool key for efficient lookups
   */
  const getBufferPoolKey = useCallback((sampleRate: number, length: number): string => {
    return `${sampleRate}_${length}`;
  }, []);

  /**
   * Highly optimized audio buffer creation with advanced pooling
   */
  const createAudioBufferOptimized = useCallback(
    (frame: AudioFrame): AudioBuffer | null => {
      if (!audioContext) return null;

      const startTime = performance.now();

      try {
        const poolKey = getBufferPoolKey(frame.sample_rate, frame.channel_a.length);
        let pool = audioBufferPoolRef.current.get(poolKey);

        if (!pool) {
          pool = [];
          audioBufferPoolRef.current.set(poolKey, pool);
        }

        // Try to reuse buffer from pool
        let buffer = pool.pop();

        if (!buffer) {
          buffer = audioContext.createBuffer(2, frame.channel_a.length, frame.sample_rate);
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
        stats.processingTimeIndex = (stats.processingTimeIndex + 1) % stats.processingTimes.length;

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
  const returnBufferToPool = useCallback((buffer: AudioBuffer) => {
    const poolKey = getBufferPoolKey(buffer.sampleRate, buffer.length);
    const pool = audioBufferPoolRef.current.get(poolKey);

    if (pool && pool.length < BUFFER_POOL_SIZE) {
      pool.push(buffer);
    }
  }, [getBufferPoolKey]);

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
  }, [audioContext, isAudioReady, createAudioBufferOptimized, scheduleAudioBufferOptimized]);

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
        console.warn(`Audio queue length: ${audioBufferQueueRef.current.length}, consider optimization`);
        // Increase queue size dynamically instead of dropping
        maxBufferQueueSizeRef.current = Math.min(50, maxBufferQueueSizeRef.current + 5);
      }

      // Intelligent processing scheduling
      const now = performance.now();
      if (now - lastProcessingTimeRef.current >= PROCESSING_THROTTLE_MS) {
        lastProcessingTimeRef.current = now;

        // Use requestIdleCallback for better performance when available
        if ('requestIdleCallback' in window) {
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
   * Fixed FPS calculation - tracks all frames, throttled display update
   */
  const updateFps = useCallback(() => {
    const now = Date.now();

    // Always add frame timestamp for accurate FPS calculation
    fpsCalculationRef.current.push(now);

    // Keep only last 1 second of data
    const oneSecondAgo = now - 1000;
    fpsCalculationRef.current = fpsCalculationRef.current.filter(
      (time) => time > oneSecondAgo,
    );

    // Only update the display every FPS_UPDATE_INTERVAL ms to reduce UI overhead
    if (now - fpsDisplayThrottleRef.current >= FPS_UPDATE_INTERVAL) {
      fpsDisplayThrottleRef.current = now;

      // Calculate FPS based on all frames from the last 1 second
      if (fpsCalculationRef.current.length > 1) {
        setFps(fpsCalculationRef.current.length);
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

  /**
   * Fixed frame size calculation
   */
  const calculateFrameSize = useCallback((frame: AudioFrame, rawData?: string): number => {
    if (rawData) {
      // Use actual raw data size if available
      return new TextEncoder().encode(rawData).length;
    }

    if (useFastFormat) {
      // For fast format, estimate based on base64 data + metadata
      const base64Size = Math.ceil(frame.channel_a.length * 2 * 4 * 1.34); // Base64 overhead ~34%
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
      bufferPoolSizes: Array.from(audioBufferPoolRef.current.entries()).map(([key, pool]) => ({
        key,
        size: pool.length,
      })),
      processingEfficiency: stats.totalReceivedFrames > 0
        ? Math.round((stats.totalProcessedFrames / stats.totalReceivedFrames) * 100)
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

  /**
   * Add frame to batch with intelligent scheduling
   */
  const addFrameToBatch = useCallback((line: string) => {
    frameProcessingBatchRef.current.push(line);

    if (frameProcessingBatchRef.current.length >= BATCH_SIZE) {
      processBatchedFrames();
    } else if (!batchProcessingTimeoutRef.current) {
      batchProcessingTimeoutRef.current = setTimeout(() => {
        processBatchedFrames();
      }, BATCH_TIMEOUT_MS);
    }
  }, [processBatchedFrames]);

  // --- SERVER-SENT EVENTS HANDLING ---

  /**
   * Fixed server-sent event processing with proper FPS calculation
   */
  const processServerSentEvent = useCallback(
    async (line: string) => {
      try {
        if (!line.startsWith("data:") && !line.startsWith("data: ")) return;

        const data = line.replace(/^data:\s*/, "");
        if (data === '{"type":"heartbeat"}' || !data.trim()) return;

        performanceStatsRef.current.totalReceivedFrames++;

        let frame: AudioFrame;
        let frameSize: number;

        if (useFastFormat) {
          const fastFrame: AudioFastFrame = JSON.parse(data);
          if (
            fastFrame.frame_number !== undefined &&
            fastFrame.channel_a &&
            fastFrame.channel_b &&
            fastFrame.channels_length &&
            fastFrame.sample_rate
          ) {
            frame = convertFastFrameOptimized(fastFrame);
            // Use actual raw data size for fast format
            frameSize = data.length; // Raw JSON size
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
          // Use actual raw data size for normal format
          frameSize = data.length; // Raw JSON size
        }

        // Update statistics with actual data
        updateFrameSizeStats(frameSize);

        // Update frame state every 5th frame to reduce overhead
        if (frameCount % 5 === 0) {
          setCurrentFrame(frame);
        }

        setFrameCount((prev) => prev + 1);
        updateFps(); // Call updateFps for every frame (not throttled)
        queueAudioFrameOptimized(frame);

        // Simplified frame drop detection
        const lastFrameNumber = lastFrameTimeRef.current;
        if (lastFrameNumber > 0 && frame.frame_number > lastFrameNumber + 1) {
          const missed = frame.frame_number - lastFrameNumber - 1;
          setDroppedFrames((prev) => prev + missed);
        }
        lastFrameTimeRef.current = frame.frame_number;
      } catch (parseError) {
        // Reduce error logging frequency
        if (Math.random() < 0.1) {
          console.error("Parse error:", parseError);
        }
      }
    },
    [updateFps, queueAudioFrameOptimized, useFastFormat, convertFastFrameOptimized, updateFrameSizeStats, frameCount],
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

      // Reset frame counters and statistics properly
      setFrameCount(0);
      setDroppedFrames(0);
      setFps(0);
      setCurrentFrame(null);
      setAverageFrameSizeBytes(0);
      lastFrameTimeRef.current = 0;
      fpsCalculationRef.current = [];
      fpsDisplayThrottleRef.current = 0; // Reset FPS display throttle
      frameSizesRef.current = []; // Reset to empty array

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
    audioBufferQueueRef.current = [];
    nextPlayTimeRef.current = 0;
  }, [audioStreamNode, audioContext, resetPerformanceStats]);

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
    getPerformanceStats,
  };
};
