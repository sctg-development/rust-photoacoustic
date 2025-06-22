/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 *
 * AudioStreamAnalyzer Component - Reusable audio streaming dashboard
 *
 * This component provides a comprehensive interface for:
 * 1. Displaying connection status and controls
 * 2. Showing stream statistics and performance metrics
 * 3. Displaying current frame information
 * 4. Visualizing audio spectrum with real-time analyzer
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
  TimestampValidationConfig,
  useAudioStream,
} from "@/hooks/useAudioStream";

export interface AudioStreamAnalyzerProps {
  /** Base URL for the audio stream endpoint */
  streamUrl?: string;
  /** URL for stream statistics endpoint */
  statsUrl?: string;
  /** Whether to display the statistics card */
  isStatisticsDisplayed?: boolean;
  /** Whether to display the current frame statistics card */
  isCurrentFrameStatisticsDisplayed?: boolean;
  /** Whether to display the status card with connection controls */
  isStatusDisplayed?: boolean;
  /** Whether to display the prestate card with audio initialization controls */
  isPrestateDisplayed?: boolean;
  /** Whether to show a universal control button when status controls are hidden */
  showUniversalControl?: boolean;
  /** Custom function to handle audio initialization when status is not displayed */
  onInitializeAudio?: () => Promise<void>;
  /** Custom function to handle stream connection when status is not displayed */
  onConnect?: () => void;
  /** Custom function to handle stream disconnection when status is not displayed */
  onDisconnect?: () => void;
  /** Custom class name for the component container */
  className?: string;
  /** Optional title for the component */
  title?: string;
  /** Optional title for the analyzer section (defaults to translated "analyzer") */
  analyzerTitle?: string;
}

export default function AudioStreamAnalyzer({
  streamUrl,
  statsUrl,
  isStatisticsDisplayed = true,
  isCurrentFrameStatisticsDisplayed = true,
  isStatusDisplayed = true,
  isPrestateDisplayed = true,
  showUniversalControl = false,
  onInitializeAudio,
  onConnect,
  onDisconnect,
  className = "",
  title,
  analyzerTitle,
}: AudioStreamAnalyzerProps) {
  const { t } = useTranslation();

  // Audio analyzer states - manages the visual spectrum analyzer
  const [audioAnalyzer, setAudioAnalyzer] = useState<any>(null);
  const [isAnalyzerInitialized, setIsAnalyzerInitialized] = useState(false);
  const [showAnalyzer, setShowAnalyzer] = useState(true);
  const analyzerContainerRef = useRef<HTMLDivElement>(null);

  // Track manual disconnection to prevent auto-reconnect
  const manuallyDisconnectedRef = useRef(false);

  /**
   * Audio Stream Hook - Core audio processing functionality
   */
  const {
    isConnected,
    isConnecting,
    error,
    currentFrame,
    frameCount,
    droppedFrames,
    fps,
    connect,
    disconnect,
    initializeAudio,
    audioContext,
    audioStreamNode,
    isAudioReady,
    averageFrameSizeBytes,
    isDualChannel,
  } = useAudioStream(
    streamUrl,
    statsUrl,
    false, // Disable auto-connect
    {
      enabled: false, // Disable timestamp validation for now
    } as TimestampValidationConfig,
  );

  /**
   * Initialize Audio Spectrum Analyzer
   */
  const initializeAnalyzer = async () => {
    if (!analyzerContainerRef.current) {
      console.warn(t("cannot-initialize-analyzer-container-not-ready"));

      return;
    }

    if (!audioContext || !audioStreamNode) {
      console.warn(t("cannot-initialize-analyzer-audio-not-ready"), {
        hasAudioContext: !!audioContext,
        hasAudioStreamNode: !!audioStreamNode,
        isAudioReady,
      });

      return;
    }

    try {
      // Clean up existing analyzer to prevent memory leaks
      if (audioAnalyzer) {
        console.log(t("cleaning-up-existing-analyzer"));
        audioAnalyzer.destroy();
      }

      console.log(t("initializing-analyzer-with"), {
        sampleRate: audioContext.sampleRate,
        analyserNode: audioStreamNode.analyserNode,
        containerReady: !!analyzerContainerRef.current,
      });

      // Create new analyzer instance with optimized settings for photoacoustic data
      const analyzer = new AudioMotionAnalyzer(analyzerContainerRef.current, {
        source: audioStreamNode.analyserNode,
        height: 300,
        mode: 3,
        showBgColor: true,
        bgAlpha: 0.7,
        overlay: true,
        showPeaks: true,
        showFPS: false,
        showScaleY: true,
        connectSpeakers: false,
        barSpace: 0.1,
        ledBars: false,
        lumiBars: false,
        radial: false,
        reflexRatio: 0.3,
        gradient: "rainbow",
        linearAmplitude: true,
        linearBoost: 1.8,
        maxDecibels: -10,
        minDecibels: -85,
        smoothing: 0.8,
        channelLayout: isDualChannel ? "dual-horizontal" : "single",
      });

      console.log(t("audio-analyzer-initialized-successfully"));
      setAudioAnalyzer(analyzer);
      setIsAnalyzerInitialized(true);
    } catch (error) {
      console.error(t("failed-to-initialize-audio-analyzer"), error);
      setIsAnalyzerInitialized(false);
    }
  };

  /**
   * Clean up Audio Analyzer Resources
   */
  const cleanupAnalyzer = () => {
    if (audioAnalyzer) {
      try {
        audioAnalyzer.destroy();
        console.log(t("audio-analyzer-cleaned-up"));
      } catch (error) {
        console.error(t("error-cleaning-up-analyzer"), error);
      }
      setAudioAnalyzer(null);
      setIsAnalyzerInitialized(false);
    }
  };

  /**
   * Conditional Analyzer Initialization
   */
  const initializeAnalyzerIfReady = useCallback(async () => {
    if (
      isAudioReady &&
      audioContext &&
      audioStreamNode &&
      showAnalyzer &&
      !isAnalyzerInitialized
    ) {
      console.log(t("conditions-met-initializing-analyzer"));
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
   */
  const autoConnectIfNeeded = useCallback(() => {
    // Only auto-connect if:
    // 1. Audio is ready
    // 2. Not currently connected or connecting
    // 3. No error state
    // 4. User hasn't manually disconnected
    if (
      isAudioReady &&
      !isConnected &&
      !isConnecting &&
      !error &&
      !manuallyDisconnectedRef.current
    ) {
      console.log(t("audio-ready-auto-connecting-to-stream"));
      connect();
    }
  }, [isAudioReady, isConnected, isConnecting, error, connect]);

  // Effect: Handle analyzer initialization
  useEffect(() => {
    initializeAnalyzerIfReady();
  }, [initializeAnalyzerIfReady]);

  // Effect: Handle auto-connection after audio is ready
  useEffect(() => {
    autoConnectIfNeeded();
  }, [autoConnectIfNeeded]);

  // Effect: Reset manual disconnection flag when audio becomes ready
  useEffect(() => {
    if (isAudioReady && manuallyDisconnectedRef.current) {
      // Reset the flag when audio system is reinitialized
      manuallyDisconnectedRef.current = false;
    }
  }, [isAudioReady]);

  // Effect: Handle analyzer cleanup when visibility is toggled
  useEffect(() => {
    if (!showAnalyzer && isAnalyzerInitialized) {
      console.log(t("show-analyzer-disabled-cleaning-up"));
      cleanupAnalyzer();
    }
  }, [showAnalyzer, isAnalyzerInitialized]);

  // Effect: Handle component unmount cleanup
  useEffect(() => {
    return () => {
      if (isAnalyzerInitialized) {
        console.log(t("component-unmounting-cleaning-up-analyzer"));
        cleanupAnalyzer();
      }
    };
  }, [isAnalyzerInitialized]);

  /**
   * Handle Analyzer Visibility Toggle
   */
  const handleAnalyzerToggle = (visible: boolean) => {
    console.log(t("analyzer-toggle"), visible);
    setShowAnalyzer(visible);
    if (!visible) {
      cleanupAnalyzer();
    } else if (isAudioReady && audioContext && audioStreamNode) {
      setTimeout(() => initializeAnalyzer(), 100);
    }
  };

  /**
   * Handle Audio Initialization
   */
  const handleInitializeAudio = useCallback(async () => {
    console.log(t("initialize-audio-button-clicked"));
    try {
      if (onInitializeAudio) {
        await onInitializeAudio();
      } else {
        await initializeAudio();
      }
      console.log(t("audio-initialization-completed"));
    } catch (error) {
      console.error(t("failed-to-initialize-audio"), error);
    }
  }, [initializeAudio, onInitializeAudio]);

  /**
   * Handle Stream Connection
   */
  const handleConnect = useCallback(() => {
    // Reset manual disconnection flag when user manually connects
    manuallyDisconnectedRef.current = false;

    if (onConnect) {
      onConnect();
    } else {
      connect();
    }
  }, [connect, onConnect]);

  /**
   * Handle Stream Disconnection
   */
  const handleDisconnect = useCallback(() => {
    // Mark as manually disconnected to prevent auto-reconnect
    manuallyDisconnectedRef.current = true;

    if (onDisconnect) {
      onDisconnect();
    } else {
      disconnect();
    }
  }, [disconnect, onDisconnect]);

  // Calculate grid layout based on displayed cards
  const visibleCardsCount = [
    isStatusDisplayed,
    isStatisticsDisplayed,
    isCurrentFrameStatisticsDisplayed,
  ].filter(Boolean).length;

  // Determine grid classes based on number of visible cards
  const getGridClasses = () => {
    if (visibleCardsCount === 1) {
      return "grid-cols-1";
    } else if (visibleCardsCount === 2) {
      return "grid-cols-1 md:grid-cols-2";
    } else {
      return "grid-cols-1 md:grid-cols-2 lg:grid-cols-3";
    }
  };

  return (
    <div className={`w-full max-w-4xl ${className}`}>
      {title && (
        <div className="mb-6 text-center">
          <h2 className="text-xl font-semibold">{title}</h2>
        </div>
      )}

      {/* Main dashboard grid */}
      <div
        className={`w-full grid ${getGridClasses()} gap-4 ${visibleCardsCount < 3 ? "lg:h-auto" : "lg:h-96 lg:max-h-96"
          }`}
      >
        {/* Connection Status Card */}
        {isStatusDisplayed && (
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
                  <span>{analyzerTitle || t("analyzer")}:</span>
                  <Chip
                    color={isAnalyzerInitialized ? "success" : "default"}
                    variant="flat"
                  >
                    {isAnalyzerInitialized ? t("active") : t("inactive")}
                  </Chip>
                </div>

                {/* Control Buttons */}
                <div className="flex gap-2">
                  {/* Initialize Audio Context */}
                  {!isAudioReady && (
                    <Button
                      color="primary"
                      size="sm"
                      onPress={handleInitializeAudio}
                    >
                      {t("initialize-audio-context")}
                    </Button>
                  )}

                  {/* Connect to Stream */}
                  {isAudioReady && !isConnected && !isConnecting && (
                    <Button
                      aria-label={t("connect-to-audio-stream")}
                      color="primary"
                      size="sm"
                      onPress={handleConnect}
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
                      onPress={handleDisconnect}
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

                {/* Error Display */}
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
        )}

        {/* Stream Statistics Card */}
        {isStatisticsDisplayed && (
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

                {/* FPS Performance Indicator */}
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
        )}

        {/* Current Frame Information Card */}
        {isCurrentFrameStatisticsDisplayed && (
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

                  {/* Sample Data Preview */}
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
        )}
      </div>

      {/* Audio Spectrum Analyzer Visualization */}
      {showAnalyzer && (
        <Card className="w-full mt-6">
          <CardHeader className="pb-2">
            <div className="flex items-center justify-between w-full">
              <h3 className="text-lg font-semibold">
                {analyzerTitle || t("analyzer")}
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
              {/* Analyzer Container with Universal Control Button Overlay */}
              <div className="relative">
                <div
                  ref={analyzerContainerRef}
                  className={`w-full rounded-lg bg-black ${showAnalyzer ? "h-[300px]" : "h-0"
                    } overflow-hidden`}
                />

                {/* Universal Control Button - Positioned in top-right corner */}
                {showUniversalControl && !isStatusDisplayed && (
                  <div className="absolute top-2 right-4 z-10">
                    {!isAudioReady && (
                      <Button
                        color="primary"
                        size="sm"
                        variant="solid"
                        className="shadow-lg backdrop-blur-sm bg-primary/90"
                        onPress={handleInitializeAudio}
                      >
                        {t("connect")}
                      </Button>
                    )}

                    {isAudioReady && !isConnected && !isConnecting && (
                      <Button
                        aria-label={t("connect-to-audio-stream")}
                        color="primary"
                        size="sm"
                        variant="solid"
                        className="shadow-lg backdrop-blur-sm bg-primary/90"
                        onPress={handleConnect}
                      >
                        {t("connect")}
                      </Button>
                    )}

                    {isConnected && (
                      <Button
                        aria-label={t("disconnect-from-audio-stream")}
                        color="danger"
                        size="sm"
                        variant="solid"
                        className="shadow-lg backdrop-blur-sm bg-danger/90"
                        onPress={handleDisconnect}
                      >
                        {t("disconnect")}
                      </Button>
                    )}

                    {isConnecting && (
                      <Button
                        isDisabled
                        isLoading
                        color="warning"
                        size="sm"
                        variant="solid"
                        className="shadow-lg backdrop-blur-sm bg-warning/90"
                      >
                        {t("connecting")}
                      </Button>
                    )}
                  </div>
                )}
              </div>

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
              {!isAudioReady && isPrestateDisplayed && (
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
    </div>
  );
}
