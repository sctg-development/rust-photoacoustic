# Guide Pratique du Module Audio Streaming

## Documentation d'Intégration pour les Endpoints SSE

### Table des Matières

1. [Vue d'ensemble du Module](#vue-densemble-du-module)
2. [Architecture des Endpoints SSE](#architecture-des-endpoints-sse)
3. [Configuration et Déploiement](#configuration-et-déploiement)
4. [Intégration Client-Serveur](#intégration-client-serveur)
5. [Gestion des États de Connexion](#gestion-des-états-de-connexion)
6. [Optimisation des Performances](#optimisation-des-performances)
7. [Monitoring et Debug](#monitoring-et-debug)
8. [Exemples d'Implémentation](#exemples-dimplémentation)
9. [Tests et Validation](#tests-et-validation)
10. [Troubleshooting Avancé](#troubleshooting-avancé)

---

## Vue d'ensemble du Module

Le module `audio_streaming` fournit des endpoints HTTP/SSE (Server-Sent Events) pour diffuser en temps réel les données audio acquises par l'`AcquisitionDaemon` vers des clients web. Il constitue le pont entre le système d'acquisition Rust et les interfaces utilisateur web.

### Composants Principaux

```rust
// Structure de gestion d'état
pub struct AudioStreamState {
    stream: Arc<SharedAudioStream>,
    stats: Arc<RwLock<StreamStats>>,
    connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
}

// Réponse API optimisée
#[derive(Serialize, Deserialize)]
pub struct AudioFrameResponse {
    pub channel_a: Vec<f32>,
    pub channel_b: Vec<f32>,
    pub sample_rate: u32,
    pub timestamp: u64,
    pub frame_number: u64,
    pub duration_ms: f64,
}

// Endpoints principaux
#[get("/stream")]
pub fn audio_stream_sse(state: &State<AudioStreamState>) -> EventStream;

#[get("/stats")]
pub fn stream_stats(state: &State<AudioStreamState>) -> Json<StreamStats>;
```

### Flux de Données

```
    ┌──────────────────┐       ┌──────────────────┐       ┌──────────────────┐
    │ AcquisitionDaemon │─────▶│ SharedAudioStream│─────▶│   Audio SSE      │
    │                  │       │                  │       │   Endpoints      │
    │ • read_frame()   │       │ • broadcast()    │       │ • /stream        │
    │ • target_fps     │       │ • subscribers    │       │ • /stats         │
    └──────────────────┘       └──────────────────┘       └──────────────────┘
                                                                    │
                                                                    │ SSE
                                                                    ▼
    ┌──────────────────┐       ┌──────────────────┐       ┌──────────────────┐
    │  Web Clients     │◀───── │  HTTP/SSE        │◀─────│  Rocket Server   │
    │                  │       │  Transport       │       │                  │
    │ • EventSource    │       │ • CORS headers   │       │ • Rate limiting  │
    │ • JSON parsing   │       │ • Compression    │       │ • Auth/Security  │
    │ • Reconnection   │       │ • Keep-alive     │       │ • Monitoring     │
    └──────────────────┘       └──────────────────┘       └──────────────────┘
```

---

## Architecture des Endpoints SSE

### Endpoint de Streaming Principal

```rust
#[get("/stream")]
pub fn audio_stream_sse(
    state: &State<AudioStreamState>,
    user: AuthenticatedUser,  // Optionnel selon configuration
) -> EventStream![Event + '_] {
    let mut consumer = AudioStreamConsumer::new(&state.stream);
    let connection_id = generate_connection_id();

    // Enregistrement de la connexion
    state.register_connection(connection_id.clone(), &user).await;

    EventStream! {
        // Heartbeat initial
        yield Event::data("connected").id(&connection_id);

        let mut heartbeat_counter = 0;

        loop {
            tokio::select! {
                // Réception de nouvelles frames
                frame_result = consumer.next_frame() => {
                    match frame_result {
                        Ok(Some(frame)) => {
                            let response = AudioFrameResponse::from(frame);
                            yield Event::json(&response)
                                .id(&format!("frame-{}", response.frame_number))
                                .event("audio-frame");
                        }
                        Ok(None) => {
                            // Aucune donnée, continuer
                            continue;
                        }
                        Err(e) => {
                            error!("Erreur réception frame: {}", e);
                            yield Event::data(&format!("error: {}", e))
                                .event("error");
                            break;
                        }
                    }
                }

                // Heartbeat périodique (toutes les 30 secondes)
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    heartbeat_counter += 1;
                    yield Event::data(&format!("heartbeat-{}", heartbeat_counter))
                        .event("heartbeat");
                }
            }
        }

        // Nettoyage à la déconnexion
        state.unregister_connection(&connection_id).await;
    }
}
```

### Endpoint de Statistiques

```rust
#[get("/stats")]
pub async fn stream_stats(
    state: &State<AudioStreamState>
) -> Json<DetailedStreamStats> {
    let base_stats = state.stream.get_stats().await;
    let connections = state.get_active_connections().await;

    let detailed_stats = DetailedStreamStats {
        // Statistiques de base
        total_frames: base_stats.total_frames,
        frames_per_second: base_stats.frames_per_second,
        active_subscribers: base_stats.active_subscribers,
        dropped_frames: base_stats.dropped_frames,

        // Statistiques détaillées
        connection_count: connections.len(),
        connections: connections,
        memory_usage: calculate_memory_usage().await,
        cpu_usage: get_cpu_usage().await,
        uptime_seconds: state.get_uptime().as_secs(),

        // Métriques de performance
        average_latency_ms: calculate_average_latency(&connections),
        bandwidth_usage_mbps: calculate_bandwidth_usage(&connections),
        error_rate_percent: calculate_error_rate(&base_stats),
    };

    Json(detailed_stats)
}

#[derive(Serialize)]
pub struct DetailedStreamStats {
    // Statistiques héritées
    pub total_frames: u64,
    pub frames_per_second: f64,
    pub active_subscribers: usize,
    pub dropped_frames: u64,

    // Statistiques étendues
    pub connection_count: usize,
    pub connections: Vec<ConnectionInfo>,
    pub memory_usage: MemoryUsage,
    pub cpu_usage: f64,
    pub uptime_seconds: u64,

    // Métriques de performance
    pub average_latency_ms: f64,
    pub bandwidth_usage_mbps: f64,
    pub error_rate_percent: f64,
}
```

### Endpoint de Contrôle

```rust
#[post("/control", data = "<command>")]
pub async fn stream_control(
    state: &State<AudioStreamState>,
    command: Json<StreamCommand>,
    user: AuthenticatedUser,
) -> Result<Json<CommandResponse>, Status> {
    // Vérification des permissions
    if !user.has_permission("stream_control") {
        return Err(Status::Forbidden);
    }

    let response = match command.action.as_str() {
        "pause" => {
            state.pause_stream().await?;
            CommandResponse::success("Stream paused")
        }
        "resume" => {
            state.resume_stream().await?;
            CommandResponse::success("Stream resumed")
        }
        "adjust_quality" => {
            let quality = command.params.get("quality")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            state.adjust_quality(quality).await?;
            CommandResponse::success(&format!("Quality adjusted to {}", quality))
        }
        _ => return Err(Status::BadRequest),
    };

    Ok(Json(response))
}

#[derive(Deserialize)]
pub struct StreamCommand {
    pub action: String,
    pub params: Map<String, Value>,
}

#[derive(Serialize)]
pub struct CommandResponse {
    pub success: bool,
    pub message: String,
    pub timestamp: u64,
}
```

---

## Configuration et Déploiement

### Intégration avec Rocket Builder

```rust
// Dans builder.rs
use crate::visualization::audio_streaming::{
    AudioStreamState, audio_stream_sse, stream_stats, stream_control
};

pub fn build_rocket(
    config: &Config,
    data_source: Arc<PhotoacousticDataSource>,
    audio_stream: Option<Arc<SharedAudioStream>>,
) -> rocket::Rocket<rocket::Build> {
    let mut rocket_builder = rocket::build()
        .manage(data_source)
        .configure(rocket::Config {
            port: config.visualization.port,
            log_level: LogLevel::Info,
            limits: Limits::default()
                .limit("json", 10.megabytes())
                .limit("stream", 100.megabytes()),
            ..rocket::Config::default()
        });

    // Configuration conditionnelle du streaming audio
    if let Some(stream) = audio_stream {
        let audio_state = AudioStreamState::new(stream);

        rocket_builder = rocket_builder
            .manage(audio_state)
            .mount("/api/audio", routes![
                audio_stream_sse,
                stream_stats,
                stream_control
            ]);

        info!("Audio streaming endpoints configurés");
    } else {
        warn!("Streaming audio non disponible - endpoints désactivés");
    }

    rocket_builder
        // Autres routes...
        .mount("/api", routes![...])
        .attach(cors_fairing())
        .attach(request_logger())
}
```

### Configuration CORS pour SSE

```rust
use rocket_cors::{AllowedOrigins, CorsOptions};

fn cors_fairing() -> rocket_cors::Cors {
    CorsOptions::default()
        .allowed_origins(AllowedOrigins::some_exact(&[
            "http://localhost:3000",  // Dev frontend
            "http://localhost:8080",  // Production frontend
        ]))
        .allowed_methods(
            vec![Method::Get, Method::Post, Method::Options]
                .into_iter()
                .map(From::from)
                .collect(),
        )
        .allowed_headers(AllowedHeaders::some(&[
            "Authorization",
            "Accept",
            "Content-Type",
            "Cache-Control",
        ]))
        .allow_credentials(true)
        .to_cors()
        .expect("Configuration CORS invalide")
}
```

### Middleware de Compression

```rust
use rocket::fairing::{Fairing, Info, Kind};
use rocket::{Request, Response};
use flate2::write::GzEncoder;
use flate2::Compression;

pub struct CompressionFairing;

#[rocket::async_trait]
impl Fairing for CompressionFairing {
    fn info(&self) -> Info {
        Info {
            name: "Response Compression",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        if request.uri().path().starts_with("/api/audio/stream") {
            // Pas de compression pour SSE - peut causer des problèmes de buffering
            return;
        }

        if let Some(accept_encoding) = request.headers().get_one("Accept-Encoding") {
            if accept_encoding.contains("gzip") {
                // Appliquer compression gzip pour autres endpoints
                apply_gzip_compression(response);
            }
        }
    }
}
```

---

## Intégration Client-Serveur

### Client JavaScript Optimisé

```javascript
class AudioStreamClient {
  constructor(baseUrl, authToken) {
    this.baseUrl = baseUrl;
    this.authToken = authToken;
    this.eventSource = null;
    this.reconnectAttempts = 0;
    this.maxReconnectAttempts = 5;
    this.reconnectDelay = 1000; // ms

    // Buffers pour données audio
    this.audioBuffer = [];
    this.maxBufferSize = 100;

    // Callbacks
    this.onFrame = null;
    this.onStats = null;
    this.onError = null;
    this.onConnect = null;
    this.onDisconnect = null;
  }

  connect() {
    const url = `${this.baseUrl}/api/audio/stream`;
    const headers = this.authToken
      ? { Authorization: `Bearer ${this.authToken}` }
      : {};

    this.eventSource = new EventSource(url);

    this.eventSource.onopen = (event) => {
      console.log("Connexion SSE établie");
      this.reconnectAttempts = 0;
      if (this.onConnect) this.onConnect(event);
    };

    this.eventSource.addEventListener("audio-frame", (event) => {
      try {
        const frame = JSON.parse(event.data);
        this.handleAudioFrame(frame);
      } catch (error) {
        console.error("Erreur parsing frame audio:", error);
        if (this.onError) this.onError(error);
      }
    });

    this.eventSource.addEventListener("heartbeat", (event) => {
      console.debug("Heartbeat reçu:", event.data);
    });

    this.eventSource.addEventListener("error", (event) => {
      console.error("Erreur SSE:", event);
      if (this.onError) this.onError(event);
    });

    this.eventSource.onerror = (event) => {
      console.error("Erreur connexion SSE");
      this.handleConnectionError();
    };
  }

  handleAudioFrame(frame) {
    // Validation des données
    if (!this.validateFrame(frame)) {
      console.warn("Frame audio invalide:", frame);
      return;
    }

    // Gestion du buffer
    this.audioBuffer.push(frame);
    if (this.audioBuffer.length > this.maxBufferSize) {
      this.audioBuffer.shift(); // FIFO
    }

    // Traitement de la frame
    if (this.onFrame) {
      this.onFrame(frame);
    }
  }

  validateFrame(frame) {
    return (
      frame &&
      Array.isArray(frame.channel_a) &&
      Array.isArray(frame.channel_b) &&
      frame.channel_a.length === frame.channel_b.length &&
      typeof frame.sample_rate === "number" &&
      typeof frame.timestamp === "number"
    );
  }

  async handleConnectionError() {
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      this.reconnectAttempts++;
      const delay =
        this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);

      console.log(
        `Tentative de reconnexion ${this.reconnectAttempts}/${this.maxReconnectAttempts} dans ${delay}ms`
      );

      setTimeout(() => {
        this.disconnect();
        this.connect();
      }, delay);
    } else {
      console.error(
        "Impossible de se reconnecter après",
        this.maxReconnectAttempts,
        "tentatives"
      );
      if (this.onDisconnect) this.onDisconnect();
    }
  }

  async getStats() {
    try {
      const response = await fetch(`${this.baseUrl}/api/audio/stats`, {
        headers: this.authToken
          ? { Authorization: `Bearer ${this.authToken}` }
          : {},
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const stats = await response.json();
      if (this.onStats) this.onStats(stats);
      return stats;
    } catch (error) {
      console.error("Erreur récupération statistiques:", error);
      if (this.onError) this.onError(error);
      throw error;
    }
  }

  async sendCommand(action, params = {}) {
    try {
      const response = await fetch(`${this.baseUrl}/api/audio/control`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...(this.authToken
            ? { Authorization: `Bearer ${this.authToken}` }
            : {}),
        },
        body: JSON.stringify({ action, params }),
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      return await response.json();
    } catch (error) {
      console.error("Erreur envoi commande:", error);
      if (this.onError) this.onError(error);
      throw error;
    }
  }

  disconnect() {
    if (this.eventSource) {
      this.eventSource.close();
      this.eventSource = null;
    }
    this.audioBuffer = [];
    console.log("Connexion SSE fermée");
  }

  // Utilitaires pour traitement audio
  getLatestFrame() {
    return this.audioBuffer[this.audioBuffer.length - 1];
  }

  getFrameHistory(count = 10) {
    return this.audioBuffer.slice(-count);
  }

  calculateRMS(channel) {
    if (!Array.isArray(channel) || channel.length === 0) return 0;

    const sumSquares = channel.reduce(
      (sum, sample) => sum + sample * sample,
      0
    );
    return Math.sqrt(sumSquares / channel.length);
  }
}
```

### Intégration avec Interface Utilisateur

```javascript
// Exemple d'utilisation dans une application React/Vue
class AudioVisualization {
  constructor(canvasElement) {
    this.canvas = canvasElement;
    this.ctx = this.canvas.getContext("2d");
    this.audioClient = new AudioStreamClient("/api", getAuthToken());

    this.setupEventHandlers();
    this.initializeVisualization();
  }

  setupEventHandlers() {
    this.audioClient.onFrame = (frame) => {
      this.visualizeFrame(frame);
      this.updateMetrics(frame);
    };

    this.audioClient.onStats = (stats) => {
      this.updateStatisticsDisplay(stats);
    };

    this.audioClient.onError = (error) => {
      this.showErrorMessage(error);
    };

    this.audioClient.onConnect = () => {
      this.showStatusMessage("Connecté", "success");
    };

    this.audioClient.onDisconnect = () => {
      this.showStatusMessage("Déconnecté", "error");
    };
  }

  visualizeFrame(frame) {
    const { channel_a, channel_b } = frame;

    // Effacer le canvas
    this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);

    // Dessiner les formes d'onde
    this.drawWaveform(channel_a, "blue", 0, this.canvas.height / 2 - 50);
    this.drawWaveform(
      channel_b,
      "red",
      this.canvas.height / 2 + 50,
      this.canvas.height / 2 - 50
    );

    // Afficher les métadonnées
    this.drawFrameInfo(frame);
  }

  drawWaveform(samples, color, yOffset, height) {
    const width = this.canvas.width;
    const stepX = width / samples.length;

    this.ctx.strokeStyle = color;
    this.ctx.lineWidth = 1;
    this.ctx.beginPath();

    for (let i = 0; i < samples.length; i++) {
      const x = i * stepX;
      const y = yOffset + (samples[i] * height) / 2;

      if (i === 0) {
        this.ctx.moveTo(x, y);
      } else {
        this.ctx.lineTo(x, y);
      }
    }

    this.ctx.stroke();
  }

  drawFrameInfo(frame) {
    this.ctx.fillStyle = "black";
    this.ctx.font = "12px Arial";
    this.ctx.fillText(`Frame: ${frame.frame_number}`, 10, 20);
    this.ctx.fillText(`SR: ${frame.sample_rate}Hz`, 10, 35);
    this.ctx.fillText(`Duration: ${frame.duration_ms.toFixed(1)}ms`, 10, 50);

    // RMS levels
    const rmsA = this.audioClient.calculateRMS(frame.channel_a);
    const rmsB = this.audioClient.calculateRMS(frame.channel_b);
    this.ctx.fillText(`RMS A: ${rmsA.toFixed(3)}`, 150, 20);
    this.ctx.fillText(`RMS B: ${rmsB.toFixed(3)}`, 150, 35);
  }

  start() {
    this.audioClient.connect();

    // Mise à jour périodique des statistiques
    setInterval(() => {
      this.audioClient.getStats();
    }, 5000);
  }

  stop() {
    this.audioClient.disconnect();
  }
}

// Initialisation
const canvas = document.getElementById("audioCanvas");
const visualization = new AudioVisualization(canvas);
visualization.start();
```

---

## Gestion des États de Connexion

### Suivi des Connexions Actives

```rust
use uuid::Uuid;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionInfo {
    pub id: String,
    pub user_id: Option<String>,
    pub ip_address: String,
    pub user_agent: String,
    pub connected_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
    pub frames_sent: u64,
    pub bytes_sent: u64,
    pub errors_count: u32,
}

impl AudioStreamState {
    pub async fn register_connection(
        &self,
        connection_id: String,
        user: &AuthenticatedUser,
        request: &Request<'_>
    ) {
        let connection_info = ConnectionInfo {
            id: connection_id.clone(),
            user_id: Some(user.id.clone()),
            ip_address: request.client_ip()
                .map(|ip| ip.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            user_agent: request.headers()
                .get_one("User-Agent")
                .unwrap_or("unknown")
                .to_string(),
            connected_at: Utc::now(),
            last_heartbeat: Utc::now(),
            frames_sent: 0,
            bytes_sent: 0,
            errors_count: 0,
        };

        let mut connections = self.connections.write().await;
        connections.insert(connection_id, connection_info);

        info!("Nouvelle connexion audio: {} (utilisateur: {})",
              connection_id, user.id);
    }

    pub async fn update_connection_metrics(
        &self,
        connection_id: &str,
        frames_sent: u64,
        bytes_sent: u64
    ) {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.get_mut(connection_id) {
            connection.frames_sent = frames_sent;
            connection.bytes_sent = bytes_sent;
            connection.last_heartbeat = Utc::now();
        }
    }

    pub async fn increment_connection_errors(&self, connection_id: &str) {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.get_mut(connection_id) {
            connection.errors_count += 1;
        }
    }

    pub async fn unregister_connection(&self, connection_id: &str) {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.remove(connection_id) {
            let duration = Utc::now().signed_duration_since(connection.connected_at);

            info!(
                "Connexion fermée: {} (durée: {}s, frames: {}, erreurs: {})",
                connection_id,
                duration.num_seconds(),
                connection.frames_sent,
                connection.errors_count
            );
        }
    }

    // Nettoyage des connexions inactives
    pub async fn cleanup_stale_connections(&self) {
        let threshold = Utc::now() - chrono::Duration::minutes(5);
        let mut connections = self.connections.write().await;

        let stale_connections: Vec<String> = connections
            .iter()
            .filter(|(_, conn)| conn.last_heartbeat < threshold)
            .map(|(id, _)| id.clone())
            .collect();

        for connection_id in stale_connections {
            connections.remove(&connection_id);
            warn!("Connexion inactive supprimée: {}", connection_id);
        }
    }
}
```

### Limitation du Débit par Connexion

```rust
use std::time::Instant;
use tokio::time::{sleep, Duration};

pub struct RateLimiter {
    max_frames_per_second: f64,
    last_frame_time: Instant,
    frame_count: u32,
    window_start: Instant,
}

impl RateLimiter {
    pub fn new(max_fps: f64) -> Self {
        let now = Instant::now();
        Self {
            max_frames_per_second: max_fps,
            last_frame_time: now,
            frame_count: 0,
            window_start: now,
        }
    }

    pub async fn wait_if_needed(&mut self) -> bool {
        let now = Instant::now();
        let window_duration = Duration::from_secs(1);

        // Reset du compteur toutes les secondes
        if now.duration_since(self.window_start) >= window_duration {
            self.frame_count = 0;
            self.window_start = now;
        }

        // Vérification de la limite
        if self.frame_count as f64 >= self.max_frames_per_second {
            let sleep_duration = window_duration - now.duration_since(self.window_start);
            if sleep_duration > Duration::from_millis(0) {
                sleep(sleep_duration).await;
                self.frame_count = 0;
                self.window_start = Instant::now();
            }
        }

        self.frame_count += 1;
        self.last_frame_time = now;
        true
    }
}

// Intégration dans le streaming SSE
#[get("/stream?<max_fps>")]
pub fn audio_stream_sse_with_rate_limit(
    state: &State<AudioStreamState>,
    max_fps: Option<f64>,
    user: AuthenticatedUser,
) -> EventStream![Event + '_] {
    let rate_limiter = RateLimiter::new(max_fps.unwrap_or(30.0));
    let mut consumer = AudioStreamConsumer::new(&state.stream);

    EventStream! {
        let mut rate_limiter = rate_limiter;

        loop {
            // Attendre si nécessaire pour respecter la limite
            rate_limiter.wait_if_needed().await;

            match consumer.next_frame().await {
                Ok(Some(frame)) => {
                    let response = AudioFrameResponse::from(frame);
                    yield Event::json(&response);
                }
                Ok(None) => continue,
                Err(_) => break,
            }
        }
    }
}
```

---

## Optimisation des Performances

### Sérialisation Optimisée

```rust
use serde_json::ser::to_vec;
use flate2::write::DeflateEncoder;
use flate2::Compression;

impl AudioFrameResponse {
    /// Sérialisation optimisée avec compression optionnelle
    pub fn serialize_optimized(&self, compress: bool) -> Result<Vec<u8>, serde_json::Error> {
        if compress {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
            serde_json::to_writer(&mut encoder, self)?;
            Ok(encoder.finish().unwrap_or_default())
        } else {
            to_vec(self)
        }
    }

    /// Version allégée pour clients avec bande passante limitée
    pub fn to_lightweight(&self) -> LightweightAudioFrame {
        LightweightAudioFrame {
            // Sous-échantillonnage 2:1
            channel_a: self.channel_a.iter().step_by(2).copied().collect(),
            channel_b: self.channel_b.iter().step_by(2).copied().collect(),
            sample_rate: self.sample_rate / 2,
            timestamp: self.timestamp,
            frame_number: self.frame_number,
        }
    }
}

#[derive(Serialize)]
pub struct LightweightAudioFrame {
    pub channel_a: Vec<f32>,
    pub channel_b: Vec<f32>,
    pub sample_rate: u32,
    pub timestamp: u64,
    pub frame_number: u64,
}
```

### Cache et Buffering Intelligents

```rust
use lru::LruCache;
use std::num::NonZeroUsize;

pub struct SmartBuffer {
    cache: LruCache<u64, AudioFrameResponse>,
    compression_cache: LruCache<u64, Vec<u8>>,
    max_size: usize,
}

impl SmartBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(max_size).unwrap()),
            compression_cache: LruCache::new(NonZeroUsize::new(max_size / 2).unwrap()),
            max_size,
        }
    }

    pub fn get_frame(&mut self, frame_number: u64) -> Option<&AudioFrameResponse> {
        self.cache.get(&frame_number)
    }

    pub fn get_compressed_frame(&mut self, frame_number: u64) -> Option<&Vec<u8>> {
        self.compression_cache.get(&frame_number)
    }

    pub fn store_frame(&mut self, frame: AudioFrameResponse) {
        let frame_number = frame.frame_number;

        // Compression asynchrone pour le cache
        if let Ok(compressed) = frame.serialize_optimized(true) {
            self.compression_cache.put(frame_number, compressed);
        }

        self.cache.put(frame_number, frame);
    }

    pub fn clear_old_frames(&mut self, before_frame: u64) {
        self.cache.retain(|&k, _| k >= before_frame);
        self.compression_cache.retain(|&k, _| k >= before_frame);
    }
}
```

### Monitoring des Performances

```rust
use std::time::{Duration, Instant};
use prometheus::{Counter, Histogram, Gauge};

lazy_static! {
    static ref SSE_CONNECTIONS: Gauge = register_gauge!(
        "audio_sse_connections_active",
        "Number of active SSE connections"
    ).unwrap();

    static ref FRAMES_SENT: Counter = register_counter!(
        "audio_frames_sent_total",
        "Total number of audio frames sent via SSE"
    ).unwrap();

    static ref SSE_LATENCY: Histogram = register_histogram!(
        "audio_sse_latency_seconds",
        "Latency of SSE frame delivery"
    ).unwrap();
}

pub struct PerformanceMonitor {
    start_time: Instant,
    frame_times: VecDeque<Instant>,
    error_count: u64,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            frame_times: VecDeque::with_capacity(1000),
            error_count: 0,
        }
    }

    pub fn record_frame_sent(&mut self) {
        let now = Instant::now();
        self.frame_times.push_back(now);

        // Garde seulement les 1000 dernières frames
        if self.frame_times.len() > 1000 {
            self.frame_times.pop_front();
        }

        FRAMES_SENT.inc();

        // Calcul latence
        let latency = now.duration_since(self.start_time);
        SSE_LATENCY.observe(latency.as_secs_f64());
    }

    pub fn record_error(&mut self) {
        self.error_count += 1;
    }

    pub fn get_fps(&self) -> f64 {
        if self.frame_times.len() < 2 {
            return 0.0;
        }

        let duration = self.frame_times.back().unwrap()
            .duration_since(*self.frame_times.front().unwrap());

        self.frame_times.len() as f64 / duration.as_secs_f64()
    }
}
```

---

## Tests et Validation

### Tests d'Intégration SSE

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rocket::testing::TestClient;
    use tokio_test;

    #[tokio::test]
    async fn test_sse_connection() {
        let audio_stream = Arc::new(SharedAudioStream::new(100));
        let state = AudioStreamState::new(audio_stream.clone());

        let rocket = rocket::build()
            .manage(state)
            .mount("/", routes![audio_stream_sse]);

        let client = TestClient::new(rocket).expect("valid rocket instance");

        // Simulation d'une connexion SSE
        let response = client.get("/stream").dispatch();
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.content_type(), Some(ContentType::new("text", "event-stream")));
    }

    #[tokio::test]
    async fn test_stats_endpoint() {
        let audio_stream = Arc::new(SharedAudioStream::new(50));
        let state = AudioStreamState::new(audio_stream);

        let rocket = rocket::build()
            .manage(state)
            .mount("/", routes![stream_stats]);

        let client = TestClient::new(rocket).expect("valid rocket instance");

        let response = client.get("/stats").dispatch();
        assert_eq!(response.status(), Status::Ok);

        let stats: DetailedStreamStats = response.into_json().expect("valid json");
        assert_eq!(stats.active_subscribers, 0);
    }

    #[tokio::test]
    async fn test_frame_serialization() {
        let frame = AudioFrameResponse {
            channel_a: vec![0.1, 0.2, 0.3],
            channel_b: vec![0.4, 0.5, 0.6],
            sample_rate: 48000,
            timestamp: 1234567890,
            frame_number: 42,
            duration_ms: 64.0,
        };

        // Test sérialisation normale
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("\"frame_number\":42"));

        // Test sérialisation optimisée
        let optimized = frame.serialize_optimized(true).unwrap();
        assert!(optimized.len() < json.len()); // Compression effective

        // Test version allégée
        let lightweight = frame.to_lightweight();
        assert_eq!(lightweight.channel_a.len(), 2); // Sous-échantillonnage 2:1
    }
}
```

### Tests de Charge

```rust
use std::sync::Arc;
use tokio::sync::Barrier;

#[tokio::test]
async fn test_multiple_concurrent_connections() {
    const NUM_CLIENTS: usize = 50;
    const FRAMES_PER_CLIENT: usize = 100;

    let audio_stream = Arc::new(SharedAudioStream::new(1000));
    let state = Arc::new(AudioStreamState::new(audio_stream.clone()));

    // Barrière pour synchroniser les clients
    let barrier = Arc::new(Barrier::new(NUM_CLIENTS));

    let mut handles = Vec::new();

    // Lancement de plusieurs clients simulés
    for client_id in 0..NUM_CLIENTS {
        let state_clone = state.clone();
        let barrier_clone = barrier.clone();

        let handle = tokio::spawn(async move {
            // Attendre que tous les clients soient prêts
            barrier_clone.wait().await;

            let mut consumer = AudioStreamConsumer::new(&state_clone.stream);
            let mut frames_received = 0;

            while frames_received < FRAMES_PER_CLIENT {
                match consumer.next_frame().await {
                    Ok(Some(_)) => frames_received += 1,
                    Ok(None) => continue,
                    Err(_) => break,
                }
            }

            println!("Client {} reçu {} frames", client_id, frames_received);
            frames_received
        });

        handles.push(handle);
    }

    // Simulation de la production de frames
    let producer_handle = tokio::spawn(async move {
        for i in 0..(FRAMES_PER_CLIENT * 2) {
            let frame = AudioFrame::new(
                vec![0.1; 1024],
                vec![0.2; 1024],
                48000,
                i as u64
            );

            audio_stream.publish(frame).await.unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    // Attendre tous les clients
    let results: Vec<usize> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    producer_handle.await.unwrap();

    // Vérification des résultats
    let total_frames: usize = results.iter().sum();
    let expected_total = NUM_CLIENTS * FRAMES_PER_CLIENT;

    println!("Total frames reçues: {} / {} attendues", total_frames, expected_total);

    // Tolérance de 5% pour les pertes de frames
    assert!(total_frames >= (expected_total * 95) / 100);
}
```

---

## Troubleshooting Avancé

### Diagnostics des Connexions SSE

```rust
#[get("/debug/connections")]
pub async fn debug_connections(
    state: &State<AudioStreamState>,
    user: AuthenticatedUser
) -> Json<ConnectionDebugInfo> {
    // Vérifier permissions admin
    if !user.has_permission("debug_access") {
        return Json(ConnectionDebugInfo::access_denied());
    }

    let connections = state.get_active_connections().await;
    let mut debug_info = ConnectionDebugInfo::new();

    for connection in connections {
        let health = analyze_connection_health(&connection).await;
        debug_info.add_connection_health(connection.id.clone(), health);
    }

    debug_info.system_stats = get_system_diagnostics().await;
    Json(debug_info)
}

#[derive(Serialize)]
pub struct ConnectionHealth {
    pub status: String,
    pub latency_ms: f64,
    pub throughput_mbps: f64,
    pub error_rate: f64,
    pub last_activity: String,
    pub warnings: Vec<String>,
}

async fn analyze_connection_health(connection: &ConnectionInfo) -> ConnectionHealth {
    let mut warnings = Vec::new();

    // Analyser latence
    let latency = calculate_connection_latency(connection).await;
    if latency > 100.0 {
        warnings.push(format!("Latence élevée: {:.1}ms", latency));
    }

    // Analyser débit
    let throughput = calculate_throughput(connection);
    if throughput < 1.0 {
        warnings.push("Débit faible détecté".to_string());
    }

    // Analyser taux d'erreur
    let error_rate = connection.errors_count as f64 / connection.frames_sent as f64 * 100.0;
    if error_rate > 5.0 {
        warnings.push(format!("Taux d'erreur élevé: {:.1}%", error_rate));
    }

    let status = if warnings.is_empty() { "healthy" } else { "warning" };

    ConnectionHealth {
        status: status.to_string(),
        latency_ms: latency,
        throughput_mbps: throughput,
        error_rate,
        last_activity: connection.last_heartbeat.to_rfc3339(),
        warnings,
    }
}
```

### Monitoring des Métriques Critiques

```rust
use sysinfo::{SystemExt, ProcessExt};

#[derive(Serialize)]
pub struct SystemDiagnostics {
    pub cpu_usage: f32,
    pub memory_usage: MemoryInfo,
    pub network_stats: NetworkStats,
    pub audio_pipeline_health: PipelineHealth,
}

#[derive(Serialize)]
pub struct PipelineHealth {
    pub acquisition_active: bool,
    pub frame_rate: f64,
    pub buffer_utilization: f64,
    pub consumer_count: usize,
    pub last_error: Option<String>,
}

async fn get_system_diagnostics() -> SystemDiagnostics {
    let mut system = sysinfo::System::new_all();
    system.refresh_all();

    let process = system.process(sysinfo::get_current_pid().unwrap()).unwrap();

    SystemDiagnostics {
        cpu_usage: system.global_cpu_info().cpu_usage(),
        memory_usage: MemoryInfo {
            used_bytes: process.memory(),
            virtual_bytes: process.virtual_memory(),
            available_bytes: system.available_memory(),
        },
        network_stats: get_network_statistics().await,
        audio_pipeline_health: get_pipeline_health().await,
    }
}

// Alertes automatiques
pub struct AlertManager {
    thresholds: AlertThresholds,
    notification_channels: Vec<Box<dyn NotificationChannel>>,
}

impl AlertManager {
    pub async fn check_and_alert(&self, diagnostics: &SystemDiagnostics) {
        // Alerte CPU
        if diagnostics.cpu_usage > self.thresholds.cpu_usage_percent {
            self.send_alert(Alert {
                level: AlertLevel::Warning,
                message: format!("Usage CPU élevé: {:.1}%", diagnostics.cpu_usage),
                timestamp: Utc::now(),
            }).await;
        }

        // Alerte mémoire
        let memory_percent = (diagnostics.memory_usage.used_bytes as f64 /
                             diagnostics.memory_usage.available_bytes as f64) * 100.0;
        if memory_percent > self.thresholds.memory_usage_percent {
            self.send_alert(Alert {
                level: AlertLevel::Critical,
                message: format!("Usage mémoire critique: {:.1}%", memory_percent),
                timestamp: Utc::now(),
            }).await;
        }

        // Alerte pipeline audio
        if !diagnostics.audio_pipeline_health.acquisition_active {
            self.send_alert(Alert {
                level: AlertLevel::Critical,
                message: "Pipeline d'acquisition audio arrêté".to_string(),
                timestamp: Utc::now(),
            }).await;
        }
    }
}
```

---

Ce guide complet du module audio_streaming fournit toute l'information nécessaire pour comprendre, intégrer, optimiser et maintenir le système de streaming audio temps réel. Il couvre les aspects techniques avancés, les patterns de performance, et les meilleures pratiques pour assurer un fonctionnement robuste en production.
