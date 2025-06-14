// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation();
  const { statistics, performance_summary } = graph;

  // Get sorted nodes by performance
  const nodesByPerformance = ProcessingGraphUtils.getNodesByPerformance(graph);

  return (
    <div className={`space-y-6 ${className}`}>
      {/* Overall Performance Summary */}
      <Card>
        <CardHeader>
          <h3 className="text-lg font-semibold">
            {t("stats-performance-summary")}
          </h3>
        </CardHeader>
        <CardBody>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div className="text-center">
              <p className="text-2xl font-bold text-blue-600">
                {performance_summary.total_executions.toLocaleString()}
              </p>
              <p className="text-sm text-gray-600">
                {t("stats-total-executions")}
              </p>
            </div>
            <div className="text-center">
              <p className="text-2xl font-bold text-green-600">
                {performance_summary.throughput_fps.toFixed(1)}
              </p>
              <p className="text-sm text-gray-600">{t("fps")}</p>
            </div>
            <div className="text-center">
              <p className="text-2xl font-bold text-purple-600">
                {performance_summary.average_execution_time_ms.toFixed(2)}ms
              </p>
              <p className="text-sm text-gray-600">{t("avg-time")}</p>
            </div>
            <div className="text-center">
              <p className="text-2xl font-bold text-orange-600">
                {performance_summary.efficiency_percentage.toFixed(1)}%
              </p>
              <p className="text-sm text-gray-600">{t("stats-efficiency")}</p>
            </div>
          </div>

          {/* Efficiency Progress */}
          <div className="mt-4">
            <div className="flex justify-between items-center mb-2">
              <span className="text-sm font-medium">
                {t("stats-processing-efficiency")}
              </span>
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
                {t("stats-best")}:{" "}
                {ProcessingGraphUtils.formatDuration(
                  statistics.fastest_graph_execution,
                )}
              </span>
              <span>
                {t("stats-worst")}:{" "}
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
                ⚠️ {t("stats-bottleneck-detected")}
              </Chip>
            </div>
            <p className="text-sm">
              {t("stats-bottleneck-message", {
                node: performance_summary.slowest_node,
              })}
            </p>
          </CardBody>
        </Card>
      )}

      {/* Node Performance Table */}
      <Card>
        <CardHeader>
          <h3 className="text-lg font-semibold">
            {t("stats-node-performance-details")}
          </h3>
        </CardHeader>
        <CardBody>
          <Table aria-label={t("stats-node-performance-table-aria")}>
            <TableHeader>
              <TableColumn>{t("stats-table-node")}</TableColumn>
              <TableColumn>{t("stats-table-type")}</TableColumn>
              <TableColumn>{t("stats-table-frames")}</TableColumn>
              <TableColumn>{t("stats-table-avg-time")}</TableColumn>
              <TableColumn>{t("stats-table-total-time")}</TableColumn>
              <TableColumn>{t("stats-table-status")}</TableColumn>
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
                          {t("stats-chip-bottleneck")}
                        </Chip>
                      )}
                      {isFastest && !isBottleneck && (
                        <Chip color="success" size="sm" variant="flat">
                          {t("stats-chip-fastest")}
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
          <h3 className="text-lg font-semibold">
            {t("stats-graph-structure")}
          </h3>
        </CardHeader>
        <CardBody>
          <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
            <div>
              <p className="text-sm text-gray-600">{t("stats-total-nodes")}</p>
              <p className="text-xl font-bold">
                {performance_summary.total_nodes}
              </p>
            </div>
            <div>
              <p className="text-sm text-gray-600">
                {t("stats-total-connections")}
              </p>
              <p className="text-xl font-bold">
                {performance_summary.total_connections}
              </p>
            </div>
            <div>
              <p className="text-sm text-gray-600">{t("stats-input-node")}</p>
              <p className="text-xl font-bold">
                {graph.input_node || t("none")}
              </p>
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
                {graph.is_valid ? `✓ ${t("valid")}` : `✗ ${t("invalid")}`}
              </Chip>
              <span className="text-sm text-gray-600">
                {t("stats-graph-structure")}
              </span>
            </div>

            {graph.validation_errors.length > 0 && (
              <div className="mt-2">
                <p className="text-sm font-medium text-red-600 mb-1">
                  {t("stats-validation-errors")}:
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
