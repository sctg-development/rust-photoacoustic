import { Trans, useTranslation } from "react-i18next";
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

  // Hook pour le streaming audio
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
  } = useAudioStream(`${generixConfig?.api_base_url}/audio`);

  useEffect(() => {
    const loadGenerixConfig = async () => {
      const config = await getGenerixConfig();

      console.log("Config is :", config);
      setGenerixConfig(config);
    };

    const loadAccessToken = async () => {
      const token = await getAccessToken();

      console.log("Access token is :", token);
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
  }, [isAuthenticated, generixConfig, user?.username]);

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
          <h1 className={title()}>
            <Trans t={t}>api-answer</Trans>
          </h1>
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
          {/* Statut de connexion */}
          <Card className="w-full">
            <CardHeader className="pb-2">
              <h3 className="text-lg font-semibold">État de Connexion</h3>
            </CardHeader>
            <CardBody>
              <div className="flex flex-col gap-3">
                <div className="flex items-center justify-between">
                  <span>Statut:</span>
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
                      ? "Connecté"
                      : isConnecting
                        ? "Connexion..."
                        : "Déconnecté"}
                  </Chip>
                </div>

                <div className="flex gap-2">
                  {!isConnected && !isConnecting && (
                    <Button color="primary" size="sm" onClick={connect}>
                      Se connecter
                    </Button>
                  )}
                  {isConnected && (
                    <Button color="danger" size="sm" onClick={disconnect}>
                      Déconnecter
                    </Button>
                  )}
                  <Button color="secondary" size="sm" onClick={reconnect}>
                    Reconnecter
                  </Button>
                </div>

                {error && (
                  <div className="text-red-500 text-sm bg-red-50 p-2 rounded">
                    <strong>Erreur ({error.type}):</strong> {error.message}
                  </div>
                )}
              </div>
            </CardBody>
          </Card>

          {/* Statistiques du stream */}
          <Card className="w-full">
            <CardHeader className="pb-2">
              <h3 className="text-lg font-semibold">Statistiques</h3>
            </CardHeader>
            <CardBody>
              <div className="flex flex-col gap-3">
                <div className="flex justify-between">
                  <span>Frames reçues:</span>
                  <span className="font-bold text-blue-600">{frameCount}</span>
                </div>

                <div className="flex justify-between">
                  <span>Frames perdues:</span>
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

                {/* Barre de progression FPS */}
                <div className="mt-2">
                  <div className="flex justify-between text-sm mb-1">
                    <span>Performance FPS</span>
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

          {/* Informations de la frame actuelle */}
          <Card className="w-full">
            <CardHeader className="pb-2">
              <h3 className="text-lg font-semibold">Frame Actuelle</h3>
            </CardHeader>
            <CardBody>
              {currentFrame ? (
                <div className="flex flex-col gap-2 text-sm">
                  <div className="flex justify-between">
                    <span>Numéro:</span>
                    <span className="font-mono">
                      {currentFrame.frame_number}
                    </span>
                  </div>

                  <div className="flex justify-between">
                    <span>Timestamp:</span>
                    <span className="font-mono">{currentFrame.timestamp}</span>
                  </div>

                  <div className="flex justify-between">
                    <span>Durée (ms):</span>
                    <span className="font-mono">
                      {currentFrame.duration_ms}
                    </span>
                  </div>

                  <div className="flex justify-between">
                    <span>Sample Rate:</span>
                    <span className="font-mono">
                      {currentFrame.sample_rate} Hz
                    </span>
                  </div>

                  <div className="flex justify-between">
                    <span>Canal A (samples):</span>
                    <span className="font-mono">
                      {currentFrame.channel_a.length}
                    </span>
                  </div>

                  <div className="flex justify-between">
                    <span>Canal B (samples):</span>
                    <span className="font-mono">
                      {currentFrame.channel_b.length}
                    </span>
                  </div>

                  {/* Visualisation simple des échantillons */}
                  <div className="mt-2">
                    <div className="text-xs text-gray-600 mb-1">
                      Aperçu Canal A (10 premiers échantillons):
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
                  Aucune frame reçue
                </div>
              )}
            </CardBody>
          </Card>
        </div>

        {/* Section de débogage */}
        {currentFrame && (
          <Card className="w-full max-w-4xl mt-4">
            <CardHeader>
              <h3 className="text-lg font-semibold">
                Données de la Frame (JSON)
              </h3>
            </CardHeader>
            <CardBody>
              <Snippet className="w-full" symbol="" title="frame-data">
                <div className="max-h-64 overflow-y-auto whitespace-pre-wrap text-wrap break-words">
                  {JSON.stringify(currentFrame, null, 2)}
                </div>
              </Snippet>
            </CardBody>
          </Card>
        )}
      </section>
    </DefaultLayout>
  );
}
