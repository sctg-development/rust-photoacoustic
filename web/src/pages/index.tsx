import { useTranslation } from "react-i18next";
import { useEffect, useState, useRef } from "react";
import GaugeChart from "react-gauge-chart";
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

import { Card, CardBody, CardHeader } from "@heroui/card";
import { Button } from "@heroui/button";
import { Spinner } from "@heroui/spinner";
import { Alert } from "@heroui/alert";
import { Switch } from "@heroui/switch";
import { Select, SelectItem } from "@heroui/select";

import DefaultLayout from "@/layouts/default";
import { useGenerixConfig } from "@/authentication/providers/generix-config";
import { useAuth } from "@/authentication";
import { VisualizationOutputItem, MeasurementData } from "@/types";
import { title } from "@/components/primitives";

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

interface DataPointsOption {
  value: string;
  label: string;
  limit: number;
}

export default function IndexPage() {
  const { t } = useTranslation();
  const { user, isAuthenticated, getJson } = useAuth();
  // Configuration state - holds API endpoints and authentication details
  const { config: generixConfig } = useGenerixConfig();
  const [outputItems, setOutputItems] = useState<VisualizationOutputItem[]>([]);
  const [measurements, setMeasurements] = useState<MeasurementData[][]>([]);

  // UI states
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [selectedDataPoints, setSelectedDataPoints] = useState<string>("100");

  // Refs for cleanup
  const refreshIntervalRef = useRef<number | null>(null);

  // Data points options
  const dataPointsOptions: DataPointsOption[] = [
    { value: "10", label: t("last-10-points"), limit: 10 },
    { value: "50", label: t("last-50-points"), limit: 50 },
    { value: "100", label: t("last-100-points"), limit: 100 },
    { value: "200", label: t("last-200-points"), limit: 200 },
  ];

  // Fetch functions
  const fetchOutputItems = async () => {
    if (!generixConfig || !isAuthenticated) {
      return;
    }

    try {
      const outputs = (await getJson(
        `${generixConfig.api_base_url}/config/visualization/output`,
      )) as VisualizationOutputItem[];

      // Filter and sort by display_order
      const visibleItems = outputs
        .filter((item) => item.display_order > 0)
        .sort((a, b) => a.display_order - b.display_order);

      setOutputItems(visibleItems);
    } catch (error) {
      console.error("Error fetching output items:", error);
      throw error;
    }
  };

  const fetchMeasurements = async () => {
    if (outputItems.length === 0 || !generixConfig) {
      return;
    }

    try {
      const selectedOption = dataPointsOptions.find(
        (option) => option.value === selectedDataPoints,
      );
      const limit = selectedOption?.limit || 100;

      // Fetch data for all visible items
      const measurementsData = await Promise.all(
        outputItems.map(async (item) => {
          const response = (await getJson(
            `${generixConfig.api_base_url}/action/${item.action_node_id}/history?limit=${limit}`,
          )) as MeasurementData[];

          return response;
        }),
      );

      setMeasurements(measurementsData);
    } catch (error) {
      console.error("Error fetching measurements:", error);
      throw error;
    }
  };

  // Load all data
  const loadAllData = async () => {
    if (!generixConfig || !isAuthenticated) {
      console.warn(t("authentication_required"));
      setLoading(false);

      return;
    }

    try {
      setError(null);
      await fetchOutputItems();
    } catch (error) {
      console.error(t("measurement-data-error"), error);
      setError(
        error instanceof Error ? error.message : t("measurement-data-error"),
      );
    } finally {
      setLoading(false);
    }
  };

  // Retrieve the output elements
  useEffect(() => {
    if (isAuthenticated && generixConfig && user) {
      loadAllData();
    }
  }, [generixConfig, isAuthenticated, user]);

  // Populate the measurements from the output items
  useEffect(() => {
    if (outputItems.length > 0) {
      fetchMeasurements();
    }
  }, [outputItems, selectedDataPoints, generixConfig?.api_base_url]);

  // Auto-refresh effect
  useEffect(() => {
    if (refreshIntervalRef.current) {
      clearInterval(refreshIntervalRef.current);
    }

    if (autoRefresh && generixConfig && isAuthenticated) {
      refreshIntervalRef.current = window.setInterval(() => {
        fetchMeasurements();
      }, 30000); // Refresh every 30 seconds
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
    outputItems,
    selectedDataPoints,
  ]);

  const handleRefresh = () => {
    setLoading(true);
    loadAllData();
  };

  const handleDataPointsChange = (keys: any) => {
    const selectedKey = Array.from(keys)[0] as string;

    setSelectedDataPoints(selectedKey);
  };

  // Get the latest measurement value for a gauge
  const getLatestValue = (index: number): number => {
    if (measurements[index] && measurements[index].length > 0) {
      return measurements[index][0].concentration_ppm; // Most recent value is first
    }

    return 0;
  };

  // Prepare chart data for concentration history
  const prepareConcentrationChartData = () => {
    if (!measurements || measurements.length === 0) {
      return null;
    }

    const colors = [
      "rgb(59, 130, 246)", // blue
      "rgb(16, 185, 129)", // green
      "rgb(245, 101, 101)", // red
      "rgb(139, 92, 246)", // purple
      "rgb(251, 191, 36)", // yellow
      "rgb(236, 72, 153)", // pink
      "rgb(14, 165, 233)", // sky
      "rgb(34, 197, 94)", // emerald
    ];

    const datasets = outputItems.map((item, index) => {
      const color = colors[index % colors.length];
      const itemMeasurements = measurements[index] || [];

      return {
        label: `${item.molecule} (${item.unit})`,
        data: itemMeasurements.map((measurement) => ({
          x: new Date(measurement.timestamp), // timestamp is ISO 8601 string
          y: measurement.concentration_ppm,
        })),
        borderColor: color,
        backgroundColor: color + "20",
        tension: 0.1,
      };
    });

    return { datasets };
  };

  const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
      legend: {
        position: "top" as const,
      },
      title: {
        display: true,
        text: t("concentration-history"),
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
          text: t("time"),
        },
      },
      y: {
        beginAtZero: true,
        title: {
          display: true,
          text: t("concentration-ppm"),
        },
      },
    },
  };

  const concentrationChartData = prepareConcentrationChartData();

  return (
    <DefaultLayout>
      <section className="flex flex-col items-center justify-center gap-4 py-8 md:py-10">
        <div className="inline-block max-w-lg text-center justify-center">
          <h1 className={title()}>{t("measurement-dashboard")}</h1>
          <p className="text-lg text-gray-600 mt-2">
            {t("concentration-monitoring")}
          </p>
        </div>

        {/* Loading State */}
        {loading && outputItems.length === 0 && (
          <div className="flex items-center justify-center min-h-96">
            <div className="flex flex-col items-center gap-4">
              <Spinner size="lg" />
              <p className="text-gray-600">{t("loading-measurement-data")}</p>
            </div>
          </div>
        )}

        {/* Error State */}
        {error && outputItems.length === 0 && (
          <div className="max-w-2xl mx-auto mt-8">
            <Alert
              color="danger"
              description={error}
              endContent={
                <Button color="danger" variant="flat" onPress={handleRefresh}>
                  {t("retry")}
                </Button>
              }
              title={t("measurement-data-error")}
            />
          </div>
        )}

        {/* No Data State */}
        {!loading && !error && outputItems.length === 0 && (
          <div className="max-w-2xl mx-auto mt-8">
            <Alert
              color="warning"
              description={t("no-measurement-data")}
              title={t("measurement-data-error")}
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
        {outputItems.length > 0 && isAuthenticated && (
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
                label={t("data-points")}
                selectedKeys={[selectedDataPoints]}
                size="sm"
                onSelectionChange={handleDataPointsChange}
              >
                {dataPointsOptions.map((option) => (
                  <SelectItem key={option.value}>{option.label}</SelectItem>
                ))}
              </Select>
            </div>

            {/* Gauges Grid */}
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6 mb-8">
              {outputItems.map((item, index) => (
                <Card key={item.id} className="p-4">
                  <CardHeader className="pb-2">
                    <div className="flex flex-col items-center w-full">
                      <h3 className="text-lg font-semibold text-center">
                        {item.molecule}
                      </h3>
                      <p className="text-sm text-gray-600 text-center">
                        {item.description}
                      </p>
                    </div>
                  </CardHeader>
                  <CardBody className="pt-2">
                    <div className="flex flex-col items-center">
                      {/* Gauge */}
                      <div className="w-48 h-32 mb-4">
                        {measurements[index] &&
                          measurements[index].length > 0 ? (
                          <GaugeChart
                            formatTextValue={(value: number) =>
                              `${value.toFixed(1)} ${item.unit}`
                            }
                            nrOfLevels={20}
                            percent={Math.min(
                              Math.max(
                                (getLatestValue(index) -
                                  item.concentration_min) /
                                (item.concentration_max -
                                  item.concentration_min),
                                0,
                              ),
                              1,
                            )}
                            textColor="#000000"
                          />
                        ) : (
                          <div className="flex items-center justify-center h-full">
                            <p className="text-gray-500 text-sm">
                              {t("no-data-available")}
                            </p>
                          </div>
                        )}
                      </div>

                      {/* Current Value */}
                      <div className="text-center">
                        <p className="text-sm text-gray-600">
                          {t("current-value")}
                        </p>
                        <p className="text-xl font-bold">
                          {measurements[index] && measurements[index].length > 0
                            ? `${getLatestValue(index).toFixed(2)} ${item.unit}`
                            : "--"}
                        </p>
                      </div>
                    </div>
                  </CardBody>
                </Card>
              ))}
            </div>

            {/* Concentration History Chart */}
            <Card className="min-h-[500px]">
              <CardHeader>
                <h2 className="text-xl font-semibold">
                  {t("concentration-history")}
                </h2>
              </CardHeader>
              <CardBody className="h-[450px]">
                {concentrationChartData ? (
                  <Line data={concentrationChartData} options={chartOptions} />
                ) : (
                  <div className="flex items-center justify-center h-full">
                    <p className="text-gray-500">{t("no-measurement-data")}</p>
                  </div>
                )}
              </CardBody>
            </Card>
          </div>
        )}
      </section>
    </DefaultLayout>
  );
}
