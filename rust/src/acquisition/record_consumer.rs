//! Mock Consumer Daemon Module
//!
//! Ce module fournit un daemon consommateur mock pour valider le système producteur/consommateur
//! d'audio en temps réel. Il consomme les frames audio du SharedAudioStream et les sauvegarde
//! dans un fichier WAV avec la même précision et fréquence d'échantillonnage que le producteur.
//!
//! Le mock consumer produit également des messages de log détaillés pour analyser le comportement
//! du système de consommation d'audio.

use crate::acquisition::{AudioFrame, AudioStreamConsumer, SharedAudioStream};
use anyhow::{anyhow, Result};
use hound::{SampleFormat, WavSpec, WavWriter};
use log::{debug, error, info, warn};
use std::fs::File;
use std::io::BufWriter;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::timeout;

/// Mock Consumer Daemon pour la validation du système producteur/consommateur
///
/// Ce daemon consomme les frames audio depuis le SharedAudioStream et les écrit
/// dans un fichier WAV pour validation. Il produit des logs détaillés pour analyser
/// le comportement du consommateur.
pub struct RecordConsumerDaemon {
    /// Stream audio partagé à consommer
    audio_stream: Arc<SharedAudioStream>,
    /// Flag de contrôle d'exécution
    running: Arc<AtomicBool>,
    /// Compteur de frames consommées
    frames_consumed: Arc<AtomicU64>,
    /// Compteur de frames perdues (lag)
    frames_dropped: Arc<AtomicU64>,
    /// Chemin du fichier WAV de sortie
    output_path: String,
    /// Writer WAV pour sauvegarder l'audio
    wav_writer: Option<WavWriter<BufWriter<File>>>,
    /// Consommateur de stream audio
    consumer: Option<AudioStreamConsumer>,
    /// Timestamp du dernier frame reçu pour mesurer les délais
    last_frame_time: Option<Instant>,
    /// Statistiques de débit
    throughput_stats: ThroughputStats,
}

/// Statistiques de débit pour le mock consumer
#[derive(Debug, Clone)]
struct ThroughputStats {
    /// Nombre de frames dans la fenêtre actuelle
    frames_in_window: u64,
    /// Timestamp du début de la fenêtre actuelle
    window_start: Instant,
    /// Durée de la fenêtre de mesure (en secondes)
    window_duration: Duration,
    /// FPS moyen de la fenêtre actuelle
    current_fps: f64,
    /// Délai moyen entre les frames (en ms)
    avg_frame_delay: f64,
    /// Délai min/max observés
    min_frame_delay: f64,
    max_frame_delay: f64,
}

impl ThroughputStats {
    fn new(window_duration_secs: u64) -> Self {
        Self {
            frames_in_window: 0,
            window_start: Instant::now(),
            window_duration: Duration::from_secs(window_duration_secs),
            current_fps: 0.0,
            avg_frame_delay: 0.0,
            min_frame_delay: f64::MAX,
            max_frame_delay: 0.0,
        }
    }

    #[allow(dead_code)]
    fn stop(&self) -> u64 {
        self.frames_in_window
    }

    #[allow(dead_code)]
    fn frames_dropped(&self) -> f64 {
        self.max_frame_delay
    }
    #[allow(dead_code)]
    fn get_throughput_stats(&self) -> (f64, f64, f64, f64) {
        (
            self.current_fps,
            self.avg_frame_delay,
            self.min_frame_delay,
            self.max_frame_delay,
        )
    }

    fn update(&mut self, frame_delay_ms: f64) {
        self.frames_in_window += 1;

        // Mettre à jour les délais min/max
        self.min_frame_delay = self.min_frame_delay.min(frame_delay_ms);
        self.max_frame_delay = self.max_frame_delay.max(frame_delay_ms);

        let now = Instant::now();
        let elapsed = now.duration_since(self.window_start);

        if elapsed >= self.window_duration {
            // Calculer le FPS pour cette fenêtre
            self.current_fps = self.frames_in_window as f64 / elapsed.as_secs_f64();

            // Calculer le délai moyen (approximatif)
            self.avg_frame_delay = (self.min_frame_delay + self.max_frame_delay) / 2.0;

            // Log des statistiques
            debug!(
                "RecordConsumer Stats - FPS: {:.2}, Avg Delay: {:.2}ms, Min: {:.2}ms, Max: {:.2}ms, Frames: {}",
                self.current_fps,
                self.avg_frame_delay,
                self.min_frame_delay,
                self.max_frame_delay,
                self.frames_in_window
            );

            // Réinitialiser pour la prochaine fenêtre
            self.frames_in_window = 0;
            self.window_start = now;
            self.min_frame_delay = f64::MAX;
            self.max_frame_delay = 0.0;
        }
    }
}

impl RecordConsumerDaemon {
    /// Créer un nouveau RecordConsumerDaemon
    ///
    /// # Arguments
    ///
    /// * `audio_stream` - Stream audio partagé à consommer
    /// * `output_path` - Chemin du fichier WAV de sortie
    ///
    /// # Returns
    ///
    /// Une nouvelle instance de RecordConsumerDaemon
    pub fn new(audio_stream: Arc<SharedAudioStream>, output_path: String) -> Self {
        info!("Creating RecordConsumerDaemon with output: {}", output_path);

        Self {
            audio_stream,
            running: Arc::new(AtomicBool::new(false)),
            frames_consumed: Arc::new(AtomicU64::new(0)),
            frames_dropped: Arc::new(AtomicU64::new(0)),
            output_path,
            wav_writer: None,
            consumer: None,
            last_frame_time: None,
            throughput_stats: ThroughputStats::new(5), // Fenêtre de 5 secondes
        }
    }

    /// Démarrer le daemon mock consumer
    pub async fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            warn!("RecordConsumerDaemon is already running");
            return Ok(());
        }

        info!("Starting RecordConsumerDaemon");
        self.running.store(true, Ordering::Relaxed);

        // Créer le consommateur
        self.consumer = Some(AudioStreamConsumer::new(&self.audio_stream));

        debug!("RecordConsumerDaemon: Consumer created, waiting for first frame");

        // Attendre le premier frame pour déterminer les spécifications WAV
        if let Some(first_frame) = self.wait_for_first_frame().await? {
            debug!(
                "RecordConsumerDaemon: First frame received - Sample Rate: {}Hz, Channels: A={}, B={}",
                first_frame.sample_rate,
                first_frame.channel_a.len(),
                first_frame.channel_b.len()
            );

            // Initialiser le writer WAV avec les spécifications du premier frame
            self.initialize_wav_writer(&first_frame)?;

            // Traiter le premier frame
            self.process_frame(&first_frame)?;

            // Boucle principale de consommation
            while self.running.load(Ordering::Relaxed) {
                match self.consume_next_frame().await {
                    Ok(true) => {
                        // Frame traité avec succès
                        let count = self.frames_consumed.fetch_add(1, Ordering::Relaxed);

                        if count % 100 == 0 {
                            debug!("RecordConsumerDaemon: {} frames consumed", count);
                        }
                    }
                    Ok(false) => {
                        // Timeout - pas de nouveau frame
                        debug!("RecordConsumerDaemon: Timeout waiting for frame");
                    }
                    Err(e) => {
                        error!("RecordConsumerDaemon: Error consuming frame: {}", e);
                        break;
                    }
                }
            }
        } else {
            warn!("RecordConsumerDaemon: No frames received, stopping");
        }

        // Nettoyer
        self.cleanup();
        info!(
            "RecordConsumerDaemon stopped - {} frames consumed, {} frames dropped",
            self.frames_consumed.load(Ordering::Relaxed),
            self.frames_dropped.load(Ordering::Relaxed)
        );

        Ok(())
    }
    /// Arrêter le daemon
    #[allow(dead_code)]
    pub fn stop(&self) {
        info!("Stopping RecordConsumerDaemon");
        self.running.store(false, Ordering::Relaxed);
    }

    /// Vérifier si le daemon est en cours d'exécution
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Obtenir le nombre de frames consommées
    #[allow(dead_code)]
    pub fn frames_consumed(&self) -> u64 {
        self.frames_consumed.load(Ordering::Relaxed)
    }

    /// Obtenir le nombre de frames perdues
    #[allow(dead_code)]
    pub fn frames_dropped(&self) -> u64 {
        self.frames_dropped.load(Ordering::Relaxed)
    }

    /// Attendre le premier frame pour déterminer les spécifications
    async fn wait_for_first_frame(&mut self) -> Result<Option<AudioFrame>> {
        debug!("RecordConsumerDaemon: Waiting for first frame");

        let timeout_duration = Duration::from_secs(10);
        let consumer = self
            .consumer
            .as_mut()
            .ok_or_else(|| anyhow!("Consumer not initialized"))?;

        match timeout(timeout_duration, consumer.next_frame()).await {
            Ok(Some(frame)) => {
                info!("RecordConsumerDaemon: First frame received successfully");
                Ok(Some(frame))
            }
            Ok(None) => {
                warn!("RecordConsumerDaemon: Stream closed before receiving first frame");
                Ok(None)
            }
            Err(_) => {
                error!("RecordConsumerDaemon: Timeout waiting for first frame");
                Err(anyhow!("Timeout waiting for first frame"))
            }
        }
    }

    /// Initialiser le writer WAV avec les spécifications du frame
    fn initialize_wav_writer(&mut self, frame: &AudioFrame) -> Result<()> {
        let spec = WavSpec {
            channels: 2, // Stéréo (channel_a et channel_b)
            sample_rate: frame.sample_rate,
            bits_per_sample: 32, // Utiliser 32 bits pour les données f32
            sample_format: SampleFormat::Float,
        };

        debug!(
            "RecordConsumerDaemon: Initializing WAV writer - {}Hz, {} channels, {} bits",
            spec.sample_rate, spec.channels, spec.bits_per_sample
        );

        let writer = WavWriter::create(&self.output_path, spec)
            .map_err(|e| anyhow!("Failed to create WAV writer: {}", e))?;

        self.wav_writer = Some(writer);

        info!(
            "RecordConsumerDaemon: WAV file created: {} ({}Hz, {} channels)",
            self.output_path, frame.sample_rate, 2
        );

        Ok(())
    }

    /// Consommer le prochain frame
    async fn consume_next_frame(&mut self) -> Result<bool> {
        let timeout_duration = Duration::from_millis(100);
        let consumer = self
            .consumer
            .as_mut()
            .ok_or_else(|| anyhow!("Consumer not initialized"))?;

        let now = Instant::now();

        match timeout(timeout_duration, consumer.next_frame()).await {
            Ok(Some(frame)) => {
                // Calculer le délai depuis le dernier frame
                if let Some(last_time) = self.last_frame_time {
                    let delay_ms = now.duration_since(last_time).as_millis() as f64;
                    self.throughput_stats.update(delay_ms);
                }
                self.last_frame_time = Some(now);

                // Traiter le frame
                self.process_frame(&frame)?;
                Ok(true)
            }
            Ok(None) => {
                debug!("RecordConsumerDaemon: Stream closed");
                Ok(false)
            }
            Err(_) => {
                // Timeout - pas de nouveau frame disponible
                Ok(false)
            }
        }
    }

    /// Traiter un frame audio
    fn process_frame(&mut self, frame: &AudioFrame) -> Result<()> {
        let writer = self
            .wav_writer
            .as_mut()
            .ok_or_else(|| anyhow!("WAV writer not initialized"))?;

        // Vérifier que les deux canaux ont la même taille
        if frame.channel_a.len() != frame.channel_b.len() {
            return Err(anyhow!(
                "Channel size mismatch: A={}, B={}",
                frame.channel_a.len(),
                frame.channel_b.len()
            ));
        }

        // Entrelacer les échantillons des deux canaux (LRLRLR...)
        for (sample_a, sample_b) in frame.channel_a.iter().zip(frame.channel_b.iter()) {
            writer
                .write_sample(*sample_a)
                .map_err(|e| anyhow!("Failed to write channel A sample: {}", e))?;
            writer
                .write_sample(*sample_b)
                .map_err(|e| anyhow!("Failed to write channel B sample: {}", e))?;
        }

        // Log détaillé pour analyser le comportement
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        debug!(
            "RecordConsumerDaemon: Frame processed - #{}, {} samples/channel, timestamp: {}ms",
            frame.frame_number,
            frame.channel_a.len(),
            timestamp
        );

        Ok(())
    }

    /// Nettoyer les ressources
    fn cleanup(&mut self) {
        debug!("RecordConsumerDaemon: Cleaning up resources");

        if let Some(writer) = self.wav_writer.take() {
            if let Err(e) = writer.finalize() {
                error!("RecordConsumerDaemon: Failed to finalize WAV file: {}", e);
            } else {
                info!("RecordConsumerDaemon: WAV file finalized successfully");
            }
        }

        self.consumer = None;
        self.last_frame_time = None;
    }
    /// Obtenir les statistiques de débit actuelles
    #[allow(dead_code)]
    pub fn get_throughput_stats(&self) -> (f64, f64, f64, f64) {
        (
            self.throughput_stats.current_fps,
            self.throughput_stats.avg_frame_delay,
            self.throughput_stats.min_frame_delay,
            self.throughput_stats.max_frame_delay,
        )
    }
}

impl Drop for RecordConsumerDaemon {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::{AudioFrame, SharedAudioStream};
    use std::time::Duration;
    use tempfile::NamedTempFile;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_record_consumer_creation() {
        let stream = Arc::new(SharedAudioStream::new(10));
        let temp_file = NamedTempFile::new().unwrap();
        let output_path = temp_file.path().to_string_lossy().to_string();

        let consumer = RecordConsumerDaemon::new(stream, output_path);
        assert!(!consumer.is_running());
        assert_eq!(consumer.frames_consumed(), 0);
    }

    #[tokio::test]
    async fn test_record_consumer_with_frames() {
        let stream = Arc::new(SharedAudioStream::new(10));
        let temp_file = NamedTempFile::new().unwrap();
        let output_path = temp_file.path().to_string_lossy().to_string();

        let mut consumer = RecordConsumerDaemon::new(stream.clone(), output_path);

        // Publier quelques frames de test
        let frame1 = AudioFrame::new(vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6], 48000, 1);
        let frame2 = AudioFrame::new(vec![0.7, 0.8, 0.9], vec![1.0, 1.1, 1.2], 48000, 2);

        // Publier les frames
        stream.publish(frame1).await.unwrap();
        stream.publish(frame2).await.unwrap();

        // Démarrer le consumer dans un task séparé
        let running = consumer.running.clone();
        let frames_consumed = consumer.frames_consumed.clone();

        let consumer_task = tokio::spawn(async move {
            consumer.start().await.unwrap();
        });

        // Attendre un peu pour que le consumer traite les frames
        sleep(Duration::from_millis(100)).await;

        // Arrêter le consumer
        running.store(false, Ordering::Relaxed);

        // Attendre que le task se termine
        let _ = tokio::time::timeout(Duration::from_secs(1), consumer_task).await;

        // Vérifier que des frames ont été consommées
        assert!(frames_consumed.load(Ordering::Relaxed) > 0);
    }
}
