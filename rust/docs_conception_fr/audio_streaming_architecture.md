# Architecture de Streaming Audio Temps Réel

## Documentation Technique pour Développeurs Rust

### Vue d'ensemble

Cette documentation détaille l'architecture du système de streaming audio temps réel du projet rust-photoacoustic, spécialement conçue pour la spectroscopie photoacoustique. Le système permet l'acquisition continue de données audio depuis des microphones ou des fichiers, et leur diffusion en temps réel vers des clients web via des endpoints SSE (Server-Sent Events).

## Table des Matières

1. [Architecture Globale](#architecture-globale)
2. [Module AcquisitionDaemon](#module-acquisitiondaemon)
3. [Module Audio Streaming](#module-audio-streaming)
4. [Infrastructure de Streaming Partagé](#infrastructure-de-streaming-partagé)
5. [Intégration avec Rocket](#intégration-avec-rocket)
6. [Patterns de Concurrence](#patterns-de-concurrence)
7. [Gestion des Erreurs](#gestion-des-erreurs)
8. [Tests et Validation](#tests-et-validation)
9. [Exemples d'Utilisation](#exemples-dutilisation)
10. [Bonnes Pratiques](#bonnes-pratiques)

---

## Architecture Globale

Le système de streaming audio repose sur une architecture producteur-consommateur asynchrone utilisant des canaux Tokio broadcast pour la diffusion temps réel.

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   AudioSource   │───▶│AcquisitionDaemon│───▶│SharedAudioStream│
│  (Microphone/   │    │                 │    │                 │
│     File)       │    │   Target FPS    │    │  Broadcast      │
└─────────────────┘    │   Frame Buffer  │    │   Channel       │
                       └─────────────────┘    └─────────────────┘
                                                        │
                                                        ▼
                       ┌─────────────────┐    ┌─────────────────┐
                       │   Web Clients   │◀───│ Audio Streaming │
                       │   (SSE/JSON)    │    │   Endpoints     │
                       └─────────────────┘    └─────────────────┘
```

### Composants Principaux

- **AudioSource**: Interface trait pour sources audio (microphone, fichier)
- **AcquisitionDaemon**: Daemon d'acquisition continue avec contrôle de débit
- **SharedAudioStream**: Hub de diffusion multi-consommateurs
- **Audio Streaming**: Endpoints HTTP/SSE pour clients web
- **AudioStreamConsumer**: Interface de consommation des données

---

## Module AcquisitionDaemon

### Structure et Responsabilités

Le `AcquisitionDaemon` est le cœur du système d'acquisition. Il gère l'acquisition continue des données audio avec un contrôle précis du débit et la publication vers le stream partagé.

```rust
pub struct AcquisitionDaemon {
    /// Source audio (microphone ou fichier)
    audio_source: Box<dyn AudioSource>,
    /// Stream partagé pour diffusion des frames
    stream: SharedAudioStream,
    /// Flag de contrôle d'exécution
    running: Arc<AtomicBool>,
    /// Compteur de frames
    frame_counter: Arc<AtomicU64>,
    /// Taux de frames par seconde cible
    target_fps: f64,
}
```

### Fonctionnalités Clés

#### 1. Contrôle de Débit Adaptatif

```rust
let frame_duration = Duration::from_secs_f64(1.0 / self.target_fps);
let mut interval = interval(frame_duration);

while self.running.load(Ordering::Relaxed) {
    interval.tick().await;
    // Traitement des frames...
}
```

Le daemon utilise `tokio::time::interval` pour maintenir un débit constant, calculé à partir du `target_fps` configuré.

#### 2. Gestion Asynchrone des Tâches

```rust
impl AcquisitionDaemon {
    pub async fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            warn!("Acquisition daemon is already running");
            return Ok(());
        }

        self.running.store(true, Ordering::Relaxed);
        info!("Starting acquisition daemon with target FPS: {}", self.target_fps);

        // Boucle principale d'acquisition
        while self.running.load(Ordering::Relaxed) {
            match self.read_and_publish_frame().await {
                Ok(true) => {
                    // Frame traitée avec succès
                    let frame_num = self.frame_counter.fetch_add(1, Ordering::Relaxed);

                    // Logging périodique des statistiques
                    if frame_num % 100 == 0 {
                        let stats = self.stream.get_stats().await;
                        debug!("Processed {} frames, {} subscribers",
                               stats.total_frames, stats.active_subscribers);
                    }
                },
                Ok(false) => {
                    // Fin de source (fichier terminé)
                    break;
                },
                Err(e) => {
                    error!("Error reading audio frame: {}", e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        Ok(())
    }
}
```

#### 3. Publication des Données

```rust
async fn read_and_publish_frame(&mut self) -> Result<bool> {
    match self.audio_source.read_frame() {
        Ok((channel_a, channel_b)) => {
            if channel_a.is_empty() {
                return Ok(false); // Fin de données
            }

            let frame_number = self.frame_counter.load(Ordering::Relaxed);
            let sample_rate = self.audio_source.sample_rate();

            let frame = AudioFrame::new(channel_a, channel_b, sample_rate, frame_number);

            // Publication vers le stream partagé
            self.stream.publish(frame).await?;
            Ok(true)
        },
        Err(e) => Err(e),
    }
}
```

### Patterns de Concurrence Utilisés

#### Atomic Operations pour l'État

- `Arc<AtomicBool>` pour le flag `running`
- `Arc<AtomicU64>` pour le compteur de frames
- Évite les mutex pour les opérations simples

#### Shared Ownership

- `Arc<SharedAudioStream>` permet le partage entre daemon et consommateurs
- Clonage léger des références Arc

---

## Module Audio Streaming

### Architecture des Endpoints

Le module `audio_streaming.rs` expose plusieurs endpoints HTTP utilisant le framework Rocket:

```rust
pub fn get_audio_streaming_routes() -> Vec<rocket::Route> {
    rocket::routes![
        get_stream_stats,    // GET /stream/stats
        get_latest_frame,    // GET /stream/latest
        stream_audio,        // GET /stream/audio (SSE)
        stream_spectral_analysis, // GET /stream/spectral (SSE)
    ]
}
```

### Endpoints de Streaming

#### 1. Stream Audio Temps Réel

```rust
#[get("/stream/audio")]
pub fn stream_audio(
    _user: AuthenticatedUser,
    stream_state: &State<AudioStreamState>,
) -> EventStream![Event] {
    let stream = stream_state.stream.clone();

    EventStream! {
        let mut consumer = AudioStreamConsumer::new(&stream);

        loop {
            match timeout(Duration::from_secs(5), consumer.next_frame()).await {
                Ok(Some(frame)) => {
                    let response = AudioFrameResponse::from(frame);
                    yield Event::json(&response);
                },
                Ok(None) => {
                    log::info!("Audio stream closed");
                    break;
                },
                Err(_) => {
                    // Timeout - heartbeat pour maintenir la connexion
                    yield Event::data(r#"{"type":"heartbeat"}"#);
                }
            }
        }
    }
}
```

**Caractéristiques:**

- **Server-Sent Events (SSE)**: Connexion persistante unidirectionnelle
- **Timeout avec Heartbeat**: Maintient la connexion active (5s timeout)
- **Authentification JWT**: Sécurisation via `AuthenticatedUser`
- **Gestion de Backpressure**: Le consumer peut "lag" si le client est lent

#### 2. Analyse Spectrale Temps Réel

```rust
#[get("/stream/spectral")]
pub fn stream_spectral_analysis(
    _user: AuthenticatedUser,
    stream_state: &State<AudioStreamState>,
) -> EventStream![Event] {
    let stream = stream_state.stream.clone();

    EventStream! {
        let mut consumer = AudioStreamConsumer::new(&stream);

        loop {
            match timeout(Duration::from_secs(5), consumer.next_frame()).await {
                Ok(Some(frame)) => {
                    let spectral_data = compute_spectral_analysis(&frame);
                    yield Event::json(&spectral_data);
                },
                Ok(None) => break,
                Err(_) => {
                    yield Event::data(r#"{"type":"heartbeat"}"#);
                }
            }
        }
    }
}
```

### Structures de Données

#### AudioFrameResponse

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFrameResponse {
    pub channel_a: Vec<f32>,
    pub channel_b: Vec<f32>,
    pub sample_rate: u32,
    pub timestamp: u64,
    pub frame_number: u64,
    pub duration_ms: f64,
}

impl From<AudioFrame> for AudioFrameResponse {
    fn from(frame: AudioFrame) -> Self {
        let duration_ms = frame.duration_ms();
        Self {
            channel_a: frame.channel_a,
            channel_b: frame.channel_b,
            sample_rate: frame.sample_rate,
            timestamp: frame.timestamp,
            frame_number: frame.frame_number,
            duration_ms,
        }
    }
}
```

#### SpectralDataResponse

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralDataResponse {
    pub frequencies: Vec<f32>,
    pub magnitude_a: Vec<f32>,
    pub magnitude_b: Vec<f32>,
    pub phase_a: Option<Vec<f32>>,
    pub phase_b: Option<Vec<f32>>,
    pub frame_number: u64,
    pub timestamp: u64,
    pub sample_rate: u32,
}
```

---

## Infrastructure de Streaming Partagé

### SharedAudioStream

Le `SharedAudioStream` implémente un pattern producteur-multiple consommateurs utilisant `tokio::broadcast`:

```rust
#[derive(Clone)]
pub struct SharedAudioStream {
    /// Canal broadcast pour streaming temps réel
    sender: broadcast::Sender<AudioFrame>,
    /// Dernière frame pour nouveaux abonnés
    latest_frame: Arc<RwLock<Option<AudioFrame>>>,
    /// Statistiques du stream
    stats: Arc<RwLock<StreamStats>>,
}
```

#### Fonctionnalités

##### 1. Publication Broadcast

```rust
pub async fn publish(&self, frame: AudioFrame) -> Result<()> {
    // Mise à jour de la dernière frame
    {
        let mut latest = self.latest_frame.write().await;
        *latest = Some(frame.clone());
    }

    // Mise à jour des statistiques
    {
        let mut stats = self.stats.write().await;
        stats.total_frames += 1;
        stats.active_subscribers = self.sender.receiver_count();

        // Calcul FPS
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if now - stats.last_update >= 1000 {
            let time_diff = (now - stats.last_update) as f64 / 1000.0;
            stats.fps = stats.total_frames as f64 / time_diff;
            stats.last_update = now;
        }
    }

    // Diffusion aux abonnés
    match self.sender.send(frame) {
        Ok(_) => Ok(()),
        Err(broadcast::error::SendError(_)) => {
            // Pas d'abonnés actifs - pas d'erreur
            Ok(())
        }
    }
}
```

##### 2. Abonnement de Consommateurs

```rust
pub fn subscribe(&self) -> broadcast::Receiver<AudioFrame> {
    self.sender.subscribe()
}
```

### AudioStreamConsumer

Interface de consommation avec gestion du lag:

```rust
pub struct AudioStreamConsumer {
    receiver: broadcast::Receiver<AudioFrame>,
    stream: SharedAudioStream,
}

impl AudioStreamConsumer {
    pub async fn next_frame(&mut self) -> Option<AudioFrame> {
        match self.receiver.recv().await {
            Ok(frame) => Some(frame),
            Err(broadcast::error::RecvError::Closed) => None,
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                log::warn!(
                    "Audio stream consumer lagged behind, skipped {} frames",
                    skipped
                );
                // Tentative de récupération de la frame suivante
                match self.receiver.recv().await {
                    Ok(frame) => Some(frame),
                    Err(_) => None,
                }
            }
        }
    }
}
```

---

## Intégration avec Rocket

### Configuration du Serveur

Dans `builder.rs`, l'intégration se fait conditionnellement:

```rust
pub fn build_rocket(
    config: &Config,
    figment: Figment,
    audio_stream: Option<Arc<SharedAudioStream>>
) -> Result<Rocket<Build>, Box<dyn std::error::Error>> {
    // Configuration de base du serveur...
    let rocket_builder = rocket::custom(figment)
        .attach(CORS)
        .mount("/", routes![...])
        .manage(oxide_state)
        .manage(jwt_validator);

    // Ajout conditionnel des routes audio
    if let Some(stream) = audio_stream {
        let audio_state = AudioStreamState { stream };
        rocket_builder
            .mount(
                "/api/audio",
                crate::visualization::audio_streaming::get_audio_streaming_routes(),
            )
            .manage(audio_state)
    } else {
        rocket_builder
    }
    .mount("/api/doc/", make_rapidoc(&rapidoc_config))
    .launch()
}
```

### Gestion d'État

```rust
pub struct AudioStreamState {
    pub stream: Arc<SharedAudioStream>,
}
```

L'état est géré par Rocket via `.manage()` et injecté dans les handlers via `&State<AudioStreamState>`.

---

## Patterns de Concurrence

### 1. Arc (Atomic Reference Counting)

```rust
// Partage du stream entre daemon et endpoints
let audio_stream = Arc::new(SharedAudioStream::new(buffer_size));

// Clonage léger pour chaque consommateur
let stream_clone = audio_stream.clone();
```

### 2. Atomic Operations

```rust
// Contrôle thread-safe sans mutex
let running = Arc::new(AtomicBool::new(false));
let frame_counter = Arc::new(AtomicU64::new(0));

// Opérations atomiques
running.store(true, Ordering::Relaxed);
let count = frame_counter.fetch_add(1, Ordering::Relaxed);
```

### 3. RwLock pour Données Partagées

```rust
// Lectures multiples, écritures exclusives
latest_frame: Arc<RwLock<Option<AudioFrame>>>,
stats: Arc<RwLock<StreamStats>>,

// Usage
let latest = self.latest_frame.read().await;
let mut stats = self.stats.write().await;
```

### 4. Broadcast Channel

```rust
// Un producteur, multiples consommateurs
let (sender, _) = broadcast::channel(buffer_size);

// Chaque consommateur obtient une copie de toutes les données
let receiver = sender.subscribe();
```

---

## Gestion des Erreurs

### Stratégies par Composant

#### AcquisitionDaemon

```rust
// Erreurs de lecture: retry avec delay
Err(e) => {
    error!("Error reading audio frame: {}", e);
    tokio::time::sleep(Duration::from_millis(100)).await;
}

// Erreurs de publication: log et continue
if let Err(e) = self.stream.publish(frame).await {
    warn!("Failed to publish frame: {}", e);
}
```

#### Audio Streaming

```rust
// Timeout sur réception: heartbeat
Err(_) => {
    yield Event::data(r#"{"type":"heartbeat"}"#);
}

// Consumer lag: warning et continue
Err(broadcast::error::RecvError::Lagged(skipped)) => {
    log::warn!("Consumer lagged behind, skipped {} frames", skipped);
    // Récupération gracieuse...
}
```

#### SharedAudioStream

```rust
// Pas d'abonnés: pas d'erreur
Err(broadcast::error::SendError(_)) => {
    // Comportement normal si aucun consommateur
    Ok(())
}
```

---

## Tests et Validation

### Tests Unitaires

#### Test du Daemon

```rust
#[tokio::test]
async fn test_acquisition_daemon() {
    let audio_source = get_default_audio_source().unwrap();
    let mut daemon = AcquisitionDaemon::new(audio_source, 10.0, 50);

    let mut consumer = AudioStreamConsumer::new(daemon.get_stream());

    // Démarrage en arrière-plan
    let daemon_running = daemon.running.clone();
    tokio::spawn(async move {
        daemon.start().await.unwrap();
    });

    // Vérification de réception
    let result = timeout(Duration::from_secs(2), consumer.next_frame()).await;
    assert!(result.is_ok());

    // Arrêt propre
    daemon_running.store(false, Ordering::Relaxed);
}
```

#### Test Multi-Consommateurs

```rust
#[tokio::test]
async fn test_multiple_consumers() {
    let stream = SharedAudioStream::new(10);
    let mut consumer1 = AudioStreamConsumer::new(&stream);
    let mut consumer2 = AudioStreamConsumer::new(&stream);

    let frame = AudioFrame::new(vec![1.0, 2.0], vec![3.0, 4.0], 48000, 42);
    stream.publish(frame.clone()).await.unwrap();

    // Les deux consommateurs reçoivent la même frame
    let frame1 = consumer1.next_frame().await.unwrap();
    let frame2 = consumer2.next_frame().await.unwrap();

    assert_eq!(frame1.frame_number, 42);
    assert_eq!(frame2.frame_number, 42);
}
```

### Tests d'Intégration

#### Test SSE Endpoint

```rust
#[tokio::test]
async fn test_sse_streaming() {
    // Configuration du serveur de test avec audio stream
    let audio_stream = Arc::new(SharedAudioStream::new(10));
    let rocket = build_test_rocket(Some(audio_stream.clone()));

    // Simulation de publication de données
    let frame = AudioFrame::new(vec![0.1, 0.2], vec![0.3, 0.4], 48000, 1);
    audio_stream.publish(frame).await.unwrap();

    // Test de l'endpoint SSE
    let client = Client::tracked(rocket).await.unwrap();
    let response = client.get("/api/audio/stream/audio").dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::new("text", "event-stream")));
}
```

---

## Exemples d'Utilisation

### 1. Démarrage Complet du Système

```rust
use rust_photoacoustic::{
    acquisition::{get_default_audio_source, AcquisitionDaemon, SharedAudioStream},
    config::Config,
    visualization::server::builder::build_rocket,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Configuration
    let config = Config::from_file("config.yaml")?;

    // Source audio
    let audio_source = get_default_audio_source()?;

    // Stream partagé
    let audio_stream = Arc::new(SharedAudioStream::new(1000));

    // Daemon d'acquisition
    let mut daemon = AcquisitionDaemon::new(audio_source, 30.0, 1000);
    let daemon_stream = daemon.get_stream().clone();

    // Démarrage du daemon en arrière-plan
    tokio::spawn(async move {
        daemon.start().await.unwrap();
    });

    // Serveur web avec streaming
    let rocket = build_rocket(&config, figment, Some(daemon_stream))?;
    rocket.launch().await?;

    Ok(())
}
```

### 2. Consumer Personnalisé

```rust
pub struct SpectralAnalysisConsumer {
    consumer: AudioStreamConsumer,
    frame_size: usize,
}

impl SpectralAnalysisConsumer {
    pub fn new(stream: &SharedAudioStream, frame_size: usize) -> Self {
        Self {
            consumer: AudioStreamConsumer::new(stream),
            frame_size,
        }
    }

    pub async fn process_next(&mut self) -> Option<SpectralData> {
        let frame = self.consumer.next_frame().await?;

        // Analyse spectrale personnalisée
        let fft_result = perform_fft(&frame.channel_a, self.frame_size);

        Some(SpectralData {
            frequencies: fft_result.frequencies,
            magnitudes: fft_result.magnitudes,
            frame_number: frame.frame_number,
        })
    }
}
```

### 3. Monitoring et Métriques

```rust
pub async fn monitor_stream_health(stream: &SharedAudioStream) {
    let mut last_frame_count = 0;

    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;

        let stats = stream.get_stats().await;
        let frames_processed = stats.total_frames - last_frame_count;

        info!(
            "Stream Health - FPS: {:.1}, Subscribers: {}, Frames/10s: {}",
            stats.fps,
            stats.active_subscribers,
            frames_processed
        );

        if stats.fps < 10.0 {
            warn!("Low frame rate detected: {:.1} FPS", stats.fps);
        }

        last_frame_count = stats.total_frames;
    }
}
```

---

## Bonnes Pratiques

### 1. Gestion de la Mémoire

```rust
// ✅ Bon: Utilisation d'Arc pour partage léger
let shared_stream = Arc::new(SharedAudioStream::new(buffer_size));

// ✅ Bon: Clonage d'Arc (pas de copie des données)
let stream_clone = shared_stream.clone();

// ❌ Éviter: Copies inutiles de grandes structures
let expensive_copy = (*shared_stream).clone(); // Coûteux!
```

### 2. Gestion des Buffers

```rust
// Configuration des tailles de buffer
const SMALL_BUFFER: usize = 10;   // Faible latence, risque de perte
const MEDIUM_BUFFER: usize = 100; // Équilibre latence/fiabilité
const LARGE_BUFFER: usize = 1000; // Haute fiabilité, latence plus élevée

// Adaptation selon le cas d'usage
let buffer_size = match use_case {
    UseCase::RealTime => SMALL_BUFFER,
    UseCase::Analysis => MEDIUM_BUFFER,
    UseCase::Recording => LARGE_BUFFER,
};
```

### 3. Gestion des Erreurs Asynchrones

```rust
// ✅ Bon: Gestion gracieuse avec logs
match consumer.next_frame().await {
    Some(frame) => process_frame(frame),
    None => {
        log::info!("Stream ended gracefully");
        break;
    }
}

// ✅ Bon: Retry avec backoff exponentiel
let mut retry_delay = Duration::from_millis(100);
loop {
    match risky_operation().await {
        Ok(result) => break result,
        Err(e) if e.is_retryable() => {
            log::warn!("Retrying after error: {}", e);
            tokio::time::sleep(retry_delay).await;
            retry_delay = std::cmp::min(retry_delay * 2, Duration::from_secs(30));
        },
        Err(e) => return Err(e),
    }
}
```

### 4. Monitoring et Observabilité

```rust
// Métriques utiles à surveiller
#[derive(Debug)]
pub struct SystemMetrics {
    pub frames_per_second: f64,
    pub active_consumers: usize,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub network_bandwidth_mbps: f64,
}

// Logging structuré
log::info!(
    target: "audio_streaming",
    "frame_processed";
    "frame_number" => frame.frame_number,
    "processing_time_ms" => processing_time.as_millis(),
    "consumer_count" => consumer_count,
);
```

### 5. Configuration et Tuning

```rust
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Taille du buffer broadcast
    pub broadcast_buffer_size: usize,
    /// FPS cible pour l'acquisition
    pub target_fps: f64,
    /// Timeout pour les consommateurs SSE
    pub sse_timeout_secs: u64,
    /// Taille de fenêtre pour analyse spectrale
    pub spectral_window_size: usize,
    /// Activation des heartbeats
    pub enable_heartbeats: bool,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            broadcast_buffer_size: 100,
            target_fps: 30.0,
            sse_timeout_secs: 5,
            spectral_window_size: 4096,
            enable_heartbeats: true,
        }
    }
}
```

---

## Considérations de Performance

### 1. Latence vs Débit

- **Faible Latence**: Buffer petit (10-50 frames), risque de perte
- **Haut Débit**: Buffer grand (500-1000 frames), latence élevée
- **Équilibré**: Buffer moyen (100-200 frames), bon compromis

### 2. Optimisations CPU

```rust
// Éviter les allocations fréquentes
pub struct FrameProcessor {
    // Réutilisation des buffers
    fft_buffer: Vec<f32>,
    window_buffer: Vec<f32>,
}

// Pool d'objets pour frames
pub struct FramePool {
    pool: Vec<AudioFrame>,
    available: VecDeque<usize>,
}
```

### 3. Optimisations Réseau

```rust
// Compression des données JSON
use flate2::write::GzEncoder;

// Échantillonnage adaptatif
pub fn downsample_for_client(frame: &AudioFrame, client_bandwidth: u32) -> AudioFrame {
    if client_bandwidth < 1_000_000 { // < 1 Mbps
        // Réduction de la résolution
        downsample_frame(frame, 2)
    } else {
        frame.clone()
    }
}
```

---

Cette documentation fournit une vue complète de l'architecture de streaming audio. Pour des détails d'implémentation spécifiques, consultez les fichiers source dans `src/acquisition/` et `src/visualization/audio_streaming.rs`.
