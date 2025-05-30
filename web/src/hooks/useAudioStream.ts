/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 */

import { useState, useEffect, useRef, useCallback } from "react";

import { useAuth } from "@/authentication";

interface AudioFrame {
  channel_a: number[];
  channel_b: number[];
  sample_rate: number;
  timestamp: number;
  frame_number: number;
  duration_ms: number;
}

interface StreamError {
  type: "connection" | "auth" | "parse" | "network";
  message: string;
  timestamp: number;
}

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

  // Controls
  connect: () => void;
  disconnect: () => void;
  reconnect: () => void;
}

export const useAudioStream = (baseUrl?: string): UseAudioStreamReturn => {
  const { getAccessToken, isAuthenticated } = useAuth(); // States
  const [isConnected, setIsConnected] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<StreamError | null>(null);
  const [currentFrame, setCurrentFrame] = useState<AudioFrame | null>(null);
  const [frameCount, setFrameCount] = useState(0);
  const [droppedFrames, setDroppedFrames] = useState(0);
  const [fps, setFps] = useState(0);

  // References
  const readerRef = useRef<ReadableStreamDefaultReader<Uint8Array> | null>(
    null,
  );
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );
  const lastFrameTimeRef = useRef<number>(-1);
  const fpsCalculationRef = useRef<number[]>([]);
  const abortControllerRef = useRef<AbortController | null>(null); // Configuration
  const maxReconnectAttempts = 5;
  const reconnectDelay = 2000; // 2 seconds
  const reconnectAttemptsRef = useRef(0);

  // FPS calculation
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

  // Connection function
  const connect = useCallback(async () => {
    if (!isAuthenticated || !baseUrl || isConnecting || isConnected) {
      return;
    }

    try {
      setIsConnecting(true);
      setError(null); // Get access token
      const accessToken = await getAccessToken();

      if (!accessToken) {
        throw new Error("No access token available");
      }

      // Create stream URL
      const streamUrl = `${baseUrl}/stream/audio`;

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
      setIsConnected(true);
      setIsConnecting(false);
      setError(null);
      reconnectAttemptsRef.current = 0;

      // Reset counters for new connection
      setFrameCount(0);
      setDroppedFrames(0);
      setFps(0);
      lastFrameTimeRef.current = -1;
      fpsCalculationRef.current = [];

      // Buffer for partial data
      let buffer = "";

      // Read the stream
      const readStream = async () => {
        try {
          while (true) {
            const { done, value } = await reader.read();

            if (done) {
              break;
            }

            // Decode the data
            const chunk = decoder.decode(value, { stream: true });

            buffer += chunk;

            // Process complete lines
            const lines = buffer.split("\n");

            buffer = lines.pop() || ""; // Keep the partial line

            for (const line of lines) {
              if (line.trim()) {
                console.log("Received SSE line:", line.trim());
                await processServerSentEvent(line.trim());
              }
            }
          }
        } catch (err) {
          if (err instanceof Error && err.name === "AbortError") {
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
    updateFps,
  ]); // Function to process Server-Sent Events
  const processServerSentEvent = async (line: string) => {
    try {
      // Process SSE lines (format: "data: {json}")
      if (line.startsWith("data: ")) {
        const data = line.substring(6); // Remove "data: "

        console.log("Processing SSE data:", data.substring(0, 100) + "...");

        // Handle heartbeats - don't count these as frames
        if (data === '{"type":"heartbeat"}') {
          console.log("Received heartbeat");

          return;
        }

        // Parse audio frame
        const frame: AudioFrame = JSON.parse(data);

        console.log(
          "Parsed frame:",
          frame.frame_number,
          "channels:",
          frame.channel_a?.length,
          frame.channel_b?.length,
        );

        // Validate frame structure
        if (
          typeof frame.frame_number === "number" &&
          Array.isArray(frame.channel_a) &&
          Array.isArray(frame.channel_b) &&
          frame.channel_a.length > 0 &&
          frame.channel_b.length > 0
        ) {
          console.log("Frame validation passed, updating state");
          setCurrentFrame(frame);

          // Increment total frame count
          setFrameCount((prev) => {
            console.log("Updating frame count from", prev, "to", prev + 1);

            return prev + 1;
          });

          // Update FPS calculation
          updateFps();

          // Detect dropped frames by comparing frame numbers
          const lastFrameNumber = lastFrameTimeRef.current;

          if (lastFrameNumber >= 0) {
            const expectedNextFrame = lastFrameNumber + 1;

            if (frame.frame_number > expectedNextFrame) {
              // We missed some frames
              const missed = frame.frame_number - expectedNextFrame;

              console.log("Detected", missed, "dropped frames");
              setDroppedFrames((prev) => prev + missed);
            }
          }

          // Update the last frame number reference
          lastFrameTimeRef.current = frame.frame_number;
        } else {
          console.log("Frame validation failed:", {
            frame_number: typeof frame.frame_number,
            channel_a: Array.isArray(frame.channel_a)
              ? frame.channel_a.length
              : "not array",
            channel_b: Array.isArray(frame.channel_b)
              ? frame.channel_b.length
              : "not array",
          });
        }
      } else {
        console.log("Non-data SSE line:", line);
      }
    } catch (parseError) {
      console.error("Parse error:", parseError);
      setError({
        type: "parse",
        message: "Failed to parse audio frame data",
        timestamp: Date.now(),
      });
    }
  }; // Function to handle stream errors
  const handleStreamError = (err: any) => {
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

    // Automatic reconnection attempt
    if (reconnectAttemptsRef.current < maxReconnectAttempts) {
      reconnectAttemptsRef.current++;
      console.log(
        `Attempting to reconnect (${reconnectAttemptsRef.current}/${maxReconnectAttempts})`,
      );

      reconnectTimeoutRef.current = setTimeout(() => {
        connect();
      }, reconnectDelay * reconnectAttemptsRef.current); // Progressive delay
    } else {
      console.error("Max reconnection attempts reached");
      setError({
        type: "connection",
        message: "Max reconnection attempts reached",
        timestamp: Date.now(),
      });
    }
  }; // Disconnect function
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

  // Manual reconnect function
  const reconnect = useCallback(() => {
    disconnect();
    setTimeout(() => {
      connect();
    }, 500);
  }, [disconnect, connect]);

  // Cleanup effect
  useEffect(() => {
    return () => {
      disconnect();
    };
  }, [disconnect]);

  // Auto-connect when the user is authenticated and baseUrl is available
  useEffect(() => {
    if (isAuthenticated && baseUrl && !isConnected && !isConnecting) {
      connect();
    }
  }, [isAuthenticated, baseUrl, isConnected, isConnecting, connect]);

  return {
    isConnected,
    isConnecting,
    error,
    currentFrame,
    frameCount,
    droppedFrames,
    fps,
    connect,
    disconnect,
    reconnect,
  };
};
