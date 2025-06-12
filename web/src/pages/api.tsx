/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 *
 * ApiPage Component - Demonstration of multiple AudioStreamAnalyzer instances
 *
 * This page demonstrates the reusable AudioStreamAnalyzer component with
 * different configurations to show various use cases.
 */
import { useTranslation } from "react-i18next";
import { useEffect, useState } from "react";

import {
  getGenerixConfig,
  GenerixConfig,
} from "../authentication/providers/generix-config";

import { title } from "@/components/primitives";
import DefaultLayout from "@/layouts/default";
import { useAuth } from "@/authentication";
import AudioStreamAnalyzer from "@/components/audio-stream-analyzer";

/**
 * ApiPage Component - Real-time Audio Streaming and Visualization Demo
 *
 * This component demonstrates multiple instances of the AudioStreamAnalyzer component
 * with different configurations to showcase its flexibility and reusability.
 */
export default function ApiPage() {
  const { t } = useTranslation();
  const { user, isAuthenticated, getAccessToken } = useAuth();

  // Configuration state - holds API endpoints and authentication details
  const [generixConfig, setGenerixConfig] = useState(
    null as GenerixConfig | null,
  );
  const [accessToken, setAccessToken] = useState("" as string | null);

  // Configuration loading effects
  useEffect(() => {
    /**
     * Load Generix Configuration
     */
    const loadGenerixConfig = async () => {
      const config = await getGenerixConfig();

      console.log("Config is :", config);
      setGenerixConfig(config);
    };

    /**
     * Load Access Token
     */
    const loadAccessToken = async () => {
      const token = await getAccessToken();

      setAccessToken(token);
    };

    loadAccessToken();
    loadGenerixConfig();
  }, [getAccessToken]);

  // Authentication state monitoring
  useEffect(() => {
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

  // Prepare URLs for the AudioStreamAnalyzer
  const streamUrl = generixConfig
    ? `${generixConfig.api_base_url}/stream/audio/fast`
    : undefined;
  const statsUrl = generixConfig
    ? `${generixConfig.api_base_url}/stream/audio/fast/stats`
    : undefined;

  return (
    <DefaultLayout>
      <section className="flex flex-col items-center justify-center gap-4 py-8 md:py-10">
        <div className="inline-block max-w-lg text-center justify-center">
          <h1 className={title()}>{t("audio-streaming-test")}</h1>
        </div>

        {/* Full featured AudioStreamAnalyzer - All cards visible */}
        <AudioStreamAnalyzer
          analyzerTitle="Photoacoustic Analyzer"
          className="mb-8"
          isCurrentFrameStatisticsDisplayed={true}
          isPrestateDisplayed={false}
          isStatisticsDisplayed={true}
          isStatusDisplayed={true}
          statsUrl={statsUrl}
          streamUrl={streamUrl}
          title={t("main-audio-analyzer")}
        />

        {/* Compact version - Only status and statistics */}
        <AudioStreamAnalyzer
          analyzerTitle="Compact Analyzer"
          className="mb-8"
          isCurrentFrameStatisticsDisplayed={false}
          isPrestateDisplayed={false}
          isStatisticsDisplayed={true}
          isStatusDisplayed={true}
          statsUrl={statsUrl}
          streamUrl={streamUrl}
          title={t("compact-analyzer-stats-only")}
        />

        {/* Minimal version - Only analyzer visualization */}
        <AudioStreamAnalyzer
          className="mb-8"
          isCurrentFrameStatisticsDisplayed={false}
          isPrestateDisplayed={false}
          isStatisticsDisplayed={false}
          isStatusDisplayed={false}
          showUniversalControl={true}
          statsUrl={statsUrl}
          streamUrl={streamUrl}
          title={t("minimal-analyzer-visualization-only")}
        />
      </section>
    </DefaultLayout>
  );
}
