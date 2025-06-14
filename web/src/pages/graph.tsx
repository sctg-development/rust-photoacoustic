// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

import { useTranslation } from "react-i18next";
import { useEffect, useState } from "react";
import { Card, CardBody, CardHeader } from "@heroui/card";
import { Button } from "@heroui/button";
import { Spinner } from "@heroui/spinner";
import { Alert } from "@heroui/alert";
import { Tabs, Tab } from "@heroui/tabs";
import { Switch } from "@heroui/switch";

import {
  getGenerixConfig,
  GenerixConfig,
} from "../authentication/providers/generix-config";

import { SerializableProcessingGraph } from "@/types/processing-graph";
import { useAuth, useSecuredApi } from "@/authentication";
import { title } from "@/components/primitives";
import DefaultLayout from "@/layouts/default";
import { ProcessingGraphView } from "@/components/processing-graph-view";
import { ProcessingGraphStats } from "@/components/processing-graph-stats";

export default function DocsPage() {
  const { t } = useTranslation();
  const { getJson } = useSecuredApi();
  const [graph, setGraph] = useState<SerializableProcessingGraph | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedTab, setSelectedTab] = useState<string>("graph");
  const [autoRefresh, setAutoRefresh] = useState(false);
  const { isAuthenticated, getAccessToken } = useAuth();
  // Configuration state - holds API endpoints and authentication details
  const [generixConfig, setGenerixConfig] = useState(
    null as GenerixConfig | null,
  );

  // Load Processing Graph function
  const loadProcessingGraph = async () => {
    if (!isAuthenticated || !generixConfig) {
      console.warn(t("user-not-authenticated-or-config-not-loaded"));
      setLoading(false);

      return;
    }

    try {
      setError(null);
      const graph = (await getJson(
        `${generixConfig.api_base_url}/graph`,
      )) as SerializableProcessingGraph;

      setGraph(graph);
    } catch (error) {
      console.error(t("error-loading-processing-graph"), error);
      setError(
        error instanceof Error ? error.message : t("graph-failed-to-load"),
      );
    } finally {
      setLoading(false);
    }
  };

  // Configuration loading effects
  useEffect(() => {
    /**
     * Load Generix Configuration
     */
    const loadGenerixConfig = async () => {
      const config = await getGenerixConfig();

      console.log(t("config-is"), config);
      setGenerixConfig(config);
    };

    loadGenerixConfig();
  }, [getAccessToken]);

  // Load graph when config is ready
  useEffect(() => {
    if (generixConfig && isAuthenticated) {
      loadProcessingGraph();
    }
  }, [generixConfig, isAuthenticated]);

  // Auto-refresh effect
  useEffect(() => {
    if (!autoRefresh || !generixConfig || !isAuthenticated) return;

    const interval = setInterval(() => {
      loadProcessingGraph();
    }, 60000); // Refresh every 60 seconds

    return () => clearInterval(interval);
  }, [autoRefresh, generixConfig, isAuthenticated]);

  const handleRefresh = () => {
    setLoading(true);
    loadProcessingGraph();
  };

  return (
    <DefaultLayout>
      <section className="flex flex-col items-center justify-center gap-4 py-8 md:py-10">
        <div className="inline-block max-w-lg text-center justify-center">
          <h1 className={title()}>{t("graph-title")}</h1>
        </div>

        {/* Loading State */}
        {loading && !graph && (
          <div className="flex items-center justify-center min-h-96">
            <div className="flex flex-col items-center gap-4">
              <Spinner size="lg" />
              <p className="text-gray-600">{t("graph-loading")}</p>
            </div>
          </div>
        )}

        {/* Error State */}
        {error && !graph && (
          <div className="max-w-2xl mx-auto mt-8">
            <Alert
              color="danger"
              description={error}
              endContent={
                <Button color="danger" variant="flat" onPress={handleRefresh}>
                  {t("retry")}
                </Button>
              }
              title={t("graph-failed-to-load-title")}
            />
          </div>
        )}

        {/* No Graph State */}
        {!loading && !error && !graph && (
          <div className="max-w-2xl mx-auto mt-8">
            <Alert
              color="warning"
              description={t("graph-no-data-description")}
              title={t("graph-no-data-title")}
            />
          </div>
        )}

        {/* Graph Content */}
        {graph && (
          <div className="container mx-auto px-4 max-w-7xl w-full">
            {/* Controls */}
            <div className="flex items-center justify-between mb-6">
              <div className="flex items-center gap-4">
                <Switch
                  isSelected={autoRefresh}
                  size="sm"
                  onValueChange={setAutoRefresh}
                >
                  {t("auto-refresh")}
                </Switch>

                <Button
                  color="primary"
                  isLoading={loading}
                  variant="flat"
                  onPress={handleRefresh}
                >
                  {t("refresh")}
                </Button>
              </div>
            </div>

            {/* Status Overview */}
            <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
              <Card className="bg-blue-50 border-blue-200">
                <CardBody className="text-center">
                  <p className="text-2xl font-bold text-blue-600">
                    {graph.performance_summary.total_nodes}
                  </p>
                  <p className="text-sm text-blue-800">{t("active-nodes")}</p>
                </CardBody>
              </Card>

              <Card className="bg-green-50 border-green-200">
                <CardBody className="text-center">
                  <p className="text-2xl font-bold text-green-600">
                    {graph.performance_summary.throughput_fps.toFixed(1)}
                  </p>
                  <p className="text-sm text-green-800">{t("fps")}</p>
                </CardBody>
              </Card>

              <Card className="bg-purple-50 border-purple-200">
                <CardBody className="text-center">
                  <p className="text-2xl font-bold text-purple-600">
                    {graph.performance_summary.average_execution_time_ms.toFixed(
                      2,
                    )}{" "}
                    {t("ms")}
                  </p>
                  <p className="text-sm text-purple-800">{t("avg-time")}</p>
                </CardBody>
              </Card>

              <Card
                className={`${
                  graph.is_valid
                    ? "bg-green-50 border-green-200"
                    : "bg-red-50 border-red-200"
                }`}
              >
                <CardBody className="text-center">
                  <p
                    className={`text-2xl font-bold ${
                      graph.is_valid ? "text-green-600" : "text-red-600"
                    }`}
                  >
                    {graph.is_valid ? "✓" : "✗"}
                  </p>
                  <p
                    className={`text-sm ${
                      graph.is_valid ? "text-green-800" : "text-red-800"
                    }`}
                  >
                    {graph.is_valid ? t("valid") : t("invalid")}
                  </p>
                </CardBody>
              </Card>
            </div>

            {/* Main Content Tabs */}
            <Tabs
              className="w-full"
              selectedKey={selectedTab}
              size="lg"
              onSelectionChange={(key) => setSelectedTab(key as string)}
            >
              <Tab key="graph" title={t("graph-view")}>
                <Card className="min-h-[600px]">
                  <CardHeader>
                    <h2 className="text-xl font-semibold">
                      {t("graph-visualization-title")}
                    </h2>
                    <p className="text-sm text-gray-600 ml-auto">
                      {t("graph-click-node-hint")}
                    </p>
                  </CardHeader>
                  <CardBody className="h-[600px] p-0">
                    <ProcessingGraphView className="h-full" graph={graph} />
                  </CardBody>
                </Card>
              </Tab>

              <Tab key="stats" title={t("performance-stats")}>
                <ProcessingGraphStats graph={graph} />
              </Tab>

              <Tab key="raw" title={t("raw-data")}>
                <Card>
                  <CardHeader>
                    <h2 className="text-xl font-semibold">
                      {t("raw-graph-data")}
                    </h2>
                  </CardHeader>
                  <CardBody>
                    <pre className="bg-gray-100 p-4 rounded-lg overflow-auto text-sm">
                      {JSON.stringify(graph, null, 2)}
                    </pre>
                  </CardBody>
                </Card>
              </Tab>
            </Tabs>
          </div>
        )}
      </section>
    </DefaultLayout>
  );
}
