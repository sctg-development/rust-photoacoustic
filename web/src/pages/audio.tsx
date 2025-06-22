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
import { Select, SelectItem } from "@heroui/select";

import { useGenerixConfig } from "../authentication/providers/generix-config";

import { title } from "@/components/primitives";
import DefaultLayout from "@/layouts/default";
import { useAuth } from "@/authentication";
import AudioStreamAnalyzer from "@/components/audio-stream-analyzer";
import { AudioStreamInfo } from "@/types";

/**
 * ApiPage Component - Real-time Audio Streaming and Visualization Demo
 *
 * This component demonstrates multiple instances of the AudioStreamAnalyzer component
 * with different configurations to showcase its flexibility and reusability.
 */
export default function ApiPage() {
  const { t } = useTranslation();
  const { user, isAuthenticated, getAccessToken, getJson } = useAuth();

  // Configuration state - holds API endpoints and authentication details
  const { config: generixConfig } = useGenerixConfig();
  const [audioStreamInfos, setAudioStreamInfos] = useState<AudioStreamInfo[]>(
    [],
  );
  const [selectedStreamId, setSelectedStreamId] = useState<string>("");
  const [accessToken, setAccessToken] = useState("" as string | null);

  // Configuration loading effects
  useEffect(() => {
    /**
     * Load Access Token
     */
    const loadAccessToken = async () => {
      const token = await getAccessToken();

      setAccessToken(token);
    };

    loadAccessToken();
  }, [getAccessToken]);

  // Authentication state monitoring
  useEffect(() => {
    const getAllAudioStreams = async () => {
      if (generixConfig) {
        const streams = (await getJson(
          `${generixConfig.api_base_url}/stream/audio/get-all-streams`,
        )) as AudioStreamInfo[];

        setAudioStreamInfos(streams);
        // Auto-select the first stream if none is selected
        if (streams.length > 0 && !selectedStreamId) {
          setSelectedStreamId(streams[0].id);
        }
      }
    };

    if (isAuthenticated && generixConfig && user) {
      getAllAudioStreams();
    }
  }, [accessToken, generixConfig, isAuthenticated, user, selectedStreamId]);

  // Get the currently selected stream info
  const selectedStream = audioStreamInfos.find(
    (stream) => stream.id === selectedStreamId,
  );

  // Prepare URLs for the AudioStreamAnalyzer based on selected stream
  const streamUrl =
    selectedStream && generixConfig
      ? `${generixConfig.api_base_url}${selectedStream.stream_url}`
      : undefined;
  const statsUrl =
    selectedStream && generixConfig
      ? `${generixConfig.api_base_url}${selectedStream.stats_url}`
      : undefined;

  return (
    <DefaultLayout>
      <section className="flex flex-col items-center justify-center gap-4 py-8 md:py-10">
        <div className="inline-block lg:max-w-lg 2xl:max-w-2xl  text-center justify-center">
          <h1 className={title()}>{t("audio-streaming-analysis")}</h1>
        </div>
        {audioStreamInfos.length > 0 && (
          <div className="w-full max-w-lg mb-6">
            <Select
              className="max-w-xs"
              label={t("select-audio-stream")}
              placeholder={t("choose-stream-to-analyze")}
              selectedKeys={selectedStreamId ? [selectedStreamId] : []}
              onSelectionChange={(keys) => {
                const selectedKey = Array.from(keys)[0] as string;

                if (selectedKey) {
                  setSelectedStreamId(selectedKey);
                }
              }}
            >
              {audioStreamInfos.map((stream) => (
                <SelectItem key={stream.id}>{stream.id}</SelectItem>
              ))}
            </Select>
            {selectedStream && (
              <div className="mt-2 text-sm text-gray-600">
                <p>
                  <strong>{t("stream-url")}:</strong>{" "}
                  {selectedStream.stream_url}
                </p>
                <p>
                  <strong>{t("stats-url")}:</strong> {selectedStream.stats_url}
                </p>
              </div>
            )}
          </div>
        )}
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
      </section>
    </DefaultLayout>
  );
}
