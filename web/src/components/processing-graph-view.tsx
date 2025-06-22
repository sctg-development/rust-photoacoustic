// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

import { useMemo, useCallback, useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
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
import "reactflow/dist/style.css";
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

import {
  SerializableProcessingGraph,
  NodeStatistics,
  ProcessingGraphUtils,
} from "../types/processing-graph";

import { StreamingNodeModal } from "./streaming-node-modal";
import { ComputingNodeModal } from "./computing-node-modal";

// Custom node types
const nodeTypes = {
  processingNode: ProcessingNode,
};

interface ProcessingNodeData {
  id: string;
  nodeType: string;
  acceptsInputTypes: string[];
  outputType: string | null;
  parameters: Record<string, any>;
  statistics?: NodeStatistics;
  isBottleneck?: boolean;
  onClick: (nodeId: string, nodeData: any) => void;
}

export const getNodeIcon = (nodeType: string) => {
  switch (nodeType) {
    case "input":
      return "🔆"; // Laser for laser input
    case "filter":
      return "🔬"; // Microscope for analysis/filtering
    case "differential":
      return "⚡"; // Lightning for signal processing
    case "record":
      return "💾"; // Disk for recording
    case "streaming":
      return "📡"; // Antenna for streaming
    case "gain":
      return "🔊"; // Speaker for amplification
    case "channel_mixer":
      return "🎛️"; // Control knobs for mixing
    case "channel_selector":
      return "🎯"; // Target for selection
    case "photoacoustic_output":
      return "📊"; // Chart for measurement output
    case "output":
      return "📤"; // Outbox for final output
    case "computing_concentration":
      return "🧮"; // Calculator for concentration computing
    default:
      return "⚙️"; // Gear for processing
  }
};

// Custom Processing Node Component
function ProcessingNode({ data }: NodeProps<ProcessingNodeData>) {
  const { t } = useTranslation();
  const { statistics, isBottleneck, nodeType, id } = data;

  const getNodeColor = () => {
    if (isBottleneck) return "border-red-500 bg-red-50";
    switch (nodeType) {
      case "input":
        return "border-blue-500 bg-blue-50";
      case "filter":
        return "border-purple-500 bg-purple-50";
      case "differential":
        return "border-green-500 bg-green-50";
      case "record":
        return "border-orange-500 bg-orange-50";
      case "streaming":
        return "border-cyan-500 bg-cyan-50";
      case "gain":
        return "border-emerald-500 bg-emerald-50";
      case "channel_mixer":
        return "border-pink-500 bg-pink-50";
      case "channel_selector":
        return "border-teal-500 bg-teal-50";
      case "photoacoustic_output":
        return "border-indigo-500 bg-indigo-50";
      case "output":
        return "border-gray-500 bg-gray-50";
      case "computing_concentration":
        return "border-amber-500 bg-amber-50";
      default:
        return "border-gray-500 bg-gray-50";
    }
  };

  return (
    <div
      className={`px-4 py-2 shadow-lg rounded-lg border-2 bg-white min-w-[120px] cursor-pointer hover:shadow-xl transition-shadow ${getNodeColor()}`}
      onClick={() => data.onClick(id, data)}
    >
      <Handle className="w-3 h-3" position={Position.Left} type="target" />

      <div className="flex flex-col items-center">
        <div className="text-lg mb-1">{getNodeIcon(nodeType)}</div>
        <div className="text-sm font-semibold text-center">{id}</div>
        <div className="text-xs text-gray-600 text-center">{nodeType}</div>

        {statistics && statistics.frames_processed > 0 && (
          <div className="mt-2 text-xs text-center">
            <div className="text-green-600 font-medium">
              {statistics.frames_processed} {t("view-frames")}
            </div>
            <div className="text-blue-600">
              {ProcessingGraphUtils.formatDuration(
                statistics.average_processing_time,
              )}{" "}
              {t("view-avg")}
            </div>
          </div>
        )}

        {isBottleneck && (
          <Chip className="mt-1" color="danger" size="sm" variant="flat">
            {t("view-bottleneck")}
          </Chip>
        )}
      </div>

      <Handle className="w-3 h-3" position={Position.Right} type="source" />
    </div>
  );
}

// Calculate node positions for layout
function calculateNodePositions(
  graph: SerializableProcessingGraph,
): Record<string, { x: number; y: number }> {
  const positions: Record<string, { x: number; y: number }> = {};
  const executionOrder = graph.execution_order;

  // Configuration for layout
  const NODE_WIDTH = 140; // Minimum width for nodes
  const NODE_HEIGHT = 80; // Minimum height for nodes
  const HORIZONTAL_SPACING = 240; // Space between nodes horizontally
  const VERTICAL_SPACING = 200; // Space between rows
  const MAX_NODES_PER_ROW = 4; // Maximum nodes per row before wrapping

  // Calculate positions using multi-row layout with consistent left-to-right order
  executionOrder.forEach((nodeId, index) => {
    const row = Math.floor(index / MAX_NODES_PER_ROW);
    const colInRow = index % MAX_NODES_PER_ROW;

    // All rows maintain left-to-right execution order
    const x = colInRow * HORIZONTAL_SPACING;
    const y = row * VERTICAL_SPACING + 100; // Start with some top margin

    // Add small jitter to prevent perfect overlaps while maintaining general flow
    const jitterX = (Math.random() - 0.5) * 15;
    const jitterY = (Math.random() - 0.5) * 15;

    positions[nodeId] = {
      x: x + jitterX,
      y: y + jitterY,
    };
  });

  // Post-process to ensure no overlaps
  const nodeIds = Object.keys(positions);

  for (let i = 0; i < nodeIds.length; i++) {
    for (let j = i + 1; j < nodeIds.length; j++) {
      const nodeA = positions[nodeIds[i]];
      const nodeB = positions[nodeIds[j]];

      const dx = Math.abs(nodeA.x - nodeB.x);
      const dy = Math.abs(nodeB.y - nodeA.y);

      // Check if nodes are too close
      if (dx < NODE_WIDTH && dy < NODE_HEIGHT) {
        // Move the second node to avoid overlap
        if (dx < dy) {
          // Move horizontally
          nodeB.x =
            nodeA.x +
            (nodeB.x > nodeA.x ? NODE_WIDTH + 20 : -(NODE_WIDTH + 20));
        } else {
          // Move vertically
          nodeB.y =
            nodeA.y +
            (nodeB.y > nodeA.y ? NODE_HEIGHT + 20 : -(NODE_HEIGHT + 20));
        }
      }
    }
  }

  return positions;
}

// Node Details Modal Component
interface NodeDetailsModalProps {
  isOpen: boolean;
  onClose: () => void;
  nodeData: ProcessingNodeData | null;
  graphStatistics?: SerializableProcessingGraph["statistics"];
}

function NodeDetailsModal({
  isOpen,
  onClose,
  nodeData,
}: NodeDetailsModalProps) {
  const { t } = useTranslation();

  if (!nodeData) return null;

  // For streaming and input nodes, use the special streaming modal
  if (nodeData.nodeType === "streaming" || nodeData.nodeType === "input") {
    return (
      <StreamingNodeModal
        isOpen={isOpen}
        nodeData={nodeData}
        onClose={onClose}
      />
    );
  }

  // For computing nodes, use the computing node modal
  if (nodeData.nodeType === "computing_concentration") {
    return (
      <ComputingNodeModal
        isOpen={isOpen}
        nodeData={nodeData}
        onClose={onClose}
      />
    );
  }

  const { statistics } = nodeData;
  const isBottleneck = nodeData.isBottleneck || false;

  return (
    <Modal isOpen={isOpen} scrollBehavior="inside" size="2xl" onClose={onClose}>
      <ModalContent>
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
          {/* Node Information */}
          <Card>
            <CardBody>
              <h3 className="text-lg font-semibold mb-3">
                {t("view-modal-node-information")}
              </h3>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
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

          {/* Performance Statistics */}
          {statistics && statistics.frames_processed > 0 && (
            <Card>
              <CardBody>
                <div className="flex items-center justify-between mb-3">
                  <h3 className="text-lg font-semibold">
                    {t("view-modal-performance-statistics")}
                  </h3>
                  {isBottleneck && (
                    <Chip color="danger" variant="flat">
                      {t("view-modal-bottleneck-chip")}
                    </Chip>
                  )}
                </div>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <p className="text-sm text-gray-600">
                      {t("view-modal-frames-processed")}
                    </p>
                    <p className="text-2xl font-bold text-blue-600">
                      {statistics.frames_processed.toLocaleString()}
                    </p>
                  </div>
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

                {/* Performance Visualization */}
                <div>
                  <p className="text-sm text-gray-600 mb-2">
                    {t("view-modal-processing-time-consistency")}
                  </p>
                  <div className="relative">
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

                {/* Total Processing Time */}
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

          {/* Node Parameters */}
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

        <ModalFooter>
          <Button color="primary" onPress={onClose}>
            {t("view-modal-close")}
          </Button>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}

// Flow Container Component
function FlowContainer({ graph }: { graph: SerializableProcessingGraph }) {
  const [nodes, setNodes, onNodesChange] = useNodesState([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState([]);
  const [selectedNode, setSelectedNode] = useState<ProcessingNodeData | null>(
    null,
  );
  const { isOpen, onOpen, onClose } = useDisclosure();

  const handleNodeClick = useCallback(
    (_nodeId: string, nodeData: ProcessingNodeData) => {
      setSelectedNode(nodeData);
      onOpen();
    },
    [onOpen],
  );

  // Convert graph data to React Flow format
  const { reactFlowNodes, reactFlowEdges } = useMemo(() => {
    const nodePositions = calculateNodePositions(graph);

    const reactFlowNodes: Node[] = graph.nodes.map((node, index) => {
      const statistics = graph.statistics.node_statistics[node.id];
      const isBottleneck = ProcessingGraphUtils.isBottleneck(graph, node.id);

      return {
        id: node.id,
        type: "processingNode",
        position: nodePositions[node.id] || { x: index * 200, y: 100 },
        data: {
          id: node.id,
          nodeType: node.node_type,
          acceptsInputTypes: node.accepts_input_types,
          outputType: node.output_type,
          parameters: node.parameters,
          statistics,
          isBottleneck,
          onClick: handleNodeClick,
        } as ProcessingNodeData,
      };
    });

    const reactFlowEdges: Edge[] = graph.connections.map((conn, index) => ({
      id: `${conn.from}-${conn.to}-${index}`,
      source: conn.from,
      target: conn.to,
      type: "smoothstep",
      animated: true,
      style: { stroke: "#64748b", strokeWidth: 2 },
    }));

    return { reactFlowNodes, reactFlowEdges };
  }, [graph, handleNodeClick]);

  // Update nodes and edges when graph changes
  useEffect(() => {
    setNodes(reactFlowNodes);
    setEdges(reactFlowEdges);
  }, [reactFlowNodes, reactFlowEdges, setNodes, setEdges]);

  return (
    <>
      <ReactFlow
        fitView
        className="bg-gray-50"
        edges={edges}
        nodeTypes={nodeTypes}
        nodes={nodes}
        proOptions={{ hideAttribution: true }}
        onEdgesChange={onEdgesChange}
        onNodesChange={onNodesChange}
      >
        <Controls />
        <Background color="#aaa" gap={16} />
      </ReactFlow>

      <NodeDetailsModal
        graphStatistics={graph.statistics}
        isOpen={isOpen}
        nodeData={selectedNode}
        onClose={onClose}
      />
    </>
  );
}

// Main Processing Graph View Component
interface ProcessingGraphViewProps {
  graph: SerializableProcessingGraph;
  className?: string;
}

export function ProcessingGraphView({
  graph,
  className = "",
}: ProcessingGraphViewProps) {
  return (
    <div className={`w-full h-full ${className}`}>
      <ReactFlowProvider>
        <FlowContainer graph={graph} />
      </ReactFlowProvider>
    </div>
  );
}
