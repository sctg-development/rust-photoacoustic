// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

import { useTranslation } from "react-i18next";
import { useEffect, useState, useRef, useCallback } from "react";
import { Modal, Button, Card, Chip, Switch, Tabs, Label } from "@heroui/react";

import { useGenerixConfig } from "../authentication/providers/generix-config";
import { useAuth } from "../authentication";
import { ComputingResponse, ComputingUtils } from "../types/computing";
import {
  getMathMLFromPolynomialCoefficientsClassicOrder,
  getMathMLFromPolynomialCoefficientsClassicOrderMathML,
} from "../utilities/polynomial-to-mathml";

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

interface ComputingNodeModalProps {
  isOpen: boolean;
  onClose: () => void;
  nodeData: ProcessingNodeData | null;
}

export function ComputingNodeModal({
  isOpen,
  onClose,
  nodeData,
}: ComputingNodeModalProps) {
  const { t } = useTranslation();
  const { isAuthenticated, getJson } = useAuth();

  // Configuration management with the new hook
  const {
    config: generixConfig,
    loading: configLoading,
    error: configError,
    load: loadConfig,
  } = useGenerixConfig({ autoLoad: false });

  const [computingResponse, setComputingResponse] =
    useState<ComputingResponse | null>(null);
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [selectedTab, setSelectedTab] = useState<string>("overview");

  // Refs for cleanup
  const refreshIntervalRef = useRef<number | null>(null);

  // Load Generix configuration when modal opens
  useEffect(() => {
    if (isOpen && !generixConfig) {
      loadConfig();
    }
  }, [isOpen, generixConfig, loadConfig]);

  // Memoized function to fetch computing data
  const fetchComputingResponse = useCallback(async () => {
    if (!isAuthenticated || !generixConfig) {
      return;
    }
    try {
      const response = (await getJson(
        `${generixConfig.api_base_url}/computing`,
      )) as ComputingResponse;

      setComputingResponse(response);
    } catch (error) {
      console.error("Error fetching computing response:", error);
      // Note: error is now managed by the useGenerixConfig hook
    }
  }, [isAuthenticated, generixConfig, getJson]);

  // Fetch computing data when authenticated and config is ready
  useEffect(() => {
    if (isOpen && isAuthenticated && generixConfig) {
      fetchComputingResponse();
    }
  }, [isOpen, isAuthenticated, generixConfig, fetchComputingResponse]);

  // Auto-refresh effect
  useEffect(() => {
    if (refreshIntervalRef.current) {
      clearInterval(refreshIntervalRef.current);
    }

    if (autoRefresh && generixConfig && isAuthenticated && isOpen) {
      refreshIntervalRef.current = window.setInterval(() => {
        fetchComputingResponse();
      }, 5000); // Refresh every 5 seconds for computing data
    }

    return () => {
      if (refreshIntervalRef.current) {
        clearInterval(refreshIntervalRef.current);
      }
    };
  }, [
    autoRefresh,
    generixConfig,
    isAuthenticated,
    isOpen,
    fetchComputingResponse,
  ]);

  const getPeakResult = (
    computingResponse: ComputingResponse | null,
    nodeId: string,
  ) => {
    if (!computingResponse) {
      return null;
    }

    return computingResponse.peak_results[nodeId] || null;
  };

  const handleRefresh = async () => {
    await fetchComputingResponse();
  };

  const formatTimestamp = (timestamp: any) => {
    if (!timestamp) return "N/A";

    // Handle both string ISO format and object format
    if (typeof timestamp === "string") {
      return ComputingUtils.formatTimestamp(timestamp);
    }

    // Handle object format { secs_since_epoch, nanos_since_epoch }
    if (timestamp.secs_since_epoch) {
      const date = new Date(timestamp.secs_since_epoch * 1000);

      return date.toLocaleString();
    }

    return "Invalid timestamp";
  };

  const getNodeStats = () => {
    if (!computingResponse) {
      return { totalNodes: 0, activeNodes: 0, hasLatestResult: false };
    }

    const totalNodes = Object.keys(computingResponse.peak_results).length;
    const activeNodes = computingResponse.active_node_ids.length;
    const hasLatestResult = !!computingResponse.latest_result;

    return { totalNodes, activeNodes, hasLatestResult };
  };

  if (
    !nodeData ||
    (nodeData.nodeType !== "computing_concentration" &&
      nodeData.nodeType !== "computing_peak_finder")
  ) {
    return null;
  }

  // Get appropriate modal title and subtitle based on node type
  const getModalTitles = () => {
    const isConcentration = nodeData.nodeType === "computing_concentration";

    return {
      title: isConcentration
        ? t("computing-modal-concentration-title")
        : t("computing-modal-peak-finder-title"),
      subtitle: isConcentration
        ? t("computing-modal-concentration-subtitle", { name: nodeData.id })
        : t("computing-modal-peak-finder-subtitle", { name: nodeData.id }),
      icon: isConcentration ? "🧪" : "📊",
    };
  };

  const { title, subtitle, icon } = getModalTitles();
  const peakResult = getPeakResult(
    computingResponse,
    nodeData.parameters.computing_peak_finder_id || nodeData.id,
  );
  const stats = getNodeStats();

  return (
    <Modal.Backdrop isOpen={isOpen} onOpenChange={(open) => !open && onClose()}>
      <Modal.Container>
        <Modal.Dialog className="max-w-4xl">
          <Modal.CloseTrigger />
          <Modal.Header>
            <div className="flex items-center gap-2">
              <span className="text-2xl">{icon}</span>
              <div>
                <Modal.Heading>{title || nodeData.id}</Modal.Heading>
                <p className="text-sm text-gray-600 font-normal">{subtitle}</p>
              </div>
            </div>
          </Modal.Header>

          <Modal.Body>
            {/* Configuration Loading State */}
            {configLoading && (
              <Card>
                <Card.Content>
                  <div className="flex items-center justify-center py-8">
                    <div className="text-center">
                      <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary mx-auto mb-2" />
                      <p className="text-gray-600">
                        {t("computing-modal-loading-config")}
                      </p>
                    </div>
                  </div>
                </Card.Content>
              </Card>
            )}

            {/* Configuration Error State */}
            {configError && (
              <Card className="border-red-200 bg-red-50">
                <Card.Content>
                  <div className="text-center py-4">
                    <p className="text-red-600 font-medium mb-2">
                      {t("computing-modal-config-error-title")}
                    </p>
                    <p className="text-red-500 text-sm mb-4">{configError}</p>
                    <Button variant="danger-soft" onPress={loadConfig}>
                      {t("streaming-modal-retry-config")}
                    </Button>
                  </div>
                </Card.Content>
              </Card>
            )}

            {/* Main Content */}
            {generixConfig && !configLoading && !configError && (
              <div className="space-y-4">
                {/* Controls */}
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-4">
                    <Switch
                      isSelected={autoRefresh}
                      size="sm"
                      onChange={setAutoRefresh}
                    >
                      <Switch.Control>
                        <Switch.Thumb />
                      </Switch.Control>
                      <Switch.Content>
                        <Label>{t("auto-refresh")}</Label>
                      </Switch.Content>
                    </Switch>

                    <Button variant="secondary" onPress={handleRefresh}>
                      {t("refresh")}
                    </Button>
                  </div>
                </div>

                {/* System Status Overview */}
                <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                  <Card className="bg-blue-50 border-blue-200">
                    <Card.Content className="text-center">
                      <p className="text-2xl font-bold text-blue-600">
                        {stats.totalNodes}
                      </p>
                      <p className="text-sm text-blue-800">
                        {t("computing-modal-total-nodes")}
                      </p>
                    </Card.Content>
                  </Card>

                  <Card className="bg-green-50 border-green-200">
                    <Card.Content className="text-center">
                      <p className="text-2xl font-bold text-green-600">
                        {stats.activeNodes}
                      </p>
                      <p className="text-sm text-green-800">
                        {t("computing-modal-active-nodes")}
                      </p>
                    </Card.Content>
                  </Card>

                  <Card
                    className={`${
                      stats.hasLatestResult
                        ? "bg-green-50 border-green-200"
                        : "bg-gray-50 border-gray-200"
                    }`}
                  >
                    <Card.Content className="text-center">
                      <p
                        className={`text-2xl font-bold ${
                          stats.hasLatestResult
                            ? "text-green-600"
                            : "text-gray-600"
                        }`}
                      >
                        {stats.hasLatestResult ? "✓" : "—"}
                      </p>
                      <p
                        className={`text-sm ${
                          stats.hasLatestResult
                            ? "text-green-800"
                            : "text-gray-800"
                        }`}
                      >
                        {t("computing-modal-latest-result")}
                      </p>
                    </Card.Content>
                  </Card>
                </div>

                {/* Tabs for different views */}
                <Tabs
                  className="w-full"
                  selectedKey={selectedTab}
                  onSelectionChange={(key) => setSelectedTab(key as string)}
                >
                  <Tabs.ListContainer>
                    <Tabs.List aria-label={t("computing-modal-overview")}>
                      <Tabs.Tab id="overview">
                        {t("computing-modal-overview")}
                        <Tabs.Indicator />
                      </Tabs.Tab>
                      <Tabs.Tab id="all-results">
                        {t("computing-modal-all-results")}
                        <Tabs.Indicator />
                      </Tabs.Tab>
                      <Tabs.Tab id="raw-data">
                        {t("computing-modal-raw-data")}
                        <Tabs.Indicator />
                      </Tabs.Tab>
                    </Tabs.List>
                  </Tabs.ListContainer>

                  <Tabs.Panel id="overview">
                    {/* Current Node Result */}
                    {peakResult ? (
                      <Card className="mb-4">
                        <Card.Header>
                          <h3 className="text-lg font-semibold">
                            {t("computing-modal-current-result")}
                          </h3>
                        </Card.Header>
                        <Card.Content>
                          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <div className="space-y-3">
                              <div className="flex justify-between items-center p-3 bg-blue-50 rounded-lg">
                                <span className="text-sm font-medium text-blue-700">
                                  {t("computing-modal-frequency")}:
                                </span>
                                <div>
                                  <span className="font-bold text-blue-800">
                                    {peakResult.frequency?.toFixed(2) ?? "N/A"}{" "}
                                    Hz
                                  </span>
                                  <CopyButton
                                    value={
                                      peakResult.frequency?.toString() ?? "N/A"
                                    }
                                  />
                                </div>
                              </div>
                              <div className="flex justify-between items-center p-3 bg-green-50 rounded-lg">
                                <span className="text-sm font-medium text-green-700">
                                  {t("computing-modal-amplitude")}:
                                </span>
                                <div>
                                  <span className="font-bold text-green-800">
                                    {peakResult.amplitude?.toFixed(4) ?? "N/A"}
                                  </span>
                                  <CopyButton
                                    value={
                                      peakResult.amplitude?.toString() ?? "N/A"
                                    }
                                  />
                                </div>
                              </div>
                            </div>
                            <div className="space-y-3">
                              {peakResult.concentration_ppm !== undefined &&
                                peakResult.concentration_ppm !== null && (
                                  <div className="flex justify-between items-center p-3 bg-purple-50 rounded-lg">
                                    <span className="text-sm font-medium text-purple-700">
                                      {t("computing-modal-concentration")}:
                                    </span>
                                    <div>
                                      <span className="font-bold text-purple-800">
                                        {peakResult.concentration_ppm.toFixed(
                                          2,
                                        )}{" "}
                                        ppm
                                      </span>
                                      <CopyButton
                                        value={(
                                          peakResult.concentration_ppm /
                                          10000000
                                        ).toString()}
                                      />
                                    </div>
                                  </div>
                                )}
                              <div className="flex justify-between items-center p-3 bg-gray-50 rounded-lg">
                                <span className="text-sm font-medium text-gray-700">
                                  {t("computing-modal-timestamp")}:
                                </span>
                                <span className="font-bold text-gray-800 text-sm">
                                  {formatTimestamp(peakResult.timestamp)}
                                </span>
                              </div>
                            </div>
                          </div>
                        </Card.Content>
                      </Card>
                    ) : (
                      <Card className="mb-4">
                        <Card.Content>
                          <div className="text-center py-8">
                            <p className="text-gray-500 text-lg mb-2">
                              {t("computing-modal-no-data")}
                            </p>
                            <p className="text-gray-400 text-sm">
                              {t("computing-modal-no-data-description")}
                            </p>
                          </div>
                        </Card.Content>
                      </Card>
                    )}
                    {/* Node Configuration */}
                    <Card>
                      <Card.Header>
                        <h3 className="text-lg font-semibold">
                          {t("computing-modal-node-configuration")}
                        </h3>
                      </Card.Header>
                      <Card.Content>
                        <div className="space-y-2">
                          <div className="flex justify-between">
                            <span className="text-sm text-gray-600">
                              {t("computing-modal-node-id")}:
                            </span>
                            <span className="font-semibold">{nodeData.id}</span>
                          </div>
                          <div className="flex justify-between">
                            <span className="text-sm text-gray-600">
                              {t("computing-modal-node-type")}:
                            </span>
                            <Chip color="accent" size="sm" variant="soft">
                              {nodeData.nodeType}
                            </Chip>
                          </div>
                          {nodeData.parameters.computing_peak_finder_id && (
                            <div className="flex justify-between">
                              <span className="text-sm text-gray-600">
                                {t("computing-modal-peak-finder-id")}:
                              </span>
                              <span className="font-semibold">
                                {nodeData.parameters.computing_peak_finder_id}
                              </span>
                            </div>
                          )}
                          {nodeData.parameters.polynomial_coefficients && (
                            <div className="flex justify-between">
                              <span className="text-sm text-gray-600">
                                {t("computing-modal-polynomial-coeffs")}:
                              </span>
                              <div>
                                <span
                                  dangerouslySetInnerHTML={{
                                    __html:
                                      getMathMLFromPolynomialCoefficientsClassicOrderMathML(
                                        nodeData.parameters
                                          .polynomial_coefficients,
                                      ) || "",
                                  }}
                                  className="font-mono text-xs"
                                />
                                <CopyButton
                                  value={
                                    getMathMLFromPolynomialCoefficientsClassicOrder(
                                      nodeData.parameters
                                        .polynomial_coefficients,
                                    ) || "error"
                                  }
                                />
                              </div>
                            </div>
                          )}
                        </div>
                      </Card.Content>
                    </Card>
                  </Tabs.Panel>

                  <Tabs.Panel id="all-results">
                    {computingResponse &&
                    Object.keys(computingResponse.peak_results).length > 0 ? (
                      <div className="space-y-4">
                        {Object.entries(computingResponse.peak_results).map(
                          ([nodeId, result]) => (
                            <Card key={nodeId} className="p-4">
                              <Card.Header>
                                <div className="flex justify-between items-center w-full">
                                  <h3 className="text-lg font-semibold">
                                    {nodeId}
                                  </h3>
                                  <Chip
                                    className={
                                      computingResponse.active_node_ids.includes(
                                        nodeId,
                                      )
                                        ? "bg-green-100 text-green-800"
                                        : "bg-gray-100 text-gray-600"
                                    }
                                    size="sm"
                                    variant="soft"
                                  >
                                    {computingResponse.active_node_ids.includes(
                                      nodeId,
                                    )
                                      ? t("computing-modal-active")
                                      : t("computing-modal-inactive")}
                                  </Chip>
                                </div>
                              </Card.Header>
                              <Card.Content>
                                <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                                  <div className="text-center p-2 bg-blue-50 rounded">
                                    <p className="text-xs text-blue-600 mb-1">
                                      {t("computing-modal-frequency")}
                                    </p>
                                    <p className="font-bold text-blue-800">
                                      {result.frequency?.toFixed(2) ?? "N/A"} Hz
                                      <CopyButton
                                        value={
                                          result.frequency?.toString() ?? "N/A"
                                        }
                                      />
                                    </p>
                                  </div>
                                  <div className="text-center p-2 bg-green-50 rounded">
                                    <p className="text-xs text-green-600 mb-1">
                                      {t("computing-modal-amplitude")}
                                    </p>
                                    <p className="font-bold text-green-800">
                                      {result.amplitude?.toFixed(4) ?? "N/A"}
                                      <CopyButton
                                        value={
                                          result.amplitude.toString() ?? "N/A"
                                        }
                                      />
                                    </p>
                                  </div>
                                  {result.concentration_ppm !== undefined &&
                                    result.concentration_ppm !== null && (
                                      <div className="text-center p-2 bg-purple-50 rounded">
                                        <p className="text-xs text-purple-600 mb-1">
                                          {t("computing-modal-concentration")}
                                        </p>
                                        <p className="font-bold text-purple-800">
                                          {result.concentration_ppm.toFixed(2)}{" "}
                                          ppm
                                          <CopyButton
                                            value={(
                                              result.concentration_ppm /
                                              10000000
                                            ).toString()}
                                          />
                                        </p>
                                      </div>
                                    )}
                                  <div className="text-center p-2 bg-gray-50 rounded">
                                    <p className="text-xs text-gray-600 mb-1">
                                      {t("computing-modal-timestamp")}
                                    </p>
                                    <p className="font-bold text-gray-800 text-xs">
                                      {formatTimestamp(result.timestamp)}
                                    </p>
                                  </div>
                                </div>
                              </Card.Content>
                            </Card>
                          ),
                        )}
                      </div>
                    ) : (
                      <Card>
                        <Card.Content>
                          <div className="text-center py-8">
                            <p className="text-gray-500">
                              {t("computing-modal-no-results")}
                            </p>
                          </div>
                        </Card.Content>
                      </Card>
                    )}
                  </Tabs.Panel>

                  <Tabs.Panel id="raw-data">
                    <div className="space-y-4">
                      <Card>
                        <Card.Header>
                          <h3 className="text-lg font-semibold">
                            {t("computing-modal-node-result")}
                          </h3>
                        </Card.Header>
                        <Card.Content>
                          <pre className="bg-gray-100 p-4 rounded-lg overflow-auto text-sm">
                            {JSON.stringify(peakResult, null, 2)}
                          </pre>
                        </Card.Content>
                      </Card>
                      <Card>
                        <Card.Header>
                          <h3 className="text-lg font-semibold">
                            {t("computing-modal-full-response")}
                          </h3>
                        </Card.Header>
                        <Card.Content>
                          <pre className="bg-gray-100 p-4 rounded-lg overflow-auto text-sm">
                            {JSON.stringify(computingResponse, null, 2)}
                          </pre>
                        </Card.Content>
                      </Card>
                    </div>
                  </Tabs.Panel>
                </Tabs>
              </div>
            )}
          </Modal.Body>

          <Modal.Footer>
            <Button slot="close">{t("close")}</Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
