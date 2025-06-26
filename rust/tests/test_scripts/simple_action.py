"""
Simple test script for PythonActionDriver integration tests.

This script demonstrates the basic Python API for action scripts.

Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
This file is part of the rust-photoacoustic project and is licensed under the
SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
"""

import json
import time

# Global state for testing
_state = {
    "initialized": False,
    "measurement_count": 0,
    "alert_count": 0,
    "last_concentration": None,
    "last_alert": None
}

def initialize():
    """Called when the driver is first initialized."""
    print("Python action script initialized")
    _state["initialized"] = True
    _state["start_time"] = time.time()
    return {"status": "ready", "timestamp": time.time()}

def on_measurement(data):
    """Called when new measurement data is available."""
    _state["measurement_count"] += 1
    _state["last_concentration"] = data["concentration_ppm"]
    
    print(f"[MEASUREMENT] #{_state['measurement_count']}: {data['concentration_ppm']:.2f} ppm from {data['source_node_id']}")
    
    # Example logic: react to high concentrations
    if data["concentration_ppm"] > 1000:
        print("[WARNING] High concentration detected!")
    
    return {
        "processed": True,
        "measurement_count": _state["measurement_count"],
        "timestamp": time.time()
    }

def on_alert(alert):
    """Called when an alert condition is triggered."""
    _state["alert_count"] += 1
    _state["last_alert"] = alert
    
    print(f"[ALERT] ALERT #{_state['alert_count']}: {alert['severity']} - {alert['message']}")
    
    # Example: Different responses based on severity
    if alert["severity"] == "critical":
        print("[CRITICAL] CRITICAL ALERT - Emergency protocols activated!")
    elif alert["severity"] == "warning":
        print("[WARNING] WARNING - Monitoring closely")
    else:
        print("[INFO] INFO - Noted")
    
    return {
        "alert_handled": True,
        "alert_count": _state["alert_count"],
        "severity": alert["severity"],
        "timestamp": time.time()
    }

def clear_action():
    """Called when the system is being cleared/reset."""
    print("[CLEAR] Clearing action - returning to safe state")
    
    # Reset counters but keep initialization state
    old_count = _state["measurement_count"]
    _state["measurement_count"] = 0
    _state["alert_count"] = 0
    _state["last_concentration"] = None
    _state["last_alert"] = None
    
    return {
        "cleared": True,
        "previous_measurements": old_count,
        "timestamp": time.time()
    }

def get_status():
    """Called to get current driver status."""
    uptime = time.time() - _state.get("start_time", time.time())
    
    status = {
        "custom_status": "active",
        "initialized": _state["initialized"],
        "uptime_seconds": uptime,
        "measurement_count": _state["measurement_count"],
        "alert_count": _state["alert_count"],
        "last_concentration_ppm": _state["last_concentration"],
        "last_alert_severity": _state["last_alert"]["severity"] if _state["last_alert"] else None,
        "timestamp": time.time()
    }
    
    return json.dumps(status)

def shutdown():
    """Called when the driver is being shut down."""
    print("[SHUTDOWN] Python script shutting down")
    
    final_stats = {
        "total_measurements": _state["measurement_count"],
        "total_alerts": _state["alert_count"],
        "uptime_seconds": time.time() - _state.get("start_time", time.time()),
        "shutdown_timestamp": time.time()
    }
    
    print(f"[STATS] Final stats: {final_stats}")
    
    return {
        "shutdown": "complete",
        "final_stats": final_stats
    }

# Test helper function (not part of standard API)
def test_script():
    """Self-test function for debugging."""
    print("[TEST] Running self-test...")
    
    # Test initialization
    init_result = initialize()
    print(f"Init result: {init_result}")
    
    # Test measurement
    test_data = {
        "concentration_ppm": 850.5,
        "source_node_id": "test_sensor",
        "peak_amplitude": 0.75,
        "peak_frequency": 1200.0,
        "timestamp": int(time.time()),
        "metadata": {"test": "true"}
    }
    measure_result = on_measurement(test_data)
    print(f"Measurement result: {measure_result}")
    
    # Test alert
    test_alert = {
        "alert_type": "concentration_high",
        "severity": "warning",
        "message": "Concentration above threshold",
        "timestamp": int(time.time()),
        "data": {"threshold": 800.0}
    }
    alert_result = on_alert(test_alert)
    print(f"Alert result: {alert_result}")
    
    # Test status
    status_result = get_status()
    print(f"Status result: {status_result}")
    
    # Test clear
    clear_result = clear_action()
    print(f"Clear result: {clear_result}")
    
    # Test shutdown
    shutdown_result = shutdown()
    print(f"Shutdown result: {shutdown_result}")
    
    print("[TEST] Self-test complete!")

if __name__ == "__main__":
    test_script()
