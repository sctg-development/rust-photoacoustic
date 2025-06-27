# Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
# This file is part of the rust-photoacoustic project and is licensed under the
# SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
# Enhanced Python script for photoacoustic action processing
# This file demonstrates how to create custom actions for measurement processing

import json
import time
from datetime import datetime

# Global state for tracking measurements
measurement_count = 0
last_concentration = None
start_time = time.time()

def log_info(message):
    """Helper function to ensure messages are visible"""
    print(f"[PYTHON INFO] {datetime.now().strftime('%H:%M:%S')} - {message}")

def log_warn(message):
    """Helper function for warning messages"""
    print(f"[PYTHON WARN] {datetime.now().strftime('%H:%M:%S')} - {message}")

def initialize():
    """Called when the driver is initialized"""
    global measurement_count, start_time
    measurement_count = 0
    start_time = time.time()
    log_info("Python action driver initializing...")
    log_info("Ready to process photoacoustic measurements")
    return {"status": "initialized", "timestamp": time.time()}

def on_measurement(data):
    """Called for each measurement - main processing logic"""
    global measurement_count, last_concentration
    
    measurement_count += 1
    concentration = data.get("concentration_ppm", 0)
    last_concentration = concentration
    node_id = data.get("source_node_id", "unknown")
    
    log_info(f"Processing measurement #{measurement_count} from {node_id}")
    log_info(f"Concentration: {concentration:.2f} ppm")
    
    # Custom processing logic
    status = "normal"
    alert_needed = False
    
    if concentration > 100:
        status = "high"
        alert_needed = True
        log_warn(f"High concentration detected: {concentration:.2f} ppm")
    elif concentration < 10:
        status = "low"
        log_info(f"Low concentration: {concentration:.2f} ppm")
    
    # Additional processing can be added here
    processing_result = {
        "processed": True,
        "measurement_count": measurement_count,
        "concentration": concentration,
        "status": status,
        "alert_needed": alert_needed,
        "processing_time": time.time()
    }
    
    log_info(f"Measurement processed with status: {status}")
    return processing_result

def on_alert(alert):
    """Called when an alert is triggered"""
    severity = alert.get("severity", "info")
    message = alert.get("message", "No message")
    node_id = alert.get("source_node_id", "unknown")
    
    log_warn(f"ALERT from {node_id} [{severity}]: {message}")
    
    # Custom alert processing
    if severity == "critical":
        log_warn("Critical alert - immediate action required!")
    
    return {
        "alert_handled": True,
        "severity": severity,
        "handled_at": time.time(),
        "action_taken": f"Logged {severity} alert"
    }

def get_status():
    """Return current status and statistics"""
    uptime = time.time() - start_time
    
    status_info = {
        "status": "active",
        "measurement_count": measurement_count,
        "last_concentration": last_concentration,
        "uptime_seconds": uptime,
        "uptime_human": f"{uptime/60:.1f} minutes",
        "last_update": time.time()
    }
    
    log_info(f"Status requested - {measurement_count} measurements processed, uptime: {uptime/60:.1f} min")
    return status_info

def shutdown():
    """Called when the driver is shutting down"""
    uptime = time.time() - start_time
    log_info(f"Python action driver shutting down after {uptime/60:.1f} minutes")
    log_info(f"Total measurements processed: {measurement_count}")
    
    return {
        "status": "shutdown",
        "final_measurement_count": measurement_count,
        "total_uptime_seconds": uptime,
        "shutdown_time": time.time()
    }

def clear_action():
    """Clear any active actions and reset state"""
    global measurement_count
    old_count = measurement_count
    measurement_count = 0
    
    log_info(f"Clearing actions - reset measurement count from {old_count} to 0")
    
    return {
        "status": "cleared",
        "previous_count": old_count,
        "cleared_at": time.time()
    }
