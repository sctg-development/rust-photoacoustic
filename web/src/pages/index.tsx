/**
 * IndexPage Component - Main Measurement Dashboard
 *
 * This component provides a comprehensive dashboard for monitoring photoacoustic
 * measurement data with real-time gauges and historical charts.
 *
 * Features:
 * - Real-time concentration gauges for each configured molecule
 * - Historical concentration chart with customizable data points
 * - Auto-refresh functionality for live monitoring
 * - Dynamic Y-axis scaling for optimal chart visualization
 * - Responsive grid layout for gauges
 * - Internationalization support
 *
 * Data Flow:
 * 1. Fetches visualization output configuration (outputItems)
 * 2. Filters items with display_order > 0 and sorts by display_order
 * 3. Fetches measurement history for each visible item
 * 4. Displays gauges with latest values and chart with historical data
 *
 * @returns JSX.Element The main dashboard component
 */

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

// Register Chart.js components for time-series chart functionality
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

/**
 * Interface for data points selection options
 * Defines the structure for dropdown options that control how many
 * measurement points to fetch and display in the chart
 */
interface DataPointsOption {
  value: string; // String identifier for the option
  label: string; // Localized display text
  limit: number; // Number of data points to fetch from API
}

export default function IndexPage() {
  // Hooks for internationalization and authentication
  const { t } = useTranslation();
  const { user, isAuthenticated, getJson } = useAuth();

  // Configuration state - holds API endpoints and authentication details
  const { config: generixConfig } = useGenerixConfig();

  // Core data states
  const [outputItems, setOutputItems] = useState<VisualizationOutputItem[]>([]); // Configuration items for molecules to display
  const [measurements, setMeasurements] = useState<MeasurementData[][]>([]); // 2D array: measurements[itemIndex][measurementIndex]

  // UI control states
  const [loading, setLoading] = useState(true); // Global loading state for initial data fetch
  const [error, setError] = useState<string | null>(null); // Error message for failed operations
  const [autoRefresh, setAutoRefresh] = useState(false); // Toggle for automatic data refresh
  const [selectedDataPoints, setSelectedDataPoints] = useState<string>("100"); // Number of data points to display

  // Ref for cleanup of auto-refresh interval
  const refreshIntervalRef = useRef<number | null>(null);

  // Configuration options for data points selector
  // These options control how many measurement points are fetched from the API
  const dataPointsOptions: DataPointsOption[] = [
    { value: "10", label: t("last-10-points"), limit: 10 },
    { value: "50", label: t("last-50-points"), limit: 50 },
    { value: "100", label: t("last-100-points"), limit: 100 },
    { value: "200", label: t("last-200-points"), limit: 200 },
  ];

  // ============================================================================
  // DATA FETCHING FUNCTIONS
  // ============================================================================

  /**
   * Fetches the visualization output configuration from the API
   * This determines which molecules/measurements should be displayed in the dashboard
   *
   * The function:
   * 1. Fetches all output configurations from the API
   * 2. Filters items with display_order > 0 (only visible items)
   * 3. Sorts by display_order ascending (left to right, top to bottom)
   * 4. Updates the outputItems state
   *
   * @throws Error if the API request fails
   */
  const fetchOutputItems = async () => {
    if (!generixConfig || !isAuthenticated) {
      return;
    }

    try {
      const outputs = (await getJson(
        `${generixConfig.api_base_url}/config/visualization/output`,
      )) as VisualizationOutputItem[];

      // Filter and sort by display_order
      // Only show items with positive display_order, sorted ascending
      const visibleItems = outputs
        .filter((item) => item.display_order > 0)
        .sort((a, b) => a.display_order - b.display_order);

      setOutputItems(visibleItems);
    } catch (error) {
      console.error("Error fetching output items:", error);
      throw error;
    }
  };

  /**
   * Fetches measurement history data for all visible output items
   * This populates the gauges and chart with actual measurement data
   *
   * The function:
   * 1. Determines the number of data points to fetch based on user selection
   * 2. Makes parallel API requests for all visible items using Promise.all
   * 3. Maintains the order of results to match outputItems array
   * 4. Updates the measurements state with a 2D array structure
   *
   * Data structure: measurements[itemIndex][measurementIndex]
   * - itemIndex corresponds to outputItems array index
   * - measurementIndex goes from 0 (most recent) to limit-1 (oldest)
   *
   * @throws Error if any API request fails
   */
  const fetchMeasurements = async () => {
    if (outputItems.length === 0 || !generixConfig) {
      return;
    }

    try {
      // Determine how many data points to fetch based on user selection
      const selectedOption = dataPointsOptions.find(
        (option) => option.value === selectedDataPoints,
      );
      const limit = selectedOption?.limit || 100;

      // Fetch data for all visible items in parallel
      // Promise.all preserves order, so results[i] corresponds to outputItems[i]
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

  /**
   * Loads all initial data for the dashboard
   * This is the main data loading function called on component mount
   *
   * The function:
   * 1. Validates authentication and configuration
   * 2. Clears any previous errors
   * 3. Fetches output items configuration
   * 4. Handles errors and updates loading state
   *
   * Note: fetchMeasurements is called separately via useEffect when outputItems changes
   */
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

  // ============================================================================
  // REACT EFFECTS - Component Lifecycle Management
  // ============================================================================

  /**
   * Effect: Initial data loading
   * Triggers when authentication state or configuration changes
   * This ensures data is loaded when the user logs in or config is available
   */
  useEffect(() => {
    if (isAuthenticated && generixConfig && user) {
      loadAllData();
    }
  }, [generixConfig, isAuthenticated, user]);

  /**
   * Effect: Fetch measurements when output items or data points selection changes
   * This effect runs after outputItems are loaded or when user changes the data points selector
   */
  useEffect(() => {
    if (outputItems.length > 0) {
      fetchMeasurements();
    }
  }, [outputItems, selectedDataPoints, generixConfig?.api_base_url]);

  /**
   * Effect: Auto-refresh functionality
   * Manages the automatic refresh interval for live monitoring
   *
   * Behavior:
   * - Clears any existing interval when dependencies change
   * - Sets up new interval only if auto-refresh is enabled and user is authenticated
   * - Refreshes measurements every 30 seconds
   * - Cleans up interval on component unmount or when auto-refresh is disabled
   */
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

  // ============================================================================
  // EVENT HANDLERS - User Interaction Management
  // ============================================================================

  /**
   * Handles manual refresh button click
   * Reloads all data from scratch (both output items and measurements)
   */
  const handleRefresh = () => {
    setLoading(true);
    loadAllData();
  };

  /**
   * Handles data points selection change
   * Updates the number of measurement points to fetch and display
   *
   * @param keys - Selected keys from the Select component
   */
  const handleDataPointsChange = (keys: any) => {
    const selectedKey = Array.from(keys)[0] as string;

    setSelectedDataPoints(selectedKey);
  };

  // ============================================================================
  // DATA PROCESSING UTILITIES - Chart and Gauge Data Preparation
  // ============================================================================

  /**
   * Gets the latest (most recent) measurement value for a specific gauge
   * Used to display current values in gauges and beneath them
   *
   * @param index - Index of the output item in the outputItems array
   * @returns The concentration value in ppm, or 0 if no data available
   *
   * Note: measurements[index][0] is the most recent value because the API
   * returns data in reverse chronological order (newest first)
   */
  const getLatestValue = (index: number): number => {
    if (measurements[index] && measurements[index].length > 0) {
      return measurements[index][0].concentration_ppm; // Most recent value is first
    }

    return 0;
  };

  /**
   * Prepares data for the Chart.js line chart component
   * Transforms measurement data into the format expected by Chart.js
   *
   * The function:
   * 1. Validates that measurement data exists
   * 2. Assigns colors to each molecule/dataset
   * 3. Sorts measurements chronologically for proper chart display
   * 4. Converts Rust SystemTime timestamps to JavaScript Date objects
   * 5. Creates Chart.js dataset format with styling
   *
   * @returns Chart.js compatible data object or null if no data
   */
  const prepareConcentrationChartData = () => {
    if (!measurements || measurements.length === 0) {
      return null;
    }

    // Color palette for different molecules/datasets
    // These colors are applied cyclically if there are more molecules than colors
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

    // Create a dataset for each output item (molecule)
    const datasets = outputItems.map((item, index) => {
      const color = colors[index % colors.length];
      const itemMeasurements = measurements[index] || [];

      // Sort measurements by timestamp for chronological order in chart
      // This is important because the API returns data in reverse chronological order
      // but charts should display data chronologically (left to right = old to new)
      const sortedMeasurements = [...itemMeasurements].sort(
        (a, b) =>
          a.timestamp.secs_since_epoch - b.timestamp.secs_since_epoch ||
          a.timestamp.nanos_since_epoch - b.timestamp.nanos_since_epoch,
      );

      return {
        label: `${item.molecule} (${item.unit})`, // Legend label
        data: sortedMeasurements.map((measurement) => ({
          x: new Date(
            // Convert Rust SystemTime to JavaScript Date
            // secs_since_epoch is Unix timestamp in seconds
            // nanos_since_epoch is additional nanoseconds for precision
            measurement.timestamp.secs_since_epoch * 1000 +
              measurement.timestamp.nanos_since_epoch / 1000000,
          ),
          y: measurement.concentration_ppm, // Y-axis value
        })),
        borderColor: color, // Line color
        backgroundColor: color + "20", // Fill color with transparency
        tension: 0.1, // Slight curve to the line for smoother appearance
      };
    });

    return { datasets };
  };

  /**
   * Calculates dynamic Y-axis range for optimal chart visualization
   * Instead of always starting from 0, this centers the chart around actual data values
   *
   * The function:
   * 1. Collects all concentration values from all measurements
   * 2. Finds the minimum and maximum values
   * 3. Calculates an appropriate margin (10% of range or at least 1 unit)
   * 4. Returns min/max values with margins applied
   *
   * Example: If data ranges from 24-26 ppm:
   * - Range = 2 ppm
   * - Margin = max(2 * 0.1, 1) = 1 ppm
   * - Chart shows: 23-27 ppm
   *
   * @returns Object with min/max values for Y-axis, or undefined if no data
   */
  const calculateYAxisRange = () => {
    if (!measurements || measurements.length === 0) {
      return { min: undefined, max: undefined };
    }

    // Collect all concentration values from all measurements
    const allValues: number[] = [];

    measurements.forEach((measurementArray) => {
      measurementArray.forEach((measurement) => {
        allValues.push(measurement.concentration_ppm);
      });
    });

    if (allValues.length === 0) {
      return { min: undefined, max: undefined };
    }

    const minValue = Math.min(...allValues);
    const maxValue = Math.max(...allValues);
    const range = maxValue - minValue;

    // Add 10% margin, or at least 1 unit if range is very small
    // This ensures the chart has some breathing room around the data
    const margin = Math.max(range * 0.1, 1);

    return {
      min: Math.max(0, minValue - margin), // Don't go below 0 (concentrations can't be negative)
      max: maxValue + margin,
    };
  };

  // ============================================================================
  // CHART CONFIGURATION - Chart.js Setup and Options
  // ============================================================================

  // Calculate Y-axis range for dynamic scaling
  const yAxisRange = calculateYAxisRange();

  /**
   * Chart.js configuration object for the concentration history chart
   * Defines appearance, behavior, and scaling options for the time-series chart
   */
  const chartOptions = {
    responsive: true, // Chart resizes with container
    maintainAspectRatio: false, // Allows custom height via container CSS
    plugins: {
      legend: {
        position: "top" as const, // Legend at top of chart
      },
      title: {
        display: true,
        text: t("concentration-history"), // Localized chart title
      },
    },
    scales: {
      x: {
        type: "time" as const, // X-axis displays time values
        time: {
          displayFormats: {
            // Format timestamps based on zoom level
            minute: "HH:mm", // Show hours:minutes for minute-level data
            hour: "HH:mm", // Show hours:minutes for hour-level data
            day: "MMM dd", // Show month abbreviation and day for daily data
          },
        },
        title: {
          display: true,
          text: t("time"), // Localized X-axis label
        },
      },
      y: {
        beginAtZero: false, // Don't force Y-axis to start at zero
        min: yAxisRange.min, // Dynamic minimum based on data
        max: yAxisRange.max, // Dynamic maximum based on data
        title: {
          display: true,
          text: t("concentration-ppm"), // Localized Y-axis label
        },
      },
    },
  };

  // Prepare chart data using the utility function
  const concentrationChartData = prepareConcentrationChartData();

  // ============================================================================
  // COMPONENT RENDER - JSX Structure and Layout
  // ============================================================================

  return (
    <DefaultLayout>
      <section className="flex flex-col items-center justify-center gap-4 py-8 md:py-10">
        {/* Dashboard Header */}
        <div className="inline-block max-w-lg text-center justify-center">
          <h1 className={title()}>{t("measurement-dashboard")}</h1>
          <p className="text-lg text-gray-600 mt-2">
            {t("concentration-monitoring")}
          </p>
        </div>

        {/* Loading State - Shown during initial data fetch */}
        {loading && outputItems.length === 0 && (
          <div className="flex items-center justify-center min-h-96">
            <div className="flex flex-col items-center gap-4">
              <Spinner size="lg" />
              <p className="text-gray-600">{t("loading-measurement-data")}</p>
            </div>
          </div>
        )}

        {/* Error State - Shown when data loading fails */}
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

        {/* No Data State - Shown when no output items are configured */}
        {!loading && !error && outputItems.length === 0 && (
          <div className="max-w-2xl mx-auto mt-8">
            <Alert
              color="warning"
              description={t("no-measurement-data")}
              title={t("measurement-data-error")}
            />
          </div>
        )}

        {/* Authentication Required State - Shown when user is not logged in */}
        {!isAuthenticated && (
          <div className="max-w-2xl mx-auto mt-8">
            <Alert
              color="warning"
              description={t("authentication_required")}
              title={t("authentication_error")}
            />
          </div>
        )}

        {/* Main Dashboard Content - Shown when data is loaded and user is authenticated */}
        {outputItems.length > 0 && isAuthenticated && (
          <div className="container mx-auto px-4 max-w-7xl w-full">
            {/* Control Panel - Auto-refresh toggle and data points selector */}
            <div className="flex items-center justify-between mb-6 flex-wrap gap-4">
              <div className="flex items-center gap-4">
                {/* Auto-refresh toggle switch */}
                <Switch
                  isSelected={autoRefresh}
                  size="sm"
                  onValueChange={setAutoRefresh}
                >
                  {t("auto-refresh")}
                </Switch>

                {/* Manual refresh button */}
                <Button
                  color="primary"
                  isLoading={loading}
                  variant="flat"
                  onPress={handleRefresh}
                >
                  {t("refresh")}
                </Button>
              </div>

              {/* Data points selector - Controls how many measurement points to display */}
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

            {/* Gauges Grid - Real-time concentration displays */}
            {/* Responsive grid: 1 column on mobile, 2 on tablet, 3 on desktop, 4 on wide screens */}
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6 mb-8">
              {outputItems.map((item, index) => (
                <Card key={item.id} className="p-4">
                  {/* Gauge Card Header - Molecule name and description */}
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

                  {/* Gauge Card Body - Gauge chart and current value */}
                  <CardBody className="pt-2">
                    <div className="flex flex-col items-center">
                      {/* Gauge Chart Container */}
                      <div className="w-48 h-32 mb-4">
                        {measurements[index] &&
                        measurements[index].length > 0 ? (
                          <GaugeChart
                            formatTextValue={(value: number) =>
                              `${value.toFixed(1)} ${item.unit}`
                            }
                            nrOfLevels={20} // Number of color segments in the gauge
                            percent={Math.min(
                              Math.max(
                                // Calculate percentage within the configured range
                                (getLatestValue(index) -
                                  item.concentration_min) /
                                  (item.concentration_max -
                                    item.concentration_min),
                                0, // Minimum 0%
                              ),
                              1, // Maximum 100%
                            )}
                            textColor="#000000" // Black text for readability
                          />
                        ) : (
                          // No data available state
                          <div className="flex items-center justify-center h-full">
                            <p className="text-gray-500 text-sm">
                              {t("no-data-available")}
                            </p>
                          </div>
                        )}
                      </div>

                      {/* Current Value Display */}
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
                  // Chart.js Line Chart Component
                  <Line data={concentrationChartData} options={chartOptions} />
                ) : (
                  // No data available state for chart
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
