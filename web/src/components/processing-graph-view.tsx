import React, { useMemo, useCallback, useState, useEffect } from 'react';
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
  useReactFlow,
} from 'reactflow';
import 'reactflow/dist/style.css';
import { 
  Modal, 
  ModalContent, 
  ModalHeader, 
  ModalBody, 
  ModalFooter,
  useDisclosure
} from '@heroui/modal';
import { Button } from '@heroui/button';
import { Card, CardBody } from '@heroui/card';
import { Chip } from '@heroui/chip';
import { Progress } from '@heroui/progress';
import { Divider } from '@heroui/divider';
import { 
  SerializableProcessingGraph, 
  NodeStatistics,
  ProcessingGraphUtils 
} from '../types/processing-graph';

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

// Custom Processing Node Component
function ProcessingNode({ data }: NodeProps<ProcessingNodeData>) {
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
      default:
        return "border-gray-500 bg-gray-50";
    }
  };

  const getNodeIcon = () => {
    switch (nodeType) {
      case "input":
        return "📥";
      case "filter":
        return "🔧";
      case "differential":
        return "📊";
      case "record":
        return "💾";
      case "streaming":
        return "📡";
      default:
        return "⚙️";
    }
  };

  return (
    <div
      className={`px-4 py-2 shadow-lg rounded-lg border-2 bg-white min-w-[120px] cursor-pointer hover:shadow-xl transition-shadow ${getNodeColor()}`}
      onClick={() => data.onClick(id, data)}
    >
      <Handle className="w-3 h-3" position={Position.Left} type="target" />

      <div className="flex flex-col items-center">
        <div className="text-lg mb-1">{getNodeIcon()}</div>
        <div className="text-sm font-semibold text-center">{id}</div>
        <div className="text-xs text-gray-600 text-center">{nodeType}</div>

        {statistics && statistics.frames_processed > 0 && (
          <div className="mt-2 text-xs text-center">
            <div className="text-green-600 font-medium">
              {statistics.frames_processed} frames
            </div>
            <div className="text-blue-600">
              {ProcessingGraphUtils.formatDuration(
                statistics.average_processing_time,
              )}{" "}
              avg
            </div>
          </div>
        )}

        {isBottleneck && (
          <Chip className="mt-1" color="danger" size="sm" variant="flat">
            Bottleneck
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

  // Simple horizontal layout based on execution order
  executionOrder.forEach((nodeId, index) => {
    positions[nodeId] = {
      x: index * 200,
      y: 100,
    };
  });

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
  graphStatistics,
}: NodeDetailsModalProps) {
  if (!nodeData) return null;

  const { statistics } = nodeData;
  const isBottleneck = nodeData.isBottleneck || false;

  return (
    <Modal isOpen={isOpen} scrollBehavior="inside" size="2xl" onClose={onClose}>
      <ModalContent>
        <ModalHeader className="flex flex-col gap-1">
          <div className="flex items-center gap-2">
            <span className="text-2xl">
              {nodeData.nodeType === "input"
                ? "📥"
                : nodeData.nodeType === "filter"
                  ? "🔧"
                  : "⚙️"}
            </span>
            <div>
              <h2 className="text-xl font-bold">{nodeData.id}</h2>
              <p className="text-sm text-gray-600 font-normal">
                {nodeData.nodeType} node
              </p>
            </div>
          </div>
        </ModalHeader>

        <ModalBody>
          {/* Node Information */}
          <Card>
            <CardBody>
              <h3 className="text-lg font-semibold mb-3">Node Information</h3>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <p className="text-sm text-gray-600">Node ID</p>
                  <p className="font-medium">{nodeData.id}</p>
                </div>
                <div>
                  <p className="text-sm text-gray-600">Node Type</p>
                  <p className="font-medium">{nodeData.nodeType}</p>
                </div>
                <div>
                  <p className="text-sm text-gray-600">Output Type</p>
                  <p className="font-medium">
                    {nodeData.outputType || "Dynamic"}
                  </p>
                </div>
                <div>
                  <p className="text-sm text-gray-600">Input Types</p>
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
                    Performance Statistics
                  </h3>
                  {isBottleneck && (
                    <Chip color="danger" variant="flat">
                      Bottleneck ⚠️
                    </Chip>
                  )}
                </div>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <p className="text-sm text-gray-600">Frames Processed</p>
                    <p className="text-2xl font-bold text-blue-600">
                      {statistics.frames_processed.toLocaleString()}
                    </p>
                  </div>
                  <div>
                    <p className="text-sm text-gray-600">
                      Average Processing Time
                    </p>
                    <p className="text-2xl font-bold text-green-600">
                      {ProcessingGraphUtils.formatDuration(
                        statistics.average_processing_time,
                      )}
                    </p>
                  </div>
                  <div>
                    <p className="text-sm text-gray-600">Fastest Processing</p>
                    <p className="text-lg font-semibold text-green-500">
                      {ProcessingGraphUtils.formatDuration(
                        statistics.fastest_processing_time,
                      )}
                    </p>
                  </div>
                  <div>
                    <p className="text-sm text-gray-600">Slowest Processing</p>
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
                    Processing Time Consistency
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
                      <span>Consistent</span>
                      <span>Variable</span>
                    </div>
                  </div>
                </div>

                {/* Total Processing Time */}
                <div className="mt-4">
                  <p className="text-sm text-gray-600">Total Processing Time</p>
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
                  Configuration Parameters
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
            Close
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
    (nodeId: string, nodeData: ProcessingNodeData) => {
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
