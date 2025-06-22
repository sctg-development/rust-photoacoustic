// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

import { useTranslation } from "react-i18next";
import { Accordion, AccordionItem } from "@heroui/accordion";
import { useEffect } from "react";
import {
  Modal,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalFooter,
} from "@heroui/modal";
import { Button } from "@heroui/button";
import { Card, CardBody } from "@heroui/card";
import { Chip } from "@heroui/chip";

import { useGenerixConfig } from "../authentication/providers/generix-config";

import AudioStreamAnalyzer from "./audio-stream-analyzer";
import { CopyButton } from "./copy-button";

interface ProcessingNodeData {
  id: string;
  nodeType: string;
  acceptsInputTypes: string[];
  outputType: string | null;
  parameters: Record<string, any>;
  statistics?: any;
  isBottleneck?: boolean;
}

interface StreamingNodeModalProps {
  isOpen: boolean;
  onClose: () => void;
  nodeData: ProcessingNodeData | null;
}

export function StreamingNodeModal({
  isOpen,
  onClose,
  nodeData,
}: StreamingNodeModalProps) {
  const { t } = useTranslation();

  // Configuration management with the new hook
  const {
    config: generixConfig,
    loading: configLoading,
    error: configError,
    load: loadConfig,
  } = useGenerixConfig({ autoLoad: false });

  // Load Generix configuration when modal opens
  useEffect(() => {
    if (isOpen && !generixConfig) {
      loadConfig();
    }
  }, [isOpen, generixConfig, loadConfig]);

  if (
    !nodeData ||
    (nodeData.nodeType !== "streaming" && nodeData.nodeType !== "input")
  ) {
    return null;
  }

  // Get the stream name from parameters or use node ID as fallback
  const streamName = nodeData.parameters.name || nodeData.id;
  const streamId = nodeData.parameters.stream_id || nodeData.id;

  // Construct stream URLs based on node type and generix configuration
  const getStreamUrls = () => {
    if (!generixConfig) return { streamUrl: undefined, statsUrl: undefined };

    if (nodeData.nodeType === "input") {
      // For input nodes, use the main audio stream endpoints
      return {
        streamUrl: `${generixConfig.api_base_url}/stream/audio/fast`,
        statsUrl: `${generixConfig.api_base_url}/stream/audio/fast/stats`,
      };
    } else {
      // For streaming nodes, use node-specific endpoints
      return {
        streamUrl: `${generixConfig.api_base_url}/stream/audio/fast/${streamId}`,
        statsUrl: `${generixConfig.api_base_url}/stream/nodes/${streamId}/stats`,
      };
    }
  };

  const { streamUrl, statsUrl } = getStreamUrls();

  // Get appropriate modal title and subtitle based on node type
  const getModalTitles = () => {
    if (nodeData.nodeType === "input") {
      return {
        subtitle: t("streaming-modal-input-subtitle", { name: streamName }),
        liveAudioTitle: t("streaming-modal-input-live-audio", {
          name: streamName,
        }),
      };
    } else {
      return {
        subtitle: t("streaming-modal-subtitle", { name: streamName }),
        liveAudioTitle: t("streaming-modal-live-audio", { name: streamName }),
      };
    }
  };

  const { subtitle, liveAudioTitle } = getModalTitles();

  return (
    <Modal isOpen={isOpen} scrollBehavior="inside" size="4xl" onClose={onClose}>
      <ModalContent>
        <ModalHeader className="flex flex-col gap-1">
          <div className="flex items-center gap-2">
            <span className="text-2xl">
              {nodeData.nodeType === "input" ? "📥" : "📡"}
            </span>
            <div>
              <h2 className="text-xl font-bold">{nodeData.id}</h2>
              <p className="text-sm text-gray-600 font-normal">{subtitle}</p>
            </div>
          </div>
        </ModalHeader>

        <ModalBody>
          {/* Node Information */}
          <Card>
            <CardBody>
              <Accordion>
                <AccordionItem
                  className="text-xl font-semibold"
                  title={t("streaming-modal-node-information")}
                >
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-4 font-normal">
                    <div>
                      <p className="text-sm text-gray-600">
                        {t("streaming-modal-node-id")}
                      </p>
                      <p className="font-medium">{nodeData.id}</p>
                    </div>
                    <div>
                      <p className="text-sm text-gray-600">
                        {nodeData.nodeType === "input"
                          ? t("streaming-modal-input-name")
                          : t("streaming-modal-stream-name")}
                      </p>
                      <p className="font-medium">{streamName}</p>
                    </div>
                    {nodeData.nodeType === "streaming" && (
                      <div>
                        <p className="text-sm text-gray-600">
                          {t("streaming-modal-stream-id")}
                        </p>
                        <p className="font-medium">{streamId}</p>
                      </div>
                    )}

                    <div>
                      <p className="text-sm text-gray-600">
                        {t("streaming-modal-output-type")}
                      </p>
                      <p className="font-medium">
                        {nodeData.outputType || t("streaming-modal-dynamic")}
                      </p>
                    </div>
                  </div>

                  <div className="mt-4">
                    <p className="text-sm text-gray-600">
                      {t("streaming-modal-input-types")}
                    </p>
                    <div className="flex flex-wrap gap-1 mt-1">
                      {nodeData.acceptsInputTypes.map((type) => (
                        <Chip key={type} size="sm" variant="flat">
                          {type}
                        </Chip>
                      ))}
                    </div>
                  </div>
                </AccordionItem>
              </Accordion>
            </CardBody>
          </Card>

          {/* Configuration Loading State */}
          {configLoading && (
            <Card>
              <CardBody>
                <div className="flex items-center justify-center py-8">
                  <div className="text-center">
                    <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary mx-auto mb-2" />
                    <p className="text-gray-600">
                      {t("streaming-modal-loading-config")}
                    </p>
                  </div>
                </div>
              </CardBody>
            </Card>
          )}

          {/* Configuration Error State */}
          {configError && (
            <Card className="border-red-200 bg-red-50">
              <CardBody>
                <div className="text-center py-4">
                  <p className="text-red-600 font-medium mb-2">
                    {t("streaming-modal-config-error-title")}
                  </p>
                  <p className="text-red-500 text-sm mb-4">{configError}</p>
                  <Button color="danger" variant="flat" onPress={loadConfig}>
                    {t("streaming-modal-retry-config")}
                  </Button>
                </div>
              </CardBody>
            </Card>
          )}

          {/* Audio Stream Analyzer */}
          {generixConfig && streamUrl && !configLoading && !configError && (
            <Card>
              <CardBody className="p-0">
                <AudioStreamAnalyzer
                  analyzerTitle={t("streaming-modal-spectrum-analyzer")}
                  className="p-4"
                  isCurrentFrameStatisticsDisplayed={false}
                  isPrestateDisplayed={false}
                  isStatisticsDisplayed={false}
                  isStatusDisplayed={false}
                  showUniversalControl={true}
                  statsUrl={statsUrl}
                  streamUrl={streamUrl}
                  title={liveAudioTitle}
                />
              </CardBody>
            </Card>
          )}

          {/* Stream URLs Information (for debugging) */}
          {generixConfig && (
            <Card>
              <CardBody>
                <Accordion>
                  <AccordionItem
                    className="text-xl font-semibold"
                    title={t("streaming-modal-connection-details")}
                  >
                    <div className="font-light space-y-2 text-xs">
                      <div>
                        <p className="text-gray-600">
                          {nodeData.nodeType === "input"
                            ? t("streaming-modal-stream-url")
                            : t("streaming-modal-websocket-url")}
                          :
                        </p>
                        <div className="relative">
                          <p className="font-mono text-xs bg-gray-100 p-2 rounded break-all">
                            {streamUrl}
                          </p>
                          <CopyButton
                            aria-label={t("streaming-modal-copy-stream-url")}
                            className="absolute top-0 right-2"
                            value={streamUrl}
                          />
                        </div>
                      </div>
                      {statsUrl && (
                        <div>
                          <p className="text-gray-600">
                            {t("streaming-modal-stats-url")}:
                          </p>
                          <div className="relative">
                            <p className="font-mono text-xs bg-gray-100 p-2 rounded break-all">
                              {statsUrl}
                            </p>
                            <CopyButton
                              aria-label={t("streaming-modal-copy-stats-url")}
                              className="absolute top-0 right-2"
                              value={statsUrl}
                            />
                          </div>
                        </div>
                      )}
                    </div>
                  </AccordionItem>
                </Accordion>
              </CardBody>
            </Card>
          )}
        </ModalBody>

        <ModalFooter>
          <Button color="primary" onPress={onClose}>
            {t("streaming-modal-close")}
          </Button>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}
