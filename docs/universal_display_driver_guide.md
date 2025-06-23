# Universal Display ActionNode Driver Architecture

## Overview

The `UniversalDisplayActionNode` provides a flexible, pluggable architecture for outputting photoacoustic sensor data to various display technologies. Through the `DisplayDriver` trait, the same ActionNode can output to web dashboards, message queues, databases, physical displays, and future output technologies without changing the core processing logic.

## Architecture

```text
UniversalDisplayActionNode
          ↓
   DisplayDriver trait
          ↓
┌─────────────┬─────────────┬─────────────┬─────────────┐
│   HTTPS     │    Redis    │    Kafka    │  Physical   │
│  Callback   │   Driver    │   Driver    │   Drivers   │
│   Driver    │             │             │  (Future)   │
└─────────────┴─────────────┴─────────────┴─────────────┘
```

## Available Drivers

### 1. HttpsCallbackDisplayDriver

**Purpose**: Send data to web dashboards, cloud APIs, and external monitoring systems via HTTP/HTTPS.

**Use Cases**:
- Real-time web dashboards
- Cloud monitoring platforms (AWS, Azure, GCP)
- External system integration
- Alert webhooks

**Configuration**:
```yaml
driver:
  type: "https_callback"
  config:
    callback_url: "https://dashboard.company.com/api/display"
    auth_header: "Authorization"
    auth_token: "Bearer your_token_here"
    timeout_ms: 5000
    retry_count: 3
    verify_ssl: true
```

**Code Example**:
```rust
let http_driver = HttpsCallbackDisplayDriver::new()
    .with_callback_url("https://api.company.com/sensors/display")
    .with_auth_header("Authorization", "Bearer token")
    .with_timeout_ms(5000)
    .build()?;

let display_node = UniversalDisplayActionNode::new("web_display".to_string())
    .with_driver(Box::new(http_driver))
    .with_concentration_threshold(1000.0);
```

### 2. RedisDisplayDriver

**Purpose**: Real-time data streaming and caching via Redis pub/sub and data structures.

**Use Cases**:
- Real-time data streaming
- Caching recent measurements
- Inter-service communication
- Live dashboard updates

**Configuration**:
```yaml
driver:
  type: "redis"
  config:
    connection_string: "redis://localhost:6379"
    channel: "photoacoustic:realtime"
    expiry_seconds: 3600
    max_retries: 5
```

**Code Example**:
```rust
let redis_driver = RedisDisplayDriver::new()
    .with_connection_string("redis://redis.company.com:6379")
    .with_channel("sensors:photoacoustic:data")
    .with_expiry_seconds(3600)
    .build()?;

let stream_node = UniversalDisplayActionNode::new("redis_stream".to_string())
    .with_driver(Box::new(redis_driver))
    .with_update_interval(500); // High frequency streaming
```

### 3. KafkaDisplayDriver

**Purpose**: Enterprise-grade event streaming for large-scale distributed systems.

**Use Cases**:
- Enterprise event streaming
- Data lake ingestion
- Microservices communication
- Audit trails and compliance

**Configuration**:
```yaml
driver:
  type: "kafka"
  config:
    bootstrap_servers: "kafka1:9092,kafka2:9092"
    topic: "industrial.sensors.photoacoustic"
    producer_configs:
      acks: "all"
      retries: "10"
      compression.type: "gzip"
```

**Code Example**:
```rust
let kafka_driver = KafkaDisplayDriver::new()
    .with_bootstrap_servers("kafka.company.com:9092")
    .with_topic("industrial.sensors.display")
    .with_producer_config("acks", "all")
    .build()?;

let event_node = UniversalDisplayActionNode::new("kafka_events".to_string())
    .with_driver(Box::new(kafka_driver))
    .with_concentration_threshold(750.0);
```

## Planned Physical Drivers

### USBDisplayDriver (Future)
- USB-connected displays and HID devices
- Plug-and-play display modules
- USB LED indicators and meters

### SerialDisplayDriver (Future)
- RS232/RS485 serial communication
- Industrial display panels
- Legacy equipment integration

### I2CDisplayDriver (Future)
- OLED/LCD displays for embedded systems
- Raspberry Pi and Arduino integration
- Low-power embedded displays

### LEDStripDriver (Future)
- Addressable LED strips (WS2812, APA102)
- Visual indicators and status displays
- Color-coded concentration levels

### GPIODisplayDriver (Future)
- Direct GPIO control for custom hardware
- Relay control for alarms and indicators
- Digital output signals

## Creating Custom Drivers

To create a new display driver, implement the `DisplayDriver` trait:

```rust
use async_trait::async_trait;
use crate::processing::computing_nodes::display_drivers::{DisplayDriver, DisplayData, AlertData};

#[derive(Debug)]
pub struct MyCustomDisplayDriver {
    // Your driver-specific configuration
    config: MyDriverConfig,
    // Connection state, hardware handles, etc.
    connection: Option<MyConnection>,
}

#[async_trait]
impl DisplayDriver for MyCustomDisplayDriver {
    async fn initialize(&mut self) -> Result<()> {
        // Initialize hardware/service connection
        self.connection = Some(MyConnection::new(&self.config)?);
        Ok(())
    }

    async fn update_display(&mut self, data: &DisplayData) -> Result<()> {
        // Update your display with concentration data
        if let Some(ref mut conn) = self.connection {
            conn.send_update(data.concentration_ppm, data.timestamp).await?;
        }
        Ok(())
    }

    async fn show_alert(&mut self, alert: &AlertData) -> Result<()> {
        // Show alert on your display
        if let Some(ref mut conn) = self.connection {
            conn.trigger_alarm(&alert.message, &alert.severity).await?;
        }
        Ok(())
    }

    async fn clear_display(&mut self) -> Result<()> {
        // Clear display and return to idle state
        if let Some(ref mut conn) = self.connection {
            conn.clear().await?;
        }
        Ok(())
    }

    async fn get_status(&self) -> Result<serde_json::Value> {
        // Return driver status information
        Ok(json!({
            "driver_type": "my_custom",
            "connected": self.connection.is_some(),
            "config": self.config
        }))
    }

    fn driver_type(&self) -> &str {
        "my_custom"
    }
}
```

## Driver Integration Patterns

### Multi-Output Configuration

Use multiple display nodes for different output destinations:

```yaml
processing_graph:
  nodes:
    # Web dashboard
    - id: "web_display"
      node_type: "action_universal_display"
      parameters:
        driver:
          type: "https_callback"
          config:
            callback_url: "https://dashboard.company.com/api"

    # Real-time stream
    - id: "realtime_stream"
      node_type: "action_universal_display"
      parameters:
        driver:
          type: "redis"
          config:
            channel: "realtime:data"

    # Enterprise events
    - id: "enterprise_events"
      node_type: "action_universal_display"
      parameters:
        driver:
          type: "kafka"
          config:
            topic: "industrial.sensors"

  connections:
    - from: concentration_calculator
      to: web_display
    - from: concentration_calculator
      to: realtime_stream
    - from: concentration_calculator
      to: enterprise_events
```

### Environment-Based Configuration

Use different drivers for different environments:

```yaml
# Development - simple logging
driver:
  type: "console"  # Logs to console only

# Staging - Redis for testing
driver:
  type: "redis"
  config:
    connection_string: "redis://staging-redis:6379"

# Production - Kafka for enterprise
driver:
  type: "kafka"
  config:
    bootstrap_servers: "prod-kafka1:9092,prod-kafka2:9092"
```

## Performance Considerations

### Update Intervals
- **HTTP callbacks**: 1-5 seconds (avoid overwhelming APIs)
- **Redis pub/sub**: 100-500ms (real-time streaming)
- **Kafka events**: 1-10 seconds (event-driven)
- **Physical displays**: 500-2000ms (human-readable updates)

### Buffer Sizing
- **Real-time streaming**: 50-100 entries (minimal memory)
- **Web dashboards**: 200-500 entries (trend analysis)
- **Enterprise logging**: 1000+ entries (historical data)

### Error Handling
All drivers implement graceful error handling:
- Network failures don't stop the processing pipeline
- Retry logic with exponential backoff
- Fallback to logging when drivers fail
- Health monitoring and status reporting

## Testing and Development

### Mock Driver for Testing
```rust
#[derive(Debug)]
pub struct MockDisplayDriver {
    pub updates: Vec<DisplayData>,
    pub alerts: Vec<AlertData>,
}

// Implement DisplayDriver trait for testing
// Collects all calls for verification in tests
```

### Driver Initialization
```rust
// Initialize driver before using the node
let mut display_node = create_display_node()?;
display_node.initialize_driver().await?;

// Check driver status
let status = display_node.get_driver_status().await?;
println!("Driver status: {}", status.unwrap_or_default());
```

## Troubleshooting

### Common Issues
1. **Driver not configured**: Node falls back to logging only
2. **Network timeouts**: Check firewall and connectivity
3. **Authentication failures**: Verify tokens and credentials
4. **Message format errors**: Check data serialization

### Debugging
- Enable debug logging: `RUST_LOG=debug`
- Check driver status via API or logs
- Use mock drivers for isolated testing
- Monitor network connectivity and latency

## Future Enhancements

### Planned Features
- **Dynamic driver switching**: Change drivers without restart
- **Driver multiplexing**: Send to multiple drivers simultaneously
- **Configuration templates**: Pre-built configurations for common setups
- **Driver marketplace**: Community-contributed drivers
- **Performance metrics**: Driver-specific monitoring and analytics

### Hardware Integration Roadmap
1. **Phase 1**: USB and Serial drivers for common displays
2. **Phase 2**: I2C and SPI for embedded systems
3. **Phase 3**: GPIO and custom hardware protocols
4. **Phase 4**: Wireless displays (WiFi, Bluetooth, LoRa)
5. **Phase 5**: Cloud-native drivers (AWS IoT, Azure IoT, GCP)

The Universal Display ActionNode provides a foundation for scalable, flexible display integration that can grow with your system requirements while maintaining compatibility with existing processing pipelines.
