import { useTranslation } from "react-i18next";
import { useEffect, useState, useRef } from "react";
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

export default function ApiPage() {
  const { t } = useTranslation();
  const { user, isAuthenticated, getAccessToken } = useAuth();
  const [generixConfig, setGenerixConfig] = useState(
    null as GenerixConfig | null,
  );
  const [accessToken, setAccessToken] = useState("" as string | null);

  // Audio analyzer states
  const [audioAnalyzer, setAudioAnalyzer] = useState<any>(null);
  const [isAnalyzerInitialized, setIsAnalyzerInitialized] = useState(false);
  const [showAnalyzer, setShowAnalyzer] = useState(true);
  const analyzerContainerRef = useRef<HTMLDivElement>(null);

  // Hook for audio streaming - now auto-detects format
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
    reconnect,
    initializeAudio,
    audioContext,
    audioStreamNode,
    isAudioReady,
    averageFrameSizeBytes,
  } = useAudioStream(
    generixConfig
      ? `${generixConfig.api_base_url}/stream/audio/fast`
      : undefined, //Stream endpoint URL
    generixConfig
      ? `${generixConfig.api_base_url}/stream/audio/fast/stats`
      : undefined, //Stats endpoint URL
    false, // Disable auto-connect
    true, // Enable auto audio context initialization
    {
      enabled: false,
    } as TimestampValidationConfig,
  );

  // Initialize audio analyzer
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
        },
      );

      return;
    }

    try {
      // Clean up existing analyzer
      if (audioAnalyzer) {
        console.log("Cleaning up existing analyzer");
        audioAnalyzer.destroy();
      }

      console.log("Initializing analyzer with:", {
        sampleRate: audioContext.sampleRate,
        analyserNode: audioStreamNode.analyserNode,
        containerReady: !!analyzerContainerRef.current,
      });

      // Create new analyzer instance
      const analyzer = new AudioMotionAnalyzer(analyzerContainerRef.current, {
        source: audioStreamNode.analyserNode, // Connect to the analyser node
        height: 300,
        mode: 3, // 1/3-octave bands
        showBgColor: true,
        bgAlpha: 0.7,
        overlay: true,
        showPeaks: true,
        showFPS: true,
        showScaleY: true,
        connectSpeakers: false, // Don't connect to speakers to avoid feedback
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
        channelLayout: "dual-horizontal",
      });

      console.log("Audio analyzer initialized successfully");
      setAudioAnalyzer(analyzer);
      setIsAnalyzerInitialized(true);
    } catch (error) {
      console.error("Failed to initialize audio analyzer:", error);
      setIsAnalyzerInitialized(false);
    }
  };

  // Clean up analyzer
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

  // Initialize analyzer when audio context and stream node are ready
  useEffect(() => {
    console.log("Analyzer effect triggered:", {
      isAudioReady,
      hasAudioContext: !!audioContext,
      hasAudioStreamNode: !!audioStreamNode,
      showAnalyzer,
      isAnalyzerInitialized,
    });

    if (
      isAudioReady &&
      audioContext &&
      audioStreamNode &&
      showAnalyzer &&
      !isAnalyzerInitialized
    ) {
      console.log("Conditions met, initializing analyzer");
      setTimeout(() => initializeAnalyzer(), 100); // Small delay to ensure everything is ready
    }

    // Remove the cleanup logic that was interfering with the audio hook
    // The audio hook manages its own lifecycle
  }, [
    isAudioReady,
    audioContext,
    audioStreamNode,
    showAnalyzer,
    isAnalyzerInitialized,
  ]);

  // Separate effect to handle analyzer cleanup when showAnalyzer becomes false
  useEffect(() => {
    if (!showAnalyzer && isAnalyzerInitialized) {
      console.log("Show analyzer disabled, cleaning up analyzer only");
      cleanupAnalyzer();
    }
  }, [showAnalyzer, isAnalyzerInitialized]);

  // Handle component unmount cleanup
  useEffect(() => {
    return () => {
      if (isAnalyzerInitialized) {
        console.log("Component unmounting, cleaning up analyzer");
        cleanupAnalyzer();
      }
    };
  }, [isAnalyzerInitialized]);

  // Handle analyzer visibility toggle
  const handleAnalyzerToggle = (visible: boolean) => {
    console.log("Analyzer toggle:", visible);
    setShowAnalyzer(visible);
    if (!visible) {
      cleanupAnalyzer();
    } else if (isAudioReady && audioContext && audioStreamNode) {
      setTimeout(() => initializeAnalyzer(), 100); // Small delay to ensure DOM is ready
    }
  };

  useEffect(() => {
    const loadGenerixConfig = async () => {
      const config = await getGenerixConfig();

      console.log("Config is :", config);
      setGenerixConfig(config);
    };

    const loadAccessToken = async () => {
      const token = await getAccessToken();

      setAccessToken(token);
    };

    loadAccessToken();
    loadGenerixConfig();
  }, []);

  useEffect(() => {
    // Connected user is authenticated and the route is protected with the access token and the right permissions
    if (isAuthenticated && generixConfig && user) {
      console.log(
        "User is authenticated, Generix config and user are available.",
      );
      console.log("Access Token:", accessToken);
    } else {
      console.log(
        "User is not authenticated or Generix config/user is not available.",
      );
    }
  }, [accessToken, generixConfig, isAuthenticated, user]);

  return (
    <DefaultLayout>
      <section className="flex flex-col items-center justify-center gap-4 py-8 md:py-10">
        <div className="inline-block max-w-lg text-center justify-center">
          <h1 className={title()}>{t("audio-streaming-test")}</h1>
        </div>

        <div className="w-full max-w-4xl grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 lg:h-96 lg:max-h-96 gap-4">
          {/* Connection status */}
          <Card className="w-full">
            <CardHeader className="pb-2">
              <h3 className="text-lg font-semibold">
                {t("connection-status")}
              </h3>
            </CardHeader>
            <CardBody>
              <div className="flex flex-col gap-3">
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

                <div className="flex items-center justify-between">
                  <span>{t("audio-ready")}</span>
                  <Chip
                    color={isAudioReady ? "success" : "default"}
                    variant="flat"
                  >
                    {isAudioReady ? t("ready") : t("not-ready")}
                  </Chip>
                </div>

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

                <div className="flex items-center justify-between">
                  <span>{t("analyzer")}:</span>
                  <Chip
                    color={isAnalyzerInitialized ? "success" : "default"}
                    variant="flat"
                  >
                    {isAnalyzerInitialized ? t("active") : t("inactive")}
                  </Chip>
                </div>

                <div className="flex gap-2">
                  {!isConnected && !isConnecting && (
                    <Button
                      aria-label={t("connect-to-audio-stream")}
                      color="primary"
                      size="sm"
                      onPress={connect}
                    >
                      {t("connect")}
                    </Button>
                  )}
                  {isConnected && (
                    <>
                      <Button
                        aria-label={t("disconnect-from-audio-stream")}
                        color="danger"
                        size="sm"
                        onPress={disconnect}
                      >
                        {t("disconnect")}
                      </Button>
                      <Button
                        aria-label={t("reconnect-to-audio-stream")}
                        color="secondary"
                        size="sm"
                        onPress={reconnect}
                      >
                        {t("reconnect")}
                      </Button>
                    </>
                  )}
                </div>
                <div className="flex items-center justify-between">
                  <span>{t("show-analyzer")}</span>
                  <Switch
                    aria-label={t("toggle-audio-analyzer-visibility")}
                    isSelected={showAnalyzer}
                    onValueChange={handleAnalyzerToggle}
                  />
                </div>

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

          {/* Stream statistics */}
          <Card className="w-full">
            <CardHeader className="pb-2">
              <h3 className="text-lg font-semibold">{t("statistics")}</h3>
            </CardHeader>
            <CardBody>
              <div className="flex flex-col gap-3">
                <div className="flex justify-between">
                  <span>{t("frames-received")}:</span>
                  <span className="font-bold text-blue-600">{frameCount}</span>
                </div>

                <div className="flex justify-between">
                  <span>{t("frames-lost")}:</span>
                  <span
                    className={`font-bold ${droppedFrames > 0 ? "text-red-600" : "text-green-600"}`}
                  >
                    {droppedFrames}
                  </span>
                </div>

                <div className="flex justify-between">
                  <span>FPS:</span>
                  <span className="font-bold text-purple-600">
                    {fps.toFixed(1)}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span>{t("average-frame-size")}:</span>
                  <span className="font-mono">
                    {(averageFrameSizeBytes / 1024).toFixed(2)} kB
                  </span>
                </div>

                {/* FPS progress bar */}
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

          {/* Current frame information */}
          <Card className="w-full">
            <CardHeader className="pb-2">
              <h3 className="text-lg font-semibold">{t("current-frame")}</h3>
            </CardHeader>
            <CardBody>
              {currentFrame ? (
                <div className="flex flex-col gap-2 text-sm">
                  <div className="flex justify-between">
                    <span>{t("number")}:</span>
                    <span className="font-mono">
                      {currentFrame.frame_number}
                    </span>
                  </div>

                  <div className="flex justify-between">
                    <span>{t("timestamp")}:</span>
                    <span className="font-mono">{currentFrame.timestamp}</span>
                  </div>

                  <div className="flex justify-between">
                    <span>{t("duration-ms")}:</span>
                    <span className="font-mono">
                      {currentFrame.duration_ms}
                    </span>
                  </div>

                  <div className="flex justify-between">
                    <span>{t("sample-rate")}:</span>
                    <span className="font-mono">
                      {currentFrame.sample_rate} Hz
                    </span>
                  </div>

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

                  {/* Simple sample visualization */}
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

        {/* Audio Analyzer Visualization */}
        {showAnalyzer && (
          <Card className="w-full max-w-4xl mt-6">
            <CardHeader className="pb-2">
              <div className="flex items-center justify-between w-full">
                <h3 className="text-lg font-semibold">
                  {t("audio-spectrum-analyzer")}
                </h3>
                <div className="flex items-center gap-2">
                  {isAnalyzerInitialized && (
                    <Chip color="success" size="sm" variant="flat">
                      {t("live")}
                    </Chip>
                  )}
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
                <div
                  ref={analyzerContainerRef}
                  className="w-full"
                  style={{
                    height: showAnalyzer ? "300px" : "0px",
                    overflow: "hidden",
                    borderRadius: "8px",
                    backgroundColor: "#000",
                  }}
                />
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
                      <Button
                        color="primary"
                        disabled={!audioContext || !audioStreamNode}
                        size="sm"
                        onPress={initializeAnalyzer}
                      >
                        {t("initialize-analyzer")}
                      </Button>
                    </div>
                  </div>
                )}
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
                      <div className="flex gap-2 justify-center">
                        <Button
                          color="primary"
                          size="sm"
                          onPress={async () => {
                            console.log("Initialize audio button clicked");
                            try {
                              await initializeAudio();
                            } catch (error) {
                              console.error(
                                "Failed to initialize audio:",
                                error,
                              );
                            }
                          }}
                        >
                          {t("initialize-audio-context")}
                        </Button>
                        {!isConnected && generixConfig && (
                          <Button color="secondary" size="sm" onPress={connect}>
                            {t("connect-stream")}
                          </Button>
                        )}
                      </div>
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
