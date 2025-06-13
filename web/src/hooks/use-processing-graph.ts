import { useState, useEffect, useCallback } from "react";

import { SerializableProcessingGraph } from "../types/processing-graph";

interface UseProcessingGraphOptions {
  autoRefresh?: boolean;
  refreshInterval?: number;
  onError?: (error: Error) => void;
}

interface UseProcessingGraphResult {
  graph: SerializableProcessingGraph | null;
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
}

export function useProcessingGraph(
  options: UseProcessingGraphOptions = {},
): UseProcessingGraphResult {
  const {
    autoRefresh = true,
    refreshInterval = 5000, // 5 seconds
    onError,
  } = options;

  const [graph, setGraph] = useState<SerializableProcessingGraph | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const fetchGraph = useCallback(async () => {
    try {
      const token = localStorage.getItem("token"); // Adjust based on your auth implementation
      const response = await fetch("/api/graph", {
        headers: {
          Authorization: `Bearer ${token}`,
          "Content-Type": "application/json",
        },
      });

      if (!response.ok) {
        throw new Error(
          `Failed to fetch graph: ${response.status} ${response.statusText}`,
        );
      }

      const graphData: SerializableProcessingGraph = await response.json();

      setGraph(graphData);
      setError(null);
    } catch (err) {
      const error =
        err instanceof Error ? err : new Error("Unknown error occurred");

      setError(error);
      onError?.(error);
    } finally {
      setLoading(false);
    }
  }, [onError]);

  const refetch = useCallback(async () => {
    setLoading(true);
    await fetchGraph();
  }, [fetchGraph]);

  // Initial fetch
  useEffect(() => {
    fetchGraph();
  }, [fetchGraph]);

  // Auto-refresh
  useEffect(() => {
    if (!autoRefresh) return;

    const interval = setInterval(fetchGraph, refreshInterval);

    return () => clearInterval(interval);
  }, [autoRefresh, refreshInterval, fetchGraph]);

  return {
    graph,
    loading,
    error,
    refetch,
  };
}
