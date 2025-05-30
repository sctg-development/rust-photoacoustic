# Complete Guide to the AcquisitionDaemon

## Developer Documentation for the Audio Acquisition System

### Table of Contents

1. [Introduction](#introduction)
2. [Daemon Architecture](#daemon-architecture)
3. [Lifecycle and State Management](#lifecycle-and-state-management)
4. [Configuration and Parameters](#configuration-and-parameters)
5. [Integration with Audio Sources](#integration-with-audio-sources)
6. [Monitoring and Observability](#monitoring-and-observability)
7. [Performance Patterns](#performance-patterns)
8. [Troubleshooting](#troubleshooting)
9. [Practical Examples](#practical-examples)
10. [API Reference](#api-reference)

---

## Introduction

The `AcquisitionDaemon` is the core of the real-time audio acquisition system. It orchestrates the continuous reading of audio data from various sources (microphones, WAV files) and broadcasts them via a shared streaming system to web clients.

### Key Responsibilities

- **Continuous Acquisition**: Periodic reading of audio frames at a configurable rate
- **Rate Control**: Maintaining a constant FPS for real-time applications
- **Multi-Client Broadcasting**: Broadcasting to multiple simultaneous consumers
- **Error Handling**: Automatic recovery and logging of issues
- **Observability**: Performance metrics and statistics

### Main Use Cases

```rust
// Real-time photoacoustic spectroscopy
let daemon = AcquisitionDaemon::new(microphone_source, 30.0, 1024);

// Acquisition from file for testing
let daemon = AcquisitionDaemon::new(file_source, 44.1, 4096);

// Audio performance monitoring
let daemon = AcquisitionDaemon::new(device_source, 60.0, 512);
```

---

## Daemon Architecture

### Internal Structure

```rust
pub struct AcquisitionDaemon {
    audio_source: Box<dyn AudioSource>,     // Abstract audio source
    stream: SharedAudioStream,              // Broadcast hub
    running: Arc<AtomicBool>,               // Execution control
    frame_counter: Arc<AtomicU64>,          // Frame counter
    target_fps: f64,                        // Target rate
}
```

### Flow Diagram

```
┌───────────────────┐
│ AcquisitionDaemon │
│                   │
│ ┌───────────────┐ │    ┌─────────────────┐
│ │ Timer/Interval│ │──▶│ read_frame()    │
│ │  (target_fps) │ │    │ from AudioSource│
│ └───────────────┘ │    └─────────────────┘
│                   │              │
│ ┌───────────────┐ │              ▼
│ │ Error Handler │ │    ┌─────────────────┐
│ │  & Recovery   │ │◀───│ Process & Pack  │
│ └───────────────┘ │    │   AudioFrame    │
│                   │    └─────────────────┘
│ ┌───────────────┐ │              │
│ │ Frame Counter │ │              ▼
│ │  & Metrics    │ │    ┌──────────────────┐
│ └───────────────┘ │    │ Broadcast via    │
│                   │    │ SharedAudioStream│
└───────────────────┘    └──────────────────┘
```

### Daemon States

```
┌─────────┐ start() ┌─────────┐ read_frame()  ┌─────────┐
│ Created │────────▶│ Running │─────────────▶│ Active  │
└─────────┘         └─────────┘               └─────────┘
     ▲                   │                        │
     │                   │ stop()                 │ error
     │               ┌─────────┐                  │
     └───────────────│ Stopped │◀─────────────────┘
                     └─────────┘
```

---

## Lifecycle and State Management

### Creation and Initialization

```rust
impl AcquisitionDaemon {
    /// Creates a new acquisition daemon
    pub fn new(
        audio_source: Box<dyn AudioSource>,
        target_fps: f64,
        buffer_size: usize,
    ) -> Self {
        Self {
            audio_source,
            stream: SharedAudioStream::new(buffer_size),
            running: Arc::new(AtomicBool::new(false)),
            frame_counter: Arc::new(AtomicU64::new(0)),
            target_fps,
        }
    }
}
```

### Asynchronous Start

```rust
pub async fn start(&mut self) -> Result<()> {
    // State check
    if self.running.load(Ordering::Relaxed) {
        warn!("Daemon already running");
        return Ok(());
    }

    // Timing setup
    self.running.store(true, Ordering::Relaxed);
    let frame_duration = Duration::from_secs_f64(1.0 / self.target_fps);
    let mut interval = interval(frame_duration);

    info!("Starting acquisition daemon - Target FPS: {}", self.target_fps);

    // Main acquisition loop
    while self.running.load(Ordering::Relaxed) {
        interval.tick().await;

        match self.read_and_publish_frame().await {
            Ok(true) => {
                // Frame processed successfully
                self.update_metrics().await;
            }
            Ok(false) => {
                // No data available
                continue;
            }
            Err(e) => {
                error!("Acquisition error: {}", e);
                // Recovery strategy
                self.handle_error(&e).await?;
            }
        }
    }

    info!("Acquisition daemon stopped");
    Ok(())
}
```

### Graceful Stop

```rust
pub fn stop(&self) {
    info!("Stop requested for acquisition daemon");
    self.running.store(false, Ordering::Relaxed);
}

pub fn is_running(&self) -> bool {
    self.running.load(Ordering::Relaxed)
}
```

---

## Configuration and Parameters

### Performance Parameters

| Parameter     | Type    | Description            | Recommended Values          |
| ------------- | ------- | ---------------------- | --------------------------- |
| `target_fps`  | `f64`   | Frames per second rate | 30.0 - 60.0 for real-time   |
| `buffer_size` | `usize` | Broadcast buffer size  | 512 - 4096 depending on RAM |
| `window_size` | `u32`   | Spectral window size   | 1024, 2048, 4096            |
| `sample_rate` | `u32`   | Sampling frequency     | 44100, 48000 Hz             |

### Target FPS Calculation

```rust
// Formula: FPS = sample_rate / (window_size * channels * bytes_per_sample)
fn calculate_target_fps(config: &Config) -> f64 {
    let sample_rate = config.photoacoustic.sample_rate as f64;
    let window_size = config.photoacoustic.window_size as f64;
    let channels = 2.0; // Stereo
    let bytes_per_sample = (config.photoacoustic.precision as f64) / 8.0;

    sample_rate / (window_size * channels * bytes_per_sample)
}

// Example for 44.1kHz, window 4096, 16-bit, stereo
// FPS = 44100 / (4096 * 2 * 2) = 2.69 FPS
```

### Adaptive Configuration

```rust
impl AcquisitionDaemon {
    /// Adjusts parameters based on system load
    pub fn adjust_performance(&mut self, cpu_usage: f64, memory_usage: f64) {
        if cpu_usage > 80.0 {
            self.target_fps *= 0.8; // Reduce by 20%
            warn!("FPS reduced due to CPU load: {:.1}", self.target_fps);
        }

        if memory_usage > 90.0 {
            // Reduce buffer size
            self.stream.resize_buffer(self.stream.capacity() / 2);
            warn!("Buffer reduced due to limited memory");
        }
    }
}
```

---

## Integration with Audio Sources

### AudioSource Interface

```rust
pub trait AudioSource: Send {
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)>;
    fn sample_rate(&self) -> u32;
}
```

### Microphone Implementation

```rust
use cpal::{Device, Stream, StreamConfig};

pub struct MicrophoneSource {
    device: Device,
    config: StreamConfig,
    buffer: Arc<Mutex<VecDeque<(Vec<f32>, Vec<f32>)>>>,
    stream: Option<Stream>,
}

impl AudioSource for MicrophoneSource {
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)> {
        let mut buffer = self.buffer.lock().unwrap();

        if let Some((ch_a, ch_b)) = buffer.pop_front() {
            Ok((ch_a, ch_b))
        } else {
            // No audio data available
            Err(anyhow!("No audio data available"))
        }
    }

    fn sample_rate(&self) -> u32 {
        self.config.sample_rate.0
    }
}
```

### WAV File Implementation

```rust
use hound::{WavReader, WavSpec};

pub struct FileSource {
    reader: WavReader<BufReader<File>>,
    spec: WavSpec,
    samples_per_frame: usize,
}

impl AudioSource for FileSource {
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)> {
        let mut channel_a = Vec::with_capacity(self.samples_per_frame);
        let mut channel_b = Vec::with_capacity(self.samples_per_frame);

        for _ in 0..self.samples_per_frame {
            match (self.reader.next(), self.reader.next()) {
                (Some(Ok(sample_a)), Some(Ok(sample_b))) => {
                    channel_a.push(sample_a as f32 / i16::MAX as f32);
                    channel_b.push(sample_b as f32 / i16::MAX as f32);
                }
                _ => break, // End of file or error
            }
        }

        if channel_a.is_empty() {
            Err(anyhow!("End of file reached"))
        } else {
            Ok((channel_a, channel_b))
        }
    }

    fn sample_rate(&self) -> u32 {
        self.spec.sample_rate
    }
}
```

---

## Monitoring and Observability

### Collected Metrics

```rust
#[derive(Debug, Clone, Serialize)]
pub struct AcquisitionMetrics {
    pub total_frames: u64,
    pub frames_per_second: f64,
    pub average_latency_ms: f64,
    pub error_count: u64,
    pub buffer_utilization: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
}

impl AcquisitionDaemon {
    pub async fn get_metrics(&self) -> AcquisitionMetrics {
        let stats = self.stream.get_stats().await;
        let frame_count = self.frame_counter.load(Ordering::Relaxed);

        AcquisitionMetrics {
            total_frames: frame_count,
            frames_per_second: stats.frames_per_second,
            average_latency_ms: stats.average_latency_ms,
            error_count: stats.error_count,
            buffer_utilization: stats.buffer_utilization_percent,
            memory_usage_mb: get_memory_usage(),
            cpu_usage_percent: get_cpu_usage(),
        }
    }
}
```

### Structured Logging

```rust
use slog::{info, warn, error, debug};

impl AcquisitionDaemon {
    async fn log_metrics_periodically(&self) {
        let mut interval = interval(Duration::from_secs(10));

        while self.running.load(Ordering::Relaxed) {
            interval.tick().await;

            let metrics = self.get_metrics().await;

            info!(
                "Acquisition Stats";
                "frames" => metrics.total_frames,
                "fps" => metrics.frames_per_second,
                "latency_ms" => metrics.average_latency_ms,
                "cpu_percent" => metrics.cpu_usage_percent,
                "memory_mb" => metrics.memory_usage_mb
            );

            // Automatic alerts
            if metrics.cpu_usage_percent > 85.0 {
                warn!("High CPU usage: {:.1}%", metrics.cpu_usage_percent);
            }

            if metrics.average_latency_ms > 100.0 {
                warn!("High latency: {:.1}ms", metrics.average_latency_ms);
            }
        }
    }
}
```

### Prometheus Integration

```rust
use prometheus::{Counter, Gauge, Histogram, register_counter, register_gauge, register_histogram};

lazy_static! {
    static ref FRAMES_PROCESSED: Counter = register_counter!(
        "audio_frames_processed_total",
        "Total number of audio frames processed"
    ).unwrap();

    static ref CURRENT_FPS: Gauge = register_gauge!(
        "audio_acquisition_fps",
        "Current frames per second rate"
    ).unwrap();

    static ref LATENCY_HISTOGRAM: Histogram = register_histogram!(
        "audio_acquisition_latency_seconds",
        "Histogram of acquisition latency"
    ).unwrap();
}

impl AcquisitionDaemon {
    async fn update_prometheus_metrics(&self) {
        FRAMES_PROCESSED.inc();

        let metrics = self.get_metrics().await;
        CURRENT_FPS.set(metrics.frames_per_second);
        LATENCY_HISTOGRAM.observe(metrics.average_latency_ms / 1000.0);
    }
}
```

---

## Performance Patterns

### Optimized Audio Reading

```rust
impl AcquisitionDaemon {
    /// Optimized reading with circular buffer
    async fn read_and_publish_frame(&mut self) -> Result<bool> {
        // Timer to measure latency
        let start_time = Instant::now();

        // Non-blocking read with timeout
        let frame_data = match timeout(
            Duration::from_millis(50),
            self.read_frame_async()
        ).await {
            Ok(Ok(data)) => data,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                debug!("Frame read timeout, skip");
                return Ok(false);
            }
        };

        // Create frame with metadata
        let frame_number = self.frame_counter.fetch_add(1, Ordering::Relaxed);
        let frame = AudioFrame::new(
            frame_data.0,
            frame_data.1,
            self.audio_source.sample_rate(),
            frame_number,
        );

        // Non-blocking publish
        match self.stream.try_publish(frame) {
            Ok(_) => {
                let latency = start_time.elapsed();
                self.update_latency_metrics(latency).await;
                Ok(true)
            }
            Err(e) => {
                warn!("Frame publish failed: {}", e);
                Ok(false)
            }
        }
    }
}
```

### Advanced Memory Management

```rust
use std::alloc::{alloc, dealloc, Layout};

/// Pool of pre-allocated buffers to reduce allocations
pub struct AudioBufferPool {
    buffers: Vec<Vec<f32>>,
    available: VecDeque<usize>,
    buffer_size: usize,
}

impl AudioBufferPool {
    pub fn new(pool_size: usize, buffer_size: usize) -> Self {
        let mut buffers = Vec::with_capacity(pool_size);
        let mut available = VecDeque::with_capacity(pool_size);

        for i in 0..pool_size {
            buffers.push(vec![0.0; buffer_size]);
            available.push_back(i);
        }

        Self { buffers, available, buffer_size }
    }

    pub fn get_buffer(&mut self) -> Option<Vec<f32>> {
        self.available.pop_front().map(|idx| {
            std::mem::replace(&mut self.buffers[idx], Vec::new())
        })
    }

    pub fn return_buffer(&mut self, mut buffer: Vec<f32>) {
        if buffer.capacity() >= self.buffer_size {
            buffer.clear();
            buffer.resize(self.buffer_size, 0.0);

            if let Some(available_idx) = self.available.back() {
                self.buffers[*available_idx] = buffer;
            }
        }
    }
}
```

### Parallel Channel Processing

```rust
use rayon::prelude::*;

impl AcquisitionDaemon {
    /// Parallel processing of audio channels
    async fn process_channels_parallel(
        &self,
        channel_a: Vec<f32>,
        channel_b: Vec<f32>
    ) -> Result<(Vec<f32>, Vec<f32>)> {

        let (processed_a, processed_b) = tokio::task::spawn_blocking(move || {
            // Parallel processing with rayon
            let proc_a = channel_a.par_iter()
                .map(|&sample| self.apply_filters(sample))
                .collect::<Vec<f32>>();

            let proc_b = channel_b.par_iter()
                .map(|&sample| self.apply_filters(sample))
                .collect::<Vec<f32>>();

            (proc_a, proc_b)
        }).await?;

        Ok((processed_a, processed_b))
    }

    fn apply_filters(&self, sample: f32) -> f32 {
        // Optimized DSP filters
        sample * 0.95 // Simple example
    }
}
```

---

## Troubleshooting

### Common Issues

#### 1. High Latency

**Symptoms:**

- Noticeable delay between acquisition and broadcast
- `average_latency_ms > 100ms` metrics

**Diagnosis:**

```rust
// Check bottlenecks
async fn diagnose_latency(&self) -> LatencyReport {
    let start = Instant::now();

    // Source read test
    let read_time = {
        let t = Instant::now();
        self.audio_source.read_frame()?;
        t.elapsed()
    };

    // Publish test
    let publish_time = {
        let t = Instant::now();
        self.stream.try_publish(test_frame)?;
        t.elapsed()
    };

    LatencyReport {
        total_latency: start.elapsed(),
        read_latency: read_time,
        publish_latency: publish_time,
        queue_depth: self.stream.queue_depth(),
    }
}
```

**Solutions:**

- Reduce `target_fps`
- Increase `buffer_size`
- Use pre-allocated buffers
- Optimize DSP filters

#### 2. Frame Loss

**Symptoms:**

- "Frame publish failed" errors
- Data discontinuities

**Diagnosis:**

```rust
#[derive(Debug)]
pub struct FrameLossReport {
    pub total_frames: u64,
    pub dropped_frames: u64,
    pub loss_percentage: f64,
    pub buffer_overruns: u64,
}

impl AcquisitionDaemon {
    pub fn analyze_frame_loss(&self) -> FrameLossReport {
        let stats = self.stream.get_stats().await;
        let total = self.frame_counter.load(Ordering::Relaxed);
        let dropped = stats.dropped_frames;

        FrameLossReport {
            total_frames: total,
            dropped_frames: dropped,
            loss_percentage: (dropped as f64 / total as f64) * 100.0,
            buffer_overruns: stats.buffer_overruns,
        }
    }
}
```

#### 3. Excessive Memory Consumption

**Diagnosis:**

```rust
use sysinfo::{System, SystemExt, ProcessExt};

fn monitor_memory_usage() -> MemoryReport {
    let mut system = System::new_all();
    system.refresh_all();

    let process = system.process(sysinfo::get_current_pid().unwrap()).unwrap();

    MemoryReport {
        virtual_memory: process.virtual_memory(),
        physical_memory: process.memory(),
        heap_size: get_heap_size(),
        buffer_memory: calculate_buffer_memory(),
    }
}
```

### Debug Tools

```rust
#[cfg(debug_assertions)]
impl AcquisitionDaemon {
    /// Debug mode with detailed logging
    pub fn enable_debug_mode(&mut self) {
        self.debug_mode = true;
        info!("Debug mode enabled for AcquisitionDaemon");
    }

    async fn debug_log_frame(&self, frame: &AudioFrame) {
        if self.debug_mode {
            debug!(
                "Frame {}: {} samples, {:.2}ms duration",
                frame.frame_number,
                frame.channel_a.len(),
                frame.duration_ms()
            );

            // Sample statistics
            let avg_a = frame.channel_a.iter().sum::<f32>() / frame.channel_a.len() as f32;
            let avg_b = frame.channel_b.iter().sum::<f32>() / frame.channel_b.len() as f32;

            debug!("Averages: A={:.4}, B={:.4}", avg_a, avg_b);
        }
    }
}
```

---

## Practical Examples

### Example 1: Basic Configuration

```rust
use rust_photoacoustic::acquisition::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Log configuration
    env_logger::init();

    // Create audio source from microphone
    let audio_source = get_default_audio_source()?;

    // Daemon configuration
    let mut daemon = AcquisitionDaemon::new(
        audio_source,
        30.0,    // 30 FPS
        1024     // Buffer 1024 frames
    );

    // Asynchronous start
    let daemon_handle = tokio::spawn(async move {
        daemon.start().await
    });

    // Create a consumer
    let mut consumer = AudioStreamConsumer::new(daemon.get_stream());

    // Read a few frames
    for i in 0..10 {
        if let Some(frame) = consumer.next_frame().await? {
            println!("Frame {}: {} samples", i, frame.channel_a.len());
        }
    }

    // Graceful stop
    daemon.stop();
    daemon_handle.await??;

    Ok(())
}
```

### Example 2: Integration with Rocket

```rust
use rocket::{State, get, routes};
use rust_photoacoustic::acquisition::*;

#[get("/audio/stats")]
async fn get_audio_stats(
    daemon: &State<Arc<AcquisitionDaemon>>
) -> Json<AcquisitionMetrics> {
    Json(daemon.get_metrics().await)
}

#[get("/audio/stream")]
async fn audio_stream_sse(
    stream: &State<Arc<SharedAudioStream>>
) -> EventStream![Event + '_] {
    let mut consumer = AudioStreamConsumer::new(stream.as_ref());

    EventStream! {
        loop {
            match consumer.next_frame().await {
                Ok(Some(frame)) => {
                    yield Event::json(&frame);
                }
                Ok(None) => {
                    yield Event::data("heartbeat");
                }
                Err(_) => break,
            }
        }
    }
}

#[rocket::launch]
fn rocket() -> _ {
    // Daemon configuration
    let daemon = Arc::new(AcquisitionDaemon::new(
        get_default_audio_source().unwrap(),
        44.1,
        2048
    ));

    let stream = daemon.get_stream().clone();

    // Start daemon in background
    let daemon_clone = daemon.clone();
    tokio::spawn(async move {
        daemon_clone.start().await.unwrap();
    });

    rocket::build()
        .manage(daemon)
        .manage(Arc::new(stream))
        .mount("/api", routes![get_audio_stats, audio_stream_sse])
}
```

### Example 3: Processing Pipeline

```rust
use rust_photoacoustic::dsp::*;

pub struct AudioProcessor {
    daemon: AcquisitionDaemon,
    fft_processor: FFTProcessor,
    spectrum_analyzer: SpectrumAnalyzer,
}

impl AudioProcessor {
    pub async fn process_realtime(&mut self) -> Result<()> {
        let mut consumer = AudioStreamConsumer::new(self.daemon.get_stream());

        while let Some(frame) = consumer.next_frame().await? {
            // Processing pipeline
            let windowed = self.apply_window(&frame)?;
            let spectrum = self.fft_processor.transform(&windowed)?;
            let analysis = self.spectrum_analyzer.analyze(&spectrum)?;

            // Publish results
            self.publish_analysis(analysis).await?;
        }

        Ok(())
    }

    fn apply_window(&self, frame: &AudioFrame) -> Result<AudioFrame> {
        // Apply Hanning window
        let windowed_a = apply_hanning_window(&frame.channel_a);
        let windowed_b = apply_hanning_window(&frame.channel_b);

        Ok(AudioFrame::new(
            windowed_a,
            windowed_b,
            frame.sample_rate,
            frame.frame_number
        ))
    }
}
```

---

## API Reference

### AcquisitionDaemon Structure

```rust
impl AcquisitionDaemon {
    /// Constructor
    pub fn new(
        audio_source: Box<dyn AudioSource>,
        target_fps: f64,
        buffer_size: usize,
    ) -> Self;

    /// Starts acquisition asynchronously
    pub async fn start(&mut self) -> Result<()>;

    /// Stops acquisition
    pub fn stop(&self);

    /// Checks if the daemon is active
    pub fn is_running(&self) -> bool;

    /// Returns the number of processed frames
    pub fn frame_count(&self) -> u64;

    /// Access to the shared stream
    pub fn get_stream(&self) -> &SharedAudioStream;

    /// Performance metrics
    pub async fn get_metrics(&self) -> AcquisitionMetrics;

    /// Dynamic configuration
    pub fn set_target_fps(&mut self, fps: f64);
    pub fn resize_buffer(&mut self, new_size: usize);
}
```

### Specific Errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum AcquisitionError {
    #[error("Audio source unavailable: {0}")]
    AudioSourceUnavailable(String),

    #[error("Buffer overflow: {frames} frames lost")]
    BufferOverflow { frames: u64 },

    #[error("Acquisition timeout: {timeout_ms}ms")]
    AcquisitionTimeout { timeout_ms: u64 },

    #[error("Invalid configuration: {reason}")]
    InvalidConfiguration { reason: String },

    #[error("System error: {0}")]
    SystemError(#[from] std::io::Error),
}
```

---

## Conclusion

The `AcquisitionDaemon` provides a robust infrastructure for real-time audio acquisition with:

- **Performance**: Optimizations for low latency and high throughput
- **Reliability**: Error handling and automatic recovery
- **Observability**: Detailed metrics and structured logging
- **Flexibility**: Multiple source support and dynamic configuration
- **Scalability**: Efficient multi-consumer architecture

For specific questions or implementation issues, refer to the practical examples or troubleshooting guides in this documentation.
