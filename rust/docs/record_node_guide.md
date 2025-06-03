# RecordNode Documentation

## Overview

The `RecordNode` is a processing node that records audio streams to PCM (WAV) files while allowing the audio data to pass through unchanged to subsequent nodes in the processing pipeline. This enables stream recording without interrupting the real-time processing flow.

## Features

- **Pass-through recording**: Records audio while preserving the original data flow
- **Multiple format support**: Handles both mono (SingleChannel) and stereo (DualChannel) audio data
- **File rotation**: Automatically creates new files when size limits are exceeded
- **Auto-cleanup**: Optional automatic deletion of old files
- **Configurable parameters**: Flexible configuration via YAML or programmatic setup
- **16-bit PCM output**: Converts f32 samples to standard 16-bit PCM format
- **Robust error handling**: Graceful degradation when recording fails

## Configuration Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `record_file` | String | Yes | - | Output file path for recordings |
| `max_size` | Number | No | 1024 | Maximum file size in KB before rotation |
| `auto_delete` | Boolean | No | false | Whether to delete old files when creating new ones |

## Usage Examples

### Basic Configuration

```yaml
- id: "audio_recorder"
  node_type: "record"
  parameters:
    record_file: "./recordings/stream.wav"
    max_size: 1024
    auto_delete: false
```

### Programmatic Usage

```rust
use rust_photoacoustic::processing::nodes::{RecordNode, ProcessingNode};
use std::path::PathBuf;

// Create a new RecordNode
let mut record_node = RecordNode::new(
    "recorder_1".to_string(),           // Node ID
    PathBuf::from("recording.wav"),     // Output file
    2048,                               // Max size in KB (2MB)
    true,                               // Auto-delete old files
);

// Process audio data (pass-through with recording)
let result = record_node.process(input_data)?;
// result contains the same data as input, but audio was recorded to file
```

## File Naming and Rotation

When file size limits are exceeded, the RecordNode automatically creates new files with timestamps:

- Original: `recording.wav`
- After rotation: `recording_20250603_143022.wav`
- Next rotation: `recording_20250603_143155.wav`

## Integration in Processing Graphs

The RecordNode can be placed anywhere in the processing pipeline:

### Recording Input Audio
```yaml
connections:
  - from: "audio_input"
    to: "recorder"
  - from: "recorder" 
    to: "filter"
```

### Recording Filtered Audio
```yaml
connections:
  - from: "audio_input"
    to: "filter"
  - from: "filter"
    to: "recorder"
  - from: "recorder"
    to: "output"
```

### Multiple Recording Points
```yaml
connections:
  - from: "audio_input"
    to: "raw_recorder"      # Record original
  - from: "raw_recorder"
    to: "filter"
  - from: "filter"
    to: "filtered_recorder" # Record filtered
  - from: "filtered_recorder"
    to: "output"
```

## Audio Format Details

- **Input**: Accepts `ProcessingData::SingleChannel` and `ProcessingData::DualChannel`
- **Output**: Creates standard WAV files with 16-bit PCM encoding
- **Sample rate**: Preserves the original sample rate from input data
- **Channels**: Mono for SingleChannel, stereo for DualChannel
- **Conversion**: f32 samples are scaled and clipped to i16 range

## Error Handling

The RecordNode is designed for robust operation:

- **Recording failures**: Logged but don't interrupt data flow
- **File system issues**: Gracefully handled with error logging
- **Invalid parameters**: Validation during node creation
- **Memory management**: Efficient WAV writer with proper cleanup

## Performance Considerations

- **Memory usage**: WAV writer buffers data efficiently
- **File I/O**: Asynchronous operations don't block processing
- **CPU impact**: Minimal overhead for format conversion
- **Storage**: Monitor disk space with large file limits

## Common Use Cases

1. **Debug Recording**: Capture audio at specific pipeline points for analysis
2. **Data Collection**: Record streams for offline processing or training
3. **Quality Assurance**: Monitor audio quality at different processing stages
4. **Backup**: Create backup recordings during real-time processing
5. **Analysis**: Record data for spectral analysis or visualization

## Troubleshooting

### File Not Created
- Check file path permissions
- Verify directory exists
- Check disk space availability

### Large File Sizes
- Reduce `max_size` parameter
- Enable `auto_delete` to manage storage
- Monitor recording duration

### Audio Quality Issues
- Verify input sample rate compatibility
- Check for clipping in f32 to i16 conversion
- Ensure adequate file system performance

## Related Nodes

- **InputNode**: Provides audio data for recording
- **FilterNode**: Can be recorded before/after filtering
- **ChannelSelectorNode**: Record specific channels
- **DifferentialNode**: Record differential analysis results

## Implementation Notes

The RecordNode implements the `ProcessingNode` trait with these key methods:

- `process()`: Records data and returns it unchanged
- `node_id()`: Returns the configured node identifier  
- `node_type()`: Returns "record"
- `accepts_input()`: Accepts SingleChannel and DualChannel data
- `output_type()`: Returns same type as input

The node uses the `hound` crate for efficient WAV file writing and includes comprehensive error handling for production use.
