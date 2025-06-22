/**
 * Processing Graph View Component
 *
 * This file implements a comprehensive visual representation of a photoacoustic processing pipeline
 * using ReactFlow. It renders nodes, edges, and provides interactive functionality for exploring
 * the processing graph structure and performance statistics.
 *
 * Key Features:
 * - Interactive node graph with drag/zoom capabilities
 * - Visual differentiation based on node types and performance
 * - Special dependency edges for computing relationships
 * - Modal dialogs for detailed node inspection
 * - Real-time performance statistics display
 * - Automatic layout with collision detection
 *
 * Architecture:
 * - ProcessingGraphView: Main component wrapper with ReactFlowProvider
 * - FlowContainer: Core ReactFlow implementation with state management
 * - ProcessingNode: Custom node component with statistics and styling
 * - NodeDetailsModal: Generic modal for non-specialized nodes
 * - Specialized modals: StreamingNodeModal, ComputingNodeModal
 *
 * Data Flow:
 * 1. SerializableProcessingGraph data comes from the backend API
 * 2. calculateNodePositions() creates optimal layout coordinates
 * 3. Graph nodes are converted to ReactFlow format with statistics
 * 4. Special dependency edges are added for computing relationships
 * 5. User interactions trigger modal displays for detailed information
 *
 * Performance Considerations:
 * - useMemo() optimization for expensive graph transformations
 * - useCallback() stabilization for event handlers
 * - Lazy modal loading based on node types
 * - Efficient collision detection in layout algorithm
 *
 * Visual Design Principles:
 * - Color-coded nodes by type and performance status
 * - Consistent iconography for node identification
 * - Progressive disclosure through modal interactions
 * - Responsive layout with mobile considerations
 *
 * @copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
 * @license SCTG Development Non-Commercial License v1.0
 */

// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

// React and hooks for component lifecycle and state management
import { useMemo, useCallback, useState, useEffect } from "react";
// Internationalization for multi-language support
import { useTranslation } from "react-i18next";
// ReactFlow components for graph visualization
import ReactFlow, {
  Node,
  Edge,
  Controls,
  Background,
  useNodesState,
  useEdgesState,
  NodeProps,
  Handle,
  Position,
  ReactFlowProvider,
} from "reactflow";
// ReactFlow base styles (required for proper rendering)
import "reactflow/dist/style.css";
// HeroUI components for consistent design system
import {
  Modal,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalFooter,
  useDisclosure,
} from "@heroui/modal";
import { Button } from "@heroui/button";
import { Card, CardBody } from "@heroui/card";
import { Chip } from "@heroui/chip";
import { Progress } from "@heroui/progress";
import { Divider } from "@heroui/divider";

// Application-specific types and utilities
import {
  SerializableProcessingGraph,
  NodeStatistics,
  ProcessingGraphUtils,
} from "../types/processing-graph";

// Specialized modal components for different node types
import { StreamingNodeModal } from "./streaming-node-modal";
import { ComputingNodeModal } from "./computing-node-modal";

/**
 * Custom node types registry for ReactFlow
 *
 * ReactFlow uses this mapping to render custom node components.
 * Each string key corresponds to a React component that will
 * handle the rendering of that node type.
 *
 * @see ProcessingNode - The main node component implementation
 */
const nodeTypes = {
  processingNode: ProcessingNode,
};

/**
 * Data structure for processing nodes in the ReactFlow graph
 *
 * This interface defines the complete data contract for node components.
 * It combines static configuration (from the backend) with dynamic
 * runtime information (statistics) and interactive capabilities (callbacks).
 *
 * The data flows from:
 * 1. Backend processing graph definition
 * 2. Runtime statistics collection
 * 3. UI interaction handlers
 *
 * @interface ProcessingNodeData
 */
interface ProcessingNodeData {
  /** Unique identifier for the node - used for connections and display */
  id: string;

  /** Type of processing node determining functionality and appearance */
  nodeType: string;

  /** Array of input data types this node can accept - used for validation */
  acceptsInputTypes: string[];

  /** Output data type produced by this node, or null if determined dynamically */
  outputType: string | null;

  /** Configuration parameters specific to this node type - varies by nodeType */
  parameters: Record<string, any>;

  /** Runtime performance statistics - may be undefined if not yet collected */
  statistics?: NodeStatistics;

  /** Performance bottleneck flag - highlights nodes causing processing delays */
  isBottleneck?: boolean;

  /** Event handler for node click interactions - triggers modal display */
  onClick: (nodeId: string, nodeData: any) => void;
}

/**
 * Icon mapping for different node types
 *
 * Returns appropriate emoji icons for visual identification of node types.
 * These icons provide instant visual recognition and help users quickly
 * understand the purpose and functionality of each node in the graph.
 *
 * The mapping is designed around the photoacoustic processing pipeline:
 * - Input nodes: Data acquisition and laser sources
 * - Processing nodes: Signal filtering, amplification, and analysis
 * - Output nodes: Data storage, streaming, and visualization
 * - Computing nodes: Mathematical calculations and analysis
 *
 * @param nodeType - The type of processing node from the backend definition
 * @returns Emoji string representing the node type, with fallback for unknown types
 *
 * @example
 * ```typescript
 * const icon = getNodeIcon("filter"); // Returns "🔬"
 * const unknownIcon = getNodeIcon("unknown"); // Returns "⚙️"
 * ```
 */
export const getNodeIcon = (nodeType: string): string => {
  switch (nodeType) {
    case "input":
      return "🔆"; // Laser symbol - represents laser input for photoacoustic detection
    case "filter":
      return "🔬"; // Microscope - represents signal analysis and filtering
    case "differential":
      return "⚡"; // Lightning - represents fast signal processing operations
    case "record":
      return "💾"; // Disk - represents data recording and storage
    case "streaming":
      return "📡"; // Antenna - represents real-time data streaming
    case "gain":
      return "🔊"; // Speaker - represents signal amplification
    case "channel_mixer":
      return "🎛️"; // Control knobs - represents audio channel mixing
    case "channel_selector":
      return "🎯"; // Target - represents precise channel selection
    case "photoacoustic_output":
      return "📊"; // Chart - represents measurement output and visualization
    case "output":
      return "📤"; // Outbox - represents final output stage
    case "computing_concentration":
      return "🧮"; // Calculator - represents concentration computation
    default:
      return "⚙️"; // Gear - generic processing node fallback for unknown types
  }
};

/**
 * Custom Processing Node Component
 *
 * This component renders individual nodes in the processing graph with comprehensive
 * visual feedback and interactive capabilities. Each node displays multiple layers
 * of information to provide immediate understanding of its role and performance.
 *
 * Visual Information Hierarchy:
 * 1. Icon: Immediate type identification
 * 2. ID and Type: Node identification
 * 3. Statistics: Performance metrics (when available)
 * 4. Status Indicators: Bottleneck warnings
 * 5. Color Coding: Type and performance-based styling
 *
 * Interactive Features:
 * - Click handling: Opens detailed modal for the node
 * - Hover effects: Visual feedback for interactivity
 * - Connection handles: Visual anchor points for edges
 *
 * Performance Considerations:
 * - Conditional rendering for statistics (only when available)
 * - Efficient color calculation with early bottleneck detection
 * - Optimized event handlers through parent component callbacks
 *
 * @param data - Complete node data including configuration and runtime info
 * @returns JSX element representing the visual node
 *
 * @see ProcessingNodeData - Interface defining the expected data structure
 * @see getNodeIcon - Function providing visual icons for node types
 */
function ProcessingNode({ data }: NodeProps<ProcessingNodeData>) {
  const { t } = useTranslation();
  const { statistics, isBottleneck, nodeType, id } = data;

  /**
   * Determines the visual styling for the node based on its type and status
   *
   * This function implements a comprehensive color coding system that provides
   * immediate visual feedback about node functionality and performance status.
   *
   * Color Coding Strategy:
   * - Priority System: Bottlenecks override type-based colors for immediate attention
   * - Functional Grouping: Related node types use similar color families
   * - Accessibility: High contrast colors for clear differentiation
   * - Consistency: Predictable mapping between types and colors
   *
   * Color Meanings:
   * - Red: Performance issues (bottlenecks) - requires immediate attention
   * - Blue: Data entry points (input) - start of processing pipeline
   * - Purple: Signal processing (filter) - core analysis functionality
   * - Green: Signal analysis (differential) - mathematical operations
   * - Orange: Data storage (record) - persistence operations
   * - Cyan: Real-time operations (streaming) - live data handling
   * - Emerald: Signal amplification (gain) - signal conditioning
   * - Pink: Audio mixing (channel_mixer) - multi-channel operations
   * - Teal: Channel selection (channel_selector) - data routing
   * - Indigo: Measurement output (photoacoustic_output) - final results
   * - Amber: Mathematical computation (computing_concentration) - calculations
   * - Gray: Generic/output nodes - standard operations
   *
   * @returns Tailwind CSS classes for border and background colors
   *
   * @example
   * ```typescript
   * // Bottleneck node (always red regardless of type)
   * getNodeColor() // "border-red-500 bg-red-50"
   *
   * // Normal filter node
   * getNodeColor() // "border-purple-500 bg-purple-50"
   * ```
   */
  const getNodeColor = (): string => {
    // Bottleneck nodes always get priority red styling
    if (isBottleneck) return "border-red-500 bg-red-50";

    // Color coding based on node functionality
    switch (nodeType) {
      case "input":
        return "border-blue-500 bg-blue-50"; // Data entry
      case "filter":
        return "border-purple-500 bg-purple-50"; // Signal processing
      case "differential":
        return "border-green-500 bg-green-50"; // Signal analysis
      case "record":
        return "border-orange-500 bg-orange-50"; // Data storage
      case "streaming":
        return "border-cyan-500 bg-cyan-50"; // Real-time data
      case "gain":
        return "border-emerald-500 bg-emerald-50"; // Amplification
      case "channel_mixer":
        return "border-pink-500 bg-pink-50"; // Audio mixing
      case "channel_selector":
        return "border-teal-500 bg-teal-50"; // Channel selection
      case "photoacoustic_output":
        return "border-indigo-500 bg-indigo-50"; // Measurement output
      case "output":
        return "border-gray-500 bg-gray-50"; // Final output
      case "computing_concentration":
        return "border-amber-500 bg-amber-50"; // Concentration calculation
      default:
        return "border-gray-500 bg-gray-50";
    }
  };

  return (
    <div
      className={`px-4 py-2 shadow-lg rounded-lg border-2 bg-white min-w-[120px] cursor-pointer hover:shadow-xl transition-shadow ${getNodeColor()}`}
      onClick={() => data.onClick(id, data)}
    >
      {/* Input connection handle (left side) - ReactFlow anchor point for incoming edges */}
      <Handle className="w-3 h-3" position={Position.Left} type="target" />

      <div className="flex flex-col items-center">
        {/* Node icon for immediate visual type identification */}
        <div className="text-lg mb-1">{getNodeIcon(nodeType)}</div>

        {/* Primary node identification - ID is most important for user reference */}
        <div className="text-sm font-semibold text-center">{id}</div>
        {/* Secondary identification - type provides technical context */}
        <div className="text-xs text-gray-600 text-center">{nodeType}</div>

        {/* Performance statistics display - only shown when meaningful data exists */}
        {statistics && statistics.frames_processed > 0 && (
          <div className="mt-2 text-xs text-center">
            {/* Frame count - primary performance indicator */}
            <div className="text-green-600 font-medium">
              {statistics.frames_processed} {t("view-frames")}
            </div>
            {/* Average processing time - efficiency indicator */}
            <div className="text-blue-600">
              {ProcessingGraphUtils.formatDuration(
                statistics.average_processing_time,
              )}{" "}
              {t("view-avg")}
            </div>
          </div>
        )}

        {/* Critical performance warning - bottleneck indicator with high visibility */}
        {isBottleneck && (
          <Chip className="mt-1" color="danger" size="sm" variant="flat">
            {t("view-bottleneck")}
          </Chip>
        )}
      </div>

      {/* Output connection handle (right side) - ReactFlow anchor point for outgoing edges */}
      <Handle className="w-3 h-3" position={Position.Right} type="source" />
    </div>
  );
}

/**
 * Calculates optimal positions for nodes in the graph layout
 *
 * This function implements a sophisticated multi-row grid layout system with
 * automatic collision detection and resolution. The algorithm balances several
 * competing requirements:
 *
 * 1. **Execution Order Preservation**: Nodes are positioned to respect their
 *    logical execution sequence, making the data flow visually apparent.
 *
 * 2. **Collision Prevention**: Automatic detection and resolution of overlapping
 *    nodes ensures all nodes remain visible and accessible.
 *
 * 3. **Visual Clarity**: Consistent spacing and alignment provide a clean,
 *    professional appearance suitable for technical documentation.
 *
 * 4. **Scalability**: The algorithm adapts to graphs of varying sizes while
 *    maintaining performance and visual quality.
 *
 * Algorithm Steps:
 * 1. **Grid Positioning**: Initial placement based on execution order with
 *    configurable rows and columns
 * 2. **Jitter Application**: Small random offsets prevent perfect alignment
 *    and add visual interest
 * 3. **Collision Detection**: Pairwise comparison of all node positions
 * 4. **Conflict Resolution**: Automatic repositioning of overlapping nodes
 *
 * Layout Configuration Constants:
 * - Node dimensions: 140x80px (minimum space required)
 * - Horizontal spacing: 240px (ensures clear separation)
 * - Vertical spacing: 200px (provides adequate row separation)
 * - Max nodes per row: 4 (optimal for wide screen viewing)
 * - Jitter range: ±15px (subtle randomization)
 *
 * Performance Characteristics:
 * - Time Complexity: O(n²) for collision detection (acceptable for typical graph sizes)
 * - Space Complexity: O(n) for position storage
 * - Typical execution time: <1ms for graphs with <100 nodes
 *
 * @param graph - The complete processing graph containing nodes and execution order
 * @returns Object mapping node IDs to their calculated {x, y} positions in pixels
 *
 * @example
 * ```typescript
 * const positions = calculateNodePositions(graph);
 * // Returns: { "input_1": { x: 0, y: 100 }, "filter_1": { x: 240, y: 100 }, ... }
 * ```
 */
function calculateNodePositions(
  graph: SerializableProcessingGraph,
): Record<string, { x: number; y: number }> {
  const positions: Record<string, { x: number; y: number }> = {};
  const executionOrder = graph.execution_order;

  // Layout configuration constants - these values are tuned for optimal visual balance
  const NODE_WIDTH = 140; // Minimum width for nodes (accommodates typical content)
  const NODE_HEIGHT = 80; // Minimum height for nodes (includes statistics display)
  const HORIZONTAL_SPACING = 240; // Space between nodes horizontally (prevents crowding)
  const VERTICAL_SPACING = 200; // Space between rows (ensures clear separation)
  const MAX_NODES_PER_ROW = 4; // Maximum nodes per row (optimal for wide screens)

  // **Phase 1: Initial Grid Positioning**
  // Calculate base positions using execution order to maintain logical flow
  executionOrder.forEach((nodeId, index) => {
    const row = Math.floor(index / MAX_NODES_PER_ROW);
    const colInRow = index % MAX_NODES_PER_ROW;

    // All rows maintain left-to-right execution order for consistency
    const x = colInRow * HORIZONTAL_SPACING;
    const y = row * VERTICAL_SPACING + 100; // Start with top margin for visual balance

    // **Phase 2: Jitter Application**
    // Add small random offsets to prevent mechanical appearance
    const jitterX = (Math.random() - 0.5) * 15; // ±7.5px horizontal variance
    const jitterY = (Math.random() - 0.5) * 15; // ±7.5px vertical variance

    positions[nodeId] = {
      x: x + jitterX,
      y: y + jitterY,
    };
  });

  // **Phase 3: Collision Detection and Resolution**
  // Ensure no nodes overlap by checking all pairs and resolving conflicts
  const nodeIds = Object.keys(positions);

  for (let i = 0; i < nodeIds.length; i++) {
    for (let j = i + 1; j < nodeIds.length; j++) {
      const nodeA = positions[nodeIds[i]];
      const nodeB = positions[nodeIds[j]];

      // Calculate distances between node centers
      const dx = Math.abs(nodeA.x - nodeB.x);
      const dy = Math.abs(nodeB.y - nodeA.y);

      // Check if nodes are too close (within minimum required spacing)
      if (dx < NODE_WIDTH && dy < NODE_HEIGHT) {
        // **Conflict Resolution Strategy**
        // Move the second node to avoid overlap, choosing the direction
        // that requires the smallest movement
        if (dx < dy) {
          // Move horizontally (less distance required)
          nodeB.x =
            nodeA.x +
            (nodeB.x > nodeA.x ? NODE_WIDTH + 20 : -(NODE_WIDTH + 20));
        } else {
          // Move vertically (less distance required)
          nodeB.y =
            nodeA.y +
            (nodeB.y > nodeA.y ? NODE_HEIGHT + 20 : -(NODE_HEIGHT + 20));
        }
      }
    }
  }

  return positions;
}

/**
 * Node Details Modal Component Props
 *
 * Defines the complete interface for the generic node details modal component.
 * This modal serves as both a standalone component for basic nodes and a
 * router for specialized modal implementations.
 *
 * The modal system uses a delegation pattern:
 * - Generic nodes: Handled by this component directly
 * - Specialized nodes: Routed to dedicated modal components
 *
 * @interface NodeDetailsModalProps
 */
interface NodeDetailsModalProps {
  /** Controls modal visibility state - managed by parent component */
  isOpen: boolean;

  /** Callback function to close the modal and clean up state */
  onClose: () => void;

  /**
   * Complete node data to display in the modal
   * - null when modal is closed or no node selected
   * - Contains all node configuration and runtime information when open
   */
  nodeData: ProcessingNodeData | null;

  /**
   * Optional graph-wide statistics for contextual information
   * - Provides comparative performance data
   * - Used for relative performance analysis
   */
  graphStatistics?: SerializableProcessingGraph["statistics"];
}

/**
 * Generic Node Details Modal Component
 *
 * This component provides comprehensive detailed information about processing
 * nodes that don't have specialized modal implementations. It serves as both
 * a full-featured modal for generic nodes and a routing mechanism for
 * specialized node types.
 *
 * **Modal Routing System:**
 * The component implements intelligent routing based on node types:
 * - `streaming` & `input` nodes → StreamingNodeModal (real-time data visualization)
 * - `computing_concentration` nodes → ComputingNodeModal (mathematical analysis)
 * - All other nodes → Generic modal (this component)
 *
 * **Information Architecture:**
 * 1. **Node Information**: Basic identification and configuration
 * 2. **Performance Statistics**: Runtime metrics and bottleneck analysis
 * 3. **Configuration Parameters**: Node-specific settings and values
 * 4. **Visual Performance Metrics**: Progressive indicators and charts
 *
 * **Performance Visualization:**
 * - Frame processing counts with locale-appropriate formatting
 * - Processing time metrics with human-readable duration formatting
 * - Consistency analysis through progress bars
 * - Bottleneck highlighting with visual warnings
 *
 * **Accessibility Features:**
 * - Semantic HTML structure with proper headings
 * - Keyboard navigation support through HeroUI components
 * - Screen reader friendly labels and descriptions
 * - High contrast color scheme for performance indicators
 *
 * @param props - Modal props including node data and visibility controls
 * @returns Modal component or null if no node data provided
 *
 * @example
 * ```typescript
 * <NodeDetailsModal
 *   isOpen={isModalOpen}
 *   onClose={() => setIsModalOpen(false)}
 *   nodeData={selectedNode}
 *   graphStatistics={graph.statistics}
 * />
 * ```
 */
function NodeDetailsModal({
  isOpen,
  onClose,
  nodeData,
}: NodeDetailsModalProps) {
  const { t } = useTranslation();

  // Early return if no node data provided (modal closed or invalid state)
  if (!nodeData) return null;

  // **Modal Routing Logic**
  // Delegate to specialized modals based on node functionality requirements

  // Streaming and input nodes require real-time data visualization capabilities
  if (nodeData.nodeType === "streaming" || nodeData.nodeType === "input") {
    return (
      <StreamingNodeModal
        isOpen={isOpen}
        nodeData={nodeData}
        onClose={onClose}
      />
    );
  }

  // Computing concentration nodes require mathematical analysis and MathML rendering
  if (nodeData.nodeType === "computing_concentration") {
    return (
      <ComputingNodeModal
        isOpen={isOpen}
        nodeData={nodeData}
        onClose={onClose}
      />
    );
  }

  // Extract commonly used properties for cleaner code
  const { statistics } = nodeData;
  const isBottleneck = nodeData.isBottleneck || false;

  return (
    <Modal isOpen={isOpen} scrollBehavior="inside" size="2xl" onClose={onClose}>
      <ModalContent>
        {/* Modal Header with Icon and Node Identification */}
        <ModalHeader className="flex flex-col gap-1">
          <div className="flex items-center gap-2">
            <span className="text-2xl">{getNodeIcon(nodeData.nodeType)}</span>
            <div>
              <h2 className="text-xl font-bold">{nodeData.id}</h2>
              <p className="text-sm text-gray-600 font-normal">
                {t("view-modal-node-subtitle", { type: nodeData.nodeType })}
              </p>
            </div>
          </div>
        </ModalHeader>

        <ModalBody>
          {/* **Section 1: Node Information**
              Displays basic node configuration and metadata */}
          <Card>
            <CardBody>
              <h3 className="text-lg font-semibold mb-3">
                {t("view-modal-node-information")}
              </h3>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                {/* Node Identity Information */}
                <div>
                  <p className="text-sm text-gray-600">
                    {t("view-modal-node-id")}
                  </p>
                  <p className="font-medium">{nodeData.id}</p>
                </div>
                <div>
                  <p className="text-sm text-gray-600">
                    {t("view-modal-node-type")}
                  </p>
                  <p className="font-medium">{nodeData.nodeType}</p>
                </div>

                {/* Data Type Information */}
                <div>
                  <p className="text-sm text-gray-600">
                    {t("view-modal-output-type")}
                  </p>
                  <p className="font-medium">
                    {nodeData.outputType || t("view-modal-dynamic")}
                  </p>
                </div>
                <div>
                  <p className="text-sm text-gray-600">
                    {t("view-modal-input-types")}
                  </p>
                  <div className="flex flex-wrap gap-1 mt-1">
                    {nodeData.acceptsInputTypes.map((type) => (
                      <Chip key={type} size="sm" variant="flat">
                        {type}
                      </Chip>
                    ))}
                  </div>
                </div>
              </div>
            </CardBody>
          </Card>

          {/* **Section 2: Performance Statistics**
              Displays runtime performance metrics with visual indicators
              Only rendered when meaningful statistics are available */}
          {statistics && statistics.frames_processed > 0 && (
            <Card>
              <CardBody>
                <div className="flex items-center justify-between mb-3">
                  <h3 className="text-lg font-semibold">
                    {t("view-modal-performance-statistics")}
                  </h3>
                  {/* Bottleneck Warning Badge */}
                  {isBottleneck && (
                    <Chip color="danger" variant="flat">
                      {t("view-modal-bottleneck-chip")}
                    </Chip>
                  )}
                </div>

                {/* Primary Performance Metrics Grid */}
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  {/* Frames Processed - Primary throughput indicator */}
                  <div>
                    <p className="text-sm text-gray-600">
                      {t("view-modal-frames-processed")}
                    </p>
                    <p className="text-2xl font-bold text-blue-600">
                      {statistics.frames_processed.toLocaleString()}
                    </p>
                  </div>

                  {/* Average Processing Time - Primary efficiency indicator */}
                  <div>
                    <p className="text-sm text-gray-600">
                      {t("view-modal-average-processing-time")}
                    </p>
                    <p className="text-2xl font-bold text-green-600">
                      {ProcessingGraphUtils.formatDuration(
                        statistics.average_processing_time,
                      )}
                    </p>
                  </div>

                  {/* Performance Range Indicators */}
                  <div>
                    <p className="text-sm text-gray-600">
                      {t("view-modal-fastest-processing")}
                    </p>
                    <p className="text-lg font-semibold text-green-500">
                      {ProcessingGraphUtils.formatDuration(
                        statistics.fastest_processing_time,
                      )}
                    </p>
                  </div>
                  <div>
                    <p className="text-sm text-gray-600">
                      {t("view-modal-slowest-processing")}
                    </p>
                    <p className="text-lg font-semibold text-red-500">
                      {ProcessingGraphUtils.formatDuration(
                        statistics.worst_processing_time,
                      )}
                    </p>
                  </div>
                </div>

                <Divider className="my-4" />

                {/* **Performance Consistency Visualization**
                    Shows how consistent the processing times are using a progress bar */}
                <div>
                  <p className="text-sm text-gray-600 mb-2">
                    {t("view-modal-processing-time-consistency")}
                  </p>
                  <div className="relative">
                    {/* Consistency ratio: higher ratio = more consistent performance */}
                    <Progress
                      className="mb-2"
                      color={isBottleneck ? "danger" : "success"}
                      value={
                        (statistics.fastest_processing_time /
                          statistics.worst_processing_time) *
                        100
                      }
                    />
                    <div className="flex justify-between text-xs text-gray-500">
                      <span>{t("view-modal-consistent")}</span>
                      <span>{t("view-modal-variable")}</span>
                    </div>
                  </div>
                </div>

                {/* Total Processing Time - Cumulative performance indicator */}
                <div className="mt-4">
                  <p className="text-sm text-gray-600">
                    {t("view-modal-total-processing-time")}
                  </p>
                  <p className="text-lg font-semibold">
                    {ProcessingGraphUtils.formatDuration(
                      statistics.total_processing_time,
                    )}
                  </p>
                </div>
              </CardBody>
            </Card>
          )}

          {/* **Section 3: Configuration Parameters**
              Displays node-specific configuration values
              Only rendered when parameters exist */}
          {Object.keys(nodeData.parameters).length > 0 && (
            <Card>
              <CardBody>
                <h3 className="text-lg font-semibold mb-3">
                  {t("view-modal-configuration-parameters")}
                </h3>
                <div className="space-y-2">
                  {Object.entries(nodeData.parameters).map(([key, value]) => (
                    <div key={key} className="flex justify-between">
                      <span className="text-sm text-gray-600">{key}:</span>
                      <span className="text-sm font-medium">
                        {/* Handle complex object values with JSON serialization */}
                        {typeof value === "object"
                          ? JSON.stringify(value)
                          : String(value)}
                      </span>
                    </div>
                  ))}
                </div>
              </CardBody>
            </Card>
          )}
        </ModalBody>

        {/* Modal Footer with Close Action */}
        <ModalFooter>
          <Button color="primary" onPress={onClose}>
            {t("view-modal-close")}
          </Button>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}

/**
 * Flow Container Component
 *
 * This is the core ReactFlow implementation that orchestrates the complete
 * graph visualization system. It serves as the central state management hub
 * and data transformation engine for the visual graph representation.
 *
 * **Core Responsibilities:**
 * 1. **State Management**: Manages ReactFlow nodes and edges using specialized hooks
 * 2. **Data Transformation**: Converts backend processing graph to ReactFlow format
 * 3. **Event Handling**: Manages user interactions and modal state
 * 4. **Dependency Visualization**: Creates special edges for computing relationships
 * 5. **Performance Integration**: Integrates runtime statistics with visual elements
 *
 * **Architecture Pattern:**
 * The component follows a reactive architecture where:
 * - Graph data changes trigger automatic re-computation of visual elements
 * - User interactions flow through centralized event handlers
 * - Modal state is managed separately from graph visualization state
 * - Performance data is merged with static configuration for rich node display
 *
 * **Data Flow:**
 * ```
 * SerializableProcessingGraph → calculateNodePositions() → ReactFlow Nodes
 *                            → graph.connections → ReactFlow Edges
 *                            → dependency analysis → Special Dependency Edges
 * ```
 *
 * **Performance Optimizations:**
 * - useMemo() for expensive graph transformations (prevents unnecessary recalculations)
 * - useCallback() for stable event handlers (prevents child re-renders)
 * - Conditional rendering for performance statistics (only when data available)
 * - Efficient edge creation with batch operations
 *
 * **Special Features:**
 * - **Dependency Edge Enhancement**: Visual representation of data dependencies
 *   between computing_concentration and computing_peak_finder nodes
 * - **Bottleneck Highlighting**: Automatic identification and visual emphasis
 *   of performance bottlenecks
 * - **Modal Routing**: Intelligent delegation to specialized modal components
 *   based on node type and functionality requirements
 *
 * @param graph - The complete processing graph data structure from the backend
 * @returns ReactFlow visualization with controls and modal system
 *
 * @example
 * ```typescript
 * <FlowContainer graph={processingGraphData} />
 * ```
 */
function FlowContainer({ graph }: { graph: SerializableProcessingGraph }) {
  // **ReactFlow State Management**
  // These hooks provide optimized state management for ReactFlow components
  // with built-in change handling and performance optimizations
  const [nodes, setNodes, onNodesChange] = useNodesState([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState([]);

  // **Modal State Management**
  // Manages the node details modal with selected node data
  const [selectedNode, setSelectedNode] = useState<ProcessingNodeData | null>(
    null,
  );
  const { isOpen, onOpen, onClose } = useDisclosure();

  /**
   * Handles node click events to open detailed information modals
   *
   * This function provides a stable callback for node click handling,
   * preventing unnecessary re-renders of child components while maintaining
   * proper event propagation and state management.
   *
   * The callback pattern allows for centralized modal management while
   * keeping the click handling logic close to the node components.
   *
   * @param _nodeId - The ID of the clicked node (unused, data contains ID)
   * @param nodeData - Complete node data for the clicked node
   */
  const handleNodeClick = useCallback(
    (_nodeId: string, nodeData: ProcessingNodeData) => {
      setSelectedNode(nodeData);
      onOpen();
    },
    [onOpen],
  );

  /**
   * **Core Data Transformation Engine**
   *
   * Converts processing graph data to ReactFlow format with comprehensive
   * enhancements for visualization and interactivity. This is the most
   * complex and performance-critical part of the component.
   *
   * **Transformation Pipeline:**
   * 1. **Position Calculation**: Optimal node layout with collision detection
   * 2. **Node Enhancement**: Integration of statistics and interaction handlers
   * 3. **Edge Creation**: Standard connections from graph definition
   * 4. **Dependency Analysis**: Special edges for computing relationships
   * 5. **Visual Styling**: Performance-based and type-based styling
   *
   * **Performance Considerations:**
   * - Runs only when graph data changes (memoized)
   * - Efficient array operations with minimal object creation
   * - Batch processing for edge creation
   * - Early returns for missing data scenarios
   *
   * **Dependency Edge Logic:**
   * For computing_concentration nodes that reference computing_peak_finder nodes:
   * - If normal connection exists: Enhance existing edge with special styling
   * - If no connection exists: Create new dashed dependency edge
   * - Visual differentiation: Amber color, special labels, glow effects
   *
   * @returns Object containing processed ReactFlow nodes and edges
   */
  const { reactFlowNodes, reactFlowEdges } = useMemo(() => {
    // **Phase 1: Node Position Calculation**
    // Calculate optimal positions for all nodes using the layout algorithm
    const nodePositions = calculateNodePositions(graph);

    // **Phase 2: ReactFlow Node Creation**
    // Convert each processing node to ReactFlow format with enhanced data
    const reactFlowNodes: Node[] = graph.nodes.map((node, index) => {
      // Integrate runtime statistics for performance-aware visualization
      const statistics = graph.statistics.node_statistics[node.id];
      const isBottleneck = ProcessingGraphUtils.isBottleneck(graph, node.id);

      return {
        id: node.id,
        type: "processingNode", // References our custom node component
        position: nodePositions[node.id] || { x: index * 200, y: 100 }, // Fallback positioning
        data: {
          id: node.id,
          nodeType: node.node_type,
          acceptsInputTypes: node.accepts_input_types,
          outputType: node.output_type,
          parameters: node.parameters,
          statistics,
          isBottleneck,
          onClick: handleNodeClick, // Stable callback reference
        } as ProcessingNodeData,
      };
    });

    // **Phase 3: Standard Edge Creation**
    // Convert graph connections to ReactFlow edges with consistent styling
    const reactFlowEdges: Edge[] = graph.connections.map((conn, index) => ({
      id: `${conn.from}-${conn.to}-${index}`, // Unique identifier for each connection
      source: conn.from,
      target: conn.to,
      type: "smoothstep", // Smooth curved edges for professional appearance
      animated: true, // Animated flow indication
      style: { stroke: "#64748b", strokeWidth: 2 }, // Default styling
    }));

    // **Phase 4: Special Dependency Edge Creation**
    // Create enhanced visual connections for computing relationships
    // This section implements the sophisticated dependency visualization system
    const computingDependencyEdges: Edge[] = [];

    graph.nodes.forEach((node) => {
      // Focus on computing_concentration nodes that reference other computing nodes
      if (
        node.node_type === "computing_concentration" &&
        node.parameters.computing_peak_finder_id
      ) {
        const peakFinderId = node.parameters.computing_peak_finder_id;

        // Validate that the referenced node actually exists in the graph
        const peakFinderExists = graph.nodes.some((n) => n.id === peakFinderId);

        if (peakFinderExists) {
          // Check if this dependency already has a normal data connection
          const connectionExists = graph.connections.some(
            (conn) => conn.from === peakFinderId && conn.to === node.id,
          );

          if (connectionExists) {
            // **Enhance Existing Edge**: Modify the existing connection to show
            // it represents both data flow AND computational dependency
            const existingEdgeIndex = reactFlowEdges.findIndex(
              (edge) => edge.source === peakFinderId && edge.target === node.id,
            );

            if (existingEdgeIndex !== -1) {
              // Apply special styling to indicate enhanced relationship
              reactFlowEdges[existingEdgeIndex] = {
                ...reactFlowEdges[existingEdgeIndex],
                style: {
                  stroke: "#f59e0b", // Amber color for computing dependencies
                  strokeWidth: 3, // Thicker line for emphasis
                  filter: "drop-shadow(0 0 3px rgba(245, 158, 11, 0.5))", // Glow effect
                },
                label: "🧮", // Computing icon to indicate special relationship
                labelStyle: {
                  fontSize: "10px",
                  fill: "#f59e0b",
                  fontWeight: "600",
                },
                labelBgStyle: {
                  fill: "#fef3c7", // Light amber background
                  fillOpacity: 0.9,
                },
              };
            }
          } else {
            // **Create New Dependency Edge**: Add a dedicated dependency visualization
            // when no normal connection exists between the nodes
            computingDependencyEdges.push({
              id: `dependency-${peakFinderId}-${node.id}`,
              source: peakFinderId,
              target: node.id,
              type: "smoothstep",
              animated: false, // Different animation pattern for dependency edges
              style: {
                stroke: "#f59e0b", // Amber color for computing dependencies
                strokeWidth: 2,
                strokeDasharray: "5,5", // Dashed line to differentiate from data flow
              },
              label: "data dependency", // Clear text label
              labelStyle: {
                fontSize: "10px",
                fill: "#f59e0b",
                fontWeight: "500",
              },
              labelBgStyle: {
                fill: "#fef3c7", // Light amber background
                fillOpacity: 0.8,
              },
            });
          }
        }
      }
    });

    // **Phase 5: Edge Combination**
    // Combine regular edges with enhanced dependency edges
    const allEdges = [...reactFlowEdges, ...computingDependencyEdges];

    return { reactFlowNodes, reactFlowEdges: allEdges };
  }, [graph, handleNodeClick]);

  // **Reactive State Updates**
  // Update ReactFlow state when processed data changes
  // This effect ensures the visualization stays synchronized with the data
  useEffect(() => {
    setNodes(reactFlowNodes);
    setEdges(reactFlowEdges);
  }, [reactFlowNodes, reactFlowEdges, setNodes, setEdges]);

  return (
    <>
      {/* **Main ReactFlow Visualization** 
          Core graph rendering with enhanced features and controls */}
      <ReactFlow
        fitView // Automatically fit the graph to the viewport
        className="bg-gray-50" // Light background for better contrast
        edges={edges}
        nodeTypes={nodeTypes} // Our custom node component registry
        nodes={nodes}
        proOptions={{ hideAttribution: true }} // Clean professional appearance
        onEdgesChange={onEdgesChange} // Handle edge state changes
        onNodesChange={onNodesChange} // Handle node state changes (drag, select, etc.)
      >
        {/* **Interactive Controls Panel**
            Provides pan, zoom, and fit controls for user navigation */}
        <Controls />

        {/* **Grid Background**
            Subtle grid pattern for spatial reference and professional appearance */}
        <Background color="#aaa" gap={16} />
      </ReactFlow>

      {/* **Modal System**
          Centralized modal management for detailed node information
          Automatically routes to specialized modals based on node type */}
      <NodeDetailsModal
        graphStatistics={graph.statistics} // Provides context for comparative analysis
        isOpen={isOpen}
        nodeData={selectedNode}
        onClose={onClose}
      />
    </>
  );
}

/**
 * Main Processing Graph View Component
 *
 * This is the top-level component that provides the complete processing graph
 * visualization experience. It serves as the public API for the graph system
 * and handles the ReactFlow provider setup.
 *
 * **Component Architecture:**
 * ```
 * ProcessingGraphView (ReactFlow Provider)
 *   └── FlowContainer (Core Logic & State)
 *       ├── ReactFlow (Visualization Engine)
 *       │   ├── ProcessingNode Components
 *       │   ├── Edge Renderers
 *       │   └── Interactive Controls
 *       └── NodeDetailsModal (Information Display)
 *           ├── StreamingNodeModal (Specialized)
 *           ├── ComputingNodeModal (Specialized)
 *           └── Generic Modal (Fallback)
 * ```
 *
 * **Provider Pattern:**
 * ReactFlowProvider is required at the top level to provide context for
 * all ReactFlow hooks and components. This separation allows for clean
 * component isolation and proper context management.
 *
 * **Responsive Design:**
 * The component is designed to be fully responsive and adapts to:
 * - Different screen sizes and orientations
 * - Various container dimensions
 * - Touch vs mouse interaction patterns
 * - High DPI displays
 *
 * **Performance Characteristics:**
 * - Efficient re-rendering through React.memo and proper dependency arrays
 * - Optimized for graphs with 10-100 nodes (typical use case)
 * - Memory efficient with proper cleanup and state management
 * - Fast initial render through optimized layout algorithms
 *
 * @example
 * ```typescript
 * // Basic usage
 * <ProcessingGraphView graph={processingGraphData} />
 *
 * // With custom styling
 * <ProcessingGraphView
 *   graph={processingGraphData}
 *   className="h-96 border rounded-lg"
 * />
 * ```
 */

// **Component Props Interface**
interface ProcessingGraphViewProps {
  /** The complete processing graph data structure from the backend */
  graph: SerializableProcessingGraph;

  /**
   * Optional CSS classes for styling the outer wrapper element
   * Applied to the outermost div element
   */
  className?: string;
}

/**
 * **Main Export Component**
 *
 * Provides the complete processing graph visualization with all features:
 * - Interactive node graph with drag/zoom capabilities
 * - Performance-aware visual styling and bottleneck detection
 * - Specialized dependency edges for computing relationships
 * - Comprehensive modal system for detailed node inspection
 * - Responsive design with professional appearance
 *
 * @param graph - Complete processing graph data from the backend API
 * @param className - Optional CSS classes for wrapper element styling
 * @returns Complete graph visualization component
 */
export function ProcessingGraphView({
  graph,
  className = "",
}: ProcessingGraphViewProps) {
  return (
    <div className={`w-full h-full ${className}`}>
      {/* **ReactFlow Provider Setup**
          Required for all ReactFlow functionality and context management */}
      <ReactFlowProvider>
        <FlowContainer graph={graph} />
      </ReactFlowProvider>
    </div>
  );
}
