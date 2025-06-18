// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! HTML Report Generator with SVG Charts
//!
//! This module generates comprehensive HTML reports for PID tuning results,
//! including interactive SVG charts showing step response, performance metrics,
//! and tuning recommendations.

use crate::{PerformanceMetrics, StepResponseData, TuningResult};
use anyhow::{anyhow, Result};
use handlebars::Handlebars;
use serde_json::json;
use std::path::PathBuf;

/// HTML report generator for PID tuning results
pub struct HtmlReportGenerator<'a> {
    handlebars: Handlebars<'a>,
}

impl<'a> HtmlReportGenerator<'a> {
    /// Create a new HTML report generator
    pub fn new() -> Result<Self> {
        let mut handlebars = Handlebars::new();

        // Register the HTML template
        let template_content = include_str!("report_template.hbs");
        handlebars
            .register_template_string("report", template_content)
            .map_err(|e| anyhow!("Failed to register template: {}", e))?;

        Ok(Self { handlebars })
    }

    /// Generate complete HTML report with SVG charts
    pub fn generate_report(&self, result: &TuningResult, output_path: &PathBuf) -> Result<()> {
        let html_content = self.build_html_report(result)?;

        std::fs::write(output_path, html_content)
            .map_err(|e| anyhow!("Failed to write HTML report: {}", e))?;

        Ok(())
    }

    /// Build the complete HTML report using Handlebars
    fn build_html_report(&self, result: &TuningResult) -> Result<String> {
        let step_response_svg = self.generate_step_response_chart(&result.step_response)?;
        let recommendations = self.generate_recommendations(result);
        let generation_time = chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string();

        let algorithm_description = match result.algorithm.as_str() {
            "Ziegler-Nichols" => {
                "Classical Ziegler-Nichols step response method for general-purpose PID tuning"
            }
            "Cohen-Coon" => "Cohen-Coon method optimized for processes with significant dead time",
            _ => "Advanced PID tuning algorithm",
        };

        // Create template data
        let template_data = json!({
            "REGULATOR_ID": "Development Cell Temperature",
            "TIMESTAMP": generation_time,
            "GENERATION_TIME": generation_time,
            "ALGORITHM_NAME": result.algorithm,
            "ALGORITHM_DESCRIPTION": algorithm_description,
            "STEP_SIZE": format!("{:.1}", result.step_response.setpoint.last().unwrap_or(&0.0) - result.step_response.setpoint.first().unwrap_or(&0.0)),
            "TEST_DURATION": format!("{:.0}", result.step_response.time.last().unwrap_or(&0.0)),
            "INITIAL_TEMP": format!("{:.1}", result.step_response.temperature.first().unwrap_or(&0.0)),
            "TARGET_TEMP": format!("{:.1}", result.step_response.setpoint.last().unwrap_or(&0.0)),
            "DRIVER_TYPE": "Mock Thermal Driver",
            "SAMPLING_RATE": "1.0",
            "TEST_STATUS": "SUCCESS",
            "STATUS_CLASS": "status-success",
            "SVG_CHART": step_response_svg,
            "KP": format!("{:.6}", result.kp),
            "KI": format!("{:.6}", result.ki),
            "KD": format!("{:.6}", result.kd),
            "RISE_TIME": format!("{:.1}", result.performance_metrics.rise_time),
            "SETTLING_TIME": format!("{:.1}", result.performance_metrics.settling_time),
            "OVERSHOOT": format!("{:.1}", result.performance_metrics.overshoot),
            "STEADY_STATE_ERROR": format!("{:.2}", result.performance_metrics.steady_state_error),
            "PROCESS_GAIN": format!("{:.3}", result.performance_metrics.process_gain),
            "TIME_CONSTANT": format!("{:.1}", result.performance_metrics.time_constant),
            "DEAD_TIME": format!("{:.1}", result.performance_metrics.dead_time),
            "STABILITY_MARGIN": self.assess_stability_margin(&result.performance_metrics),
            "RESPONSE_QUALITY": self.assess_response_quality(&result.performance_metrics),
            "ROBUSTNESS": self.assess_robustness(&result.performance_metrics),
            "RECOMMENDATIONS_CONTENT": recommendations
        });

        // Render the template
        self.handlebars
            .render("report", &template_data)
            .map_err(|e| anyhow!("Failed to render template: {}", e))
    }

    /// Generate SVG chart for step response
    fn generate_step_response_chart(&self, data: &StepResponseData) -> Result<String> {
        if data.time.is_empty() {
            return Err(anyhow!("No data to plot"));
        }

        let width = 800.0;
        let height = 400.0;
        let margin = 60.0;
        let plot_width = width - 2.0 * margin;
        let plot_height = height - 2.0 * margin;

        // Find data ranges
        let time_min = data.time.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let time_max = data.time.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let temp_min = data
            .temperature
            .iter()
            .fold(f64::INFINITY, |a, &b| a.min(b));
        let temp_max = data
            .temperature
            .iter()
            .fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        // Add some padding to temperature range
        let temp_range = temp_max - temp_min;
        let temp_min_plot = temp_min - temp_range * 0.1;
        let temp_max_plot = temp_max + temp_range * 0.1;

        // Generate temperature line path
        let temp_path = self.generate_line_path(
            &data.time,
            &data.temperature,
            time_min,
            time_max,
            temp_min_plot,
            temp_max_plot,
            margin,
            plot_width,
            plot_height,
        );

        // Generate setpoint line path
        let setpoint_path = self.generate_line_path(
            &data.time,
            &data.setpoint,
            time_min,
            time_max,
            temp_min_plot,
            temp_max_plot,
            margin,
            plot_width,
            plot_height,
        );

        // Generate control output path (secondary y-axis)
        let control_max = data
            .control_output
            .iter()
            .fold(f64::NEG_INFINITY, |a, &b| a.max(b.abs()));
        let control_path = self.generate_line_path(
            &data.time,
            &data.control_output,
            time_min,
            time_max,
            -control_max * 1.1,
            control_max * 1.1,
            margin,
            plot_width,
            plot_height,
        );

        // Generate grid lines and axes
        let grid_lines = self.generate_grid_lines(
            time_min,
            time_max,
            temp_min_plot,
            temp_max_plot,
            margin,
            plot_width,
            plot_height,
        );

        let axes = self.generate_axes(
            time_min,
            time_max,
            temp_min_plot,
            temp_max_plot,
            margin,
            plot_width,
            plot_height,
        );

        let svg = format!(
            "<svg width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" xmlns=\"http://www.w3.org/2000/svg\">\
                <defs>\
                    <style>\
                        .chart-title {{ font: bold 16px Arial, sans-serif; text-anchor: middle; }}\
                        .axis-label {{ font: 12px Arial, sans-serif; text-anchor: middle; }}\
                        .axis-tick {{ font: 10px Arial, sans-serif; text-anchor: middle; }}\
                        .legend {{ font: 12px Arial, sans-serif; }}\
                        .grid-line {{ stroke: #e0e0e0; stroke-width: 0.5; }}\
                        .temp-line {{ stroke: #ff6b6b; stroke-width: 2; fill: none; }}\
                        .setpoint-line {{ stroke: #4ecdc4; stroke-width: 2; fill: none; stroke-dasharray: 5,5; }}\
                        .control-line {{ stroke: #45b7d1; stroke-width: 1.5; fill: none; opacity: 0.7; }}\
                        .axis-line {{ stroke: #333; stroke-width: 1; }}\
                    </style>\
                </defs>\
                <rect width=\"100%\" height=\"100%\" fill=\"white\"/>\
                <text x=\"{title_x}\" y=\"25\" class=\"chart-title\">Step Response Analysis</text>\
                {grid_lines}\
                {axes}\
                <path d=\"{temp_path}\" class=\"temp-line\"/>\
                <path d=\"{setpoint_path}\" class=\"setpoint-line\"/>\
                <path d=\"{control_path}\" class=\"control-line\"/>\
                <g transform=\"translate({legend_x}, 40)\">\
                    <rect x=\"0\" y=\"0\" width=\"150\" height=\"70\" fill=\"white\" stroke=\"#ccc\" stroke-width=\"1\"/>\
                    <line x1=\"10\" y1=\"15\" x2=\"30\" y2=\"15\" class=\"temp-line\"/>\
                    <text x=\"35\" y=\"19\" class=\"legend\">Temperature</text>\
                    <line x1=\"10\" y1=\"30\" x2=\"30\" y2=\"30\" class=\"setpoint-line\"/>\
                    <text x=\"35\" y=\"34\" class=\"legend\">Setpoint</text>\
                    <line x1=\"10\" y1=\"45\" x2=\"30\" y2=\"45\" class=\"control-line\"/>\
                    <text x=\"35\" y=\"49\" class=\"legend\">Control Output</text>\
                </g>\
            </svg>",
            width = width,
            height = height,
            title_x = width / 2.0,
            grid_lines = grid_lines,
            axes = axes,
            temp_path = temp_path,
            setpoint_path = setpoint_path,
            control_path = control_path,
            legend_x = width - 160.0
        );

        Ok(svg)
    }

    /// Generate SVG path for a data line
    fn generate_line_path(
        &self,
        x_data: &[f64],
        y_data: &[f64],
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        margin: f64,
        plot_width: f64,
        plot_height: f64,
    ) -> String {
        if x_data.is_empty() || y_data.is_empty() {
            return String::new();
        }

        let mut path = String::new();

        for (i, (&x, &y)) in x_data.iter().zip(y_data.iter()).enumerate() {
            let px = margin + (x - x_min) / (x_max - x_min) * plot_width;
            let py = margin + plot_height - (y - y_min) / (y_max - y_min) * plot_height;

            if i == 0 {
                path.push_str(&format!("M {:.1} {:.1}", px, py));
            } else {
                path.push_str(&format!(" L {:.1} {:.1}", px, py));
            }
        }

        path
    }

    /// Generate grid lines for the chart
    fn generate_grid_lines(
        &self,
        time_min: f64,
        time_max: f64,
        temp_min: f64,
        temp_max: f64,
        margin: f64,
        plot_width: f64,
        plot_height: f64,
    ) -> String {
        let mut grid = String::new();

        // Vertical grid lines (time)
        let time_step = (time_max - time_min) / 10.0;
        for i in 0..=10 {
            let time = time_min + i as f64 * time_step;
            let x = margin + (time - time_min) / (time_max - time_min) * plot_width;
            grid.push_str(&format!(
                "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" class=\"grid-line\"/>",
                x,
                margin,
                x,
                margin + plot_height
            ));
        }

        // Horizontal grid lines (temperature)
        let temp_step = (temp_max - temp_min) / 8.0;
        for i in 0..=8 {
            let temp = temp_min + i as f64 * temp_step;
            let y = margin + plot_height - (temp - temp_min) / (temp_max - temp_min) * plot_height;
            grid.push_str(&format!(
                "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" class=\"grid-line\"/>",
                margin,
                y,
                margin + plot_width,
                y
            ));
        }

        grid
    }

    /// Generate axes for the chart
    fn generate_axes(
        &self,
        time_min: f64,
        time_max: f64,
        temp_min: f64,
        temp_max: f64,
        margin: f64,
        plot_width: f64,
        plot_height: f64,
    ) -> String {
        let mut axes = String::new();

        // Left axis (temperature)
        axes.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" class=\"axis-line\"/>",
            margin,
            margin,
            margin,
            margin + plot_height
        ));

        // Bottom axis (time)
        axes.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" class=\"axis-line\"/>",
            margin,
            margin + plot_height,
            margin + plot_width,
            margin + plot_height
        ));

        // Temperature axis labels
        let temp_step = (temp_max - temp_min) / 8.0;
        for i in 0..=8 {
            let temp = temp_min + i as f64 * temp_step;
            let y = margin + plot_height - (temp - temp_min) / (temp_max - temp_min) * plot_height;
            axes.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" class=\"axis-tick\">{:.1}</text>",
                margin - 10.0,
                y + 3.0,
                temp
            ));
        }

        // Time axis labels
        let time_step = (time_max - time_min) / 10.0;
        for i in 0..=10 {
            let time = time_min + i as f64 * time_step;
            let x = margin + (time - time_min) / (time_max - time_min) * plot_width;
            axes.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" class=\"axis-tick\">{:.0}</text>",
                x,
                margin + plot_height + 15.0,
                time
            ));
        }

        // Axis labels
        axes.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" class=\"axis-label\">Time (seconds)</text>",
            margin + plot_width / 2.0,
            margin + plot_height + 35.0
        ));

        axes.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" class=\"axis-label\" transform=\"rotate(-90, {:.1}, {:.1})\">Temperature (°C)</text>",
            margin - 40.0, margin + plot_height / 2.0, margin - 40.0, margin + plot_height / 2.0
        ));

        axes
    }

    /// Generate recommendations based on performance metrics
    fn generate_recommendations(&self, result: &TuningResult) -> String {
        let mut recommendations = Vec::new();

        let metrics = &result.performance_metrics;

        if metrics.overshoot > 20.0 {
            recommendations.push("⚠️ High overshoot detected. Consider reducing Kp or increasing Kd for better stability.".to_string());
        }

        if metrics.settling_time > 180.0 {
            recommendations.push("⚠️ Slow settling time. Consider increasing Ki for faster response to disturbances.".to_string());
        }

        if metrics.steady_state_error > 5.0 {
            recommendations.push(
                "⚠️ High steady-state error. Consider increasing Ki to reduce offset.".to_string(),
            );
        }

        if metrics.overshoot < 5.0 && metrics.settling_time < 120.0 {
            recommendations
                .push("✅ Good balance between stability and response time.".to_string());
        }

        if metrics.dead_time / metrics.time_constant > 0.5 {
            recommendations.push(
                "⚠️ High dead time ratio. Cohen-Coon method recommended for better performance."
                    .to_string(),
            );
        }

        if recommendations.is_empty() {
            recommendations
                .push("✅ System appears well-tuned with the current parameters.".to_string());
        }

        recommendations.join("<br/>")
    }

    /// Assess stability margin
    fn assess_stability_margin(&self, metrics: &PerformanceMetrics) -> String {
        if metrics.overshoot > 25.0 {
            "Poor - High overshoot risk".to_string()
        } else if metrics.overshoot > 15.0 {
            "Fair - Moderate stability".to_string()
        } else if metrics.overshoot > 5.0 {
            "Good - Stable response".to_string()
        } else {
            "Excellent - Very stable".to_string()
        }
    }

    /// Assess response quality
    fn assess_response_quality(&self, metrics: &PerformanceMetrics) -> String {
        if metrics.rise_time > 120.0 {
            "Slow - Long rise time".to_string()
        } else if metrics.rise_time > 60.0 {
            "Moderate - Acceptable response".to_string()
        } else if metrics.rise_time > 30.0 {
            "Fast - Good response".to_string()
        } else {
            "Very Fast - Excellent response".to_string()
        }
    }

    /// Assess robustness
    fn assess_robustness(&self, metrics: &PerformanceMetrics) -> String {
        let dead_time_ratio = metrics.dead_time / metrics.time_constant;
        if dead_time_ratio > 0.5 {
            "Challenging - High dead time".to_string()
        } else if dead_time_ratio > 0.3 {
            "Moderate - Some dead time".to_string()
        } else if dead_time_ratio > 0.1 {
            "Good - Low dead time".to_string()
        } else {
            "Excellent - Minimal dead time".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Create a sample tuning result for testing
    fn create_sample_tuning_result() -> TuningResult {
        TuningResult {
            kp: 2.5,
            ki: 0.25,
            kd: 6.0,
            algorithm: "Ziegler-Nichols".to_string(),
            step_response: StepResponseData {
                time: vec![
                    0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0,
                ],
                temperature: vec![
                    25.0, 25.1, 25.5, 26.2, 27.1, 28.0, 28.8, 29.4, 29.7, 29.9, 30.0,
                ],
                setpoint: vec![
                    25.0, 25.0, 25.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0,
                ],
                control_output: vec![
                    0.0, 0.0, 0.0, 50.0, 45.0, 40.0, 35.0, 30.0, 25.0, 20.0, 15.0,
                ],
            },
            performance_metrics: PerformanceMetrics {
                rise_time: 45.0,
                settling_time: 85.0,
                overshoot: 2.5,
                steady_state_error: 0.5,
                process_gain: 0.8,
                time_constant: 60.0,
                dead_time: 5.0,
            },
        }
    }

    #[test]
    fn test_html_report_generator_creation() {
        let generator = HtmlReportGenerator::new();
        assert!(generator.is_ok(), "Failed to create HTML report generator");
    }

    #[test]
    fn test_step_response_chart_generation() {
        let generator = HtmlReportGenerator::new().unwrap();
        let result = create_sample_tuning_result();

        let svg_chart = generator.generate_step_response_chart(&result.step_response);
        assert!(svg_chart.is_ok(), "Failed to generate SVG chart");

        let svg_content = svg_chart.unwrap();

        // Check that the SVG contains expected elements
        assert!(svg_content.contains("<svg"), "SVG should contain svg tag");
        assert!(
            svg_content.contains("temp-line"),
            "SVG should contain temperature line"
        );
        assert!(
            svg_content.contains("setpoint-line"),
            "SVG should contain setpoint line"
        );
        assert!(
            svg_content.contains("control-line"),
            "SVG should contain control output line"
        );
        assert!(
            svg_content.contains("Step Response Analysis"),
            "SVG should contain title"
        );
        assert!(
            svg_content.contains("Temperature"),
            "SVG should contain temperature label"
        );
        assert!(
            svg_content.contains("Time (seconds)"),
            "SVG should contain time axis label"
        );
    }

    #[test]
    fn test_empty_data_handling() {
        let generator = HtmlReportGenerator::new().unwrap();
        let empty_data = StepResponseData {
            time: vec![],
            temperature: vec![],
            setpoint: vec![],
            control_output: vec![],
        };

        let result = generator.generate_step_response_chart(&empty_data);
        assert!(result.is_err(), "Should fail with empty data");
    }

    #[test]
    fn test_line_path_generation() {
        let generator = HtmlReportGenerator::new().unwrap();
        let x_data = vec![0.0, 1.0, 2.0, 3.0];
        let y_data = vec![0.0, 1.0, 4.0, 9.0];

        let path =
            generator.generate_line_path(&x_data, &y_data, 0.0, 3.0, 0.0, 9.0, 60.0, 680.0, 280.0);

        assert!(path.starts_with("M"), "Path should start with Move command");
        assert!(path.contains("L"), "Path should contain Line commands");

        // Check that we have 4 points (one M and three L commands)
        let l_count = path.matches("L").count();
        assert_eq!(l_count, 3, "Should have 3 L commands for 4 points");
    }

    #[test]
    fn test_grid_lines_generation() {
        let generator = HtmlReportGenerator::new().unwrap();
        let grid_lines = generator.generate_grid_lines(0.0, 100.0, 20.0, 40.0, 60.0, 680.0, 280.0);

        assert!(grid_lines.contains("<line"), "Should contain line elements");
        assert!(
            grid_lines.contains("grid-line"),
            "Should contain grid-line class"
        );

        // Should have vertical and horizontal grid lines
        let line_count = grid_lines.matches("<line").count();
        assert!(line_count > 10, "Should have multiple grid lines");
    }

    #[test]
    fn test_axes_generation() {
        let generator = HtmlReportGenerator::new().unwrap();
        let axes = generator.generate_axes(0.0, 100.0, 20.0, 40.0, 60.0, 680.0, 280.0);

        assert!(axes.contains("axis-line"), "Should contain axis lines");
        assert!(axes.contains("axis-tick"), "Should contain axis ticks");
        assert!(axes.contains("axis-label"), "Should contain axis labels");
        assert!(
            axes.contains("Time (seconds)"),
            "Should contain time axis label"
        );
        assert!(
            axes.contains("Temperature (°C)"),
            "Should contain temperature axis label"
        );
    }

    #[test]
    fn test_recommendations_generation() {
        let generator = HtmlReportGenerator::new().unwrap();

        // Test with high overshoot
        let mut result = create_sample_tuning_result();
        result.performance_metrics.overshoot = 25.0;
        let recommendations = generator.generate_recommendations(&result);
        assert!(
            recommendations.contains("High overshoot"),
            "Should detect high overshoot"
        );

        // Test with slow settling
        result.performance_metrics.overshoot = 5.0;
        result.performance_metrics.settling_time = 200.0;
        let recommendations = generator.generate_recommendations(&result);
        assert!(
            recommendations.contains("Slow settling"),
            "Should detect slow settling"
        );

        // Test with high steady-state error
        result.performance_metrics.settling_time = 100.0;
        result.performance_metrics.steady_state_error = 10.0;
        let recommendations = generator.generate_recommendations(&result);
        assert!(
            recommendations.contains("steady-state error"),
            "Should detect high steady-state error"
        );

        // Test with good performance
        result.performance_metrics.overshoot = 3.0;
        result.performance_metrics.settling_time = 80.0;
        result.performance_metrics.steady_state_error = 1.0;
        let recommendations = generator.generate_recommendations(&result);
        assert!(
            recommendations.contains("Good balance"),
            "Should recognize good performance"
        );
    }

    #[test]
    fn test_stability_margin_assessment() {
        let generator = HtmlReportGenerator::new().unwrap();

        // Test excellent stability (low overshoot)
        let metrics = PerformanceMetrics {
            overshoot: 2.0,
            ..create_sample_tuning_result().performance_metrics
        };
        let assessment = generator.assess_stability_margin(&metrics);
        assert!(
            assessment.contains("Excellent"),
            "Should assess as excellent for low overshoot"
        );

        // Test poor stability (high overshoot)
        let metrics = PerformanceMetrics {
            overshoot: 30.0,
            ..create_sample_tuning_result().performance_metrics
        };
        let assessment = generator.assess_stability_margin(&metrics);
        assert!(
            assessment.contains("Poor"),
            "Should assess as poor for high overshoot"
        );
    }

    #[test]
    fn test_response_quality_assessment() {
        let generator = HtmlReportGenerator::new().unwrap();

        // Test very fast response
        let metrics = PerformanceMetrics {
            rise_time: 20.0,
            ..create_sample_tuning_result().performance_metrics
        };
        let assessment = generator.assess_response_quality(&metrics);
        assert!(
            assessment.contains("Very Fast"),
            "Should assess as very fast for short rise time"
        );

        // Test slow response
        let metrics = PerformanceMetrics {
            rise_time: 150.0,
            ..create_sample_tuning_result().performance_metrics
        };
        let assessment = generator.assess_response_quality(&metrics);
        assert!(
            assessment.contains("Slow"),
            "Should assess as slow for long rise time"
        );
    }

    #[test]
    fn test_robustness_assessment() {
        let generator = HtmlReportGenerator::new().unwrap();

        // Test excellent robustness (low dead time ratio)
        let metrics = PerformanceMetrics {
            dead_time: 2.0,
            time_constant: 100.0,
            ..create_sample_tuning_result().performance_metrics
        };
        let assessment = generator.assess_robustness(&metrics);
        assert!(
            assessment.contains("Excellent"),
            "Should assess as excellent for low dead time ratio"
        );

        // Test challenging robustness (high dead time ratio)
        let metrics = PerformanceMetrics {
            dead_time: 60.0,
            time_constant: 100.0,
            ..create_sample_tuning_result().performance_metrics
        };
        let assessment = generator.assess_robustness(&metrics);
        assert!(
            assessment.contains("Challenging"),
            "Should assess as challenging for high dead time ratio"
        );
    }

    #[test]
    fn test_html_report_building() {
        let generator = HtmlReportGenerator::new().unwrap();
        let result = create_sample_tuning_result();

        let html_content = generator.build_html_report(&result);
        assert!(
            html_content.is_ok(),
            "Should successfully build HTML report"
        );

        let html = html_content.unwrap();

        // Check that HTML contains expected sections
        assert!(html.contains("<!DOCTYPE html>"), "Should be valid HTML");
        assert!(html.contains("PID Tuning Report"), "Should contain title");
        assert!(
            html.contains("Ziegler-Nichols"),
            "Should contain algorithm name"
        );
        assert!(html.contains("2.500000"), "Should contain Kp value");
        assert!(html.contains("0.250000"), "Should contain Ki value");
        assert!(html.contains("6.000000"), "Should contain Kd value");
        assert!(html.contains("45.0"), "Should contain rise time");
        assert!(html.contains("85.0"), "Should contain settling time");
        assert!(html.contains("2.5"), "Should contain overshoot");
        assert!(html.contains("0.50"), "Should contain steady-state error");
        assert!(html.contains("0.800"), "Should contain process gain");
        assert!(html.contains("60.0"), "Should contain time constant");
        assert!(html.contains("5.0"), "Should contain dead time");
    }

    #[test]
    fn test_cohen_coon_algorithm_description() {
        let generator = HtmlReportGenerator::new().unwrap();
        let mut result = create_sample_tuning_result();
        result.algorithm = "Cohen-Coon".to_string();

        let html_content = generator.build_html_report(&result).unwrap();
        assert!(
            html_content.contains("Cohen-Coon method optimized"),
            "Should contain Cohen-Coon description"
        );
    }

    #[test]
    fn test_report_file_generation() {
        let generator = HtmlReportGenerator::new().unwrap();
        let result = create_sample_tuning_result();

        // Create a temporary file path
        let temp_path = PathBuf::from("test_report.html");

        // Generate the report
        let generate_result = generator.generate_report(&result, &temp_path);
        assert!(
            generate_result.is_ok(),
            "Should successfully generate report file"
        );

        // Check that file was created
        assert!(temp_path.exists(), "Report file should exist");

        // Read and verify content
        let content = std::fs::read_to_string(&temp_path).unwrap();
        assert!(
            content.contains("<!DOCTYPE html>"),
            "File should contain valid HTML"
        );
        assert!(
            content.contains("PID Tuning Report"),
            "File should contain report title"
        );

        // Clean up
        std::fs::remove_file(&temp_path).unwrap_or(());
    }

    #[test]
    fn test_algorithm_descriptions() {
        let generator = HtmlReportGenerator::new().unwrap();

        // Test Ziegler-Nichols description
        let mut result = create_sample_tuning_result();
        result.algorithm = "Ziegler-Nichols".to_string();
        let html = generator.build_html_report(&result).unwrap();
        assert!(
            html.contains("Classical Ziegler-Nichols"),
            "Should contain ZN description"
        );

        // Test Cohen-Coon description
        result.algorithm = "Cohen-Coon".to_string();
        let html = generator.build_html_report(&result).unwrap();
        assert!(
            html.contains("Cohen-Coon method optimized"),
            "Should contain CC description"
        );

        // Test unknown algorithm
        result.algorithm = "Unknown".to_string();
        let html = generator.build_html_report(&result).unwrap();
        assert!(
            html.contains("Advanced PID tuning"),
            "Should contain fallback description"
        );
    }

    #[test]
    fn test_svg_viewbox_and_dimensions() {
        let generator = HtmlReportGenerator::new().unwrap();
        let result = create_sample_tuning_result();

        let svg = generator
            .generate_step_response_chart(&result.step_response)
            .unwrap();

        // Check SVG dimensions
        assert!(svg.contains("width=\"800\""), "Should have correct width");
        assert!(svg.contains("height=\"400\""), "Should have correct height");
        assert!(
            svg.contains("viewBox=\"0 0 800 400\""),
            "Should have correct viewBox"
        );
    }

    #[test]
    fn test_edge_cases() {
        let generator = HtmlReportGenerator::new().unwrap();

        // Test with single data point
        let single_point_data = StepResponseData {
            time: vec![0.0],
            temperature: vec![25.0],
            setpoint: vec![25.0],
            control_output: vec![0.0],
        };

        let result = generator.generate_step_response_chart(&single_point_data);
        assert!(result.is_ok(), "Should handle single data point");

        // Test with identical values
        let identical_data = StepResponseData {
            time: vec![0.0, 1.0, 2.0],
            temperature: vec![25.0, 25.0, 25.0],
            setpoint: vec![25.0, 25.0, 25.0],
            control_output: vec![0.0, 0.0, 0.0],
        };

        let result = generator.generate_step_response_chart(&identical_data);
        assert!(result.is_ok(), "Should handle identical values");
    }

    #[test]
    fn test_template_data_completeness() {
        let generator = HtmlReportGenerator::new().unwrap();
        let result = create_sample_tuning_result();

        let html = generator.build_html_report(&result).unwrap();

        // Ensure no template placeholders are left unfilled
        assert!(
            !html.contains("{{"),
            "Should not contain unfilled template placeholders"
        );
        assert!(
            !html.contains("}}"),
            "Should not contain unfilled template placeholders"
        );
    }
}
