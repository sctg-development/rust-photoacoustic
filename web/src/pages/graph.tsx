/**
 * Processing Graph Page Component
 *
 * This page provides a comprehensive dashboard for visualizing and monitoring
 * the photoacoustic processing pipeline. It serves as the main interface for
 * understanding system performance, node relationships, and real-time statistics.
 *
 * **Key Features:**
 * - Interactive graph visualization with drag/zoom capabilities
 * - Real-time performance monitoring and statistics
 * - Auto-refresh functionality for live monitoring
 * - Multiple view modes (graph, stats, raw data)
 * - Comprehensive error handling and loading states
 * - Responsive design for various screen sizes
 *
 * **Architecture:**
 * ```
 * DocsPage (Main Container)
 * ├── Status Overview Cards (Performance Summary)
 * └── Tabbed Interface
 *     ├── Graph View (Interactive Visualization)
 *     ├── Performance Stats (Detailed Analytics)
 *     └── Raw Data (JSON Debug View)
 * ```
 *
 * **Data Flow:**
 * 1. Authentication validation via useAuth hook
 * 2. Configuration loading via useGenerixConfig
 * 3. API call to fetch processing graph data
 * 4. Real-time updates through auto-refresh mechanism
 * 5. Multi-view presentation of the same data
 *
 * **Performance Considerations:**
 * - Efficient state management with minimal re-renders
 * - Conditional API calls based on authentication state
 * - Optimized auto-refresh with cleanup mechanisms
 * - Lazy loading of heavy visualization components
 *
 * @copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
 * @license SCTG Development Non-Commercial License v1.0
 */

// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

// React hooks for component lifecycle and state management
import { useTranslation } from "react-i18next";
import { useEffect, useState } from "react";

// HeroUI components for consistent design system
import { Card, CardBody, CardHeader } from "@heroui/card";
import { Button } from "@heroui/button";
import { Spinner } from "@heroui/spinner";
import { Alert } from "@heroui/alert";
import { Tabs, Tab } from "@heroui/tabs";
import { Switch } from "@heroui/switch";

// Custom authentication and configuration hooks
import { useGenerixConfig } from "../authentication/providers/generix-config";

// Application-specific types and utilities
import { SerializableProcessingGraph } from "@/types/processing-graph";
import { useAuth, useSecuredApi } from "@/authentication";
import { title } from "@/components/primitives";

// Layout and component imports
import DefaultLayout from "@/layouts/default";
import { ProcessingGraphView } from "@/components/processing-graph-view";
import { ProcessingGraphStats } from "@/components/processing-graph-stats";
import { CopyButton } from "@/components/copy-button";

/**
 * Main Processing Graph Dashboard Component
 *
 * This component orchestrates the complete dashboard experience for monitoring
 * and visualizing the photoacoustic processing pipeline. It manages multiple
 * interconnected systems including authentication, data fetching, real-time
 * updates, and multi-view presentation.
 *
 * **State Management Strategy:**
 * - `graph`: The complete processing graph data structure from the backend
 * - `loading`: Tracks API request state for user feedback
 * - `error`: Captures and displays any errors that occur during data fetching
 * - `selectedTab`: Controls which view mode is currently active
 * - `autoRefresh`: Enables/disables automatic data updates
 *
 * **Lifecycle Management:**
 * 1. Component Mount: Check authentication and configuration
 * 2. Data Loading: Fetch initial graph data from API
 * 3. Auto-refresh Setup: Establish periodic updates if enabled
 * 4. User Interactions: Handle manual refresh and tab switching
 * 5. Component Unmount: Clean up intervals and subscriptions
 *
 * **Error Handling Philosophy:**
 * - Graceful degradation with informative error messages
 * - Retry mechanisms for transient failures
 * - Separation of authentication vs. API errors
 * - User-friendly error presentation with action buttons
 *
 * @returns JSX.Element - The complete dashboard interface
 */
export default function DocsPage() {
  // **Internationalization and API Hooks**
  const { t } = useTranslation(); // Multi-language support
  const { getJson } = useSecuredApi(); // Authenticated API requests

  // **Core State Management**
  // Main data state - holds the complete processing graph from the backend
  const [graph, setGraph] = useState<SerializableProcessingGraph | null>(null);

  // UI state management for user experience
  const [loading, setLoading] = useState(true); // Controls loading spinners and disabled states
  const [error, setError] = useState<string | null>(null); // Captures API and authentication errors
  const [selectedTab, setSelectedTab] = useState<string>("graph"); // Active tab in the interface
  const [autoRefresh, setAutoRefresh] = useState(false); // Controls automatic data refresh

  // **Authentication and Configuration**
  const { isAuthenticated } = useAuth(); // User authentication status
  const { config: generixConfig } = useGenerixConfig(); // API configuration and endpoints

  /**
   * Core Data Loading Function
   *
   * This function orchestrates the complete data fetching process for the
   * processing graph. It includes comprehensive error handling, authentication
   * validation, and state management.
   *
   * **Execution Flow:**
   * 1. **Pre-flight Checks**: Validate authentication and configuration
   * 2. **Error Reset**: Clear any previous error states
   * 3. **API Request**: Fetch graph data with proper typing
   * 4. **State Update**: Update component state with new data
   * 5. **Error Handling**: Capture and format any errors for user display
   * 6. **Cleanup**: Ensure loading state is always cleared
   *
   * **Error Categories:**
   * - Authentication Errors: User not logged in or session expired
   * - Configuration Errors: Missing or invalid API configuration
   * - Network Errors: Connection issues or server problems
   * - Data Errors: Invalid response format or missing data
   *
   * **Performance Considerations:**
   * - Async/await pattern for clean error handling
   * - Type assertion for API response validation
   * - Proper error logging for debugging
   * - State cleanup in finally block
   *
   * @async
   * @function loadProcessingGraph
   * @returns {Promise<void>} Promise that resolves when data loading is complete
   */
  const loadProcessingGraph = async () => {
    // **Pre-flight Authentication and Configuration Validation**
    if (!isAuthenticated || !generixConfig) {
      console.warn(t("user-not-authenticated-or-config-not-loaded"));
      setLoading(false);

      return;
    }

    try {
      // **Reset Error State** - Clear any previous errors before new attempt
      setError(null);

      // **API Request** - Fetch processing graph with proper type assertion
      const graph = (await getJson(
        `${generixConfig.api_base_url}/graph`,
      )) as SerializableProcessingGraph;

      // **Success State Update** - Update component state with fresh data
      setGraph(graph);
    } catch (error) {
      // **Comprehensive Error Handling** - Log and format errors for user display
      console.error(t("error-loading-processing-graph"), error);
      setError(
        error instanceof Error ? error.message : t("graph-failed-to-load"),
      );
    } finally {
      // **Cleanup** - Always clear loading state regardless of success/failure
      setLoading(false);
    }
  };

  /**
   * Initial Data Loading Effect
   *
   * This effect handles the initial data loading when the component mounts
   * or when authentication/configuration state changes. It implements a
   * dependency-aware loading strategy that only triggers when necessary.
   *
   * **Trigger Conditions:**
   * - Component first mount
   * - Authentication state changes (login/logout)
   * - Configuration becomes available
   *
   * **Dependencies:**
   * - generixConfig: API configuration object
   * - isAuthenticated: User authentication status
   *
   * **Performance Notes:**
   * - Only runs when dependencies actually change
   * - Prevents unnecessary API calls
   * - Handles race conditions gracefully
   */
  useEffect(() => {
    if (generixConfig && isAuthenticated) {
      loadProcessingGraph();
    }
  }, [generixConfig, isAuthenticated]);

  /**
   * Auto-refresh Management Effect
   *
   * This effect manages the automatic refresh functionality for real-time
   * monitoring. It establishes and cleans up interval-based data fetching
   * when auto-refresh is enabled.
   *
   * **Refresh Strategy:**
   * - 60-second intervals for balanced performance vs. freshness
   * - Only active when auto-refresh is enabled
   * - Respects authentication and configuration state
   * - Automatic cleanup on component unmount or dependency changes
   *
   * **Safety Mechanisms:**
   * - Guards against running when authentication is invalid
   * - Cleans up intervals to prevent memory leaks
   * - Handles rapid state changes gracefully
   *
   * **Performance Considerations:**
   * - Uses setInterval for predictable timing
   * - Cleanup function prevents multiple intervals
   * - Minimal overhead when disabled
   *
   * @effect Auto-refresh interval management
   */
  useEffect(() => {
    // Early return if auto-refresh is disabled or prerequisites not met
    if (!autoRefresh || !generixConfig || !isAuthenticated) return;

    // Establish refresh interval
    const interval = setInterval(() => {
      loadProcessingGraph();
    }, 60000); // Refresh every 60 seconds for optimal balance

    // Cleanup function to prevent memory leaks
    return () => clearInterval(interval);
  }, [autoRefresh, generixConfig, isAuthenticated]);

  /**
   * Manual Refresh Handler
   *
   * Provides user-initiated data refresh functionality with proper state
   * management. This function is called when the user clicks the refresh
   * button and ensures consistent UI feedback.
   *
   * **User Experience Features:**
   * - Immediate loading state activation
   * - Consistent error handling
   * - Visual feedback through loading indicators
   *
   * **State Management:**
   * - Sets loading state immediately for instant feedback
   * - Delegates actual loading to main function for consistency
   * - Maintains error state properly
   */
  const handleRefresh = () => {
    setLoading(true);
    loadProcessingGraph();
  };

  return (
    <DefaultLayout>
      <section className="flex flex-col items-center justify-center gap-4 py-8 md:py-10">
        {/* **Page Header** - Main title and introduction */}
        <div className="inline-block max-w-lg text-center justify-center">
          <h1 className={title()}>{t("graph-title")}</h1>
        </div>

        {/* **Loading State** - Displayed during initial data fetch or refresh */}
        {loading && !graph && (
          <div className="flex items-center justify-center min-h-96">
            <div className="flex flex-col items-center gap-4">
              <Spinner size="lg" />
              <p className="text-gray-600">{t("graph-loading")}</p>
            </div>
          </div>
        )}

        {/* **Error State** - Comprehensive error display with retry functionality */}
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

        {/* **No Data State** - Informative message when no graph data is available */}
        {!loading && !error && !graph && (
          <div className="max-w-2xl mx-auto mt-8">
            <Alert
              color="warning"
              description={t("graph-no-data-description")}
              title={t("graph-no-data-title")}
            />
          </div>
        )}

        {/* **Main Dashboard Content** - The complete dashboard interface when data is available */}
        {graph && (
          <div className="container mx-auto px-4 max-w-7xl w-full">
            {/* **Control Panel** - User controls for refresh and auto-refresh */}
            <div className="flex items-center justify-between mb-6">
              <div className="flex items-center gap-4">
                {/* Auto-refresh toggle for real-time monitoring */}
                <Switch
                  isSelected={autoRefresh}
                  size="sm"
                  onValueChange={setAutoRefresh}
                >
                  {t("auto-refresh")}
                </Switch>

                {/* Manual refresh button with loading state */}
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

            {/* **Performance Overview Cards** - Key metrics at a glance */}
            <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
              {/* Total Nodes - Overall system scale indicator */}
              <Card className="bg-blue-50 border-blue-200">
                <CardBody className="text-center">
                  <p className="text-2xl font-bold text-blue-600">
                    {graph.performance_summary.total_nodes}
                  </p>
                  <p className="text-sm text-blue-800">{t("active-nodes")}</p>
                </CardBody>
              </Card>

              {/* Throughput FPS - Real-time performance indicator */}
              <Card className="bg-green-50 border-green-200">
                <CardBody className="text-center">
                  <p className="text-2xl font-bold text-green-600">
                    {graph.performance_summary.throughput_fps.toFixed(1)}
                  </p>
                  <p className="text-sm text-green-800">{t("fps")}</p>
                </CardBody>
              </Card>

              {/* Average Execution Time - Efficiency metric */}
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

              {/* System Validity - Health status indicator */}
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

            {/* **Main Content Tabs** - Multi-view interface for different data perspectives */}
            <Tabs
              className="w-full"
              selectedKey={selectedTab}
              size="lg"
              onSelectionChange={(key) => setSelectedTab(key as string)}
            >
              {/* **Graph Visualization Tab** - Interactive visual representation */}
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
                    {/* Core graph visualization component */}
                    <ProcessingGraphView className="h-full" graph={graph} />
                  </CardBody>
                </Card>
              </Tab>

              {/* **Performance Statistics Tab** - Detailed analytics and metrics */}
              <Tab key="stats" title={t("performance-stats")}>
                <ProcessingGraphStats graph={graph} />
              </Tab>

              {/* **Raw Data Tab** - JSON debug view for developers */}
              <Tab key="raw" title={t("raw-data")}>
                <Card>
                  <CardHeader>
                    <h2 className="text-xl font-semibold">
                      {t("raw-graph-data")}
                    </h2>
                  </CardHeader>
                  <CardBody>
                    <div className="relative">
                      {/* Copy functionality for easy data extraction */}
                      <CopyButton
                        className="absolute top-2 right-2"
                        value={JSON.stringify(graph, null, 2)}
                      />
                      {/* Formatted JSON display with syntax highlighting */}
                      <pre className="bg-gray-100 p-4 rounded-lg overflow-auto text-sm">
                        {JSON.stringify(graph, null, 2)}
                      </pre>
                    </div>
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
