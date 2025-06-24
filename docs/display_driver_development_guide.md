# Guide du Développeur : Implémentation d'un Driver ActionDriver

Ce guide explique comment implémenter un nouveau driver pour le système `UniversalActionNode` de rust-photoacoustic.

## Vue d'ensemble

Le système utilise une architecture pluggable avec le trait `ActionDriver` qui permet d'abstraire différentes technologies d'affichage et protocoles de communication.

## Architecture du Système

```text
UniversalActionNode
        ↓
 ActionDriver trait  
        ↓
┌─────────────┬─────────────┬─────────────┬─────────────┐
│   HTTPS     │    Redis    │    Kafka    │  Physical   │
│  Callback   │   Driver    │   Driver    │   Drivers   │
│   Driver    │             │             │  (Future)   │
└─────────────┴─────────────┴─────────────┴─────────────┘
```

## Étapes pour Créer un Nouveau Driver

### 1. Structure de Base

Créez un nouveau fichier dans `src/processing/computing_nodes/action_drivers/` :

```rust
// src/processing/computing_nodes/action_drivers/my_driver.rs

use anyhow::Result;
use async_trait::async_trait;
use log::{debug, error, info, warn};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::SystemTime;

use super::{AlertData, MeasurementData, ActionDriver};

/// Mon driver personnalisé pour [décrire votre technologie]
#[derive(Debug)]
pub struct MyCustomDriver {
    // Configuration du driver
    endpoint_url: String,
    connection_timeout_ms: u64,
    retry_attempts: u32,
    
    // État interne
    is_connected: bool,
    last_update: Option<SystemTime>,
    error_count: u64,
    
    // Optionnel : client/connection spécifique à votre technologie
    // client: Option<MyTechnologyClient>,
}

impl MyCustomDriver {
    /// Créer une nouvelle instance du driver
    pub fn new(endpoint_url: String) -> Self {
        Self {
            endpoint_url,
            connection_timeout_ms: 5000,
            retry_attempts: 3,
            is_connected: false,
            last_update: None,
            error_count: 0,
        }
    }
    
    /// Pattern builder pour la configuration
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.connection_timeout_ms = timeout_ms;
        self
    }
    
    pub fn with_retry_attempts(mut self, attempts: u32) -> Self {
        self.retry_attempts = attempts;
        self
    }
}

#[async_trait]
impl ActionDriver for MyCustomDriver {
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing MyCustomDriver with endpoint: {}", self.endpoint_url);
        
        // TODO: Implémentez votre logique d'initialisation ici
        // Exemple :
        // - Créer une connection réseau
        // - Initialiser le hardware
        // - Vérifier les permissions
        // - Tester la connectivité
        
        self.is_connected = true;
        info!("MyCustomDriver initialized successfully");
        Ok(())
    }

    async fn update_action(&mut self, data: &MeasurementData) -> Result<()> {
        if !self.is_connected {
            return Err(anyhow::anyhow!("Driver not initialized"));
        }

        debug!(
            "MyCustomDriver: Updating display with {:.2} ppm from node '{}'",
            data.concentration_ppm, data.source_node_id
        );

        // TODO: Implémentez votre logique d'update ici
        // Exemple :
        // - Formatter les données pour votre protocole
        // - Envoyer les données via HTTP/TCP/Serial/etc.
        // - Gérer les erreurs de transmission
        // - Implémenter retry logic si nécessaire

        self.last_update = Some(SystemTime::now());
        Ok(())
    }

    async fn show_alert(&mut self, alert: &AlertData) -> Result<()> {
        if !self.is_connected {
            return Err(anyhow::anyhow!("Driver not initialized"));
        }

        warn!(
            "MyCustomDriver: Showing {} alert: {}",
            alert.severity, alert.message
        );

        // TODO: Implémentez votre logique d'alerte ici
        // Exemple :
        // - Changer la couleur d'affichage
        // - Jouer un son d'alarme
        // - Envoyer une notification push
        // - Activer des LEDs d'alarme

        Ok(())
    }

    async fn clear_action(&mut self) -> Result<()> {
        debug!("MyCustomDriver: Clearing display");

        // TODO: Implémentez votre logique de nettoyage ici
        // Exemple :
        // - Effacer l'écran
        // - Éteindre les LEDs
        // - Arrêter les sons d'alarme
        // - Remettre à l'état par défaut

        Ok(())
    }

    async fn get_status(&self) -> Result<Value> {
        Ok(json!({
            "driver_type": self.driver_type(),
            "endpoint_url": self.endpoint_url,
            "is_connected": self.is_connected,
            "connection_timeout_ms": self.connection_timeout_ms,
            "retry_attempts": self.retry_attempts,
            "error_count": self.error_count,
            "last_update": self.last_update.map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            })
        }))
    }

    fn driver_type(&self) -> &str {
        "my_custom_driver"
    }

    fn supports_realtime(&self) -> bool {
        true // ou false selon votre technologie
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("MyCustomDriver: Shutting down");

        // TODO: Implémentez votre logique de fermeture ici
        // Exemple :
        // - Fermer les connections réseau
        // - Libérer les ressources hardware
        // - Sauvegarder l'état si nécessaire

        self.is_connected = false;
        Ok(())
    }
}
```

### 2. Mise à Jour du Module Parent

Ajoutez votre driver dans `src/processing/computing_nodes/action_drivers/mod.rs` :

```rust
// Ajoutez votre module
mod my_driver;

// Re-exportez votre driver
pub use self::my_driver::MyCustomDriver;
```

### 3. Utilisation de Votre Driver

```rust
use crate::processing::computing_nodes::{
    UniversalActionNode,
    action_drivers::MyCustomDriver
};

// Créer et configurer votre driver
let my_driver = MyCustomDriver::new("https://my-api.com/display".to_string())
    .with_timeout(10000)
    .with_retry_attempts(5);

// Utiliser le driver dans un ActionNode
let display_node = UniversalActionNode::new("my_display".to_string())
    .with_history_buffer_capacity(100)
    .with_driver(Box::new(my_driver))
    .with_concentration_threshold(1000.0);
```

## Types de Données

### MeasurementData
```rust
pub struct MeasurementData {
    pub concentration_ppm: f64,        // Concentration actuelle en ppm
    pub source_node_id: String,        // ID du nœud source
    pub peak_amplitude: f32,           // Amplitude du pic (0.0-1.0)
    pub peak_frequency: f32,           // Fréquence du pic en Hz
    pub timestamp: SystemTime,         // Timestamp de la mesure
    pub metadata: HashMap<String, Value>, // Métadonnées additionnelles
}
```

### AlertData
```rust
pub struct AlertData {
    pub alert_type: String,            // Type d'alerte
    pub severity: String,              // Sévérité (info, warning, critical)
    pub message: String,               // Message lisible
    pub data: HashMap<String, Value>,  // Données spécifiques à l'alerte
    pub timestamp: SystemTime,         // Timestamp de l'alerte
}
```

## Bonnes Pratiques

### 1. Gestion d'Erreurs
- Utilisez `anyhow::Result` pour toutes les méthodes async
- Loggez les erreurs avec des niveaux appropriés (`error!`, `warn!`, `info!`, `debug!`)
- Implémentez une logique de retry pour les opérations réseau

### 2. Configuration
- Utilisez le pattern builder pour la configuration
- Fournissez des valeurs par défaut sensées
- Validez la configuration dans `initialize()`

### 3. Threading et Async
- Le trait est `async`, utilisez `async/await` pour les opérations I/O
- Votre driver doit être `Send + Sync` pour être utilisable dans un thread
- Évitez les opérations bloquantes dans les méthodes async

### 4. Logging
- Utilisez le préfixe du nom de votre driver dans les logs
- Loggez les actions importantes (connection, erreurs, données envoyées)
- Utilisez les niveaux appropriés pour ne pas polluer les logs

### 5. Status et Monitoring
- Implémentez `get_status()` avec des informations utiles pour le debugging
- Trackez les métriques importantes (nombre d'erreurs, dernière update, etc.)
- Incluez la configuration dans le status

## Exemples de Drivers par Technologie

### Driver HTTP/REST API
```rust
// Utiliser reqwest pour HTTP
use reqwest::Client;

async fn update_action(&mut self, data: &MeasurementData) -> Result<()> {
    let client = Client::new();
    let payload = json!({
        "concentration": data.concentration_ppm,
        "timestamp": data.timestamp.duration_since(UNIX_EPOCH)?.as_secs(),
        "source": data.source_node_id
    });
    
    let response = client
        .post(&self.endpoint_url)
        .json(&payload)
        .timeout(Duration::from_millis(self.timeout_ms))
        .send()
        .await?;
        
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("HTTP error: {}", response.status()));
    }
    
    Ok(())
}
```

### Driver GPIO/Hardware
```rust
// Utiliser rppal ou gpio-utils pour Raspberry Pi
async fn update_action(&mut self, data: &MeasurementData) -> Result<()> {
    // Exemple : contrôler des LEDs selon la concentration
    if data.concentration_ppm > 1000.0 {
        self.red_led.set_high();
        self.green_led.set_low();
    } else {
        self.red_led.set_low();
        self.green_led.set_high();
    }
    Ok(())
}
```

### Driver Base de Données
```rust
// Utiliser sqlx ou diesel
async fn update_action(&mut self, data: &MeasurementData) -> Result<()> {
    sqlx::query!(
        "INSERT INTO display_data (concentration, source_node, timestamp) VALUES ($1, $2, $3)",
        data.concentration_ppm,
        data.source_node_id,
        data.timestamp
    )
    .execute(&self.pool)
    .await?;
    
    Ok(())
}
```

## Tests

Créez des tests pour votre driver :

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_driver_initialization() {
        let mut driver = MyCustomDriver::new("test://localhost".to_string());
        assert!(driver.initialize().await.is_ok());
        assert!(driver.is_connected);
    }

    #[tokio::test]
    async fn test_display_update() {
        let mut driver = MyCustomDriver::new("test://localhost".to_string());
        driver.initialize().await.unwrap();
        
        let data = MeasurementData {
            concentration_ppm: 500.0,
            source_node_id: "test_node".to_string(),
            peak_amplitude: 0.5,
            peak_frequency: 2000.0,
            timestamp: SystemTime::now(),
            metadata: HashMap::new(),
        };
        
        assert!(driver.update_action(&data).await.is_ok());
    }
}
```

## Ressources Utiles

- **Async Programming** : [Tokio Guide](https://tokio.rs/tokio/tutorial)
- **Error Handling** : [anyhow documentation](https://docs.rs/anyhow/)
- **Logging** : [log crate documentation](https://docs.rs/log/)
- **JSON** : [serde_json documentation](https://docs.rs/serde_json/)

