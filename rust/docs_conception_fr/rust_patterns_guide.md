# Patterns Rust Avancés dans rust-photoacoustic

## Guide des Bonnes Pratiques et Patterns de Concurrence

### Vue d'ensemble

Cette documentation détaille les patterns Rust avancés utilisés dans le projet rust-photoacoustic, avec un focus sur la programmation asynchrone, la gestion de la mémoire partagée, et les patterns de concurrence pour le streaming temps réel.

## Table des Matières

1. [Patterns de Propriété et Borrowing](#patterns-de-propriété-et-borrowing)
2. [Patterns de Concurrence Asynchrone](#patterns-de-concurrence-asynchrone)
3. [Patterns de Communication Inter-Tâches](#patterns-de-communication-inter-tâches)
4. [Patterns de Gestion d'État](#patterns-de-gestion-détat)
5. [Patterns d'Erreur et Récupération](#patterns-derreur-et-récupération)
6. [Patterns de Types et Traits](#patterns-de-types-et-traits)
7. [Patterns de Performance](#patterns-de-performance)
8. [Anti-Patterns à Éviter](#anti-patterns-à-éviter)

---

## Patterns de Propriété et Borrowing

### 1. Arc (Atomic Reference Counting) Pattern

**Cas d'usage**: Partage de données entre plusieurs tâches asynchrones

```rust
// ✅ Pattern Arc pour partage thread-safe
use std::sync::Arc;

pub struct Daemon {
    // Stream partagé entre daemon et consumers
    audio_stream: Option<Arc<SharedAudioStream>>,
    // Flag partagé pour contrôle d'arrêt
    running: Arc<AtomicBool>,
}

impl Daemon {
    pub fn get_audio_stream(&self) -> Option<Arc<SharedAudioStream>> {
        // Clonage léger d'Arc (juste un compteur de références)
        self.audio_stream.clone()
    }

    pub fn start_background_task(&self) {
        let running = self.running.clone();
        let stream = self.audio_stream.clone();

        tokio::spawn(async move {
            while running.load(Ordering::Relaxed) {
                if let Some(ref stream) = stream {
                    // Utilisation du stream dans la tâche
                    process_stream(stream).await;
                }
            }
        });
    }
}
```

**Avantages**:

- Partage thread-safe sans copies coûteuses
- Gestion automatique de la mémoire (drop quand refcount = 0)
- Composition avec d'autres smart pointers (`Arc<RwLock<T>>`)

### 2. Pattern RAII (Resource Acquisition Is Initialization)

```rust
pub struct AcquisitionDaemon {
    audio_source: Box<dyn AudioSource>,
    stream: SharedAudioStream,
    running: Arc<AtomicBool>,
}

impl Drop for AcquisitionDaemon {
    fn drop(&mut self) {
        // Nettoyage automatique lors de la destruction
        self.running.store(false, Ordering::Relaxed);
        log::info!("AcquisitionDaemon dropped, resources cleaned up");
    }
}
```

### 3. Pattern Borrowed Data vs Owned Data

```rust
// ✅ Bon: Interface flexible qui accepte les deux
pub fn process_audio_data<T: AsRef<[f32]>>(data: T) -> Result<Vec<f32>> {
    let data_slice = data.as_ref();
    // Traitement...
    Ok(filtered_data)
}

// Usage:
let owned_data = vec![1.0, 2.0, 3.0];
let borrowed_data = &[1.0, 2.0, 3.0];

process_audio_data(owned_data)?;     // Prend possession
process_audio_data(borrowed_data)?;  // Emprunte seulement
process_audio_data(&owned_data)?;    // Emprunte depuis owned
```

---

## Patterns de Concurrence Asynchrone

### 1. Producer-Consumer Pattern avec Broadcast

```rust
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct SharedAudioStream {
    sender: broadcast::Sender<AudioFrame>,
    latest_frame: Arc<RwLock<Option<AudioFrame>>>,
}

impl SharedAudioStream {
    // Producer: Un seul producteur
    pub async fn publish(&self, frame: AudioFrame) -> Result<()> {
        // Mise à jour atomique de la dernière frame
        *self.latest_frame.write().await = Some(frame.clone());

        // Diffusion vers tous les consumers
        match self.sender.send(frame) {
            Ok(_) => Ok(()),
            Err(broadcast::error::SendError(_)) => {
                // Pas de consumers actifs - comportement normal
                Ok(())
            }
        }
    }

    // Consumer factory: Multiples consumers possibles
    pub fn subscribe(&self) -> broadcast::Receiver<AudioFrame> {
        self.sender.subscribe()
    }
}

// Usage pattern
async fn consumer_task(mut receiver: broadcast::Receiver<AudioFrame>) {
    while let Ok(frame) = receiver.recv().await {
        match process_frame(frame).await {
            Ok(_) => continue,
            Err(e) => {
                log::error!("Frame processing failed: {}", e);
                // Décision: continue ou break selon la criticité
                continue;
            }
        }
    }
}
```

### 2. Supervision Pattern pour Tâches

```rust
pub struct TaskSupervisor {
    tasks: Vec<JoinHandle<Result<()>>>,
    running: Arc<AtomicBool>,
}

impl TaskSupervisor {
    pub fn spawn_supervised<F, Fut>(&mut self, name: &str, f: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>> + Send,
    {
        let running = self.running.clone();
        let task_name = name.to_string();

        let handle = tokio::spawn(async move {
            log::info!("Starting supervised task: {}", task_name);

            let result = f().await;

            match &result {
                Ok(_) => log::info!("Task {} completed successfully", task_name),
                Err(e) => log::error!("Task {} failed: {}", task_name, e),
            }

            // Signaler l'arrêt si une tâche critique échoue
            if result.is_err() {
                running.store(false, Ordering::Relaxed);
            }

            result
        });

        self.tasks.push(handle);
    }

    pub async fn shutdown_all(&mut self) -> Result<()> {
        self.running.store(false, Ordering::Relaxed);

        for task in self.tasks.drain(..) {
            match tokio::time::timeout(Duration::from_secs(5), task).await {
                Ok(Ok(_)) => log::debug!("Task shutdown cleanly"),
                Ok(Err(e)) => log::warn!("Task shutdown with error: {}", e),
                Err(_) => log::warn!("Task shutdown timeout"),
            }
        }

        Ok(())
    }
}
```

### 3. Rate Limiting Pattern

```rust
pub struct RateLimitedProcessor {
    target_fps: f64,
    last_process_time: Instant,
}

impl RateLimitedProcessor {
    pub async fn process_with_rate_limit<F, Fut, T>(&mut self, f: F) -> Option<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let frame_duration = Duration::from_secs_f64(1.0 / self.target_fps);
        let elapsed = self.last_process_time.elapsed();

        if elapsed < frame_duration {
            tokio::time::sleep(frame_duration - elapsed).await;
        }

        let result = f().await;
        self.last_process_time = Instant::now();

        Some(result)
    }
}

// Usage dans AcquisitionDaemon
impl AcquisitionDaemon {
    pub async fn start(&mut self) -> Result<()> {
        let mut rate_limiter = RateLimitedProcessor::new(self.target_fps);

        while self.running.load(Ordering::Relaxed) {
            rate_limiter.process_with_rate_limit(|| async {
                self.read_and_publish_frame().await
            }).await;
        }

        Ok(())
    }
}
```

---

## Patterns de Communication Inter-Tâches

### 1. Message Passing Pattern

```rust
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum DaemonCommand {
    Start,
    Stop,
    ChangeTargetFps(f64),
    GetStats { reply: oneshot::Sender<StreamStats> },
}

pub struct ControllableDaemon {
    command_tx: mpsc::UnboundedSender<DaemonCommand>,
    task_handle: Option<JoinHandle<Result<()>>>,
}

impl ControllableDaemon {
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        let task_handle = tokio::spawn(Self::daemon_loop(command_rx));

        Self {
            command_tx,
            task_handle: Some(task_handle),
        }
    }

    async fn daemon_loop(mut command_rx: mpsc::UnboundedReceiver<DaemonCommand>) -> Result<()> {
        let mut daemon_state = DaemonState::new();

        while let Some(command) = command_rx.recv().await {
            match command {
                DaemonCommand::Start => {
                    daemon_state.start().await?;
                },
                DaemonCommand::Stop => {
                    daemon_state.stop().await?;
                },
                DaemonCommand::ChangeTargetFps(fps) => {
                    daemon_state.set_target_fps(fps);
                },
                DaemonCommand::GetStats { reply } => {
                    let stats = daemon_state.get_stats().await;
                    let _ = reply.send(stats); // Ignore si receiver dropped
                },
            }
        }

        Ok(())
    }

    // Interface publique
    pub async fn start(&self) -> Result<()> {
        self.command_tx.send(DaemonCommand::Start)
            .map_err(|_| anyhow!("Daemon task has stopped"))?;
        Ok(())
    }

    pub async fn get_stats(&self) -> Result<StreamStats> {
        let (reply_tx, reply_rx) = oneshot::channel();

        self.command_tx.send(DaemonCommand::GetStats { reply: reply_tx })
            .map_err(|_| anyhow!("Daemon task has stopped"))?;

        reply_rx.await
            .map_err(|_| anyhow!("Failed to receive stats"))
    }
}
```

### 2. Event Bus Pattern

```rust
use std::collections::HashMap;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum SystemEvent {
    AudioStreamStarted,
    AudioStreamStopped,
    FrameProcessed { frame_number: u64 },
    ErrorOccurred { error: String },
    ConfigChanged,
}

pub struct EventBus {
    channels: HashMap<String, broadcast::Sender<SystemEvent>>,
}

impl EventBus {
    pub fn subscribe(&mut self, topic: &str) -> broadcast::Receiver<SystemEvent> {
        let sender = self.channels
            .entry(topic.to_string())
            .or_insert_with(|| broadcast::channel(1000).0);

        sender.subscribe()
    }

    pub fn publish(&self, topic: &str, event: SystemEvent) {
        if let Some(sender) = self.channels.get(topic) {
            let _ = sender.send(event); // Ignore si pas de subscribers
        }
    }
}

// Usage global avec lazy_static ou once_cell
static EVENT_BUS: Lazy<Mutex<EventBus>> = Lazy::new(|| {
    Mutex::new(EventBus::new())
});

pub fn global_event_bus() -> &'static Mutex<EventBus> {
    &EVENT_BUS
}
```

---

## Patterns de Gestion d'État

### 1. State Machine Pattern

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DaemonState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

pub struct StatefulDaemon {
    state: DaemonState,
    audio_source: Option<Box<dyn AudioSource>>,
    stream: Option<SharedAudioStream>,
}

impl StatefulDaemon {
    pub async fn transition_to(&mut self, new_state: DaemonState) -> Result<()> {
        let current = self.state;

        match (current, new_state) {
            (DaemonState::Stopped, DaemonState::Starting) => {
                log::info!("Daemon starting...");
                self.initialize_resources().await?;
                self.state = DaemonState::Starting;
            },
            (DaemonState::Starting, DaemonState::Running) => {
                log::info!("Daemon now running");
                self.start_processing_loop().await?;
                self.state = DaemonState::Running;
            },
            (DaemonState::Running, DaemonState::Stopping) => {
                log::info!("Daemon stopping...");
                self.cleanup_resources().await?;
                self.state = DaemonState::Stopping;
            },
            (DaemonState::Stopping, DaemonState::Stopped) => {
                log::info!("Daemon stopped");
                self.state = DaemonState::Stopped;
            },
            (_, DaemonState::Error) => {
                log::error!("Daemon entered error state from {:?}", current);
                self.state = DaemonState::Error;
            },
            (current, new) => {
                return Err(anyhow!("Invalid state transition: {:?} -> {:?}", current, new));
            }
        }

        Ok(())
    }

    pub fn can_transition_to(&self, new_state: DaemonState) -> bool {
        matches!(
            (self.state, new_state),
            (DaemonState::Stopped, DaemonState::Starting) |
            (DaemonState::Starting, DaemonState::Running) |
            (DaemonState::Running, DaemonState::Stopping) |
            (DaemonState::Stopping, DaemonState::Stopped) |
            (_, DaemonState::Error)
        )
    }
}
```

### 2. Configuration Pattern avec Validation

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    pub target_fps: f64,
    pub buffer_size: usize,
    pub timeout_secs: u64,
}

impl StreamingConfig {
    pub fn validate(&self) -> Result<()> {
        if self.target_fps <= 0.0 || self.target_fps > 1000.0 {
            return Err(anyhow!("target_fps must be between 0 and 1000"));
        }

        if self.buffer_size == 0 || self.buffer_size > 10000 {
            return Err(anyhow!("buffer_size must be between 1 and 10000"));
        }

        if self.timeout_secs == 0 || self.timeout_secs > 300 {
            return Err(anyhow!("timeout_secs must be between 1 and 300"));
        }

        Ok(())
    }

    pub fn with_validation(self) -> Result<ValidatedConfig> {
        self.validate()?;
        Ok(ValidatedConfig(self))
    }
}

// Type wrapper pour config validée
pub struct ValidatedConfig(StreamingConfig);

impl ValidatedConfig {
    pub fn into_inner(self) -> StreamingConfig {
        self.0
    }
}

impl AsRef<StreamingConfig> for ValidatedConfig {
    fn as_ref(&self) -> &StreamingConfig {
        &self.0
    }
}
```

---

## Patterns d'Erreur et Récupération

### 1. Error Chain Pattern

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AudioStreamingError {
    #[error("Audio source error: {0}")]
    AudioSource(#[from] AudioSourceError),

    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Stream closed")]
    StreamClosed,

    #[error("Timeout after {timeout_secs}s")]
    Timeout { timeout_secs: u64 },

    #[error("Configuration error: {message}")]
    Configuration { message: String },
}

// Usage avec context
impl AcquisitionDaemon {
    async fn read_and_publish_frame(&mut self) -> Result<bool, AudioStreamingError> {
        let (channel_a, channel_b) = self.audio_source.read_frame()
            .context("Failed to read frame from audio source")?;

        if channel_a.is_empty() {
            return Ok(false);
        }

        let frame = AudioFrame::new(channel_a, channel_b, self.sample_rate, self.frame_counter);

        self.stream.publish(frame).await
            .context("Failed to publish frame to stream")?;

        Ok(true)
    }
}
```

### 2. Circuit Breaker Pattern

```rust
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub enum CircuitBreakerState {
    Closed,   // Normal operation
    Open,     // Failing fast
    HalfOpen, // Testing if service recovered
}

pub struct CircuitBreaker {
    state: CircuitBreakerState,
    failure_count: u32,
    failure_threshold: u32,
    timeout: Duration,
    last_failure_time: Option<Instant>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, timeout: Duration) -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            failure_threshold,
            timeout,
            last_failure_time: None,
        }
    }

    pub async fn call<F, Fut, T, E>(&mut self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        match self.state {
            CircuitBreakerState::Open => {
                if let Some(last_failure) = self.last_failure_time {
                    if last_failure.elapsed() > self.timeout {
                        self.state = CircuitBreakerState::HalfOpen;
                    } else {
                        return Err(CircuitBreakerError::CircuitOpen);
                    }
                }
            },
            CircuitBreakerState::HalfOpen => {
                // Une seule tentative en mode half-open
            },
            CircuitBreakerState::Closed => {
                // Operation normale
            },
        }

        match f().await {
            Ok(result) => {
                // Succès: reset circuit breaker
                self.failure_count = 0;
                self.state = CircuitBreakerState::Closed;
                self.last_failure_time = None;
                Ok(result)
            },
            Err(e) => {
                // Échec: increment failure count
                self.failure_count += 1;
                self.last_failure_time = Some(Instant::now());

                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitBreakerState::Open;
                }

                Err(CircuitBreakerError::Operation(e))
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum CircuitBreakerError<E> {
    #[error("Circuit breaker is open")]
    CircuitOpen,
    #[error("Operation failed: {0}")]
    Operation(E),
}
```

### 3. Retry Pattern avec Backoff

```rust
use tokio::time::{sleep, Duration};

pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }
}

pub async fn retry_with_backoff<F, Fut, T, E>(
    config: RetryConfig,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt = 0;
    let mut delay = config.base_delay;

    loop {
        attempt += 1;

        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt >= config.max_attempts {
                    log::error!("All retry attempts exhausted. Last error: {}", e);
                    return Err(e);
                }

                log::warn!("Attempt {} failed: {}. Retrying in {:?}", attempt, e, delay);
                sleep(delay).await;

                // Exponential backoff
                delay = std::cmp::min(
                    Duration::from_millis((delay.as_millis() as f64 * config.backoff_multiplier) as u64),
                    config.max_delay,
                );
            }
        }
    }
}

// Usage
impl AudioSource for MicrophoneSource {
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)> {
        retry_with_backoff(RetryConfig::default(), || async {
            self.try_read_frame()
        }).await
    }
}
```

---

## Patterns de Types et Traits

### 1. Builder Pattern pour Configuration Complexe

```rust
#[derive(Debug, Clone)]
pub struct AcquisitionDaemonBuilder {
    audio_source: Option<Box<dyn AudioSource>>,
    target_fps: Option<f64>,
    buffer_size: Option<usize>,
    error_handler: Option<Box<dyn Fn(AudioStreamingError) + Send + Sync>>,
}

impl AcquisitionDaemonBuilder {
    pub fn new() -> Self {
        Self {
            audio_source: None,
            target_fps: None,
            buffer_size: None,
            error_handler: None,
        }
    }

    pub fn with_audio_source(mut self, source: Box<dyn AudioSource>) -> Self {
        self.audio_source = Some(source);
        self
    }

    pub fn with_target_fps(mut self, fps: f64) -> Self {
        self.target_fps = Some(fps);
        self
    }

    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = Some(size);
        self
    }

    pub fn with_error_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(AudioStreamingError) + Send + Sync + 'static,
    {
        self.error_handler = Some(Box::new(handler));
        self
    }

    pub fn build(self) -> Result<AcquisitionDaemon> {
        let audio_source = self.audio_source
            .ok_or_else(|| anyhow!("Audio source is required"))?;

        let target_fps = self.target_fps.unwrap_or(30.0);
        let buffer_size = self.buffer_size.unwrap_or(100);

        Ok(AcquisitionDaemon::new(audio_source, target_fps, buffer_size))
    }
}

// Usage
let daemon = AcquisitionDaemonBuilder::new()
    .with_audio_source(Box::new(MicrophoneSource::new("default")?))
    .with_target_fps(60.0)
    .with_buffer_size(500)
    .with_error_handler(|e| log::error!("Daemon error: {}", e))
    .build()?;
```

### 2. Extension Trait Pattern

```rust
// Extension trait pour AudioFrame
pub trait AudioFrameExt {
    fn is_silent(&self, threshold: f32) -> bool;
    fn normalize(&mut self);
    fn apply_window(&mut self, window_type: WindowType);
    fn peak_amplitude(&self) -> f32;
}

impl AudioFrameExt for AudioFrame {
    fn is_silent(&self, threshold: f32) -> bool {
        let max_a = self.channel_a.iter().map(|x| x.abs()).fold(0.0, f32::max);
        let max_b = self.channel_b.iter().map(|x| x.abs()).fold(0.0, f32::max);

        max_a < threshold && max_b < threshold
    }

    fn normalize(&mut self) {
        let max_amplitude = self.peak_amplitude();
        if max_amplitude > 0.0 {
            let factor = 1.0 / max_amplitude;

            for sample in &mut self.channel_a {
                *sample *= factor;
            }
            for sample in &mut self.channel_b {
                *sample *= factor;
            }
        }
    }

    fn apply_window(&mut self, window_type: WindowType) {
        let window = create_window(window_type, self.channel_a.len());

        for (sample, &window_val) in self.channel_a.iter_mut().zip(&window) {
            *sample *= window_val;
        }
        for (sample, &window_val) in self.channel_b.iter_mut().zip(&window) {
            *sample *= window_val;
        }
    }

    fn peak_amplitude(&self) -> f32 {
        let max_a = self.channel_a.iter().map(|x| x.abs()).fold(0.0, f32::max);
        let max_b = self.channel_b.iter().map(|x| x.abs()).fold(0.0, f32::max);

        max_a.max(max_b)
    }
}

// Usage
let mut frame = AudioFrame::new(channel_a, channel_b, 48000, 42);

if !frame.is_silent(0.001) {
    frame.apply_window(WindowType::Hann);
    frame.normalize();

    log::info!("Processing frame with peak amplitude: {}", frame.peak_amplitude());
}
```

### 3. Type State Pattern

```rust
// États pour le type state pattern
pub struct Uninitialized;
pub struct Configured;
pub struct Running;

pub struct AudioProcessor<State> {
    config: Option<ProcessingConfig>,
    state: PhantomData<State>,
}

impl AudioProcessor<Uninitialized> {
    pub fn new() -> Self {
        Self {
            config: None,
            state: PhantomData,
        }
    }

    pub fn configure(self, config: ProcessingConfig) -> AudioProcessor<Configured> {
        AudioProcessor {
            config: Some(config),
            state: PhantomData,
        }
    }
}

impl AudioProcessor<Configured> {
    pub fn start(self) -> Result<AudioProcessor<Running>> {
        let config = self.config.unwrap(); // Safe car on est dans l'état Configured

        // Initialisation des ressources
        initialize_audio_resources(&config)?;

        Ok(AudioProcessor {
            config: Some(config),
            state: PhantomData,
        })
    }
}

impl AudioProcessor<Running> {
    pub fn process_frame(&self, frame: AudioFrame) -> Result<AudioFrame> {
        let config = self.config.as_ref().unwrap();
        // Traitement avec garantie que le processeur est configuré et démarré
        process_with_config(frame, config)
    }

    pub fn stop(self) -> AudioProcessor<Configured> {
        // Nettoyage des ressources
        cleanup_audio_resources();

        AudioProcessor {
            config: self.config,
            state: PhantomData,
        }
    }
}

// Usage avec contraintes de type
let processor = AudioProcessor::new()
    .configure(config)?
    .start()?;

// Seuls les processeurs Running peuvent traiter des frames
let processed = processor.process_frame(frame)?;
```

---

## Patterns de Performance

### 1. Object Pool Pattern

```rust
use std::collections::VecDeque;
use tokio::sync::Mutex;

pub struct AudioFramePool {
    available: Mutex<VecDeque<AudioFrame>>,
    capacity: usize,
}

impl AudioFramePool {
    pub fn new(capacity: usize, frame_size: usize) -> Self {
        let mut pool = VecDeque::with_capacity(capacity);

        // Pré-allocation des frames
        for i in 0..capacity {
            let frame = AudioFrame::new(
                vec![0.0; frame_size],
                vec![0.0; frame_size],
                48000,
                i as u64,
            );
            pool.push_back(frame);
        }

        Self {
            available: Mutex::new(pool),
            capacity,
        }
    }

    pub async fn acquire(&self) -> Option<AudioFrame> {
        let mut available = self.available.lock().await;
        available.pop_front()
    }

    pub async fn release(&self, mut frame: AudioFrame) {
        // Reset du frame
        frame.channel_a.fill(0.0);
        frame.channel_b.fill(0.0);
        frame.frame_number = 0;

        let mut available = self.available.lock().await;
        if available.len() < self.capacity {
            available.push_back(frame);
        }
        // Si pool plein, on laisse le frame être drop
    }
}

// Usage avec RAII wrapper
pub struct PooledFrame {
    frame: Option<AudioFrame>,
    pool: Arc<AudioFramePool>,
}

impl PooledFrame {
    pub async fn new(pool: Arc<AudioFramePool>) -> Option<Self> {
        let frame = pool.acquire().await?;
        Some(Self {
            frame: Some(frame),
            pool,
        })
    }

    pub fn as_mut(&mut self) -> &mut AudioFrame {
        self.frame.as_mut().unwrap()
    }
}

impl Drop for PooledFrame {
    fn drop(&mut self) {
        if let Some(frame) = self.frame.take() {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                pool.release(frame).await;
            });
        }
    }
}
```

### 2. Zero-Copy Pattern

```rust
use bytes::{Bytes, BytesMut};

pub struct ZeroCopyAudioFrame {
    // Utilisation de Bytes pour partage zero-copy
    channel_a: Bytes,
    channel_b: Bytes,
    metadata: FrameMetadata,
}

impl ZeroCopyAudioFrame {
    pub fn from_raw_data(raw_data: Bytes, channels: usize, sample_rate: u32) -> Self {
        let samples_per_channel = raw_data.len() / (channels * 4); // 4 bytes par f32

        let mid_point = samples_per_channel * 4;

        // Split zero-copy des données
        let channel_a = raw_data.slice(0..mid_point);
        let channel_b = raw_data.slice(mid_point..);

        Self {
            channel_a,
            channel_b,
            metadata: FrameMetadata::new(sample_rate, samples_per_channel),
        }
    }

    pub fn channel_a_as_f32(&self) -> &[f32] {
        // Safe cast car on contrôle l'alignement
        unsafe {
            std::slice::from_raw_parts(
                self.channel_a.as_ptr() as *const f32,
                self.channel_a.len() / 4,
            )
        }
    }

    pub fn channel_b_as_f32(&self) -> &[f32] {
        unsafe {
            std::slice::from_raw_parts(
                self.channel_b.as_ptr() as *const f32,
                self.channel_b.len() / 4,
            )
        }
    }

    // Clone zero-copy (juste increment reference count)
    pub fn cheap_clone(&self) -> Self {
        Self {
            channel_a: self.channel_a.clone(),
            channel_b: self.channel_b.clone(),
            metadata: self.metadata,
        }
    }
}
```

### 3. SIMD Optimization Pattern

```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

pub trait AudioProcessing {
    fn apply_gain(&mut self, gain: f32);
    fn mix_with(&mut self, other: &[f32], mix_ratio: f32);
}

impl AudioProcessing for Vec<f32> {
    #[cfg(target_arch = "x86_64")]
    fn apply_gain(&mut self, gain: f32) {
        if is_x86_feature_detected!("avx2") {
            unsafe { self.apply_gain_avx2(gain) }
        } else if is_x86_feature_detected!("sse2") {
            unsafe { self.apply_gain_sse2(gain) }
        } else {
            self.apply_gain_scalar(gain)
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn apply_gain(&mut self, gain: f32) {
        self.apply_gain_scalar(gain)
    }

    fn mix_with(&mut self, other: &[f32], mix_ratio: f32) {
        for (dst, &src) in self.iter_mut().zip(other.iter()) {
            *dst = *dst * (1.0 - mix_ratio) + src * mix_ratio;
        }
    }
}

impl Vec<f32> {
    fn apply_gain_scalar(&mut self, gain: f32) {
        for sample in self.iter_mut() {
            *sample *= gain;
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "sse2")]
    unsafe fn apply_gain_sse2(&mut self, gain: f32) {
        let gain_vec = _mm_set1_ps(gain);
        let chunks = self.chunks_exact_mut(4);
        let remainder = chunks.into_remainder();

        for chunk in chunks {
            let data = _mm_loadu_ps(chunk.as_ptr());
            let result = _mm_mul_ps(data, gain_vec);
            _mm_storeu_ps(chunk.as_mut_ptr(), result);
        }

        // Traitement du reste en scalar
        remainder.apply_gain_scalar(gain);
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn apply_gain_avx2(&mut self, gain: f32) {
        let gain_vec = _mm256_set1_ps(gain);
        let chunks = self.chunks_exact_mut(8);
        let remainder = chunks.into_remainder();

        for chunk in chunks {
            let data = _mm256_loadu_ps(chunk.as_ptr());
            let result = _mm256_mul_ps(data, gain_vec);
            _mm256_storeu_ps(chunk.as_mut_ptr(), result);
        }

        remainder.apply_gain_scalar(gain);
    }
}
```

---

## Anti-Patterns à Éviter

### 1. ❌ Arc<Mutex<T>> Abuse

```rust
// ❌ Anti-pattern: Arc<Mutex<>> pour données read-heavy
pub struct BadSharedState {
    data: Arc<Mutex<Vec<f32>>>,
}

// ✅ Bon: RwLock pour lectures multiples
pub struct GoodSharedState {
    data: Arc<RwLock<Vec<f32>>>,
}

// ✅ Encore mieux: données immutables
pub struct BestSharedState {
    data: Arc<Vec<f32>>, // Immutable, pas de locking
}
```

### 2. ❌ Blocking dans Async Context

```rust
// ❌ Anti-pattern: blocking dans async
async fn bad_processing() -> Result<()> {
    let data = std::fs::read("large_file.dat")?; // Bloque le runtime!
    process_data(data).await
}

// ✅ Bon: async I/O
async fn good_processing() -> Result<()> {
    let data = tokio::fs::read("large_file.dat").await?;
    process_data(data).await
}

// ✅ Bon: spawn_blocking pour CPU intensive
async fn cpu_intensive_processing(data: Vec<f32>) -> Result<Vec<f32>> {
    tokio::task::spawn_blocking(move || {
        expensive_computation(data)
    }).await?
}
```

### 3. ❌ Unbounded Channel Abuse

```rust
// ❌ Anti-pattern: unbounded channels partout
let (tx, rx) = mpsc::unbounded_channel();

// ✅ Bon: bounded channels avec backpressure
let (tx, rx) = mpsc::channel(100); // Limite la mémoire

// ✅ Alternative: tokio broadcast pour fan-out
let (tx, _) = broadcast::channel(100);
```

### 4. ❌ Erreur de Clone Coûteux

```rust
// ❌ Anti-pattern: clone de grandes structures
async fn bad_frame_processing(frame: AudioFrame) {
    let frame_copy = frame.clone(); // Clone coûteux!
    process_frame(frame_copy).await;
}

// ✅ Bon: références ou Arc
async fn good_frame_processing(frame: &AudioFrame) {
    process_frame(frame).await;
}

// ✅ Bon: Arc pour partage
async fn shared_frame_processing(frame: Arc<AudioFrame>) {
    let frame_ref = frame.clone(); // Clone d'Arc, pas des données
    tokio::spawn(async move {
        process_frame(&frame_ref).await;
    });
}
```

### 5. ❌ Memory Leak avec Cyclic References

```rust
// ❌ Anti-pattern: références cycliques avec Arc
pub struct BadParent {
    children: Vec<Arc<BadChild>>,
}

pub struct BadChild {
    parent: Arc<BadParent>, // Cycle!
}

// ✅ Bon: Weak references pour breaking cycles
pub struct GoodParent {
    children: Vec<Arc<GoodChild>>,
}

pub struct GoodChild {
    parent: Weak<GoodParent>, // Break le cycle
}
```

---

Cette documentation couvre les patterns Rust essentiels utilisés dans le projet rust-photoacoustic. Ces patterns permettent une programmation asynchrone robuste, performante et thread-safe pour les applications de streaming audio temps réel.
