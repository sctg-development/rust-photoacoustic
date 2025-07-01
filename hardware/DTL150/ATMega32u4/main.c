/*
 * TEC/Laser Driver DTL150 - USB Evolution of DTL100-A03
 * ATMega32u4 (with Arduino Micro firmware) + ADS1115
 * 
 * Features:
 * - Control of TEC DAC (LTC2641 U1) via SPI
 * - Control of Laser DAC (LTC2641 U5) via SPI  
 * - 4-channel acquisition via ADS1115 (I2C)
 * - USB communication with command protocol
 * - Safety management and real-time monitoring
 */

#include <SPI.h>
#include <Wire.h>
#include <Adafruit_ADS1X15.h>

// ============================================================================
// HARDWARE CONFIGURATION
// ============================================================================

// SPI Pins for LTC2641 DACs
#define CS_TEC_PIN      10    // Chip Select DAC TEC (U1)
#define CS_LASER_PIN    9     // Chip Select DAC Laser (U5)
#define SPI_MOSI_PIN    16    // MOSI (data)
#define SPI_SCK_PIN     15    // SPI Clock

// GPIO control pins
#define ON_OFF_TEC_PIN  4     // TEC Enable
#define ON_OFF_LASER_PIN 5    // Laser Enable
#define FAULT_READ_PIN  6     // Fault read (input)
#define STATUS_LED_PIN  13    // Arduino status LED

// I2C Pins for ADS1115 (SDA=2, SCL=3 on Arduino Micro)
#define I2C_SDA_PIN     2
#define I2C_SCL_PIN     3

// ============================================================================
// CONSTANTS
// ============================================================================

// ADS1115 configuration
#define ADS1115_ADDRESS 0x48
#define ADC_GAIN        GAIN_ONE      // ±4.096V range
#define ADC_SPS         ADS1115_DR_860SPS

// ADC channels
#define ADC_CHANNEL_I_TEC     0   // TEC current
#define ADC_CHANNEL_I_LASER   1   // Laser current  
#define ADC_CHANNEL_TEMP      2   // Temperature
#define ADC_CHANNEL_V_TEC     3   // TEC voltage

// LTC2641 DAC configuration (12 bits)
#define DAC_MAX_VALUE   4095  // 2^12 - 1
#define DAC_VREF        5.0   // Reference voltage

// Safety limits
#define MAX_TEC_CURRENT     5.0   // Amperes
#define MAX_LASER_CURRENT   10.0  // Amperes
#define MAX_TEMPERATURE     80.0  // °C
#define MIN_TEMPERATURE     -10.0 // °C

// Timeouts and delays
#define MONITORING_INTERVAL 100   // ms
#define WATCHDOG_TIMEOUT    5000  // ms
#define COMMAND_TIMEOUT     1000  // ms

// ============================================================================
// GLOBAL VARIABLES
// ============================================================================

// ADS1115 object
Adafruit_ADS1115 ads;

// System state
struct SystemState {
  bool tec_enabled;
  bool laser_enabled;
  bool fault_active;
  bool system_ready;
  uint32_t last_command_time;
  uint32_t last_monitoring_time;
};

// Real-time measurements
struct Measurements {
  float tec_current;      // A
  float laser_current;    // A
  float temperature;      // °C
  float tec_voltage;      // V
  uint32_t timestamp;     // ms
};

// Setpoints
struct Setpoints {
  uint16_t tec_dac_value;     // 0-4095
  uint16_t laser_dac_value;   // 0-4095
  float tec_current_sp;       // A
  float laser_current_sp;     // A
};

// Global variables
SystemState sys_state;
Measurements measurements;
Setpoints setpoints;

// Serial command buffer
String command_buffer = "";
bool command_ready = false;

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

void blinkStatusLED(int count, int delay_ms = 200) {
  for (int i = 0; i < count; i++) {
    digitalWrite(STATUS_LED_PIN, HIGH);
    delay(delay_ms);
    digitalWrite(STATUS_LED_PIN, LOW);
    delay(delay_ms);
  }
}

void emergencyShutdown() {
  // Emergency shutdown - cut everything
  digitalWrite(ON_OFF_TEC_PIN, LOW);
  digitalWrite(ON_OFF_LASER_PIN, LOW);
  writeDACValue(CS_TEC_PIN, 0);
  writeDACValue(CS_LASER_PIN, 0);
  sys_state.tec_enabled = false;
  sys_state.laser_enabled = false;
  sys_state.fault_active = true;
  
  Serial.println("ERROR:EMERGENCY_SHUTDOWN");
  blinkStatusLED(10, 100); // Alarm signal
}

// ============================================================================
// LTC2641 DAC FUNCTIONS
// ============================================================================

void writeDACValue(int cs_pin, uint16_t value) {
  // Limit value to 12 bits
  value = constrain(value, 0, DAC_MAX_VALUE);
  
  // LTC2641 format: 4 command bits + 12 data bits
  uint16_t dac_word = (0x3 << 12) | (value & 0x0FFF); // "Write and Update" command
  
  digitalWrite(cs_pin, LOW);
  delayMicroseconds(1);
  
  SPI.transfer16(dac_word);
  
  delayMicroseconds(1);
  digitalWrite(cs_pin, HIGH);
}

void setTECCurrent(float current_amps) {
  // Convert current to DAC value
  // Adjust according to TEC regulator characteristics
  current_amps = constrain(current_amps, 0.0, MAX_TEC_CURRENT);
  uint16_t dac_value = (uint16_t)(current_amps * DAC_MAX_VALUE / MAX_TEC_CURRENT);
  
  setpoints.tec_current_sp = current_amps;
  setpoints.tec_dac_value = dac_value;
  
  if (sys_state.tec_enabled) {
    writeDACValue(CS_TEC_PIN, dac_value);
  }
}

void setLaserCurrent(float current_amps) {
  // Convert current to DAC value
  // 1V = 200mA according to schematic
  current_amps = constrain(current_amps, 0.0, MAX_LASER_CURRENT);
  uint16_t dac_value = (uint16_t)(current_amps * DAC_MAX_VALUE / MAX_LASER_CURRENT);
  
  setpoints.laser_current_sp = current_amps;
  setpoints.laser_dac_value = dac_value;
  
  if (sys_state.laser_enabled) {
    writeDACValue(CS_LASER_PIN, dac_value);
  }
}

// ============================================================================
// ADS1115 ADC FUNCTIONS
// ============================================================================

void readAllChannels() {
  measurements.timestamp = millis();
  
  // Read the 4 channels
  int16_t adc0 = ads.readADC_SingleEnded(ADC_CHANNEL_I_TEC);
  int16_t adc1 = ads.readADC_SingleEnded(ADC_CHANNEL_I_LASER);
  int16_t adc2 = ads.readADC_SingleEnded(ADC_CHANNEL_TEMP);
  int16_t adc3 = ads.readADC_SingleEnded(ADC_CHANNEL_V_TEC);
  
  // Convert to voltages (ADS1115 resolution = 0.125mV with GAIN_ONE)
  float voltage0 = ads.computeVolts(adc0);
  float voltage1 = ads.computeVolts(adc1);
  float voltage2 = ads.computeVolts(adc2);
  float voltage3 = ads.computeVolts(adc3);
  
  // Convert to physical units (adjust according to conditioning)
  measurements.tec_current = voltageToCurrent(voltage0, true);    // TEC
  measurements.laser_current = voltageToCurrent(voltage1, false); // Laser
  measurements.temperature = voltageToTemperature(voltage2);
  measurements.tec_voltage = voltage3;
}

float voltageToCurrent(float voltage, bool is_tec) {
  // Voltage -> current conversion according to circuit conditioning
  if (is_tec) {
    // TEC: adjust according to shunt and amplification
    return voltage * 2.0; // Example: 1V = 2A (to be adjusted)
  } else {
    // Laser: 1V = 200mA according to schematic
    return voltage * 0.2; // 1V = 0.2A
  }
}

float voltageToTemperature(float voltage) {
  // Voltage -> temperature conversion according to thermistor and conditioning
  // Example with NTC 10kΩ thermistor (to be adjusted according to actual circuit)
  
  // Calculated resistance (voltage divider)
  float vcc = 5.0;
  float r_series = 10000.0; // Series resistor
  float r_thermistor = r_series * voltage / (vcc - voltage);
  
  // Simplified Steinhart-Hart equation
  float temp_k = 1.0 / (0.001129 + 0.000234 * log(r_thermistor) + 0.0000000876 * pow(log(r_thermistor), 3));
  return temp_k - 273.15; // K -> °C
}

// ============================================================================
// SAFETY FUNCTIONS
// ============================================================================

bool checkSafetyLimits() {
  bool safe = true;
  
  // Check TEC current
  if (measurements.tec_current > MAX_TEC_CURRENT * 1.1) { // 10% margin
    Serial.println("ERROR:TEC_OVERCURRENT");
    safe = false;
  }
  
  // Check Laser current
  if (measurements.laser_current > MAX_LASER_CURRENT * 1.1) {
    Serial.println("ERROR:LASER_OVERCURRENT");
    safe = false;
  }
  
  // Check temperature
  if (measurements.temperature > MAX_TEMPERATURE || 
      measurements.temperature < MIN_TEMPERATURE) {
    Serial.println("ERROR:TEMPERATURE_LIMIT");
    safe = false;
  }
  
  // Read hardware fault pin
  if (digitalRead(FAULT_READ_PIN) == HIGH) {
    Serial.println("ERROR:HARDWARE_FAULT");
    safe = false;
  }
  
  // Communication watchdog
  if (millis() - sys_state.last_command_time > WATCHDOG_TIMEOUT) {
    Serial.println("ERROR:COMM_TIMEOUT");
    safe = false;
  }
  
  if (!safe) {
    sys_state.fault_active = true;
    emergencyShutdown();
  }
  
  return safe;
}

// ============================================================================
// COMMUNICATION FUNCTIONS
// ============================================================================

void processCommand(String cmd) {
  cmd.trim();
  cmd.toUpperCase();
  
  sys_state.last_command_time = millis();
  
  if (cmd.startsWith("TEC:SET:")) {
    float value = cmd.substring(8).toFloat();
    setTECCurrent(value);
    Serial.println("OK");
    
  } else if (cmd.startsWith("LAS:SET:")) {
    float value = cmd.substring(8).toFloat();
    setLaserCurrent(value);
    Serial.println("OK");
    
  } else if (cmd == "TEC:ON") {
    if (!sys_state.fault_active) {
      digitalWrite(ON_OFF_TEC_PIN, HIGH);
      sys_state.tec_enabled = true;
      writeDACValue(CS_TEC_PIN, setpoints.tec_dac_value);
      Serial.println("OK");
    } else {
      Serial.println("ERROR:FAULT_ACTIVE");
    }
    
  } else if (cmd == "TEC:OFF") {
    digitalWrite(ON_OFF_TEC_PIN, LOW);
    writeDACValue(CS_TEC_PIN, 0);
    sys_state.tec_enabled = false;
    Serial.println("OK");
    
  } else if (cmd == "LAS:ON") {
    if (!sys_state.fault_active) {
      digitalWrite(ON_OFF_LASER_PIN, HIGH);
      sys_state.laser_enabled = true;
      writeDACValue(CS_LASER_PIN, setpoints.laser_dac_value);
      Serial.println("OK");
    } else {
      Serial.println("ERROR:FAULT_ACTIVE");
    }
    
  } else if (cmd == "LAS:OFF") {
    digitalWrite(ON_OFF_LASER_PIN, LOW);
    writeDACValue(CS_LASER_PIN, 0);
    sys_state.laser_enabled = false;
    Serial.println("OK");
    
  } else if (cmd == "STATUS?") {
    sendStatus();
    
  } else if (cmd == "RESET") {
    sys_state.fault_active = false;
    Serial.println("OK");
    
  } else if (cmd == "MONITOR:ON") {
    // Enable continuous monitoring
    Serial.println("OK");
    
  } else if (cmd == "MONITOR:OFF") {
    // Disable continuous monitoring
    Serial.println("OK");
    
  } else {
    Serial.println("ERROR:UNKNOWN_COMMAND");
  }
}

void sendStatus() {
  // Format: "TEC:<temp>,<current>;LAS:<current>,<voltage>;STATUS:<flags>\n"
  Serial.print("TEC:");
  Serial.print(measurements.temperature, 2);
  Serial.print(",");
  Serial.print(measurements.tec_current, 3);
  Serial.print(";LAS:");
  Serial.print(measurements.laser_current, 3);
  Serial.print(",");
  Serial.print(measurements.tec_voltage, 2);
  Serial.print(";STATUS:");
  
  // Status flags
  uint8_t status_flags = 0;
  if (sys_state.tec_enabled) status_flags |= 0x01;
  if (sys_state.laser_enabled) status_flags |= 0x02;
  if (sys_state.fault_active) status_flags |= 0x04;
  if (sys_state.system_ready) status_flags |= 0x08;
  
  Serial.println(status_flags, HEX);
}

// ============================================================================
// MAIN FUNCTIONS
// ============================================================================

void setup() {
  // Serial USB initialization
  Serial.begin(115200);
  while (!Serial) delay(10); // Wait for USB connection
  
  Serial.println("# TEC/Laser Driver DTL100-A03 - USB Version");
  Serial.println("# Starting initialization...");
  
  // GPIO configuration
  pinMode(CS_TEC_PIN, OUTPUT);
  pinMode(CS_LASER_PIN, OUTPUT);
  pinMode(ON_OFF_TEC_PIN, OUTPUT);
  pinMode(ON_OFF_LASER_PIN, OUTPUT);
  pinMode(FAULT_READ_PIN, INPUT_PULLUP);
  pinMode(STATUS_LED_PIN, OUTPUT);
  
  // Safe initial state
  digitalWrite(CS_TEC_PIN, HIGH);
  digitalWrite(CS_LASER_PIN, HIGH);
  digitalWrite(ON_OFF_TEC_PIN, LOW);
  digitalWrite(ON_OFF_LASER_PIN, LOW);
  
  // SPI initialization
  SPI.begin();
  SPI.setClockDivider(SPI_CLOCK_DIV16); // 1MHz for LTC2641
  SPI.setDataMode(SPI_MODE0);
  SPI.setBitOrder(MSBFIRST);
  
  // I2C and ADS1115 initialization
  Wire.begin();
  if (!ads.begin(ADS1115_ADDRESS)) {
    Serial.println("ERROR:ADS1115_INIT_FAILED");
    while (1) {
      blinkStatusLED(3, 500);
      delay(1000);
    }
  }
  
  // ADS1115 configuration
  ads.setGain(ADC_GAIN);
  ads.setDataRate(ADC_SPS);
  
  // Initialize variables
  memset(&sys_state, 0, sizeof(sys_state));
  memset(&measurements, 0, sizeof(measurements));
  memset(&setpoints, 0, sizeof(setpoints));
  
  sys_state.last_command_time = millis();
  sys_state.system_ready = true;
  
  // Initial DACs test
  writeDACValue(CS_TEC_PIN, 0);
  writeDACValue(CS_LASER_PIN, 0);
  
  // First acquisition
  delay(100);
  readAllChannels();
  
  Serial.println("# Initialization complete");
  Serial.println("# Ready for commands");
  blinkStatusLED(2, 100);
}

void loop() {
  // Serial command reading
  while (Serial.available()) {
    char c = Serial.read();
    if (c == '\n' || c == '\r') {
      if (command_buffer.length() > 0) {
        processCommand(command_buffer);
        command_buffer = "";
      }
    } else {
      command_buffer += c;
      if (command_buffer.length() > 50) { // Buffer limit
        command_buffer = "";
        Serial.println("ERROR:COMMAND_TOO_LONG");
      }
    }
  }
  
  // Periodic monitoring
  uint32_t now = millis();
  if (now - sys_state.last_monitoring_time >= MONITORING_INTERVAL) {
    sys_state.last_monitoring_time = now;
    
    // Measurement acquisition
    readAllChannels();
    
    // Safety checks
    checkSafetyLimits();
    
    // Status LED
    if (sys_state.fault_active) {
      digitalWrite(STATUS_LED_PIN, (now / 100) % 2); // Fast blinking
    } else if (sys_state.tec_enabled || sys_state.laser_enabled) {
      digitalWrite(STATUS_LED_PIN, HIGH); // Steady on
    } else {
      digitalWrite(STATUS_LED_PIN, (now / 1000) % 2); // Slow blinking
    }
  }
  
  delay(1); // Avoid CPU overload
}

// ============================================================================
// ADDITIONAL FUNCTIONS
// ============================================================================

void printSystemInfo() {
  Serial.println("# System Information:");
  Serial.print("# Firmware Version: 1.0\n");
  Serial.print("# Compile Date: ");
  Serial.print(__DATE__);
  Serial.print(" ");
  Serial.println(__TIME__);
  Serial.print("# Free RAM: ");
  Serial.println(freeMemory());
}

int freeMemory() {
  extern int __heap_start, *__brkval;
  int v;
  return (int) &v - (__brkval == 0 ? (int) &__heap_start : (int) __brkval);
}

/*
 * USB COMMUNICATION PROTOCOL
 * ==========================
 * 
 * Available commands:
 * 
 * TEC:SET:<value>   - Set TEC current (0.0 to 5.0A)
 * LAS:SET:<value>   - Set Laser current (0.0 to 10.0A)
 * TEC:ON            - Enable TEC
 * TEC:OFF           - Disable TEC
 * LAS:ON            - Enable Laser
 * LAS:OFF           - Disable Laser
 * STATUS?           - Read full status
 * RESET             - Clear faults
 * MONITOR:ON        - Enable continuous monitoring
 * MONITOR:OFF       - Disable continuous monitoring
 * 
 * Responses:
 * 
 * OK                - Command executed
 * ERROR:<code>      - Error with code
 * TEC:<temp>,<I>;LAS:<I>,<V>;STATUS:<flags> - Full status
 * 
 * Error codes:
 * 
 * TEC_OVERCURRENT      - TEC overcurrent
 * LASER_OVERCURRENT    - Laser overcurrent
 * TEMPERATURE_LIMIT    - Temperature limit
 * HARDWARE_FAULT       - Hardware fault
 * COMM_TIMEOUT         - Communication timeout
 * FAULT_ACTIVE         - Active fault (prevents activation)
 * UNKNOWN_COMMAND      - Unknown command
 * COMMAND_TOO_LONG     - Command too long
 * 
 */