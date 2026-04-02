/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 *
 * LocalPage: local-only monitoring dashboard without DefaultLayout.
 * Accessible only from local hostnames (127.0.0.1, ::1, localhost).
 * Shows a switchable GaugeChart and a line chart (100 concentration points or last-hour temperature).
 */

import { useEffect, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Line } from "react-chartjs-2";
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

import {
  VisualizationOutputItem,
  MeasurementData,
} from "@/types/visualization";
import { ThermalDataPoint } from "@/types/thermal";

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

type Mode = "concentration" | "temperature";

const LOCAL_HOSTNAMES = ["localhost", "127.0.0.1", "::1"];

export default function LocalPage() {
  const { t } = useTranslation();

  const [isLocal, setIsLocal] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<Mode>("concentration");

  const [concentrationPoints, setConcentrationPoints] = useState<
    { x: Date; y: number }[]
  >([]);
  const [temperaturePoints, setTemperaturePoints] = useState<
    { x: Date; y: number }[]
  >([]);

  const [lastConcentration, setLastConcentration] = useState<number>(0);
  const [lastTemperature, setLastTemperature] = useState<number>(0);

  const apiBase = window.location.origin;

  const fetchConcentration = useCallback(async () => {
    try {
      const visualizationConfigUrl = `${apiBase}/api/config/visualization/output`;
      const outputs = (await fetch(visualizationConfigUrl).then((res) => {
        if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
        return res.json();
      })) as VisualizationOutputItem[];

      if (!outputs || outputs.length === 0) {
        setError(t("local-no-visualization-output"));
        return;
      }

      const firstOutput = outputs[0];
      const historyUrl = `${apiBase}/api/action/${encodeURIComponent(
        firstOutput.action_node_id,
      )}/history?limit=100`;
      const history = (await fetch(historyUrl).then((res) => {
        if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
        return res.json();
      })) as MeasurementData[];

      const points = history.map((item) => ({
        x: new Date(
          item.timestamp.secs_since_epoch * 1000 +
            Math.floor(item.timestamp.nanos_since_epoch / 1000000),
        ),
        y: item.concentration_ppm,
      }));

      setConcentrationPoints(points.slice(-100));
      if (points.length > 0) {
        setLastConcentration(points[points.length - 1].y);
      }
    } catch (fetchError) {
      setError(
        (fetchError as Error).message || t("local-error-fetching-concentration"),
      );
    }
  }, [apiBase, t]);

  const fetchTemperature = useCallback(async () => {
    try {
      const regUrl = `${apiBase}/api/thermal/regulators`;
      const regulators = (await fetch(regUrl).then((res) => {
        if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
        return res.json();
      })) as string[];

      if (!regulators || regulators.length === 0) {
        setError(t("local-no-thermal-regulator"));
        return;
      }

      const firstReg = regulators[0];
      const now = Math.floor(Date.now() / 1000);
      const from = now - 3600; // last hour
      const thermalUrl = `${apiBase}/api/thermal?regulators=${encodeURIComponent(
        firstReg,
      )}&from=${from}&to=${now}&steps=60`;

      const thermalData = (await fetch(thermalUrl).then((res) => {
        if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
        return res.json();
      })) as { data: Record<string, ThermalDataPoint[]> };

      const points = (thermalData.data[firstReg] || []).map((item) => ({
        x: new Date(item.timestamp * 1000),
        y: item.temperature_celsius,
      }));

      setTemperaturePoints(points);
      if (points.length > 0) {
        setLastTemperature(points[points.length - 1].y);
      }
    } catch (fetchError) {
      setError((fetchError as Error).message || t("local-error-fetching-temperature"));
    }
  }, [apiBase, t]);

  useEffect(() => {
    const hostname = window.location.hostname;
    setIsLocal(LOCAL_HOSTNAMES.includes(hostname));
  }, []);

  useEffect(() => {
    if (!isLocal) return;

    setError(null);
    fetchConcentration();
    fetchTemperature();

    const refreshId = window.setInterval(() => {
      fetchConcentration();
      fetchTemperature();
    }, 15000);

    return () => {
      window.clearInterval(refreshId);
    };
  }, [isLocal, fetchConcentration, fetchTemperature]);

  if (!isLocal) {
    return (
      <div
        style={{
          width: "100vw",
          height: "100vh",
          display: "flex",
          justifyContent: "center",
          alignItems: "center",
          backgroundColor: "#0a0e26",
          color: "#fff",
        }}
      >
        <div>
          <h1>{t("local-only-access")}</h1>
          <p>
            {t("local-only-hostname", {
              host: window.location.hostname,
            })}
          </p>
        </div>
      </div>
    );
  }

  const gaugeValue = mode === "concentration" ? lastConcentration : lastTemperature;
  const gaugeLabel = mode === "concentration" ? t("concentration-ppm") : t("temperature-celsius");

  const chartData = {
    datasets: [
      {
        label:
          mode === "concentration"
            ? t("concentration-history")
            : t("temperature-history"),
        data: mode === "concentration" ? concentrationPoints : temperaturePoints,
        borderColor: mode === "concentration" ? "rgb(59, 130, 246)" : "rgb(245, 101, 101)",
        backgroundColor:
          mode === "concentration" ? "rgba(59, 130, 246, 0.3)" : "rgba(245, 101, 101, 0.3)",
        tension: 0.2,
        fill: true,
      },
    ],
  };

  const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    scales: {
      x: {
        type: "time" as const,
        time: {
          tooltipFormat: "HH:mm:ss",
          unit: mode === "concentration" ? "minute" as const : "minute" as const,
        },
        title: {
          display: true,
          text: t("time"),
        },
      },
      y: {
        title: {
          display: true,
          text: gaugeLabel,
        },
      },
    },
    plugins: {
      legend: {
        display: true,
        position: "top" as const,
      },
      title: {
        display: true,
        text:
          mode === "concentration"
            ? t("last-100-concentration-points")
            : t("last-hour-temperature"),
      },
    },
  };

  return (
    <div
      style={{
        width: "100vw",
        height: "100vh",
        overflow: "hidden",
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
        backgroundColor: "#071720",
      }}
    >
      <div
        style={{
          width: "800px",
          height: "480px",
          background: "#0f172a",
          borderRadius: "8px",
          border: "2px solid #0ea5e9",
          boxSizing: "border-box",
          padding: "12px",
          display: "flex",
          flexDirection: "column",
          gap: "8px",
          color: "#e2e8f0",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between" }}>
          <h1 style={{ margin: 0, fontSize: "1rem" }}>
            {t("local-mode-dashboard")}
          </h1>
          <button
            onClick={() =>
              setMode((prev) =>
                prev === "concentration" ? "temperature" : "concentration",
              )
            }
            style={{
              cursor: "pointer",
              padding: "6px 10px",
              border: "1px solid #38bdf8",
              background: "transparent",
              borderRadius: "4px",
              color: "#38bdf8",
            }}
          >
            {t("toggle-to", {
              mode:
                mode === "concentration"
                  ? t("temperature")
                  : t("concentration"),
            })}
          </button>
        </div>

        <div
          style={{
            display: "flex",
            justifyContent: "center",
            alignItems: "center",
            flex: "0 0 200px",
          }}
        >
          <div onClick={() => setMode((prev) => (prev === "concentration" ? "temperature" : "concentration"))}>
            <GaugeChart
              id="local-gauge"
              nrOfLevels={20}
              percent={Math.min(Math.max(gaugeValue / (mode === "concentration" ? 100 : 200), 0), 1)}
              textColor="#ffffff"
              needleColor="#38bdf8"
              needleBaseColor="#38bdf8"
              formatTextValue={() => `${gaugeValue.toFixed(1)} ${
                mode === "concentration" ? "ppm" : "°C"
              }`}
              animate={false}
            />
          </div>
        </div>

        <div style={{ flex: 1, overflow: "hidden" }}>
          {error && (
            <p style={{ color: "#fca5a5" }}>{t("error")}: {error}</p>
          )}
          <Line data={chartData} options={chartOptions} />
        </div>
      </div>
    </div>
  );
}
