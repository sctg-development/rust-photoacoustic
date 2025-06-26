# Dummy Python script for doctest examples
# This file is used by doctests in python.rs to avoid file not found errors

def initialize():
    """Called when the driver is initialized"""
    print("Python driver initializing...")
    return {"status": "initialized"}

def on_measurement(data):
    """Called for each measurement"""
    concentration = data.get("concentration_ppm", 0)
    print(f"Processing measurement: {concentration} ppm")
    return {"processed": True}

def on_alert(alert):
    """Called when an alert is triggered"""
    severity = alert.get("severity", "info")
    message = alert.get("message", "No message")
    print(f"ALERT [{severity}]: {message}")
    return {"alert_handled": True}

def get_status():
    """Return current status"""
    return {"status": "active"}

def shutdown():
    """Called when the driver is shutting down"""
    print("Python driver shutting down...")
    return {"status": "shutdown"}

def clear_action():
    """Clear any active actions"""
    print("Clearing actions...")
    return {"status": "cleared"}
