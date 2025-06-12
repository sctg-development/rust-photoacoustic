/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 *
 * React hook for managing a real-time authenticated audio stream with WebAudio API integration.
 * Provides functionality for connecting to a server-sent events stream, processing audio frames,
 * and managing audio context lifecycle including reconnection strategies.
 */
import { useTranslation } from "react-i18next";
import { useEffect, useState, useRef, useCallback } from "react";
import { Button } from "@heroui/button";
import { Card, CardBody, CardHeader } from "@heroui/card";
import { Progress } from "@heroui/progress";
import { Chip } from "@heroui/chip";
import { Switch } from "@heroui/switch";
// @ts-ignore - audiomotion-analyzer doesn't have TypeScript definitions
import AudioMotionAnalyzer from "audiomotion-analyzer";

import {
  getGenerixConfig,
  GenerixConfig,
} from "../authentication/providers/generix-config";

import { title } from "@/components/primitives";
import DefaultLayout from "@/layouts/default";
import { useAuth } from "@/authentication";
import {
  TimestampValidationConfig,
  useAudioStream,
} from "@/hooks/useAudioStream";

/**
 * ApiPage Component - Real-time Audio Streaming and Visualization
 *
 * This component provides a comprehensive interface for:
 * 1. Connecting to a real-time audio stream from a Rust backend
 * 2. Visualizing the audio data using a spectrum analyzer
 * 3. Monitoring stream statistics and connection health
 *
 * Architecture Overview:
 * - Uses WebSocket to receive audio frames from the backend
 * - Processes audio data through Web Audio API
 * - Visualizes spectrum using AudioMotion analyzer
 * - Manages multiple React effects for proper initialization order
 */
export default function ApiPage() {
  const { t } = useTranslation();
  const { user, isAuthenticated, getAccessToken } = useAuth();

  // Configuration state - holds API endpoints and authentication details
  const [generixConfig, setGenerixConfig] = useState(
    null as GenerixConfig | null
  );
  const [accessToken, setAccessToken] = useState("" as string | null);

  // Audio analyzer states - manages the visual spectrum analyzer
  const [audioAnalyzer, setAudioAnalyzer] = useState<any>(null);
  const [isAnalyzerInitialized, setIsAnalyzerInitialized] = useState(false);
  const [showAnalyzer, setShowAnalyzer] = useState(true);
  const analyzerContainerRef = useRef<HTMLDivElement>(null);

  /**
   * Audio Stream Hook - Core audio processing functionality
   *
   * This hook manages:
   * - WebSocket connection to the audio stream endpoint
   * - Audio frame processing and buffering
   * - Web Audio API integration for real-time playback
   * - Statistics tracking (FPS, dropped frames, etc.)
   *
   * Flow: WebSocket → Audio Frames → Web Audio API → Analyzer
   */
  const {
    isConnected, // WebSocket connection status
    isConnecting, // Connection attempt in progress
    error, // Connection or processing errors
    currentFrame, // Latest audio frame data
    frameCount, // Total frames received
    droppedFrames, // Frames lost due to processing delays
    fps, // Current frames per second
    connect, // Initiate WebSocket connection
    disconnect, // Close WebSocket connection
    initializeAudio, // Set up Web Audio API context
    audioContext, // Web Audio API context instance
    audioStreamNode, // Audio processing node with analyzer
    isAudioReady, // Audio system fully initialized
    averageFrameSizeBytes, // Average size of audio frames
  } = useAudioStream(
    generixConfig
      ? `${generixConfig.api_base_url}/stream/audio/fast`
      : undefined, // Stream endpoint URL - WebSocket for real-time audio
    generixConfig
      ? `${generixConfig.api_base_url}/stream/audio/fast/stats`
      : undefined, // Stats endpoint URL - HTTP for stream statistics
    false, // Disable auto-connect - we manage connection manually
    {
      enabled: false, // Disable timestamp validation for now
    } as TimestampValidationConfig
  );

  /**
   * Initialize Audio Spectrum Analyzer
   *
   * This function creates and configures the AudioMotion analyzer which:
   * 1. Connects to the Web Audio API analyser node
   * 2. Renders real-time frequency spectrum visualization
   * 3. Provides various display modes and customization options
   *
   * Prerequisites:
   * - audioContext must be in 'running' state
   * - audioStreamNode must be created and connected
   * - DOM container must be available
   */
  const initializeAnalyzer = async () => {
    if (!analyzerContainerRef.current) {
      console.warn("Cannot initialize analyzer: container not ready");

      return;
    }

    if (!audioContext || !audioStreamNode) {
      console.warn(
        "Cannot initialize analyzer: audio context or stream node not ready",
        {
          hasAudioContext: !!audioContext,
          hasAudioStreamNode: !!audioStreamNode,
          isAudioReady,
        }
      );

      return;
    }

    try {
      // Clean up existing analyzer to prevent memory leaks
      if (audioAnalyzer) {
        console.log("Cleaning up existing analyzer");
        audioAnalyzer.destroy();
      }

      console.log("Initializing analyzer with:", {
        sampleRate: audioContext.sampleRate,
        analyserNode: audioStreamNode.analyserNode,
        containerReady: !!analyzerContainerRef.current,
      });

      // Create new analyzer instance with optimized settings for photoacoustic data
      const analyzer = new AudioMotionAnalyzer(analyzerContainerRef.current, {
        source: audioStreamNode.analyserNode, // Connect to the Web Audio API analyser node
        height: 300, // Fixed height for consistent display
        mode: 3, // 1/3-octave bands - good for acoustic analysis
        showBgColor: true, // Enhanced visual appeal
        bgAlpha: 0.7, // Semi-transparent background
        overlay: true, // Allow UI elements over visualization
        showPeaks: true, // Show frequency peaks for analysis
        showFPS: true, // Display rendering performance
        showScaleY: true, // Show amplitude scale
        connectSpeakers: false, // IMPORTANT: Prevent audio feedback
        barSpace: 0.1, // Spacing between frequency bars
        ledBars: false, // Smooth bars instead of LED style
        lumiBars: false, // Standard brightness
        radial: false, // Linear display (not circular)
        reflexRatio: 0.3, // Bottom reflection intensity
        gradient: "rainbow", // Color scheme for frequency bands
        linearAmplitude: true, // Linear amplitude scale (not logarithmic)
        linearBoost: 1.8, // Boost for better visibility
        maxDecibels: -10, // Upper limit for dB scale
        minDecibels: -85, // Lower limit for dB scale
        smoothing: 0.8, // Temporal smoothing factor (0-1)
        channelLayout: "dual-horizontal", // Show both channels side by side
      });

      console.log("Audio analyzer initialized successfully");
      setAudioAnalyzer(analyzer);
      setIsAnalyzerInitialized(true);
    } catch (error) {
      console.error("Failed to initialize audio analyzer:", error);
      setIsAnalyzerInitialized(false);
    }
  };

  /**
   * Clean up Audio Analyzer Resources
   *
   * Properly destroys the analyzer instance to prevent:
   * - Memory leaks from animation frames
   * - Lingering event listeners
   * - Canvas context issues
   */
  const cleanupAnalyzer = () => {
    if (audioAnalyzer) {
      try {
        audioAnalyzer.destroy();
        console.log("Audio analyzer cleaned up");
      } catch (error) {
        console.error("Error cleaning up analyzer:", error);
      }
      setAudioAnalyzer(null);
      setIsAnalyzerInitialized(false);
    }
  };

  /**
   * Conditional Analyzer Initialization
   *
   * This callback ensures the analyzer is only initialized when ALL conditions are met:
   * 1. Audio system is ready (Web Audio API initialized)
   * 2. Audio context is running
   * 3. Audio stream node is created and connected
   * 4. User wants to show the analyzer
   * 5. Analyzer is not already initialized
   *
   * This prevents race conditions and ensures proper initialization order.
   */
  const initializeAnalyzerIfReady = useCallback(async () => {
    if (
      isAudioReady &&
      audioContext &&
      audioStreamNode &&
      showAnalyzer &&
      !isAnalyzerInitialized
    ) {
      console.log("Conditions met, initializing analyzer");
      await initializeAnalyzer();
    }
  }, [
    isAudioReady,
    audioContext,
    audioStreamNode,
    showAnalyzer,
    isAnalyzerInitialized,
  ]);

  /**
   * Auto-Connection Logic
   *
   * Automatically connects to the audio stream once the audio system is ready.
   * This provides a seamless user experience by eliminating manual connection steps
   * when the system is properly initialized.
   *
   * Conditions for auto-connection:
   * - Audio context is ready and running
   * - Not currently connected or connecting
   * - No existing connection errors
   */
  const autoConnectIfNeeded = useCallback(() => {
    if (isAudioReady && !isConnected && !isConnecting) {
      console.log("Audio ready, auto-connecting to stream");
      connect();
    }
  }, [isAudioReady, isConnected, isConnecting, connect]);

  // Effect 1: Handle analyzer initialization
  // Triggers when audio system becomes ready or analyzer settings change
  useEffect(() => {
    console.log("Analyzer effect triggered:", {
      isAudioReady,
      hasAudioContext: !!audioContext,
      hasAudioStreamNode: !!audioStreamNode,
      showAnalyzer,
      isAnalyzerInitialized,
    });

    initializeAnalyzerIfReady();
  }, [initializeAnalyzerIfReady]);

  // Effect 2: Handle auto-connection after audio is ready
  // Ensures seamless connection flow without user intervention
  useEffect(() => {
    autoConnectIfNeeded();
  }, [autoConnectIfNeeded]);

  // Effect 3: Handle analyzer cleanup when visibility is toggled
  // Immediately cleans up resources when analyzer is hidden
  useEffect(() => {
    if (!showAnalyzer && isAnalyzerInitialized) {
      console.log("Show analyzer disabled, cleaning up analyzer");
      cleanupAnalyzer();
    }
  }, [showAnalyzer, isAnalyzerInitialized]);

  // Effect 4: Handle component unmount cleanup
  // Prevents memory leaks when component is destroyed
  useEffect(() => {
    return () => {
      if (isAnalyzerInitialized) {
        console.log("Component unmounting, cleaning up analyzer");
        cleanupAnalyzer();
      }
    };
  }, [isAnalyzerInitialized]);

  /**
   * Handle Analyzer Visibility Toggle
   *
   * Manages the show/hide state of the spectrum analyzer with proper cleanup.
   * When hiding: immediately destroys analyzer to free resources
   * When showing: waits for DOM to be ready before initializing
   */
  const handleAnalyzerToggle = (visible: boolean) => {
    console.log("Analyzer toggle:", visible);
    setShowAnalyzer(visible);
    if (!visible) {
      cleanupAnalyzer();
    } else if (isAudioReady && audioContext && audioStreamNode) {
      // Small delay ensures DOM container is ready for initialization
      setTimeout(() => initializeAnalyzer(), 100);
    }
  };

  // Configuration loading effects
  useEffect(() => {
    /**
     * Load Generix Configuration
     *
     * Fetches API endpoints and connection settings from the authentication provider.
     * This configuration determines where to connect for audio streaming.
     */
    const loadGenerixConfig = async () => {
      const config = await getGenerixConfig();

      console.log("Config is :", config);
      setGenerixConfig(config);
    };

    /**
     * Load Access Token
     *
     * Retrieves the current authentication token for API requests.
     * Required for accessing protected audio streaming endpoints.
     */
    const loadAccessToken = async () => {
      const token = await getAccessToken();

      setAccessToken(token);
    };

    loadAccessToken();
    loadGenerixConfig();
  }, []);

  // Authentication state monitoring
  useEffect(() => {
    // Connected user is authenticated and the route is protected with the access token and the right permissions
    if (isAuthenticated && generixConfig && user) {
      console.log(
        "User is authenticated, Generix config and user are available."
      );
      console.log("Access Token:", accessToken);
    } else {
      console.log(
        "User is not authenticated or Generix config/user is not available."
      );
    }
  }, [accessToken, generixConfig, isAuthenticated, user]);

  /**
   * Manual Audio Initialization Handler
   *
   * Provides a manual trigger for audio system initialization.
   * This is necessary because browsers require user interaction before
   * allowing audio context creation (security/UX policy).
   *
   * Steps:
   * 1. Create and start Web Audio API context
   * 2. Set up audio processing nodes
   * 3. Prepare for incoming audio stream data
   */
  const handleInitializeAudio = useCallback(async () => {
    console.log("Initialize audio button clicked");
    try {
      await initializeAudio();
      console.log("Audio initialization completed");
    } catch (error) {
      console.error("Failed to initialize audio:", error);
    }
  }, [initializeAudio]);

  return (
    <DefaultLayout>
      <section className="flex flex-col items-center justify-center gap-4 py-8 md:py-10">
        <div className="inline-block max-w-lg text-center justify-center">
          <h1 className={title()}>{t("audio-streaming-test")}</h1>
        </div>

        {/* Main dashboard grid - Connection Status, Statistics, and Current Frame Info */}
        <div className="w-full max-w-4xl grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 lg:h-96 lg:max-h-96 gap-4">
          {/* Connection Status Card - Shows real-time system status */}
          <Card className="w-full">
            <CardHeader className="pb-2">
              <h3 className="text-lg font-semibold">
                {t("connection-status")}
              </h3>
            </CardHeader>
            <CardBody>
              <div className="flex flex-col gap-3">
                {/* WebSocket Connection Status */}
                <div className="flex items-center justify-between">
                  <span>{t("status")}:</span>
                  <Chip
                    color={
                      isConnected
                        ? "success"
                        : isConnecting
                          ? "warning"
                          : "danger"
                    }
                    variant="flat"
                  >
                    {isConnected
                      ? t("connected")
                      : isConnecting
                        ? "Connecting..."
                        : t("disconnected")}
                  </Chip>
                </div>

                {/* Audio System Readiness */}
                <div className="flex items-center justify-between">
                  <span>{t("audio-ready")}</span>
                  <Chip
                    color={isAudioReady ? "success" : "default"}
                    variant="flat"
                  >
                    {isAudioReady ? t("ready") : t("not-ready")}
                  </Chip>
                </div>

                {/* Web Audio API Context State */}
                <div className="flex items-center justify-between">
                  <span>{t("audiocontext-state")}</span>
                  <Chip
                    color={
                      audioContext?.state === "running" ? "success" : "warning"
                    }
                    size="sm"
                    variant="flat"
                  >
                    {audioContext?.state || t("none")}
                  </Chip>
                </div>

                {/* Spectrum Analyzer Status */}
                <div className="flex items-center justify-between">
                  <span>{t("analyzer")}:</span>
                  <Chip
                    color={isAnalyzerInitialized ? "success" : "default"}
                    variant="flat"
                  >
                    {isAnalyzerInitialized ? t("active") : t("inactive")}
                  </Chip>
                </div>

                {/* Control Buttons - Context-sensitive based on system state */}
                <div className="flex gap-2">
                  {/* Step 1: Initialize Audio Context (requires user interaction) */}
                  {!isAudioReady && (
                    <Button
                      color="primary"
                      size="sm"
                      onPress={handleInitializeAudio}
                    >
                      {t("initialize-audio-context")}
                    </Button>
                  )}

                  {/* Step 2: Connect to Stream (automatic after audio ready) */}
                  {isAudioReady && !isConnected && !isConnecting && (
                    <Button
                      aria-label={t("connect-to-audio-stream")}
                      color="primary"
                      size="sm"
                      onPress={connect}
                    >
                      {t("connect")}
                    </Button>
                  )}

                  {/* Disconnect Control */}
                  {isConnected && (
                    <Button
                      aria-label={t("disconnect-from-audio-stream")}
                      color="danger"
                      size="sm"
                      onPress={disconnect}
                    >
                      {t("disconnect")}
                    </Button>
                  )}
                </div>

                {/* Analyzer Visibility Toggle */}
                <div className="flex items-center justify-between">
                  <span>{t("show-analyzer")}</span>
                  <Switch
                    aria-label={t("toggle-audio-analyzer-visibility")}
                    isSelected={showAnalyzer}
                    onValueChange={handleAnalyzerToggle}
                  />
                </div>

                {/* Error Display - Shows connection or processing errors */}
                {error && (
                  <div className="text-red-500 text-sm bg-red-50 p-2 rounded">
                    <strong>
                      {t("error")} ({error.type}):
                    </strong>{" "}
                    {error.message}
                  </div>
                )}
              </div>
            </CardBody>
          </Card>

          {/* Stream Statistics Card - Performance metrics and health indicators */}
          <Card className="w-full">
            <CardHeader className="pb-2">
              <h3 className="text-lg font-semibold">{t("statistics")}</h3>
            </CardHeader>
            <CardBody>
              <div className="flex flex-col gap-3">
                {/* Total frames successfully processed */}
                <div className="flex justify-between">
                  <span>{t("frames-received")}:</span>
                  <span className="font-bold text-blue-600">{frameCount}</span>
                </div>

                {/* Frames lost due to processing delays or network issues */}
                <div className="flex justify-between">
                  <span>{t("frames-lost")}:</span>
                  <span
                    className={`font-bold ${droppedFrames > 0 ? "text-red-600" : "text-green-600"}`}
                  >
                    {droppedFrames}
                  </span>
                </div>

                {/* Current processing rate */}
                <div className="flex justify-between">
                  <span>FPS:</span>
                  <span className="font-bold text-purple-600">
                    {fps.toFixed(1)}
                  </span>
                </div>

                {/* Network usage indicator */}
                <div className="flex justify-between">
                  <span>{t("average-frame-size")}:</span>
                  <span className="font-mono">
                    {(averageFrameSizeBytes / 1024).toFixed(2)} kB
                  </span>
                </div>

                {/* FPS Performance Indicator - Visual health check */}
                <div className="mt-2">
                  <div className="flex justify-between text-sm mb-1">
                    <span>{t("fps-performance")}</span>
                    <span>{fps.toFixed(1)}/8</span>
                  </div>
                  <Progress
                    aria-label={t("fps-performance-progress")}
                    color={
                      fps > 4.9 ? "success" : fps > 4 ? "warning" : "danger"
                    }
                    size="sm"
                    value={Math.min((fps / 8) * 100, 100)}
                  />
                </div>
              </div>
            </CardBody>
          </Card>

          {/* Current Frame Information Card - Detailed view of latest audio data */}
          <Card className="w-full">
            <CardHeader className="pb-2">
              <h3 className="text-lg font-semibold">{t("current-frame")}</h3>
            </CardHeader>
            <CardBody>
              {currentFrame ? (
                <div className="flex flex-col gap-2 text-sm">
                  {/* Frame sequence number for tracking */}
                  <div className="flex justify-between">
                    <span>{t("number")}:</span>
                    <span className="font-mono">
                      {currentFrame.frame_number}
                    </span>
                  </div>

                  {/* Server-side timestamp */}
                  <div className="flex justify-between">
                    <span>{t("timestamp")}:</span>
                    <span className="font-mono">{currentFrame.timestamp}</span>
                  </div>

                  {/* Duration of audio data in this frame */}
                  <div className="flex justify-between">
                    <span>{t("duration-ms")}:</span>
                    <span className="font-mono">
                      {currentFrame.duration_ms}
                    </span>
                  </div>

                  {/* Audio sampling rate */}
                  <div className="flex justify-between">
                    <span>{t("sample-rate")}:</span>
                    <span className="font-mono">
                      {currentFrame.sample_rate} Hz
                    </span>
                  </div>

                  {/* Number of samples in each channel */}
                  <div className="flex justify-between">
                    <span>{t("channel-a-samples")}:</span>
                    <span className="font-mono">
                      {currentFrame.channel_a.length}
                    </span>
                  </div>

                  <div className="flex justify-between">
                    <span>{t("channel-b-samples")}:</span>
                    <span className="font-mono">
                      {currentFrame.channel_b.length}
                    </span>
                  </div>

                  {/* Sample Data Preview - Shows actual audio values */}
                  <div className="mt-2">
                    <div className="text-xs text-gray-600 mb-1">
                      {t("channel-a-preview-first-10-samples")}:
                    </div>
                    <div className="font-mono text-xs bg-gray-100 p-1 rounded overflow-hidden">
                      [
                      {currentFrame.channel_a
                        .slice(0, 10)
                        .map((val) => val.toFixed(3))
                        .join(", ")}
                      ...]
                    </div>
                  </div>
                </div>
              ) : (
                <div className="text-gray-500 text-center py-4">
                  {t("no-frame-received")}
                </div>
              )}
            </CardBody>
          </Card>
        </div>

        {/* Audio Spectrum Analyzer Visualization */}
        {showAnalyzer && (
          <Card className="w-full max-w-4xl mt-6">
            <CardHeader className="pb-2">
              <div className="flex items-center justify-between w-full">
                <h3 className="text-lg font-semibold">
                  {t("audio-spectrum-analyzer")}
                </h3>
                <div className="flex items-center gap-2">
                  {/* Live indicator when analyzer is running */}
                  {isAnalyzerInitialized && (
                    <Chip color="success" size="sm" variant="flat">
                      {t("live")}
                    </Chip>
                  )}
                  {/* Sample rate indicator */}
                  {audioContext && (
                    <Chip color="primary" size="sm" variant="flat">
                      {Math.round(audioContext.sampleRate)} Hz
                    </Chip>
                  )}
                </div>
              </div>
            </CardHeader>
            <CardBody>
              <div className="w-full">
                {/* Analyzer Container - Where the spectrum visualization renders */}
                <div
                  ref={analyzerContainerRef}
                  className="w-full"
                  style={{
                    height: showAnalyzer ? "300px" : "0px",
                    overflow: "hidden",
                    borderRadius: "8px",
                    backgroundColor: "#000", // Black background for better contrast
                  }}
                />

                {/* Debug/Status Information */}
                {!isAnalyzerInitialized && isAudioReady && (
                  <div className="flex items-center justify-center h-72 bg-gray-100 rounded-lg">
                    <div className="text-center">
                      <p className="text-gray-600 mb-2">
                        {t("audio-analyzer-not-initialized")}
                      </p>
                      <div className="text-sm text-gray-500 mb-2">
                        {t("debug-audiocontext")}
                        {audioContext
                          ? `${t("ready")} (${audioContext.sampleRate}Hz)`
                          : t("none")}
                        ,{t("streamnode")}=
                        {audioStreamNode ? t("ready") : t("none")}
                      </div>
                    </div>
                  </div>
                )}

                {/* Pre-initialization state */}
                {!isAudioReady && (
                  <div className="flex items-center justify-center h-72 bg-gray-100 rounded-lg">
                    <div className="text-center">
                      <p className="text-gray-600 mb-2">
                        {t("audio-context-not-ready")}
                      </p>
                      <div className="text-sm text-gray-500 mb-2">
                        {t("debug-samplerate")}=
                        {currentFrame?.sample_rate || t("unknown")}Hz,
                        {t("isconnected")}={isConnected ? t("yes") : t("no")},
                        {t("framecount")}={frameCount},{t("audiostate")}=
                        {audioContext?.state || t("none")}
                      </div>
                      <p className="text-sm text-gray-500 mb-2">
                        {t("initialize-audio-first-then-connect-to-stream")}
                      </p>
                    </div>
                  </div>
                )}
              </div>
            </CardBody>
          </Card>
        )}
      </section>
    </DefaultLayout>
  );
}
