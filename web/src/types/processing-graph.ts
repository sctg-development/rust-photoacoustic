/**
 * Performance statistics for individual processing nodes
 *
 * Tracks execution metrics for each node in the processing graph,
 * including timing information and frame processing counts.
 */
export interface NodeStatistics {
  /** Unique identifier for the node */
  node_id: string;

  /** Type of processing node (e.g., "input", "filter", "differential") */
  node_type: string;

  /** Total number of frames processed by this node */
  frames_processed: number;

  /** Total processing time in nanoseconds across all frames */
  total_processing_time: number;

  /** Average processing time per frame in nanoseconds */
  average_processing_time: number;

  /** Minimum processing time observed in nanoseconds */
  fastest_processing_time: number;

  /** Maximum processing time observed in nanoseconds */
  worst_processing_time: number;
}

/**
 * Overall performance statistics for the entire processing graph
 *
 * Provides comprehensive metrics about graph execution performance,
 * including timing, throughput, and structural information.
 */
export interface ProcessingGraphStatistics {
  /** Statistics for each individual node, keyed by node ID */
  node_statistics: Record<string, NodeStatistics>;

  /** Total number of complete graph executions */
  total_executions: number;

  /** Total processing time for all graph executions in nanoseconds */
  total_graph_processing_time: number;

  /** Average time per complete graph execution in nanoseconds */
  average_graph_processing_time: number;

  /** Fastest complete graph execution time in nanoseconds */
  fastest_graph_execution: number;

  /** Slowest complete graph execution time in nanoseconds */
  worst_graph_execution: number;

  /** Current number of active nodes in the graph */
  active_nodes: number;

  /** Current number of connections between nodes */
  connections_count: number;
}

/**
 * Quick-access performance summary for the processing graph
 *
 * Provides key performance metrics in a convenient format for
 * dashboards and monitoring interfaces.
 */
export interface PerformanceSummary {
  /** Total number of nodes in the graph */
  total_nodes: number;

  /** Total number of connections between nodes */
  total_connections: number;

  /** Total number of graph executions completed */
  total_executions: number;

  /** Average execution time per graph run in milliseconds */
  average_execution_time_ms: number;

  /** Processing throughput in frames per second */
  throughput_fps: number;

  /** Efficiency percentage (0-100) based on fastest vs slowest execution */
  efficiency_percentage: number;

  /** ID of the node with the highest average processing time */
  slowest_node: string | null;

  /** ID of the node with the lowest average processing time */
  fastest_node: string | null;
}

/**
 * Serializable representation of a processing node
 *
 * Contains all information about a node's configuration, capabilities,
 * and type-specific parameters.
 */
export interface SerializableNode {
  /** Unique identifier for this node */
  id: string;

  /** Type of processing node (determines behavior and capabilities) */
  node_type: string;

  /** List of input data types this node can accept */
  accepts_input_types: string[];

  /** Expected output data type (null if not deterministic) */
  output_type: string | null;

  /** Node-specific configuration parameters */
  parameters: Record<string, any>;
}

/**
 * Represents a directed connection between two nodes in the graph
 *
 * Defines how data flows from one processing node to another.
 */
export interface SerializableConnection {
  /** Source node identifier */
  from: string;

  /** Target node identifier */
  to: string;
}

/**
 * Complete serializable representation of a processing graph
 *
 * This is the main response type for the `/api/graph` endpoint.
 * It contains all information about the graph structure, execution
 * order, performance statistics, and validation status.
 *
 * @example
 * ```typescript
 * // Fetching graph data
 * const response = await fetch('/api/graph', {
 *   headers: { 'Authorization': `Bearer ${token}` }
 * });
 * const graph: SerializableProcessingGraph = await response.json();
 *
 * // Accessing node information
 * const inputNode = graph.nodes.find(n => n.node_type === 'input');
 * console.log(`Input node: ${inputNode?.id}`);
 *
 * // Checking performance
 * const slowestNode = graph.performance_summary.slowest_node;
 * if (slowestNode) {
 *   const nodeStats = graph.statistics.node_statistics[slowestNode];
 *   console.log(`Bottleneck: ${nodeStats.node_id} (${nodeStats.average_processing_time / 1000000}ms avg)`);
 * }
 * ```
 */
export interface SerializableProcessingGraph {
  /** All processing nodes in the graph with their configurations */
  nodes: SerializableNode[];

  /** All connections between nodes defining the data flow */
  connections: SerializableConnection[];

  /** Topologically sorted execution order of node IDs */
  execution_order: string[];

  /** ID of the designated input node (entry point for data) */
  input_node: string | null;

  /** IDs of designated output nodes (where final results are produced) */
  output_nodes: string[];

  /** Comprehensive performance statistics for the graph and all nodes */
  statistics: ProcessingGraphStatistics;

  /** Quick-access performance metrics summary */
  performance_summary: PerformanceSummary;

  /** Whether the graph passes all validation checks */
  is_valid: boolean;

  /** List of validation errors if any issues were found */
  validation_errors: string[];
}

/**
 * Utility functions for working with processing graph data
 */
export namespace ProcessingGraphUtils {
  /**
   * Convert nanoseconds to milliseconds for display
   * @param nanoseconds - Time in nanoseconds
   * @returns Time in milliseconds
   */
  export function nanosecondsToMilliseconds(nanoseconds: number): number {
    return nanoseconds / 1_000_000;
  }

  /**
   * Convert nanoseconds to microseconds for display
   * @param nanoseconds - Time in nanoseconds
   * @returns Time in microseconds
   */
  export function nanosecondsToMicroseconds(nanoseconds: number): number {
    return nanoseconds / 1_000;
  }

  /**
   * Format duration for human-readable display
   * @param nanoseconds - Time in nanoseconds
   * @returns Formatted duration string
   */
  export function formatDuration(nanoseconds: number): string {
    const ms = nanosecondsToMilliseconds(nanoseconds);

    if (ms < 1) {
      return `${nanosecondsToMicroseconds(nanoseconds).toFixed(1)}μs`;
    } else if (ms < 1000) {
      return `${ms.toFixed(2)}ms`;
    } else {
      return `${(ms / 1000).toFixed(2)}s`;
    }
  }

  /**
   * Find the node with the highest average processing time
   * @param graph - The processing graph
   * @returns The slowest node statistics or null if no nodes have been processed
   */
  export function findSlowestNode(
    graph: SerializableProcessingGraph,
  ): NodeStatistics | null {
    const nodeStats = Object.values(graph.statistics.node_statistics);

    if (nodeStats.length === 0) return null;

    return nodeStats.reduce((slowest, current) =>
      current.average_processing_time > slowest.average_processing_time
        ? current
        : slowest,
    );
  }

  /**
   * Find the node with the lowest average processing time
   * @param graph - The processing graph
   * @returns The fastest node statistics or null if no nodes have been processed
   */
  export function findFastestNode(
    graph: SerializableProcessingGraph,
  ): NodeStatistics | null {
    const nodeStats = Object.values(graph.statistics.node_statistics);
    const processedNodes = nodeStats.filter(
      (stats) => stats.frames_processed > 0,
    );

    if (processedNodes.length === 0) return null;

    return processedNodes.reduce((fastest, current) =>
      current.average_processing_time < fastest.average_processing_time
        ? current
        : fastest,
    );
  }

  /**
   * Get nodes sorted by average processing time (slowest first)
   * @param graph - The processing graph
   * @returns Array of node statistics sorted by performance
   */
  export function getNodesByPerformance(
    graph: SerializableProcessingGraph,
  ): NodeStatistics[] {
    return Object.values(graph.statistics.node_statistics)
      .filter((stats) => stats.frames_processed > 0)
      .sort((a, b) => b.average_processing_time - a.average_processing_time);
  }

  /**
   * Check if a node is a bottleneck (significantly slower than others)
   * @param graph - The processing graph
   * @param nodeId - ID of the node to check
   * @param threshold - Multiplier threshold (default: 2.0)
   * @returns True if the node is a bottleneck
   */
  export function isBottleneck(
    graph: SerializableProcessingGraph,
    nodeId: string,
    threshold: number = 2.0,
  ): boolean {
    const nodeStats = graph.statistics.node_statistics[nodeId];

    if (!nodeStats || nodeStats.frames_processed === 0) return false;

    const allStats = Object.values(graph.statistics.node_statistics).filter(
      (stats) => stats.frames_processed > 0 && stats.node_id !== nodeId,
    );

    if (allStats.length === 0) return false;

    const avgProcessingTime =
      allStats.reduce((sum, stats) => sum + stats.average_processing_time, 0) /
      allStats.length;

    return nodeStats.average_processing_time > avgProcessingTime * threshold;
  }

  /**
   * Get processing efficiency as a percentage
   * @param graph - The processing graph
   * @returns Efficiency percentage (0-100)
   */
  export function getEfficiencyPercentage(
    graph: SerializableProcessingGraph,
  ): number {
    const stats = graph.statistics;

    if (stats.worst_graph_execution === 0) return 100;

    return (stats.fastest_graph_execution / stats.worst_graph_execution) * 100;
  }

  /**
   * Validate graph structure and return issues
   * @param graph - The processing graph
   * @returns Array of validation issues found
   */
  export function validateGraph(graph: SerializableProcessingGraph): string[] {
    const issues: string[] = [];

    // Check for input node
    if (!graph.input_node) {
      issues.push("No input node defined");
    }

    // Check for orphaned nodes
    const connectedNodes = new Set<string>();

    graph.connections.forEach((conn) => {
      connectedNodes.add(conn.from);
      connectedNodes.add(conn.to);
    });

    graph.nodes.forEach((node) => {
      if (node.id !== graph.input_node && !connectedNodes.has(node.id)) {
        issues.push(`Node '${node.id}' is not connected to the graph`);
      }
    });

    // Check for missing nodes in connections
    const nodeIds = new Set(graph.nodes.map((n) => n.id));

    graph.connections.forEach((conn) => {
      if (!nodeIds.has(conn.from)) {
        issues.push(`Connection references missing node '${conn.from}'`);
      }
      if (!nodeIds.has(conn.to)) {
        issues.push(`Connection references missing node '${conn.to}'`);
      }
    });

    return issues;
  }
}

/**
 * Type guards for processing graph data
 */
export namespace ProcessingGraphTypeGuards {
  /**
   * Type guard to check if an object is a valid SerializableProcessingGraph
   */
  export function isSerializableProcessingGraph(
    obj: any,
  ): obj is SerializableProcessingGraph {
    return (
      obj &&
      typeof obj === "object" &&
      Array.isArray(obj.nodes) &&
      Array.isArray(obj.connections) &&
      Array.isArray(obj.execution_order) &&
      Array.isArray(obj.output_nodes) &&
      typeof obj.statistics === "object" &&
      typeof obj.performance_summary === "object" &&
      typeof obj.is_valid === "boolean" &&
      Array.isArray(obj.validation_errors)
    );
  }

  /**
   * Type guard to check if an object is a valid NodeStatistics
   */
  export function isNodeStatistics(obj: any): obj is NodeStatistics {
    return (
      obj &&
      typeof obj === "object" &&
      typeof obj.node_id === "string" &&
      typeof obj.node_type === "string" &&
      typeof obj.frames_processed === "number" &&
      typeof obj.total_processing_time === "number" &&
      typeof obj.average_processing_time === "number" &&
      typeof obj.fastest_processing_time === "number" &&
      typeof obj.worst_processing_time === "number"
    );
  }
}
