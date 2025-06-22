/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 *
 * ThermalPage Component - Thermal regulation dashboard
 *
 * This page provides a comprehensive dashboard for monitoring thermal regulators
 * with real-time temperature data, control outputs, and historical charts.
 */
import { useTranslation } from "react-i18next";
import { useEffect, useState, useRef } from "react";
import { Card, CardBody, CardHeader } from "@heroui/card";
import { Button } from "@heroui/button";
import { Spinner } from "@heroui/spinner";
import { Alert } from "@heroui/alert";
import { Tabs, Tab } from "@heroui/tabs";
import { Switch } from "@heroui/switch";
import { Select, SelectItem } from "@heroui/select";
import { Chip } from "@heroui/chip";
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  TimeScale,
} from "chart.js";
import { Line } from "react-chartjs-2";
import "chartjs-adapter-date-fns";

import {
  useGenerixConfig,
} from "../authentication/providers/generix-config";

import {
  LastTemperaturesResponse,
  ThermalDataResponse,
  ThermalUtils,
  CurrentTemperatureInfo,
} from "@/types/thermal";
import { title } from "@/components/primitives";
import DefaultLayout from "@/layouts/default";
import { useAuth } from "@/authentication";

// Register Chart.js components
ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  TimeScale,
);

interface TimeRange {
  value: string;
  label: string;
  seconds: number;
}

export default function ThermalPage() {
  const { t } = useTranslation();
  const { isAuthenticated, getJson } = useAuth();

  // Configuration state
  const {
    config: generixConfig
  } = useGenerixConfig();

  // Data states
  const [lastTemperatures, setLastTemperatures] =
    useState<LastTemperaturesResponse | null>(null);
  const [thermalHistory, setThermalHistory] =
    useState<ThermalDataResponse | null>(null);

  // UI states
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedTab, setSelectedTab] = useState<string>("overview");
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [selectedTimeRange, setSelectedTimeRange] = useState<string>("1h");

  // Refs for cleanup
  const refreshIntervalRef = useRef<number | null>(null);

  // Time range options
  const timeRanges: TimeRange[] = [
    { value: "1h", label: t("last-hour"), seconds: 3600 },
    { value: "6h", label: t("last-6-hours"), seconds: 21600 },
    { value: "12h", label: t("last-12-hours"), seconds: 43200 },
    { value: "24h", label: t("last-24-hours"), seconds: 86400 },
    //   { value: "1w", label: t("last-week"), seconds: 604800 },
  ];

  // Fetch functions
  const fetchLastTemperatures = async () => {
    if (!generixConfig || !isAuthenticated) {
      return;
    }

    try {
      const response = (await getJson(
        `${generixConfig.api_base_url}/thermal/temperatures`,
      )) as LastTemperaturesResponse;

      setLastTemperatures(response);
    } catch (error) {
      console.error("Error fetching temperatures:", error);
      throw error;
    }
  };

  const fetchThermalHistory = async (timeRangeSeconds?: number) => {
    if (!generixConfig || !isAuthenticated) {
      return;
    }

    try {
      const now = Math.floor(Date.now() / 1000);
      const from = timeRangeSeconds ? now - timeRangeSeconds : undefined;
      const params = new URLSearchParams();

      params.append("steps", "60"); // 1-minute intervals
      params.append(
        "limit",
        Math.ceil((timeRangeSeconds || 86400) / 60).toString(),
      ); // Limit to the number of steps function of the range or default to 1d if not specified
      if (from) {
        params.append("from", from.toString());
      }

      const response = (await getJson(
        `${generixConfig.api_base_url}/thermal?${params.toString()}`,
      )) as ThermalDataResponse;

      setThermalHistory(response);
    } catch (error) {
      console.error("Error fetching thermal history:", error);
      throw error;
    }
  };

  // Load all data
  const loadAllData = async () => {
    if (!generixConfig || !isAuthenticated) {
      console.warn(t("user-not-authenticated-or-config-not-loaded"));
      setLoading(false);

      return;
    }

    try {
      setError(null);
      const selectedRange = timeRanges.find(
        (r) => r.value === selectedTimeRange,
      );

      await Promise.all([
        fetchLastTemperatures(),
        fetchThermalHistory(selectedRange?.seconds),
      ]);
    } catch (error) {
      console.error(t("thermal-data-error"), error);
      setError(
        error instanceof Error ? error.message : t("thermal-data-error"),
      );
    } finally {
      setLoading(false);
    }
  };

  // Load data when dependencies are ready
  useEffect(() => {
    if (generixConfig && isAuthenticated) {
      loadAllData();
    }
  }, [generixConfig, isAuthenticated, selectedTimeRange]);

  // Auto-refresh effect
  useEffect(() => {
    if (refreshIntervalRef.current) {
      clearInterval(refreshIntervalRef.current);
    }

    if (autoRefresh && generixConfig && isAuthenticated) {
      refreshIntervalRef.current = window.setInterval(() => {
        loadAllData();
      }, 30000); // Refresh every 30 seconds
    }

    return () => {
      if (refreshIntervalRef.current) {
        clearInterval(refreshIntervalRef.current);
      }
    };
  }, [autoRefresh, generixConfig, isAuthenticated, selectedTimeRange]);

  const handleRefresh = () => {
    setLoading(true);
    loadAllData();
  };

  const handleTimeRangeChange = (keys: any) => {
    const selectedKey = Array.from(keys)[0] as string;

    setSelectedTimeRange(selectedKey);
  };

  // Calculate statistics
  const getSystemStats = () => {
    if (!lastTemperatures) {
      return { total: 0, active: 0, errors: 0 };
    }

    const total = Object.keys(lastTemperatures).length;
    let active = 0;
    let errors = 0;

    Object.values(lastTemperatures).forEach((info) => {
      if (ThermalUtils.isRunningStatus(info.status)) {
        active++;
      } else if (ThermalUtils.isErrorStatus(info.status)) {
        errors++;
      }
    });

    return { total, active, errors };
  };

  // Prepare chart data
  const prepareTemperatureChartData = () => {
    if (!thermalHistory || !thermalHistory.data) {
      return null;
    }

    const datasets = Object.entries(thermalHistory.data).map(
      ([regulatorId, dataPoints], index) => {
        const colors = [
          "rgb(59, 130, 246)", // blue
          "rgb(16, 185, 129)", // green
          "rgb(245, 101, 101)", // red
          "rgb(139, 92, 246)", // purple
          "rgb(251, 191, 36)", // yellow
        ];
        const color = colors[index % colors.length];

        return {
          label: `${regulatorId} ${t("temperature-celsius")}`,
          data: dataPoints.map((point) => ({
            x: point.timestamp * 1000, // Convert to milliseconds
            y: point.temperature_celsius,
          })),
          borderColor: color,
          backgroundColor: color + "20",
          tension: 0.1,
        };
      },
    );

    return {
      datasets,
    };
  };

  const prepareControlOutputChartData = () => {
    if (!thermalHistory || !thermalHistory.data) {
      return null;
    }

    const datasets = Object.entries(thermalHistory.data).map(
      ([regulatorId, dataPoints], index) => {
        const colors = [
          "rgb(59, 130, 246)", // blue
          "rgb(16, 185, 129)", // green
          "rgb(245, 101, 101)", // red
          "rgb(139, 92, 246)", // purple
          "rgb(251, 191, 36)", // yellow
        ];
        const color = colors[index % colors.length];

        return {
          label: `${regulatorId} ${t("control-output")}`,
          data: dataPoints.map((point) => ({
            x: point.timestamp * 1000,
            y: point.control_output_percent,
          })),
          borderColor: color,
          backgroundColor: color + "20",
          tension: 0.1,
        };
      },
    );

    return {
      datasets,
    };
  };

  const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
      legend: {
        position: "top" as const,
      },
    },
    scales: {
      x: {
        type: "time" as const,
        time: {
          displayFormats: {
            minute: "HH:mm",
            hour: "HH:mm",
            day: "MMM dd",
          },
        },
        title: {
          display: true,
          text: t("time-range"),
        },
      },
      y: {
        beginAtZero: false,
        title: {
          display: true,
          text: t("temperature-celsius"),
        },
      },
    },
  };

  const controlOutputChartOptions = {
    ...chartOptions,
    scales: {
      ...chartOptions.scales,
      y: {
        beginAtZero: true,
        title: {
          display: true,
          text: t("control-output") + " (%)",
        },
      },
    },
  };

  const stats = getSystemStats();
  const temperatureChartData = prepareTemperatureChartData();
  const controlOutputChartData = prepareControlOutputChartData();

  return (
    <DefaultLayout>
      <section className="flex flex-col items-center justify-center gap-4 py-8 md:py-10">
        <div className="inline-block max-w-lg text-center justify-center">
          <h1 className={title()}>{t("thermal-dashboard")}</h1>
        </div>

        {/* Loading State */}
        {loading && !lastTemperatures && (
          <div className="flex items-center justify-center min-h-96">
            <div className="flex flex-col items-center gap-4">
              <Spinner size="lg" />
              <p className="text-gray-600">{t("loading-thermal-data")}</p>
            </div>
          </div>
        )}

        {/* Error State */}
        {error && !lastTemperatures && (
          <div className="max-w-2xl mx-auto mt-8">
            <Alert
              color="danger"
              description={error}
              endContent={
                <Button color="danger" variant="flat" onPress={handleRefresh}>
                  {t("retry")}
                </Button>
              }
              title={t("thermal-data-error")}
            />
          </div>
        )}

        {/* No Data State */}
        {!loading && !error && !lastTemperatures && (
          <div className="max-w-2xl mx-auto mt-8">
            <Alert
              color="warning"
              description={t("no-thermal-data")}
              title={t("thermal-data-error")}
            />
          </div>
        )}

        {/* Authentication Required */}
        {!isAuthenticated && (
          <div className="max-w-2xl mx-auto mt-8">
            <Alert
              color="warning"
              description={t("authentication_required")}
              title={t("authentication_error")}
            />
          </div>
        )}

        {/* Main Content */}
        {lastTemperatures && isAuthenticated && (
          <div className="container mx-auto px-4 max-w-7xl w-full">
            {/* Controls */}
            <div className="flex items-center justify-between mb-6 flex-wrap gap-4">
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

              <Select
                className="w-48"
                label={t("time-range")}
                selectedKeys={[selectedTimeRange]}
                size="sm"
                onSelectionChange={handleTimeRangeChange}
              >
                {timeRanges.map((range) => (
                  <SelectItem key={range.value}>{range.label}</SelectItem>
                ))}
              </Select>
            </div>

            {/* System Status Overview */}
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
              <Card className="bg-blue-50 border-blue-200">
                <CardBody className="text-center">
                  <p className="text-2xl font-bold text-blue-600">
                    {stats.total}
                  </p>
                  <p className="text-sm text-blue-800">
                    {t("total-regulators")}
                  </p>
                </CardBody>
              </Card>

              <Card className="bg-green-50 border-green-200">
                <CardBody className="text-center">
                  <p className="text-2xl font-bold text-green-600">
                    {stats.active}
                  </p>
                  <p className="text-sm text-green-800">
                    {t("active-regulators")}
                  </p>
                </CardBody>
              </Card>

              <Card
                className={`${stats.errors > 0
                    ? "bg-red-50 border-red-200"
                    : "bg-gray-50 border-gray-200"
                  }`}
              >
                <CardBody className="text-center">
                  <p
                    className={`text-2xl font-bold ${stats.errors > 0 ? "text-red-600" : "text-gray-600"
                      }`}
                  >
                    {stats.errors}
                  </p>
                  <p
                    className={`text-sm ${stats.errors > 0 ? "text-red-800" : "text-gray-800"
                      }`}
                  >
                    {t("error-regulators")}
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
              <Tab key="overview" title={t("current-temperatures")}>
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                  {Object.entries(lastTemperatures).map(
                    ([regulatorId, info]: [string, CurrentTemperatureInfo]) => (
                      <Card key={regulatorId} className="p-4">
                        <CardHeader className="pb-2">
                          <div className="flex justify-between items-center w-full">
                            <h3 className="text-lg font-semibold">
                              {regulatorId}
                            </h3>
                            <Chip
                              className={ThermalUtils.getStatusColor(
                                info.status,
                              )}
                              size="sm"
                              variant="flat"
                            >
                              {info.status}
                            </Chip>
                          </div>
                        </CardHeader>
                        <CardBody>
                          <div className="space-y-2">
                            <div className="flex justify-between">
                              <span className="text-sm text-gray-600">
                                {t("temperature-celsius")}:
                              </span>
                              <span className="font-semibold">
                                {ThermalUtils.formatTemperature(
                                  info.temperature_celsius,
                                )}
                              </span>
                            </div>
                            <div className="flex justify-between">
                              <span className="text-sm text-gray-600">
                                {t("setpoint")}:
                              </span>
                              <span>
                                {ThermalUtils.formatTemperature(
                                  info.setpoint_celsius,
                                )}
                              </span>
                            </div>
                            <div className="flex justify-between">
                              <span className="text-sm text-gray-600">
                                {t("control-output")}:
                              </span>
                              <span>
                                {ThermalUtils.formatControlOutput(
                                  info.control_output_percent,
                                )}
                              </span>
                            </div>
                            <div className="flex justify-between">
                              <span className="text-sm text-gray-600">
                                {t("last-updated")}:
                              </span>
                              <span className="text-xs">
                                {ThermalUtils.timestampToDate(
                                  info.timestamp,
                                ).toLocaleTimeString()}
                              </span>
                            </div>
                          </div>
                        </CardBody>
                      </Card>
                    ),
                  )}
                </div>
              </Tab>

              <Tab key="temperature-chart" title={t("temperature-chart")}>
                <Card className="min-h-[500px]">
                  <CardHeader>
                    <h2 className="text-xl font-semibold">
                      {t("temperature-history-chart")}
                    </h2>
                  </CardHeader>
                  <CardBody className="h-[450px]">
                    {temperatureChartData ? (
                      <Line
                        data={temperatureChartData}
                        options={chartOptions}
                      />
                    ) : (
                      <div className="flex items-center justify-center h-full">
                        <p className="text-gray-500">{t("no-thermal-data")}</p>
                      </div>
                    )}
                  </CardBody>
                </Card>
              </Tab>

              <Tab key="control-chart" title={t("control-output-chart")}>
                <Card className="min-h-[500px]">
                  <CardHeader>
                    <h2 className="text-xl font-semibold">
                      {t("control-output-chart")}
                    </h2>
                  </CardHeader>
                  <CardBody className="h-[450px]">
                    {controlOutputChartData ? (
                      <Line
                        data={controlOutputChartData}
                        options={controlOutputChartOptions}
                      />
                    ) : (
                      <div className="flex items-center justify-center h-full">
                        <p className="text-gray-500">{t("no-thermal-data")}</p>
                      </div>
                    )}
                  </CardBody>
                </Card>
              </Tab>

              <Tab key="raw-data" title={t("raw-data")}>
                <div className="space-y-4">
                  <Card>
                    <CardHeader>
                      <h2 className="text-xl font-semibold">
                        {t("current-temperatures")}
                      </h2>
                    </CardHeader>
                    <CardBody>
                      <pre className="bg-gray-100 p-4 rounded-lg overflow-auto text-sm">
                        {JSON.stringify(lastTemperatures, null, 2)}
                      </pre>
                    </CardBody>
                  </Card>

                  {thermalHistory && (
                    <Card>
                      <CardHeader>
                        <h2 className="text-xl font-semibold">
                          {t("thermal-history")}
                        </h2>
                      </CardHeader>
                      <CardBody>
                        <pre className="bg-gray-100 p-4 rounded-lg overflow-auto text-sm">
                          {JSON.stringify(thermalHistory, null, 2)}
                        </pre>
                      </CardBody>
                    </Card>
                  )}
                </div>
              </Tab>
            </Tabs>
          </div>
        )}
      </section>
    </DefaultLayout>
  );
}
