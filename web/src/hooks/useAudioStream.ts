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
    type: "connection" | "auth" | "parse" | "network" | "audio";
    message: string;
    timestamp: number;
}

interface AudioStreamNode {
    context: AudioContext;
    sourceNode: AudioBufferSourceNode | null;
    gainNode: GainNode;
    analyserNode: AnalyserNode;
    outputNode: AudioNode;
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

export const useAudioStream = (baseUrl?: string, autoConnect: boolean = false): UseAudioStreamReturn => {
    const { getAccessToken, isAuthenticated } = useAuth();

    // Connection states
    const [isConnected, setIsConnected] = useState(false);
    const [isConnecting, setIsConnecting] = useState(false);
    const [error, setError] = useState<StreamError | null>(null);
    const [currentFrame, setCurrentFrame] = useState<AudioFrame | null>(null);
    const [frameCount, setFrameCount] = useState(0);
    const [droppedFrames, setDroppedFrames] = useState(0);
    const [fps, setFps] = useState(0);

    // Audio reconstruction states
    const [audioContext, setAudioContext] = useState<AudioContext | null>(null);
    const [audioStreamNode, setAudioStreamNode] = useState<AudioStreamNode | null>(null);
    const [isAudioReady, setIsAudioReady] = useState(false);
    const [currentBuffer, setCurrentBuffer] = useState<AudioBuffer | null>(null);
    const [bufferDuration, setBufferDuration] = useState(0);
    const [latency, setLatency] = useState(0);

    // References for stream handling
    const readerRef = useRef<ReadableStreamDefaultReader<Uint8Array> | null>(null);
    const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const lastFrameTimeRef = useRef<number>(0);
    const fpsCalculationRef = useRef<number[]>([]);
    const abortControllerRef = useRef<AbortController | null>(null);

    // Audio reconstruction references
    const audioBufferQueueRef = useRef<AudioFrame[]>([]);
    const nextPlayTimeRef = useRef<number>(0);
    const sampleRateRef = useRef<number>(44100); // TODO: Make this dynamic based on the first frame
    const bufferSizeRef = useRef<number>(4096); // Buffer size in samples
    const maxBufferQueueSizeRef = useRef<number>(10); // Maximum frames to queue

    // Configuration
    const maxReconnectAttempts = 5;
    const reconnectDelay = 2000; // 2 seconds
    const reconnectAttemptsRef = useRef(0);

    // Initialize AudioContext and create audio graph
    const initializeAudio = useCallback(async () => {
        try {
            console.log('initializeAudio called, current audioContext:', audioContext);

            if (audioContext && audioContext.state !== 'closed') {
                console.log('Closing existing audio context');
                await audioContext.close();
            }

            console.log('Creating new AudioContext with sample rate:', sampleRateRef.current);

            const context = new AudioContext({
                sampleRate: sampleRateRef.current,
                latencyHint: 'interactive'
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
                outputNode: analyserNode
            };

            console.log('Setting audio context and stream node');
            setAudioContext(context);
            setAudioStreamNode(streamNode);
            setIsAudioReady(true);
            setLatency(context.baseLatency + context.outputLatency);

            console.log('Audio context initialized successfully', {
                sampleRate: context.sampleRate,
                requestedSampleRate: sampleRateRef.current,
                latency: context.baseLatency + context.outputLatency,
                state: context.state
            });

        } catch (err) {
            console.error('Failed to initialize audio context:', err);
            setError({
                type: 'audio',
                message: err instanceof Error ? err.message : 'Failed to initialize audio',
                timestamp: Date.now()
            });
            setIsAudioReady(false);
        }
    }, []); // Remove all dependencies to prevent circular references

    // Resume audio context
    const resumeAudio = useCallback(async () => {
        if (audioContext && audioContext.state === 'suspended') {
            await audioContext.resume();
            console.log('Audio context resumed');
        }
    }, [audioContext]);

    // Suspend audio context
    const suspendAudio = useCallback(async () => {
        if (audioContext && audioContext.state === 'running') {
            await audioContext.suspend();
            console.log('Audio context suspended');
        }
    }, [audioContext]);

    // Convert audio frame to AudioBuffer
    const createAudioBuffer = useCallback((frame: AudioFrame): AudioBuffer | null => {
        if (!audioContext) return null;

        try {
            const buffer = audioContext.createBuffer(
                2, // stereo
                frame.channel_a.length,
                frame.sample_rate
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
            console.error('Failed to create audio buffer:', err);
            return null;
        }
    }, [audioContext]);

    // Schedule audio buffer playback
    const scheduleAudioBuffer = useCallback((buffer: AudioBuffer) => {
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
            console.error('Failed to schedule audio buffer:', err);
            setError({
                type: 'audio',
                message: 'Failed to schedule audio playback',
                timestamp: Date.now()
            });
        }
    }, [audioStreamNode, audioContext]);

    // Process queued audio frames
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

    // Queue audio frame for processing
    const queueAudioFrame = useCallback((frame: AudioFrame) => {
        // Update sample rate if changed (even before audio is ready)
        if (frame.sample_rate !== sampleRateRef.current) {
            sampleRateRef.current = frame.sample_rate;
            console.log('Sample rate updated from frame:', frame.sample_rate);
        }

        if (!isAudioReady) {
            return;
        }

        // Add to queue
        audioBufferQueueRef.current.push(frame);

        // Limit queue size to prevent memory issues
        if (audioBufferQueueRef.current.length > maxBufferQueueSizeRef.current) {
            audioBufferQueueRef.current.shift();
            setDroppedFrames(prev => prev + 1);
            console.warn('Audio buffer queue overflow, dropping frame');
        }

        // Process queue
        processAudioQueue();
    }, [isAudioReady, processAudioQueue]);

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

    // Function to process Server-Sent Events
    const processServerSentEvent = useCallback(async (line: string) => {
        try {
            // console.log('Processing SSE line:', line.substring(0, 100) + '...'); // Debug log

            // Process SSE lines (format: "data:{json}" or data: {json})
            if (line.startsWith("data:") || line.startsWith("data: ")) {
                const data = line.replace(/^data:\s*/, ""); // Remove "data:" prefix

                // Handle heartbeats
                if (data === '{"type":"heartbeat"}') {
                    // console.log("Heartbeat received");
                    return;
                }

                // Skip empty data lines
                if (!data.trim()) {
                    return;
                }

                // console.log('Parsing frame data:', data.substring(0, 200) + '...'); // Debug log

                // Parse audio frame
                const frame: AudioFrame = JSON.parse(data);

                // console.log('Parsed frame:', {
                //     frame_number: frame.frame_number,
                //     sample_rate: frame.sample_rate,
                //     channel_a_length: frame.channel_a?.length,
                //     channel_b_length: frame.channel_b?.length,
                //     timestamp: frame.timestamp,
                //     duration_ms: frame.duration_ms
                // });

                // Validate frame - be more lenient with validation
                if (frame.frame_number !== undefined && frame.channel_a && frame.channel_b && frame.sample_rate) {
                    // console.log('Frame validation passed, processing frame', frame.frame_number);

                    setCurrentFrame(frame);
                    setFrameCount((prev) => {
                        const newCount = prev + 1;
                        // console.log('Frame count updated:', newCount);
                        return newCount;
                    });
                    updateFps();

                    // Queue frame for audio processing
                    queueAudioFrame(frame);

                    // Detect missed frames
                    const lastFrameNumber = lastFrameTimeRef.current;
                    if (lastFrameNumber > 0 && frame.frame_number > lastFrameNumber + 1) {
                        const missed = frame.frame_number - lastFrameNumber - 1;
                        setDroppedFrames((prev) => prev + missed);
                        console.warn(`Missed ${missed} frames`);
                    }
                    lastFrameTimeRef.current = frame.frame_number;
                } else {
                    console.warn('Frame validation failed:', {
                        hasFrameNumber: frame.frame_number !== undefined,
                        hasChannelA: !!frame.channel_a,
                        hasChannelB: !!frame.channel_b,
                        hasSampleRate: !!frame.sample_rate,
                        frame: frame
                    });
                }
            } else {
                console.log('Non-data SSE line:', line);
            }
        } catch (parseError) {
            console.error("Failed to parse audio frame:", parseError, 'Line:', line);
            setError({
                type: "parse",
                message: "Failed to parse audio frame data",
                timestamp: Date.now(),
            });
        }
    }, [updateFps, queueAudioFrame]);

    // Function to handle stream errors
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
    }, []);

    // Connection function
    const connect = useCallback(async () => {
        if (!isAuthenticated || !baseUrl || isConnecting || isConnected) {
            console.log('Connect conditions not met:', { isAuthenticated, baseUrl, isConnecting, isConnected });
            return;
        }

        try {
            setIsConnecting(true);
            setError(null);

            // Reset frame counters
            setFrameCount(0);
            setDroppedFrames(0);
            setFps(0);
            setCurrentFrame(null);
            lastFrameTimeRef.current = 0;
            fpsCalculationRef.current = [];

            // Get access token
            const accessToken = await getAccessToken();
            if (!accessToken) {
                throw new Error("No access token available");
            }

            // Create stream URL - fix the URL construction
            const streamUrl = `${baseUrl}/stream/audio`;
            console.log(`Connecting to audio stream at ${streamUrl}`);

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
    }, [isAuthenticated, baseUrl, getAccessToken, isConnecting, isConnected, processServerSentEvent, handleStreamError]);

    // Disconnect function
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

    // Cleanup audio resources
    const cleanupAudio = useCallback(() => {
        console.log('cleanupAudio called');
        if (audioStreamNode?.sourceNode) {
            audioStreamNode.sourceNode.disconnect();
        }
        if (audioContext && audioContext.state !== 'closed') {
            console.log('Closing audio context in cleanup');
            audioContext.close();
        }
        setAudioContext(null);
        setAudioStreamNode(null);
        setIsAudioReady(false);
        setCurrentBuffer(null);
        audioBufferQueueRef.current = [];
        nextPlayTimeRef.current = 0;
    }, []); // Remove dependencies to prevent frequent recreations

    // Cleanup effect
    useEffect(() => {
        return () => {
            console.log('Component cleanup effect triggered');
            disconnect();
            cleanupAudio();
        };
    }, []); // Empty dependency array - only run on mount/unmount

    // Auto-connect when the user is authenticated and baseUrl is available
    useEffect(() => {
        if (autoConnect && isAuthenticated && baseUrl && !isConnected && !isConnecting) {
            console.log('Auto-connecting to stream');
            connect();
        }
    }, [autoConnect, isAuthenticated, baseUrl, isConnected, isConnecting, connect]);

    return {
        isConnected,
        isConnecting,
        error,
        currentFrame,
        frameCount,
        droppedFrames,
        fps,
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
