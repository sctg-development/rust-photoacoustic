import { Card, CardBody, CardHeader } from "@heroui/card";
import { Chip } from "@heroui/chip";
import { Progress } from "@heroui/progress";
import {
  Table,
  TableHeader,
  TableColumn,
  TableBody,
  TableRow,
  TableCell,
} from "@heroui/table";

import {
  SerializableProcessingGraph,
  ProcessingGraphUtils,
} from "../types/processing-graph";

interface ProcessingGraphStatsProps {
  graph: SerializableProcessingGraph;
  className?: string;
}

export function ProcessingGraphStats({
  graph,
  className = "",
}: ProcessingGraphStatsProps) {
  const { statistics, performance_summary } = graph;

  // Get sorted nodes by performance
  const nodesByPerformance = ProcessingGraphUtils.getNodesByPerformance(graph);

  return (
    <div className={`space-y-6 ${className}`}>
      {/* Overall Performance Summary */}
      <Card>
        <CardHeader>
          <h3 className="text-lg font-semibold">Graph Performance Summary</h3>
        </CardHeader>
        <CardBody>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div className="text-center">
              <p className="text-2xl font-bold text-blue-600">
                {performance_summary.total_executions.toLocaleString()}
              </p>
              <p className="text-sm text-gray-600">Total Executions</p>
            </div>
            <div className="text-center">
              <p className="text-2xl font-bold text-green-600">
                {performance_summary.throughput_fps.toFixed(1)}
              </p>
              <p className="text-sm text-gray-600">FPS</p>
            </div>
            <div className="text-center">
              <p className="text-2xl font-bold text-purple-600">
                {performance_summary.average_execution_time_ms.toFixed(2)}ms
              </p>
              <p className="text-sm text-gray-600">Avg Time</p>
            </div>
            <div className="text-center">
              <p className="text-2xl font-bold text-orange-600">
                {performance_summary.efficiency_percentage.toFixed(1)}%
              </p>
              <p className="text-sm text-gray-600">Efficiency</p>
            </div>
          </div>

          {/* Efficiency Progress */}
          <div className="mt-4">
            <div className="flex justify-between items-center mb-2">
              <span className="text-sm font-medium">Processing Efficiency</span>
              <span className="text-sm text-gray-600">
                {performance_summary.efficiency_percentage.toFixed(1)}%
              </span>
            </div>
            <Progress
              className="mb-2"
              color={
                performance_summary.efficiency_percentage > 80
                  ? "success"
                  : performance_summary.efficiency_percentage > 60
                    ? "warning"
                    : "danger"
              }
              value={performance_summary.efficiency_percentage}
            />
            <div className="flex justify-between text-xs text-gray-500">
              <span>
                Best:{" "}
                {ProcessingGraphUtils.formatDuration(
                  statistics.fastest_graph_execution,
                )}
              </span>
              <span>
                Worst:{" "}
                {ProcessingGraphUtils.formatDuration(
                  statistics.worst_graph_execution,
                )}
              </span>
            </div>
          </div>
        </CardBody>
      </Card>

      {/* Bottlenecks Alert */}
      {performance_summary.slowest_node && (
        <Card className="border-red-200 bg-red-50">
          <CardBody>
            <div className="flex items-center gap-2 mb-2">
              <Chip color="danger" variant="flat">
                ⚠️ Bottleneck Detected
              </Chip>
            </div>
            <p className="text-sm">
              Node <strong>{performance_summary.slowest_node}</strong> is the
              slowest in the pipeline. Consider optimizing this node to improve
              overall performance.
            </p>
          </CardBody>
        </Card>
      )}

      {/* Node Performance Table */}
      <Card>
        <CardHeader>
          <h3 className="text-lg font-semibold">Node Performance Details</h3>
        </CardHeader>
        <CardBody>
          <Table aria-label="Node performance table">
            <TableHeader>
              <TableColumn>Node</TableColumn>
              <TableColumn>Type</TableColumn>
              <TableColumn>Frames</TableColumn>
              <TableColumn>Avg Time</TableColumn>
              <TableColumn>Total Time</TableColumn>
              <TableColumn>Status</TableColumn>
            </TableHeader>
            <TableBody>
              {nodesByPerformance.map((nodeStats) => {
                const isBottleneck = ProcessingGraphUtils.isBottleneck(
                  graph,
                  nodeStats.node_id,
                );
                const isFastest =
                  performance_summary.fastest_node === nodeStats.node_id;

                return (
                  <TableRow key={nodeStats.node_id}>
                    <TableCell>
                      <div className="font-medium">{nodeStats.node_id}</div>
                    </TableCell>
                    <TableCell>
                      <Chip size="sm" variant="flat">
                        {nodeStats.node_type}
                      </Chip>
                    </TableCell>
                    <TableCell>
                      {nodeStats.frames_processed.toLocaleString()}
                    </TableCell>
                    <TableCell>
                      <span
                        className={
                          isBottleneck
                            ? "text-red-600 font-semibold"
                            : isFastest
                              ? "text-green-600 font-semibold"
                              : ""
                        }
                      >
                        {ProcessingGraphUtils.formatDuration(
                          nodeStats.average_processing_time,
                        )}
                      </span>
                    </TableCell>
                    <TableCell>
                      {ProcessingGraphUtils.formatDuration(
                        nodeStats.total_processing_time,
                      )}
                    </TableCell>
                    <TableCell>
                      {isBottleneck && (
                        <Chip color="danger" size="sm" variant="flat">
                          Bottleneck
                        </Chip>
                      )}
                      {isFastest && !isBottleneck && (
                        <Chip color="success" size="sm" variant="flat">
                          Fastest
                        </Chip>
                      )}
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </CardBody>
      </Card>

      {/* Graph Structure Info */}
      <Card>
        <CardHeader>
          <h3 className="text-lg font-semibold">Graph Structure</h3>
        </CardHeader>
        <CardBody>
          <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
            <div>
              <p className="text-sm text-gray-600">Total Nodes</p>
              <p className="text-xl font-bold">
                {performance_summary.total_nodes}
              </p>
            </div>
            <div>
              <p className="text-sm text-gray-600">Total Connections</p>
              <p className="text-xl font-bold">
                {performance_summary.total_connections}
              </p>
            </div>
            <div>
              <p className="text-sm text-gray-600">Input Node</p>
              <p className="text-xl font-bold">{graph.input_node || "None"}</p>
            </div>
          </div>

          {/* Validation Status */}
          <div className="mt-4">
            <div className="flex items-center gap-2">
              <Chip
                color={graph.is_valid ? "success" : "danger"}
                size="sm"
                variant="flat"
              >
                {graph.is_valid ? "✓ Valid" : "✗ Invalid"}
              </Chip>
              <span className="text-sm text-gray-600">Graph Structure</span>
            </div>

            {graph.validation_errors.length > 0 && (
              <div className="mt-2">
                <p className="text-sm font-medium text-red-600 mb-1">
                  Validation Errors:
                </p>
                <ul className="text-sm text-red-600 space-y-1">
                  {graph.validation_errors.map((error, index) => (
                    <li key={index} className="flex items-start gap-1">
                      <span>•</span>
                      <span>{error}</span>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </CardBody>
      </Card>
    </div>
  );
}
