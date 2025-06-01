# Guide Complet de l'AcquisitionDaemon

## Documentation Développeur pour le Système d'Acquisition Audio

### Table des Matières

1. [Introduction](#introduction)
2. [Architecture du Daemon](#architecture-du-daemon)
3. [Lifecycle et Gestion d'État](#lifecycle-et-gestion-détat)
4. [Configuration et Paramétrage](#configuration-et-paramétrage)
5. [Intégration avec les Sources Audio](#intégration-avec-les-sources-audio)
6. [Monitoring et Observabilité](#monitoring-et-observabilité)
7. [Patterns de Performance](#patterns-de-performance)
8. [Troubleshooting](#troubleshooting)
9. [Exemples Pratiques](#exemples-pratiques)
10. [API Reference](#api-reference)

---

## Introduction

L'`AcquisitionDaemon` est le cœur du système d'acquisition audio temps réel. Il orchestre la lecture continue de données audio depuis diverses sources (microphones, fichiers WAV) et les diffuse via un système de streaming partagé vers les clients web.

### Responsabilités Clés

- **Acquisition Continue**: Lecture périodique des frames audio à un débit configurable
- **Contrôle de Débit**: Maintien d'un FPS constant pour les applications temps réel
- **Diffusion Multi-Clients**: Broadcasting vers plusieurs consommateurs simultanés
- **Gestion d'Erreurs**: Récupération automatique et logging des problèmes
- **Observabilité**: Métriques et statistiques de performance

### Cas d'Usage Principaux

```rust
// Spectroscopie photoacoustique en temps réel
let daemon = AcquisitionDaemon::new(microphone_source, 30.0, 1024);

// Acquisition depuis fichier pour tests
let daemon = AcquisitionDaemon::new(file_source, 44.1, 4096);

// Monitoring de performance audio
let daemon = AcquisitionDaemon::new(device_source, 60.0, 512);
```

---

## Architecture du Daemon

### Structure Interne

```rust
pub struct AcquisitionDaemon {
    audio_source: Box<dyn AudioSource>,     // Source audio abstraite
    stream: SharedAudioStream,              // Hub de diffusion
    running: Arc<AtomicBool>,              // Contrôle d'exécution
    frame_counter: Arc<AtomicU64>,         // Compteur de frames
    target_fps: f64,                       // Débit cible
}
```

### Diagramme de Flow

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

### États du Daemon

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

## Lifecycle et Gestion d'État

### Création et Initialisation

```rust
impl AcquisitionDaemon {
    /// Crée un nouveau daemon d'acquisition
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

### Démarrage Asynchrone

```rust
pub async fn start(&mut self) -> Result<()> {
    // Vérification d'état
    if self.running.load(Ordering::Relaxed) {
        warn!("Daemon déjà en cours d'exécution");
        return Ok(());
    }

    // Configuration du timing
    self.running.store(true, Ordering::Relaxed);
    let frame_duration = Duration::from_secs_f64(1.0 / self.target_fps);
    let mut interval = interval(frame_duration);

    info!("Démarrage daemon acquisition - FPS cible: {}", self.target_fps);

    // Boucle principale d'acquisition
    while self.running.load(Ordering::Relaxed) {
        interval.tick().await;

        match self.read_and_publish_frame().await {
            Ok(true) => {
                // Frame traitée avec succès
                self.update_metrics().await;
            }
            Ok(false) => {
                // Pas de données disponibles
                continue;
            }
            Err(e) => {
                error!("Erreur acquisition: {}", e);
                // Stratégie de récupération
                self.handle_error(&e).await?;
            }
        }
    }

    info!("Daemon d'acquisition arrêté");
    Ok(())
}
```

### Arrêt Gracieux

```rust
pub fn stop(&self) {
    info!("Arrêt demandé pour le daemon d'acquisition");
    self.running.store(false, Ordering::Relaxed);
}

pub fn is_running(&self) -> bool {
    self.running.load(Ordering::Relaxed)
}
```

---

## Configuration et Paramétrage

### Paramètres de Performance

| Paramètre     | Type    | Description                 | Valeurs Recommandées        |
| ------------- | ------- | --------------------------- | --------------------------- |
| `target_fps`  | `f64`   | Débit de frames par seconde | 30.0 - 60.0 pour temps réel |
| `buffer_size` | `usize` | Taille du buffer broadcast  | 512 - 4096 selon mémoire    |
| `frame_size`  | `u32`   | Taille fenêtre spectrale    | 1024, 2048, 4096            |
| `sample_rate` | `u32`   | Fréquence d'échantillonnage | 44100, 48000 Hz             |

### Calcul du Target FPS

```rust
// Formule : FPS = sample_rate / (frame_size * channels * bytes_per_sample)
fn calculate_target_fps(config: &Config) -> f64 {
    let sample_rate = config.photoacoustic.sample_rate as f64;
    let frame_size = config.photoacoustic.frame_size as f64;
    let channels = 2.0; // Stéréo
    let bytes_per_sample = (config.photoacoustic.precision as f64) / 8.0;

    sample_rate / (frame_size * channels * bytes_per_sample)
}

// Exemple pour 44.1kHz, window 4096, 16-bit, stéréo
// FPS = 44100 / (4096 * 2 * 2) = 2.69 FPS
```

### Configuration Adaptative

```rust
impl AcquisitionDaemon {
    /// Ajuste les paramètres en fonction de la charge système
    pub fn adjust_performance(&mut self, cpu_usage: f64, memory_usage: f64) {
        if cpu_usage > 80.0 {
            self.target_fps *= 0.8; // Réduction de 20%
            warn!("Réduction FPS due à charge CPU: {:.1}", self.target_fps);
        }

        if memory_usage > 90.0 {
            // Réduction taille buffer
            self.stream.resize_buffer(self.stream.capacity() / 2);
            warn!("Réduction buffer due à mémoire limitée");
        }
    }
}
```

---

## Intégration avec les Sources Audio

### Interface AudioSource

```rust
pub trait AudioSource: Send {
    fn read_frame(&mut self) -> Result<(Vec<f32>, Vec<f32>)>;
    fn sample_rate(&self) -> u32;
}
```

### Implémentation pour Microphone

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
            // Aucune donnée disponible
            Err(anyhow!("Aucune donnée audio disponible"))
        }
    }

    fn sample_rate(&self) -> u32 {
        self.config.sample_rate.0
    }
}
```

### Implémentation pour Fichier WAV

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
                _ => break, // Fin de fichier ou erreur
            }
        }

        if channel_a.is_empty() {
            Err(anyhow!("Fin de fichier atteinte"))
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

## Monitoring et Observabilité

### Métriques Collectées

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

### Logging Structuré

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

            // Alertes automatiques
            if metrics.cpu_usage_percent > 85.0 {
                warn!("CPU usage élevé: {:.1}%", metrics.cpu_usage_percent);
            }

            if metrics.average_latency_ms > 100.0 {
                warn!("Latence élevée: {:.1}ms", metrics.average_latency_ms);
            }
        }
    }
}
```

### Intégration avec Prometheus

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

## Patterns de Performance

### Optimisation de la Lecture Audio

```rust
impl AcquisitionDaemon {
    /// Lecture optimisée avec buffer circulaire
    async fn read_and_publish_frame(&mut self) -> Result<bool> {
        // Timer pour mesurer latence
        let start_time = Instant::now();

        // Lecture non-bloquante avec timeout
        let frame_data = match timeout(
            Duration::from_millis(50),
            self.read_frame_async()
        ).await {
            Ok(Ok(data)) => data,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                debug!("Timeout lecture frame, skip");
                return Ok(false);
            }
        };

        // Création frame avec métadonnées
        let frame_number = self.frame_counter.fetch_add(1, Ordering::Relaxed);
        let frame = AudioFrame::new(
            frame_data.0,
            frame_data.1,
            self.audio_source.sample_rate(),
            frame_number,
        );

        // Publication non-bloquante
        match self.stream.try_publish(frame) {
            Ok(_) => {
                let latency = start_time.elapsed();
                self.update_latency_metrics(latency).await;
                Ok(true)
            }
            Err(e) => {
                warn!("Échec publication frame: {}", e);
                Ok(false)
            }
        }
    }
}
```

### Gestion Mémoire Avancée

```rust
use std::alloc::{alloc, dealloc, Layout};

/// Pool de buffers pré-alloués pour réduire les allocations
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

### Parallélisation des Traitements

```rust
use rayon::prelude::*;

impl AcquisitionDaemon {
    /// Traitement parallèle des canaux audio
    async fn process_channels_parallel(
        &self,
        channel_a: Vec<f32>,
        channel_b: Vec<f32>
    ) -> Result<(Vec<f32>, Vec<f32>)> {

        let (processed_a, processed_b) = tokio::task::spawn_blocking(move || {
            // Traitement parallèle avec rayon
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
        // Filtres DSP optimisés
        sample * 0.95 // Exemple simple
    }
}
```

---

## Troubleshooting

### Problèmes Courants

#### 1. Latence Élevée

**Symptômes:**

- Délai perceptible entre acquisition et diffusion
- Métriques `average_latency_ms > 100ms`

**Diagnostic:**

```rust
// Vérifier les goulets d'étranglement
async fn diagnose_latency(&self) -> LatencyReport {
    let start = Instant::now();

    // Test lecture source
    let read_time = {
        let t = Instant::now();
        self.audio_source.read_frame()?;
        t.elapsed()
    };

    // Test publication
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

- Réduire `target_fps`
- Augmenter `buffer_size`
- Utiliser buffers pré-alloués
- Optimiser les filtres DSP

#### 2. Perte de Frames

**Symptômes:**

- Erreurs "Échec publication frame"
- Discontinuités dans les données

**Diagnostic:**

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

#### 3. Consommation Mémoire Excessive

**Diagnostic:**

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

### Outils de Debug

```rust
#[cfg(debug_assertions)]
impl AcquisitionDaemon {
    /// Mode debug avec logging détaillé
    pub fn enable_debug_mode(&mut self) {
        self.debug_mode = true;
        info!("Mode debug activé pour AcquisitionDaemon");
    }

    async fn debug_log_frame(&self, frame: &AudioFrame) {
        if self.debug_mode {
            debug!(
                "Frame {}: {} samples, {:.2}ms duration",
                frame.frame_number,
                frame.channel_a.len(),
                frame.duration_ms()
            );

            // Statistiques des échantillons
            let avg_a = frame.channel_a.iter().sum::<f32>() / frame.channel_a.len() as f32;
            let avg_b = frame.channel_b.iter().sum::<f32>() / frame.channel_b.len() as f32;

            debug!("Moyennes: A={:.4}, B={:.4}", avg_a, avg_b);
        }
    }
}
```

---

## Exemples Pratiques

### Exemple 1: Configuration de Base

```rust
use rust_photoacoustic::acquisition::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Configuration des logs
    env_logger::init();

    // Création source audio depuis microphone
    let audio_source = get_default_audio_source()?;

    // Configuration daemon
    let mut daemon = AcquisitionDaemon::new(
        audio_source,
        30.0,    // 30 FPS
        1024     // Buffer 1024 frames
    );

    // Démarrage asynchrone
    let daemon_handle = tokio::spawn(async move {
        daemon.start().await
    });

    // Création d'un consommateur
    let mut consumer = AudioStreamConsumer::new(daemon.get_stream());

    // Lecture de quelques frames
    for i in 0..10 {
        if let Some(frame) = consumer.next_frame().await? {
            println!("Frame {}: {} échantillons", i, frame.channel_a.len());
        }
    }

    // Arrêt gracieux
    daemon.stop();
    daemon_handle.await??;

    Ok(())
}
```

### Exemple 2: Intégration avec Rocket

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
    // Configuration daemon
    let daemon = Arc::new(AcquisitionDaemon::new(
        get_default_audio_source().unwrap(),
        44.1,
        2048
    ));

    let stream = daemon.get_stream().clone();

    // Démarrage daemon en arrière-plan
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

### Exemple 3: Processing Pipeline

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
            // Pipeline de traitement
            let windowed = self.apply_window(&frame)?;
            let spectrum = self.fft_processor.transform(&windowed)?;
            let analysis = self.spectrum_analyzer.analyze(&spectrum)?;

            // Publication des résultats
            self.publish_analysis(analysis).await?;
        }

        Ok(())
    }

    fn apply_window(&self, frame: &AudioFrame) -> Result<AudioFrame> {
        // Application fenêtre de Hanning
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

### Structure AcquisitionDaemon

```rust
impl AcquisitionDaemon {
    /// Constructeur
    pub fn new(
        audio_source: Box<dyn AudioSource>,
        target_fps: f64,
        buffer_size: usize,
    ) -> Self;

    /// Démarre l'acquisition en mode asynchrone
    pub async fn start(&mut self) -> Result<()>;

    /// Arrête l'acquisition
    pub fn stop(&self);

    /// Vérifie si le daemon est actif
    pub fn is_running(&self) -> bool;

    /// Retourne le nombre de frames traitées
    pub fn frame_count(&self) -> u64;

    /// Accès au stream partagé
    pub fn get_stream(&self) -> &SharedAudioStream;

    /// Métriques de performance
    pub async fn get_metrics(&self) -> AcquisitionMetrics;

    /// Configuration dynamique
    pub fn set_target_fps(&mut self, fps: f64);
    pub fn resize_buffer(&mut self, new_size: usize);
}
```

### Erreurs Spécifiques

```rust
#[derive(Debug, thiserror::Error)]
pub enum AcquisitionError {
    #[error("Source audio non disponible: {0}")]
    AudioSourceUnavailable(String),

    #[error("Dépassement de buffer: {frames} frames perdues")]
    BufferOverflow { frames: u64 },

    #[error("Timeout acquisition: {timeout_ms}ms")]
    AcquisitionTimeout { timeout_ms: u64 },

    #[error("Configuration invalide: {reason}")]
    InvalidConfiguration { reason: String },

    #[error("Erreur système: {0}")]
    SystemError(#[from] std::io::Error),
}
```

---

## Conclusion

L'`AcquisitionDaemon` fournit une infrastructure robuste pour l'acquisition audio temps réel avec:

- **Performance**: Optimisations pour faible latence et haut débit
- **Fiabilité**: Gestion d'erreurs et récupération automatique
- **Observabilité**: Métriques détaillées et logging structuré
- **Flexibilité**: Support multiple sources et configuration dynamique
- **Scalabilité**: Architecture multi-consommateurs efficace

Pour des questions spécifiques ou des problèmes d'implémentation, consultez les exemples pratiques ou les guides de troubleshooting de cette documentation.
