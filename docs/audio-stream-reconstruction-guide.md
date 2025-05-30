# Audio Stream Reconstruction Guide for React TypeScript Developers

## Overview

This guide explains how real-time audio data is received from a server, reconstructed into playable audio buffers, and made available through Web Audio API context nodes in a React TypeScript application. The system processes Server-Sent Events (SSE) containing audio frame data and reconstructs them into a continuous audio stream using the Web Audio API.

## Architecture Overview

```
Server → SSE Stream → Frame Parser → Audio Queue → Web Audio Graph → Audio Context Node
```

The audio reconstruction pipeline consists of several key components:

1. **Server-Sent Events Stream**: Real-time data transport
2. **Frame Processing**: Parsing and validation of incoming audio data
3. **Audio Queue Management**: Buffering and ordering of audio frames
4. **Web Audio Graph**: Audio processing pipeline using Web Audio API
5. **Audio Context Node**: Final output node for audio visualization and analysis

## Data Structure

### Audio Frame Format

Each incoming audio frame from the server contains:

```typescript
interface AudioFrame {
  channel_a: number[]; // Left channel samples (32-bit float array)
  channel_b: number[]; // Right channel samples (32-bit float array)
  sample_rate: number; // Sample rate in Hz (e.g., 44100, 48000)
  timestamp: number; // Server timestamp when frame was created
  frame_number: number; // Sequential frame identifier
  duration_ms: number; // Frame duration in milliseconds
}
```

### Audio Stream Node Structure

The reconstructed audio is available through an audio processing graph:

```typescript
interface AudioStreamNode {
  context: AudioContext; // Main audio context
  sourceNode: AudioBufferSourceNode | null; // Dynamic source for audio buffers
  gainNode: GainNode; // Volume control
  analyserNode: AnalyserNode; // Frequency analysis and visualization
  outputNode: AudioNode; // Final output node (analyser)
}
```

## Step-by-Step Reconstruction Process

### 1. Server-Sent Events Connection

The system establishes an authenticated SSE connection to receive real-time audio data:

```typescript
// Connection setup with authentication
const response = await fetch(`${baseUrl}/stream/audio`, {
  method: "GET",
  headers: {
    Accept: "text/event-stream",
    "Cache-Control": "no-cache",
    Authorization: `Bearer ${accessToken}`,
  },
  signal: abortController.signal,
});

// Stream processing
const reader = response.body.getReader();
const decoder = new TextDecoder();
```

### 2. Frame Parsing and Validation

Incoming SSE data is parsed and validated:

```typescript
const processServerSentEvent = (line: string) => {
  // Parse SSE format: "data: {json}"
  if (line.startsWith("data:")) {
    const data = line.replace(/^data:\s*/, "");

    // Skip heartbeats
    if (data === '{"type":"heartbeat"}') return;

    // Parse audio frame
    const frame: AudioFrame = JSON.parse(data);

    // Validate required fields
    if (
      frame.frame_number !== undefined &&
      frame.channel_a &&
      frame.channel_b &&
      frame.sample_rate
    ) {
      queueAudioFrame(frame);
    }
  }
};
```

### 3. Audio Context Initialization

The Web Audio API context is initialized with dynamic sample rate configuration:

```typescript
const initializeAudio = async () => {
  // Create audio context with dynamic sample rate
  const context = new AudioContext({
    sampleRate: sampleRate, // From incoming frames
    latencyHint: "interactive",
  });

  // Create audio processing graph
  const gainNode = context.createGain();
  const analyserNode = context.createAnalyser();

  // Configure analyser for visualization
  analyserNode.fftSize = 2048;
  analyserNode.smoothingTimeConstant = 0.8;

  // Connect audio graph (no output to speakers)
  gainNode.connect(analyserNode);

  const streamNode = {
    context,
    sourceNode: null,
    gainNode,
    analyserNode,
    outputNode: analyserNode,
  };

  return streamNode;
};
```

### 4. Audio Buffer Creation

Each audio frame is converted to a Web Audio API AudioBuffer:

```typescript
const createAudioBuffer = (frame: AudioFrame): AudioBuffer | null => {
  try {
    // Create stereo buffer with frame's sample rate
    const buffer = audioContext.createBuffer(
      2, // Stereo (2 channels)
      frame.channel_a.length, // Sample count
      frame.sample_rate // Sample rate
    );

    // Fill channel data
    const channelAData = buffer.getChannelData(0);
    const channelBData = buffer.getChannelData(1);

    for (let i = 0; i < frame.channel_a.length; i++) {
      channelAData[i] = frame.channel_a[i];
      channelBData[i] = frame.channel_b[i];
    }

    return buffer;
  } catch (err) {
    console.error("Failed to create audio buffer:", err);
    return null;
  }
};
```

### 5. Sequential Playback Scheduling

Audio buffers are scheduled for sequential playback to maintain continuity:

```typescript
const scheduleAudioBuffer = (buffer: AudioBuffer) => {
  // Create buffer source node
  const sourceNode = audioContext.createBufferSource();
  sourceNode.buffer = buffer;

  // Connect to audio graph
  sourceNode.connect(audioStreamNode.gainNode);

  // Calculate precise timing for seamless playback
  const currentTime = audioContext.currentTime;
  const scheduledTime = Math.max(currentTime, nextPlayTime);

  // Schedule playback
  sourceNode.start(scheduledTime);

  // Update next play time for seamless continuation
  nextPlayTime = scheduledTime + buffer.duration;

  // Cleanup after playback
  sourceNode.onended = () => {
    sourceNode.disconnect();
  };
};
```

### 6. Queue Management

A queue system manages incoming frames and prevents memory overflow:

```typescript
const queueAudioFrame = (frame: AudioFrame) => {
  // Update sample rate dynamically
  if (frame.sample_rate !== currentSampleRate) {
    currentSampleRate = frame.sample_rate;
    // May trigger audio context reinitialization
  }

  // Add to processing queue
  audioBufferQueue.push(frame);

  // Prevent memory overflow
  if (audioBufferQueue.length > MAX_QUEUE_SIZE) {
    audioBufferQueue.shift(); // Drop oldest frame
    droppedFrames++;
  }

  // Process queue
  processAudioQueue();
};

const processAudioQueue = () => {
  while (audioBufferQueue.length > 0) {
    const frame = audioBufferQueue.shift();
    const buffer = createAudioBuffer(frame);
    if (buffer) {
      scheduleAudioBuffer(buffer);
    }
  }
};
```

## Accessing the Audio Context Node

### Using the Hook

```typescript
import { useAudioStream } from "@/hooks/useAudioStream";

const AudioVisualizationComponent = () => {
  const {
    audioContext,
    audioStreamNode,
    isAudioReady,
    currentFrame,
    frameCount,
    fps,
    connect,
    disconnect,
  } = useAudioStream("https://api.example.com", true);

  // Access the audio analysis node for visualization
  useEffect(() => {
    if (isAudioReady && audioStreamNode) {
      const analyser = audioStreamNode.analyserNode;

      // Setup for frequency analysis
      const bufferLength = analyser.frequencyBinCount;
      const dataArray = new Uint8Array(bufferLength);

      const updateVisualization = () => {
        analyser.getByteFrequencyData(dataArray);
        // Process frequency data for visualization
        requestAnimationFrame(updateVisualization);
      };

      updateVisualization();
    }
  }, [isAudioReady, audioStreamNode]);

  return (
    <div>
      <div>Status: {isAudioReady ? "Ready" : "Not Ready"}</div>
      <div>Frames: {frameCount}</div>
      <div>FPS: {fps}</div>
      <button onClick={connect}>Connect</button>
      <button onClick={disconnect}>Disconnect</button>
    </div>
  );
};
```

### Audio Analysis and Visualization

The audio context node provides access to real-time frequency and time-domain data:

```typescript
// Frequency domain analysis
const analyser = audioStreamNode.analyserNode;
const bufferLength = analyser.frequencyBinCount;
const frequencyData = new Uint8Array(bufferLength);

// Get frequency data
analyser.getByteFrequencyData(frequencyData);

// Time domain analysis
const timeDomainData = new Uint8Array(bufferLength);
analyser.getByteTimeDomainData(timeDomainData);

// FFT configuration
analyser.fftSize = 2048; // Frequency resolution
analyser.smoothingTimeConstant = 0.8; // Temporal smoothing
```

## Key Features

### Real-time Processing

- **Low Latency**: Interactive latency hint for minimal delay
- **Seamless Playback**: Precise timing ensures no gaps between frames
- **Dynamic Sample Rate**: Adapts to changing audio characteristics

### Error Handling and Resilience

- **Automatic Reconnection**: Progressive backoff strategy for connection failures
- **Frame Validation**: Comprehensive validation of incoming audio data
- **Queue Management**: Prevents memory overflow with configurable limits

### Performance Optimization

- **Efficient Memory Usage**: Automatic cleanup of processed audio buffers
- **Frame Dropping**: Prevents buffer overflow by dropping oldest frames
- **Adaptive Processing**: Processes frames only when audio context is ready

## Integration Patterns

### With Audio Visualization Libraries

```typescript
// Integration with visualization libraries
const useAudioVisualization = (audioStreamNode: AudioStreamNode | null) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (!audioStreamNode || !canvasRef.current) return;

    const canvas = canvasRef.current;
    const ctx = canvas.getContext("2d");
    const analyser = audioStreamNode.analyserNode;

    const render = () => {
      const bufferLength = analyser.frequencyBinCount;
      const dataArray = new Uint8Array(bufferLength);
      analyser.getByteFrequencyData(dataArray);

      // Clear canvas
      ctx.clearRect(0, 0, canvas.width, canvas.height);

      // Draw frequency bars
      const barWidth = canvas.width / bufferLength;
      for (let i = 0; i < bufferLength; i++) {
        const barHeight = (dataArray[i] / 255) * canvas.height;
        ctx.fillRect(
          i * barWidth,
          canvas.height - barHeight,
          barWidth,
          barHeight
        );
      }

      requestAnimationFrame(render);
    };

    render();
  }, [audioStreamNode]);

  return canvasRef;
};
```

### With Audio Processing Effects

```typescript
// Adding audio effects to the processing chain
const addAudioEffects = (audioStreamNode: AudioStreamNode) => {
  const context = audioStreamNode.context;

  // Create effect nodes
  const reverbNode = context.createConvolver();
  const filterNode = context.createBiquadFilter();

  // Configure effects
  filterNode.type = "lowpass";
  filterNode.frequency.value = 1000;

  // Insert into audio graph
  audioStreamNode.gainNode.disconnect();
  audioStreamNode.gainNode.connect(filterNode);
  filterNode.connect(reverbNode);
  reverbNode.connect(audioStreamNode.analyserNode);
};
```

## Best Practices

### 1. Resource Management

- Always clean up audio contexts when components unmount
- Use proper dependency arrays in useEffect hooks
- Implement proper error boundaries for audio failures

### 2. Performance Considerations

- Monitor frame drop rates and adjust queue sizes accordingly
- Use appropriate FFT sizes for your visualization needs
- Consider using Web Workers for heavy audio processing

### 3. User Experience

- Provide clear visual feedback for connection status
- Implement graceful degradation for audio API failures
- Allow users to control audio processing parameters

### 4. Development Tips

- Use browser developer tools to monitor audio performance
- Test with different sample rates and frame sizes
- Implement comprehensive logging for debugging

## Troubleshooting Common Issues

### Audio Context Suspension

```typescript
// Handle audio context suspension (browser autoplay policy)
const resumeAudioContext = async () => {
  if (audioContext.state === "suspended") {
    await audioContext.resume();
  }
};

// Call on user interaction
document.addEventListener("click", resumeAudioContext, { once: true });
```

### Sample Rate Mismatches

```typescript
// Handle dynamic sample rate changes
useEffect(() => {
  if (currentFrame && currentFrame.sample_rate !== audioContext.sampleRate) {
    console.warn("Sample rate mismatch detected, reinitializing audio context");
    initializeAudio();
  }
}, [currentFrame?.sample_rate]);
```

### Memory Leaks

```typescript
// Proper cleanup pattern
useEffect(() => {
  return () => {
    // Cleanup audio resources
    if (audioContext && audioContext.state !== "closed") {
      audioContext.close();
    }

    // Clear references
    audioBufferQueue.length = 0;
  };
}, []);
```

This guide provides a comprehensive overview of the audio stream reconstruction process, enabling React TypeScript developers to understand and effectively utilize the audio context nodes for real-time audio processing and visualization applications.
