"""
Python Action Script API for rust-photoacoustic

This is the interface that user Python scripts should implement.
All functions are optional - implement only what you need.

Example usage:
1. Save this as your action script (e.g., custom_action.py)
2. Implement the functions you need
3. Configure the PythonActionDriver to use your script
4. The driver will call your functions automatically
"""

def initialize():
    """
    Called when the driver is first initialized.
    Use this to set up connections, load configuration, etc.
    
    Returns:
        None or dict: Optional initialization status
    """
    print("🐍 Python action script initialized")
    
    # Example: Initialize hardware, load config, etc.
    # import serial
    # global serial_port
    # serial_port = serial.Serial('/dev/ttyUSB0', 9600)
    
    return {"status": "ready", "initialized_at": "2025-01-27"}

def on_measurement(data):
    """
    Called when new measurement data is available.
    
    Args:
        data (dict): Measurement data with keys:
            - concentration_ppm (float): CO2 concentration in ppm
            - source_node_id (str): ID of the source node
            - peak_amplitude (float): Peak amplitude (0.0-1.0)
            - peak_frequency (float): Peak frequency in Hz
            - timestamp (int): Unix timestamp
            - metadata (dict): Additional metadata
    
    Returns:
        None or dict: Optional response data
    """
    concentration = data['concentration_ppm']
    node_id = data['source_node_id']
    amplitude = data['peak_amplitude']
    frequency = data['peak_frequency']
    
    print(f"📊 Measurement: {concentration:.1f} ppm from {node_id}")
    print(f"   Amplitude: {amplitude:.3f}, Frequency: {frequency:.1f} Hz")
    
    # Example: Control LEDs based on concentration
    if concentration > 2000:
        print("🔴 HIGH concentration - RED LED")
        # control_led('red', True)
    elif concentration > 1000:
        print("🟡 MEDIUM concentration - YELLOW LED")
        # control_led('yellow', True)
    else:
        print("🟢 LOW concentration - GREEN LED")
        # control_led('green', True)
    
    # Example: Control external hardware
    if amplitude > 0.8:
        print("📢 High amplitude detected - activating amplifier")
        # control_amplifier(True)
    
    # Example: Data logging
    # with open('measurements.log', 'a') as f:
    #     f.write(f"{data['timestamp']},{concentration},{amplitude},{frequency}\n")
    
    return {
        "processed": True,
        "led_status": "green" if concentration < 1000 else "yellow" if concentration < 2000 else "red",
        "amplifier_active": amplitude > 0.8
    }

def on_alert(alert):
    """
    Called when an alert condition is triggered.
    
    Args:
        alert (dict): Alert data with keys:
            - alert_type (str): Type of alert
            - severity (str): Alert severity (info, warning, critical)
            - message (str): Human-readable message
            - timestamp (int): Unix timestamp
            - data (dict): Alert-specific data
    
    Returns:
        None or dict: Optional response data
    """
    alert_type = alert['alert_type']
    severity = alert['severity']
    message = alert['message']
    
    print(f"🚨 ALERT: {severity.upper()} - {message}")
    print(f"   Type: {alert_type}")
    
    # Example: Different actions based on severity
    if severity == 'critical':
        print("🚨 CRITICAL ALERT - Emergency protocols activated!")
        # emergency_shutdown()
        # send_sms_alert("+1234567890", message)
        # activate_emergency_ventilation()
        
    elif severity == 'warning':
        print("⚠️  WARNING - Increased monitoring")
        # increase_sampling_rate()
        # send_email_alert("admin@company.com", message)
        
    else:  # info
        print("ℹ️  INFO - Logging event")
        # log_event(alert)
    
    # Example: Control alarm systems
    if alert_type == 'concentration':
        concentration = alert['data'].get('concentration_ppm', 0)
        if concentration > 5000:
            print("💨 Activating emergency ventilation!")
            # activate_ventilation(speed='max')
    
    return {
        "alert_handled": True,
        "action_taken": severity,
        "emergency_protocols": severity == 'critical'
    }

def clear_action():
    """
    Called when the system is being cleared/reset.
    Use this to return hardware to safe state.
    
    Returns:
        None or dict: Optional status
    """
    print("🧹 Clearing action - returning to safe state")
    
    # Example: Reset all hardware to safe state
    # control_led('all', False)
    # control_amplifier(False)
    # stop_ventilation()
    # reset_actuators()
    
    return {
        "cleared": True,
        "safe_state": True,
        "hardware_reset": True
    }

def get_status():
    """
    Called to get current driver status.
    Return JSON-serializable data (dict, list, str, etc.)
    
    Returns:
        str or dict: Status information (JSON serializable)
    """
    # Example: Check hardware status
    # led_status = check_led_status()
    # amplifier_status = check_amplifier_status()
    # ventilation_status = check_ventilation_status()
    
    status = {
        "script_status": "active",
        "last_action": "measurement_processed",
        "hardware": {
            "leds": "operational",  # led_status
            "amplifier": "standby", # amplifier_status
            "ventilation": "auto",  # ventilation_status
            "sensors": "connected"
        },
        "counters": {
            "measurements_processed": getattr(get_status, 'measurement_count', 0),
            "alerts_handled": getattr(get_status, 'alert_count', 0)
        },
        "uptime_seconds": 3600  # Example uptime
    }
    
    # Return as JSON string or dict
    import json
    return json.dumps(status)
    # OR simply return the dict:
    # return status

def shutdown():
    """
    Called when the driver is being shut down.
    Clean up resources, close connections, etc.
    
    Returns:
        None or dict: Optional shutdown status
    """
    print("🛑 Python script shutting down...")
    
    # Example: Clean shutdown sequence
    # close_serial_connections()
    # save_state_to_file()
    # turn_off_all_hardware()
    # cleanup_temp_files()
    
    print("✅ Shutdown complete")
    
    return {
        "shutdown": "complete",
        "cleanup_performed": True,
        "final_state": "safe"
    }

# Optional: Helper functions (not called by the driver)
def control_led(color, state):
    """Example helper function for LED control"""
    print(f"LED {color}: {'ON' if state else 'OFF'}")
    # Implement your LED control logic here

def control_amplifier(active):
    """Example helper function for amplifier control"""
    print(f"Amplifier: {'ACTIVE' if active else 'STANDBY'}")
    # Implement your amplifier control logic here

def emergency_shutdown():
    """Example emergency shutdown procedure"""
    print("🚨 EMERGENCY SHUTDOWN INITIATED")
    # Implement emergency protocols here

# Optional: Global variables and state
measurement_count = 0
alert_count = 0

# Optional: Module-level initialization
print("🐍 Python action module loaded")

# Example of more advanced functionality:

class HardwareController:
    """Example class for managing hardware state"""
    
    def __init__(self):
        self.led_states = {'red': False, 'yellow': False, 'green': False}
        self.amplifier_active = False
        self.ventilation_speed = 0
    
    def set_led(self, color, state):
        if color in self.led_states:
            self.led_states[color] = state
            print(f"Hardware: LED {color} = {state}")
    
    def set_amplifier(self, active):
        self.amplifier_active = active
        print(f"Hardware: Amplifier = {active}")
    
    def set_ventilation(self, speed):
        self.ventilation_speed = max(0, min(100, speed))
        print(f"Hardware: Ventilation = {self.ventilation_speed}%")

# Global hardware controller instance
hw = HardwareController()

def advanced_measurement_handler(data):
    """Example of more sophisticated measurement processing"""
    global measurement_count
    measurement_count += 1
    
    concentration = data['concentration_ppm']
    amplitude = data['peak_amplitude']
    
    # Multi-level response system
    if concentration > 3000:
        hw.set_led('red', True)
        hw.set_led('yellow', False)
        hw.set_led('green', False)
        hw.set_ventilation(100)  # Max ventilation
    elif concentration > 1500:
        hw.set_led('red', False)
        hw.set_led('yellow', True)
        hw.set_led('green', False)
        hw.set_ventilation(50)   # Medium ventilation
    else:
        hw.set_led('red', False)
        hw.set_led('yellow', False)
        hw.set_led('green', True)
        hw.set_ventilation(20)   # Low ventilation
    
    # Amplitude-based amplifier control
    hw.set_amplifier(amplitude > 0.7)
    
    return {
        "measurement_number": measurement_count,
        "concentration_level": "high" if concentration > 3000 else "medium" if concentration > 1500 else "low",
        "hardware_state": {
            "leds": hw.led_states,
            "amplifier": hw.amplifier_active,
            "ventilation": hw.ventilation_speed
        }
    }

# You can also import and use standard Python libraries:
"""
import json
import time
import logging
import requests
import serial
import gpio  # For Raspberry Pi
import numpy as np
import matplotlib.pyplot as plt
from datetime import datetime, timedelta

# Example: Send data to external API
def send_to_cloud(data):
    try:
        response = requests.post('https://api.mycompany.com/sensors', 
                               json=data, 
                               timeout=5)
        return response.status_code == 200
    except Exception as e:
        print(f"Cloud upload failed: {e}")
        return False

# Example: Data analysis
def analyze_trend(measurements):
    if len(measurements) < 10:
        return "insufficient_data"
    
    concentrations = [m['concentration_ppm'] for m in measurements]
    trend = np.polyfit(range(len(concentrations)), concentrations, 1)[0]
    
    if trend > 50:
        return "increasing"
    elif trend < -50:
        return "decreasing"
    else:
        return "stable"
"""
