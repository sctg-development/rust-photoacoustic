"""
Advanced test script for PythonActionDriver with external dependencies.

This script demonstrates more complex Python functionality including:
- Mathematical calculations
- Data aggregation
- Conditional logic
- Error handling

Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
This file is part of the rust-photoacoustic project and is licensed under the
SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
"""

import json
import time
import sys
from collections import deque
from statistics import mean, median

# Advanced state management
class ActionState:
    def __init__(self):
        self.initialized = False
        self.start_time = None
        self.measurements = deque(maxlen=100)  # Keep last 100 measurements
        self.alerts = deque(maxlen=50)         # Keep last 50 alerts
        self.thresholds = {
            "low": 500.0,
            "medium": 1000.0,
            "high": 1500.0,
            "critical": 2000.0
        }
        self.statistics = {
            "total_measurements": 0,
            "total_alerts": 0,
            "avg_concentration": 0.0,
            "min_concentration": None,
            "max_concentration": None
        }

# Global state instance
state = ActionState()

def initialize():
    """Initialize the advanced action script."""
    try:
        print("🚀 Advanced Python action script initializing...")
        
        state.initialized = True
        state.start_time = time.time()
        
        # Test if we can use mathematical operations
        import math
        test_calculation = math.sqrt(16) + math.pi
        
        print(f"✅ Math test passed: √16 + π = {test_calculation:.3f}")
        print(f"🐍 Python version: {sys.version}")
        print(f"📊 Buffer sizes: measurements={state.measurements.maxlen}, alerts={state.alerts.maxlen}")
        
        return {
            "status": "ready",
            "python_version": sys.version,
            "math_test": test_calculation,
            "timestamp": time.time(),
            "features": ["statistics", "thresholds", "buffering"]
        }
    except Exception as e:
        print(f"❌ Initialization error: {e}")
        return {"status": "error", "error": str(e)}

def on_measurement(data):
    """Process measurement data with advanced analytics."""
    try:
        state.statistics["total_measurements"] += 1
        concentration = data["concentration_ppm"]
        
        # Add to buffer
        measurement_record = {
            "concentration": concentration,
            "timestamp": data.get("timestamp", time.time()),
            "source": data.get("source_node_id", "unknown"),
            "amplitude": data.get("peak_amplitude", 0.0),
            "frequency": data.get("peak_frequency", 0.0)
        }
        state.measurements.append(measurement_record)
        
        # Update statistics
        concentrations = [m["concentration"] for m in state.measurements]
        state.statistics["avg_concentration"] = mean(concentrations)
        state.statistics["min_concentration"] = min(concentrations)
        state.statistics["max_concentration"] = max(concentrations)
        
        # Determine concentration level
        level = get_concentration_level(concentration)
        
        # Calculate trends (if we have enough data)
        trend = "stable"
        if len(state.measurements) >= 5:
            recent_avg = mean([m["concentration"] for m in list(state.measurements)[-5:]])
            older_avg = mean([m["concentration"] for m in list(state.measurements)[-10:-5]]) if len(state.measurements) >= 10 else recent_avg
            
            if recent_avg > older_avg * 1.1:
                trend = "rising"
            elif recent_avg < older_avg * 0.9:
                trend = "falling"
        
        print(f"📊 Measurement #{state.statistics['total_measurements']}: {concentration:.2f} ppm [{level}] - Trend: {trend}")
        print(f"📈 Stats: avg={state.statistics['avg_concentration']:.1f}, min={state.statistics['min_concentration']:.1f}, max={state.statistics['max_concentration']:.1f}")
        
        result = {
            "processed": True,
            "measurement_number": state.statistics["total_measurements"],
            "concentration_level": level,
            "trend": trend,
            "statistics": {
                "current": concentration,
                "average": round(state.statistics["avg_concentration"], 2),
                "minimum": state.statistics["min_concentration"],
                "maximum": state.statistics["max_concentration"],
                "median": round(median(concentrations), 2) if concentrations else 0,
                "count": len(concentrations)
            },
            "timestamp": time.time()
        }
        
        # Add warnings for threshold violations
        if level in ["high", "critical"]:
            result["warning"] = f"Concentration level is {level} (>{state.thresholds[level]} ppm)"
        
        return result
        
    except Exception as e:
        print(f"❌ Error processing measurement: {e}")
        return {"processed": False, "error": str(e)}

def on_alert(alert):
    """Handle alerts with advanced logic."""
    try:
        state.statistics["total_alerts"] += 1
        
        # Add to buffer
        alert_record = {
            "type": alert.get("alert_type", "unknown"),
            "severity": alert.get("severity", "info"),
            "message": alert.get("message", ""),
            "timestamp": alert.get("timestamp", time.time()),
            "data": alert.get("data", {})
        }
        state.alerts.append(alert_record)
        
        severity = alert_record["severity"]
        
        # Severity-based responses
        icons = {
            "info": "🔵",
            "warning": "🟡", 
            "critical": "🔴"
        }
        
        icon = icons.get(severity, "⚪")
        print(f"{icon} ALERT #{state.statistics['total_alerts']}: {severity.upper()} - {alert_record['message']}")
        
        # Calculate alert frequency
        recent_alerts = [a for a in state.alerts if time.time() - a["timestamp"] < 300]  # Last 5 minutes
        alert_frequency = len(recent_alerts)
        
        if alert_frequency > 5:
            print("⚠️  High alert frequency detected - possible system issue!")
        
        # Advanced response logic
        response = {
            "alert_handled": True,
            "alert_number": state.statistics["total_alerts"],
            "severity": severity,
            "response_time_ms": 50,  # Simulated response time
            "alert_frequency_5min": alert_frequency,
            "timestamp": time.time()
        }
        
        # Add specific actions based on alert type
        alert_type = alert_record["type"]
        if alert_type == "concentration_high":
            response["actions"] = ["increase_ventilation", "notify_operators"]
        elif alert_type == "sensor_fault":
            response["actions"] = ["switch_backup_sensor", "schedule_maintenance"]
        elif alert_type == "calibration_needed":
            response["actions"] = ["initiate_cal_sequence", "log_drift_data"]
        
        return response
        
    except Exception as e:
        print(f"❌ Error handling alert: {e}")
        return {"alert_handled": False, "error": str(e)}

def clear_action():
    """Clear action with data preservation."""
    try:
        print("🧹 Clearing action - preserving historical data")
        
        # Preserve statistics but reset active counters
        preserved_stats = {
            "total_measurements_before_clear": state.statistics["total_measurements"],
            "total_alerts_before_clear": state.statistics["total_alerts"],
            "avg_concentration_before_clear": state.statistics["avg_concentration"],
            "clear_timestamp": time.time()
        }
        
        # Keep data in buffers but reset counters
        # (In a real system, you might want to archive this data)
        
        print(f"📊 Preserved stats: {preserved_stats}")
        
        return {
            "cleared": True,
            "preserved_stats": preserved_stats,
            "measurements_in_buffer": len(state.measurements),
            "alerts_in_buffer": len(state.alerts),
            "timestamp": time.time()
        }
        
    except Exception as e:
        print(f"❌ Error during clear: {e}")
        return {"cleared": False, "error": str(e)}

def get_status():
    """Get comprehensive status information."""
    try:
        uptime = time.time() - (state.start_time or time.time())
        
        # Calculate rates
        measurement_rate = state.statistics["total_measurements"] / max(uptime, 1) * 60  # per minute
        alert_rate = state.statistics["total_alerts"] / max(uptime, 1) * 60  # per minute
        
        # Recent activity
        recent_measurements = len([m for m in state.measurements if time.time() - m["timestamp"] < 60])
        recent_alerts = len([a for a in state.alerts if time.time() - a["timestamp"] < 60])
        
        status = {
            "driver_status": "active",
            "initialized": state.initialized,
            "uptime_seconds": round(uptime, 2),
            "performance": {
                "measurement_rate_per_minute": round(measurement_rate, 2),
                "alert_rate_per_minute": round(alert_rate, 2),
                "recent_measurements_1min": recent_measurements,
                "recent_alerts_1min": recent_alerts
            },
            "statistics": state.statistics.copy(),
            "buffers": {
                "measurements_count": len(state.measurements),
                "measurements_capacity": state.measurements.maxlen,
                "alerts_count": len(state.alerts),
                "alerts_capacity": state.alerts.maxlen
            },
            "thresholds": state.thresholds.copy(),
            "health": "good" if alert_rate < 1.0 else "degraded",
            "timestamp": time.time()
        }
        
        return json.dumps(status, indent=2)
        
    except Exception as e:
        print(f"❌ Error getting status: {e}")
        return json.dumps({"status": "error", "error": str(e)})

def shutdown():
    """Graceful shutdown with final reporting."""
    try:
        print("🔄 Advanced Python script shutting down...")
        
        uptime = time.time() - (state.start_time or time.time())
        
        # Generate final report
        final_report = {
            "shutdown_status": "complete",
            "session_summary": {
                "uptime_seconds": round(uptime, 2),
                "total_measurements": state.statistics["total_measurements"],
                "total_alerts": state.statistics["total_alerts"],
                "final_statistics": state.statistics.copy(),
                "peak_concentration": state.statistics.get("max_concentration"),
                "average_concentration": round(state.statistics.get("avg_concentration", 0), 2)
            },
            "data_summary": {
                "measurements_in_buffer": len(state.measurements),
                "alerts_in_buffer": len(state.alerts),
                "oldest_measurement": min([m["timestamp"] for m in state.measurements]) if state.measurements else None,
                "newest_measurement": max([m["timestamp"] for m in state.measurements]) if state.measurements else None
            },
            "shutdown_timestamp": time.time()
        }
        
        print(f"📋 Final Report:")
        print(f"   Uptime: {uptime/60:.1f} minutes")
        print(f"   Measurements: {state.statistics['total_measurements']}")
        print(f"   Alerts: {state.statistics['total_alerts']}")
        print(f"   Avg Concentration: {state.statistics.get('avg_concentration', 0):.2f} ppm")
        
        return final_report
        
    except Exception as e:
        print(f"❌ Error during shutdown: {e}")
        return {"shutdown": "error", "error": str(e)}

def get_concentration_level(concentration):
    """Classify concentration level based on thresholds."""
    if concentration >= state.thresholds["critical"]:
        return "critical"
    elif concentration >= state.thresholds["high"]:
        return "high"
    elif concentration >= state.thresholds["medium"]:
        return "medium"
    elif concentration >= state.thresholds["low"]:
        return "low"
    else:
        return "normal"

# Self-test functionality
def test_advanced_features():
    """Test advanced features of this script."""
    print("🧪 Running advanced feature tests...")
    
    # Test statistics
    test_concentrations = [100, 200, 300, 1200, 800]
    for i, conc in enumerate(test_concentrations):
        test_data = {
            "concentration_ppm": conc,
            "source_node_id": f"test_sensor_{i}",
            "peak_amplitude": 0.5 + i * 0.1,
            "peak_frequency": 1000 + i * 100,
            "timestamp": time.time() + i,
            "metadata": {"test_sequence": i}
        }
        result = on_measurement(test_data)
        print(f"   Test measurement {i+1}: {result.get('concentration_level', 'unknown')} level")
    
    print("✅ Advanced tests complete!")

if __name__ == "__main__":
    # Initialize and run self-test
    initialize()
    test_advanced_features()
    print(get_status())
    shutdown()
