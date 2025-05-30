import { useTranslation } from "react-i18next";
import { useEffect, useState } from "react";
import { Snippet } from "@heroui/snippet";
import { Button } from "@heroui/button";
import { Card, CardBody, CardHeader } from "@heroui/card";
import { Progress } from "@heroui/progress";
import { Chip } from "@heroui/chip";

import {
  getGenerixConfig,
  GenerixConfig,
} from "../authentication/providers/generix-config";

import { title } from "@/components/primitives";
import DefaultLayout from "@/layouts/default";
import { useAuth, useSecuredApi } from "@/authentication";
import { useAudioStream } from "@/hooks/useAudioStream";

export default function ApiPage() {
  const { t } = useTranslation();
  const { getJson } = useSecuredApi();
  const { user, isAuthenticated, getAccessToken } = useAuth();
  const [apiResponse, setApiResponse] = useState("");
  const [generixConfig, setGenerixConfig] = useState(
    null as GenerixConfig | null,
  );
  const [accessToken, setAccessToken] = useState("" as string | null);

  // Hook for audio streaming
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
  } = useAudioStream(`${generixConfig?.api_base_url}/audio`, false);

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

  // Call the API endpoint to get the response
  useEffect(() => {
    const fetchData = async () => {
      if (isAuthenticated && generixConfig && user) {
        try {
          const response = await getJson(
            `${generixConfig.api_base_url}/test/${user.sub}`,
          );

          setApiResponse(response);
        } catch (error) {
          setApiResponse((error as Error).message);
        }
      }
    };

    fetchData();
  }, [isAuthenticated, generixConfig, user]);

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
          <h1 className={title()}>{t("api-answer")}</h1>
        </div>
        <Snippet className="max-w-11/12" symbol="" title="api-response">
          <div className="max-w-2xs sm:max-w-sm md:max-w-md lg:max-w-5xl  whitespace-break-spaces  text-wrap break-words">
            {JSON.stringify(apiResponse, null, 2)}
          </div>
        </Snippet>
      </section>
      <section className="flex flex-col items-center justify-center gap-4 py-8 md:py-10">
        <div className="inline-block max-w-lg text-center justify-center">
          <h1 className={title()}>{t("audio-streaming-test")}</h1>
        </div>

        <div className="w-full max-w-4xl grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
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
                      ? t('connected')
                      : isConnecting
                        ? "Connecting..."
                        : t('disconnected')}
                  </Chip>
                </div>

                <div className="flex gap-2">
                  {!isConnected && !isConnecting && (
                    <Button color="primary" size="sm" onPress={connect}>
                      {t("connect")}
                    </Button>
                  )}
                  {isConnected && (
                    <Button color="danger" size="sm" onPress={disconnect}>
                      {t("disconnect")}
                    </Button>
                  )}
                  <Button color="secondary" size="sm" onPress={reconnect}>
                    {t("reconnect")}
                  </Button>
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

                {/* FPS progress bar */}
                <div className="mt-2">
                  <div className="flex justify-between text-sm mb-1">
                    <span>{t("fps-performance")}</span>
                    <span>{fps.toFixed(1)}/60</span>
                  </div>
                  <Progress
                    color={
                      fps > 30 ? "success" : fps > 15 ? "warning" : "danger"
                    }
                    size="sm"
                    value={Math.min((fps / 60) * 100, 100)}
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
      </section>
    </DefaultLayout>
  );
}
