# Analyse de Faisabilité : Évolution de l'Architecture vers les ComputingNode

## Résumé Exécutif

Cette analyse étudie la faisabilité technique et la pertinence business d'une extension de l'architecture de traitement du signal photoacoustique par l'introduction d'un **type spécial de ProcessingNode : les ComputingNode**. Cette évolution vise à enrichir l'architecture existante avec des nœuds de calcul analytique qui transmettent les données inchangées tout en effectuant des calculs sur celles-ci, permettant l'implémentation d'algorithmes sophistiqués de détection de pics spectraux et de calcul de concentration par polynômes de quatrième degré.

**Statut** : ✅ **IMPLÉMENTÉ ET VALIDÉ** - L'architecture a été évoluée avec succès pour supporter **plusieurs instances de PeakFinderNode et ConcentrationNode simultanément**, chaque nœud étant identifié par un ID unique. Les résultats sont stockés dans une structure partagée utilisant un HashMap pour permettre l'accès individuel aux données de chaque nœud.

**Nouvelle Extension** : 🚀 **ACTIONNODE - TRAIT IMPLÉMENTÉ** ✅ - Extension de l'architecture vers des nœuds d'action spécialisés pour la gestion d'interfaces physiques (écrans, relais, notifications email) avec buffer circulaire configurable et liaison directe aux ComputingNode. Le trait ActionNode étend ProcessingNode avec des capacités de monitoring, triggers configurables et gestion d'historique.

**Architecture UniversalActionNode Complète** : ✅ **DRIVERS PLUGGABLES OPÉRATIONNELS** - Implémentation complète du pattern de drivers modulaires avec `UniversalActionNode` supportant :
- **HttpsCallbackActionDriver** : Callbacks HTTP/HTTPS pour dashboards web et intégration cloud
- **RedisActionDriver** : Pub/sub Redis pour streaming temps réel et mise en cache
- **KafkaActionDriver** : Messaging Kafka pour architectures de streaming scalables
- **Thread-based Processing** : Traitement asynchrone via threads internes avec channels pour compatibilité sync/async
- **Configuration YAML** : Création et configuration des drivers directement depuis les fichiers de configuration

**Recommandation** : ✅ **ARCHITECTURE COMPLÈTE** - Le système dispose maintenant d'une architecture en 3 couches (Signal Processing → Analytics → Actions) parfaitement intégrée. L'implémentation du trait ActionNode avec l'UniversalActionNode et ses drivers pluggables ouvre la voie aux nœuds d'action spécialisés tout en maintenant l'intégrité du pipeline de traitement signal.

## Évolution Récente de la Nomenclature (Juin 2025)

**🔄 Changements de Noms pour Cohérence Architecture** :
- ✅ `UniversalDisplayActionNode` → **`UniversalActionNode`** : Nom plus générique et approprié
- ✅ `HttpsCallbackDisplayDriver` → **`HttpsCallbackActionDriver`** : Cohérence avec le concept d'action
- ✅ `RedisDisplayDriver` → **`RedisActionDriver`** : Simplification et cohérence
- ✅ `KafkaDisplayDriver` → **`KafkaActionDriver`** : Alignement terminologique
- ✅ `DisplayDriver` trait → **`ActionDriver`** trait : Généralisation du concept

**Justification** : Ces changements améliorent la cohérence architecturale en utilisant une terminologie uniforme autour du concept d'**ActionNode** et d'**ActionDriver**, facilitant la compréhension et l'extension future du système vers d'autres types d'actions (relais, notifications, bases de données, etc.).

---

## 1. Architecture Technique Actuelle

### 1.1 État des lieux du ProcessingGraph

L'analyse du code révèle une architecture modulaire robuste :

- **Trait ProcessingNode** : Interface unifiée pour tous les nœuds de traitement
- **ProcessingData** : Types de données standardisés transitant dans le graphe
- **Exécution séquentielle** : Ordre topologique des nœuds garantissant l'intégrité du flux
- **Gestion d'état partagé** : Système de registres (StreamingNodeRegistry) pour la coordination entre nœuds

### 1.2 Capacités existantes pertinentes

- **Shared State Management** : `SharedVisualizationState` et `StreamingNodeRegistry` démontrent la capacité du système à gérer des états partagés entre composants
- **Hot Reload** : Support de la reconfiguration dynamique des paramètres
- **Statistiques de performance** : Monitoring complet des performances par nœud
- **Sérialisation** : Capacité à exporter et synchroniser l'état du graphe

---

## 2. Conception des ComputingNode comme ProcessingNode Spécialisés

### 2.1 Héritage et spécialisation

Les ComputingNode sont des **ProcessingNode spécialisés** qui :
- **Implémentent le trait ProcessingNode** : Compatibilité totale avec l'architecture existante
- **Fonction pass-through** : Transmettent `ProcessingData` inchangé vers le nœud suivant
- **Calculs parallèles** : Effectuent des analyses sur les données transitantes
- **État partagé** : Publient leurs résultats dans un registre global accessible

#### Spécialisation du trait ProcessingNode - Multi-Instances Support
```rust
impl ProcessingNode for PeakFinderNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // 1. Analyser les données (calcul FFT, détection pic)
        if let Some(peak_info) = self.analyze_spectrum(&input)? {
            // 2. Créer le résultat avec horodatage
            let peak_result = PeakResult {
                frequency: peak_info.frequency,
                amplitude: peak_info.amplitude,
                concentration_ppm: None, // Calculé par ConcentrationNode si présent
                timestamp: SystemTime::now(),
            };
            
            // 3. Mettre à jour l'état partagé avec l'ID unique du nœud
            if let Ok(mut state) = self.shared_state.try_write() {
                state.update_peak_result(self.id.clone(), peak_result);
            }
        }
        
        // 4. Transmettre les données INCHANGÉES
        Ok(input)  // Pass-through complet
    }
    
    fn node_type(&self) -> &str { "computing_peak_finder" }
    
    /// Retourne l'ID unique de ce nœud
    fn get_id(&self) -> &str { &self.id }
}
```

**Capacités Multi-Nœuds Implémentées :**
- **Identification Unique** : Chaque `PeakFinderNode` possède un `id` unique
- **Stockage Individuel** : Les résultats sont stockés par ID dans le HashMap
- **Accès Concurrent** : Plusieurs nœuds peuvent mettre à jour l'état simultanément
- **Pas d'Interference** : Les calculs de chaque nœud sont indépendants

#### État partagé global - Architecture Multi-Nœuds
```rust
/// Result data from a peak finder node
#[derive(Debug, Clone)]
pub struct PeakResult {
    /// Detected peak frequency in Hz
    pub frequency: f32,
    /// Detected peak amplitude (normalized, 0.0 to 1.0)
    pub amplitude: f32,
    /// Concentration in parts per million (ppm) derived from frequency
    pub concentration_ppm: Option<f32>,
    /// Timestamp of when this peak was detected
    pub timestamp: SystemTime,
}

pub struct ComputingSharedData {
    /// Peak detection results from multiple nodes, keyed by node ID
    pub peak_results: HashMap<String, PeakResult>,
    
    // Legacy fields for backward compatibility
    pub peak_frequency: Option<f32>,
    pub peak_amplitude: Option<f32>,
    pub concentration_ppm: Option<f32>,
    pub polynomial_coefficients: [f64; 5], // a₀ + a₁x + a₂x² + a₃x³ + a₄x⁴
    pub last_update: SystemTime,
}

impl ComputingSharedData {
    /// Get peak result for a specific node ID
    pub fn get_peak_result(&self, node_id: &str) -> Option<&PeakResult> {
        self.peak_results.get(node_id)
    }

    /// Update peak result for a specific node ID
    pub fn update_peak_result(&mut self, node_id: String, result: PeakResult) {
        // Update the HashMap
        self.peak_results.insert(node_id.clone(), result.clone());
        
        // Update legacy fields for backward compatibility
        self.peak_frequency = Some(result.frequency);
        self.peak_amplitude = Some(result.amplitude);
        self.concentration_ppm = result.concentration_ppm;
        self.last_update = result.timestamp;
    }

    /// Get the most recent peak result across all nodes
    pub fn get_latest_peak_result(&self) -> Option<&PeakResult> {
        self.peak_results
            .values()
            .max_by_key(|result| result.timestamp)
    }

    /// Get all node IDs that have peak results
    pub fn get_peak_finder_node_ids(&self) -> Vec<String> {
        self.peak_results.keys().cloned().collect()
    }

    /// Check if a node has recent peak data (within last 30 seconds)
    pub fn has_recent_peak_data(&self, node_id: &str) -> bool {
        if let Some(result) = self.peak_results.get(node_id) {
            if let Ok(elapsed) = result.timestamp.elapsed() {
                elapsed.as_secs() < 30
            } else {
                false
            }
        } else {
            false
        }
    }
}

pub type SharedComputingState = Arc<RwLock<ComputingSharedData>>;
```

**Évolutions Clés Implémentées :**
- **Support Multi-Nœuds** : Plusieurs `PeakFinderNode` peuvent coexister avec des IDs uniques
- **Stockage par HashMap** : `peak_results` indexé par `node_id` permet l'accès individuel
- **Rétrocompatibilité** : Les champs legacy (`peak_frequency`, `peak_amplitude`) restent fonctionnels
- **Gestion Temporelle** : Horodatage et validation de fraîcheur des données par nœud
- **API Utilitaire** : Méthodes pour accéder aux résultats individuels ou collectifs

### 2.2 Intégration transparente dans le ProcessingGraph

#### Avantages de cette approche
- **Compatibilité totale** : Aucune modification du moteur ProcessingGraph
- **Insertion flexible** : ComputingNode peut s'insérer n'importe où dans le pipeline
- **Performance optimisée** : Pas de duplication de données, calculs en parallèle
- **Observabilité** : Statistiques de performance comme les autres ProcessingNode

#### Mécanisme de notification et accès partagé
- **Registre global ComputingStateRegistry** : Similaire à `StreamingNodeRegistry`
- **Accès non-bloquant** : Les ProcessingNode lisent les résultats via `try_read()`
- **Validation temporelle** : Horodatage pour éviter l'utilisation de données obsolètes
- **Pattern Observer** : Notifications optionnelles pour mise à jour en temps réel

---

## 3. Implémentations Proposées

### 3.1 PeakFinderNode (ComputingNode spécialisé) - Support Multi-Instances

#### Structure et fonctionnalités
```rust
pub struct PeakFinderNode {
    /// Unique identifier for this node - CRITIQUE pour le support multi-instances
    id: String,
    
    /// Shared state for communicating results to other nodes
    shared_state: SharedComputingState,
    
    /// FFT configuration and buffers
    fft_buffer: Vec<Complex<f32>>,
    fft_size: usize,
    sample_rate: u32,
    
    /// Detection parameters
    frequency_range: (f32, f32),  // Bande de recherche
    detection_threshold: f32,
    smoothing_factor: f32,
    
    /// Performance monitoring
    processing_count: u64,
    last_detection_time: Option<SystemTime>,
}

impl ProcessingNode for PeakFinderNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Analyse spectrale sur les données
        if let Some(peak_info) = self.analyze_spectrum(&input)? {
            // Création du résultat avec ID unique
            let peak_result = PeakResult {
                frequency: peak_info.frequency,
                amplitude: peak_info.amplitude,
                concentration_ppm: None,
                timestamp: SystemTime::now(),
            };
            
            // Mise à jour de l'état partagé avec clé unique
            if let Ok(mut state) = self.shared_state.try_write() {
                state.update_peak_result(self.id.clone(), peak_result);
            }
        }
        
        // Transmission des données inchangées
        Ok(input)
    }
    
    fn node_type(&self) -> &str { "computing_peak_finder" }
}
```

**Nouveautés Multi-Instances :**
- **ID Obligatoire** : Chaque instance doit avoir un `id` unique lors de l'instanciation
- **Stockage Cloisonné** : Chaque nœud stocke ses résultats indépendamment
- **Configuration Individuelle** : Chaque instance peut avoir des paramètres différents
  - Bandes de fréquence distinctes (ex: 800-1200 Hz vs 1500-2000 Hz)
  - Seuils de détection adaptés à chaque signal
  - Facteurs de lissage optimisés par usage

#### Fonctionnalités étendues
- **Analyse spectrale FFT** : Détection automatique de la fréquence de résonance par instance
- **Algorithme de détection de pics** : Recherche du maximum local dans la bande configurée
- **Filtrage adaptatif** : Élimination des pics parasites par analyse de cohérence temporelle
- **Suivi temporel indépendant** : Moyenne glissante et historique par nœud
- **Validation croisée** : Possibilité de comparer les résultats entre instances

### 3.2 ConcentrationNode (ComputingNode spécialisé) - IMPLÉMENTATION COMPLÈTE

#### Architecture Multi-Instances avec Polynômes Individuels

**🎯 Objectifs Atteints :**
- ✅ **Instances Multiples** : Chaque ConcentrationNode a son propre ID unique
- ✅ **Polynômes Configurables** : Coefficients individuels par instance
- ✅ **Liaison Sélective** : Paramètre `computing_peak_finder_id` pour source spécifique
- ✅ **Hot-Reload** : Reconfiguration dynamique des polynômes sans redémarrage

#### Structure complète implémentée
```rust
pub struct ConcentrationNode {
    /// Unique identifier for this node
    id: String,

    /// ID of the PeakFinderNode to use as data source
    /// If None, uses the most recent peak data available
    computing_peak_finder_id: Option<String>,

    /// Polynomial coefficients for concentration calculation [a₀, a₁, a₂, a₃, a₄]
    /// Concentration(ppm) = a₀ + a₁·A + a₂·A² + a₃·A³ + a₄·A⁴
    /// where A is the normalized peak amplitude
    polynomial_coefficients: [f64; 5],

    /// Enable temperature compensation for improved accuracy
    temperature_compensation: bool,

    /// Optional identifier for the spectral line being analyzed
    spectral_line_id: Option<String>,

    /// Minimum amplitude threshold for valid concentration calculation
    min_amplitude_threshold: f32,

    /// Maximum concentration limit for safety/validation
    max_concentration_ppm: f32,

    /// Shared state for communicating results to other nodes
    shared_state: Arc<RwLock<ComputingSharedData>>,

    /// Statistics for monitoring performance
    processing_count: u64,
    calculation_count: u64,
    last_calculation_time: Option<SystemTime>,
}

impl ProcessingNode for ConcentrationNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Lecture sélective depuis l'état partagé basée sur computing_peak_finder_id
        let peak_result = match self.shared_state.try_read() {
            Ok(state) => {
                if let Some(source_id) = &self.computing_peak_finder_id {
                    // Lecture depuis un PeakFinder spécifique
                    state.get_peak_result(source_id).cloned()
                } else {
                    // Lecture du résultat le plus récent (comportement automatique)
                    state.get_latest_peak_result().cloned()
                }
            }
            Err(_) => None
        };

        // Calcul de concentration si données disponibles
        if let Some(peak_data) = peak_result {
            if peak_data.amplitude >= self.min_amplitude_threshold {
                let concentration = self.calculate_concentration(peak_data.amplitude);
                self.update_shared_state(&peak_data, concentration);
            }
        }

        // Transmission des données inchangées (pass-through)
        Ok(input)
    }
}
```

#### Calcul Polynomial Avancé - Équation Physique

**Modèle Physique Implémenté** : Relation amplitude-concentration selon la loi de Beer-Lambert modifiée pour la photoacoustique

```mathematica
C(ppm) = a₀ + a₁·A + a₂·A² + a₃·A³ + a₄·A⁴
```

**Où** :
- `A` = amplitude normalisée du pic détecté (0.0 à 1.0)
- `C` = concentration en parties par million (ppm)
- `[a₀, a₁, a₂, a₃, a₄]` = coefficients du polynôme de 4ème degré

**Caractéristiques d'Implémentation** :
- **Validation d'Amplitude** : Seuil minimum configurable (`min_amplitude_threshold`)
- **Limitation de Sécurité** : Concentration maximale configurable (`max_concentration_ppm`)
- **Précision Numérique** : Calculs en double précision (f64) avec conversion finale f32
- **Compensation Thermique** : Support optionnel pour correction de température

#### Avantages Multi-Instances - Cas d'Usage Validés

**🔬 1. Test de Nouveaux Polynômes**
```yaml
processing:
  nodes:
    # Polynôme de référence (actuellement utilisé)
    - id: "concentration_reference"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "primary_peak_finder"
        polynomial_coefficients: [0.0, 0.45, -0.002, 0.0001, 0.0]
        spectral_line_id: "reference_polynomial_v2.1"
        temperature_compensation: true

    # Nouveau polynôme en test (parallèle)
    - id: "concentration_test"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "primary_peak_finder"  # Même source
        polynomial_coefficients: [0.1, 0.52, -0.0025, 0.00015, -0.000001]
        spectral_line_id: "experimental_polynomial_v3.0"
        temperature_compensation: true
        min_amplitude_threshold: 0.002  # Plus restrictif pour tests
```

**Avantages** :
- **Comparaison Temps Réel** : Deux calculs simultanés sur les mêmes données
- **Validation Progressive** : Test de nouveaux modèles sans arrêter la production
- **Analyse de Performance** : Métriques comparatives automatiques

**🎯 2. Calculs Multi-Raies Spectrales**
```yaml
processing:
  nodes:
    # Raie principale du méthane (2ν₃ à ~2100 cm⁻¹)
    - id: "concentration_main_line"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "main_line_detector"
        polynomial_coefficients: [0.0, 0.45, -0.002, 0.0001, 0.0]
        spectral_line_id: "CH4_2v3_main"

    # Raie secondaire pour validation croisée (ν₄ à ~1300 cm⁻¹)
    - id: "concentration_secondary_line"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "secondary_line_detector"
        polynomial_coefficients: [0.05, 0.38, -0.0018, 0.00008, 0.0]
        spectral_line_id: "CH4_v4_secondary"
        temperature_compensation: false  # Différente configuration
```

**Applications Métrologiques** :
- **Redondance Spectrale** : Mesure sur plusieurs raies pour fiabilité
- **Validation Croisée** : Cohérence entre différentes transitions moléculaires
- **Compensation d'Interférences** : Correction des interférences spectrales

**⚡ 3. Positionnement dans le Pipeline - Flexibilité Architecturale**

**Configuration Avant Filtrage** (Recommandée pour analyse large bande) :
```yaml
connections:
- from: differential_detection
  to: concentration_early      # AVANT le filtrage
- from: concentration_early
  to: bandpass_filter
```

**Configuration Après Filtrage** (Optimisée pour SNR) :
```yaml
connections:
- from: bandpass_filter
  to: concentration_late       # APRÈS le filtrage
- from: concentration_late
  to: gain_amplifier
```

**Impacts Techniques Analysés** :

| Position | Avantages | Inconvénients | Cas d'Usage |
|----------|-----------|---------------|-------------|
| **Avant Filtrage** | • Analyse large bande<br>• Détection multi-harmoniques<br>• Moins de distorsion | • Plus de bruit<br>• Calculs sur signal brut | • Test de nouveaux polynômes<br>• Analyse exploratoire |
| **Après Filtrage** | • Meilleur SNR<br>• Signal optimisé<br>• Précision maximale | • Bande limitée<br>• Dépendant du filtre | • Production<br>• Mesures de précision |

#### Configuration Hot-Reload - Paramètres Supportés

**Paramètres Reconfigurables en Temps Réel** :
```json
{
  "polynomial_coefficients": [0.1, 0.52, -0.0025, 0.00015, -0.000001],
  "min_amplitude_threshold": 0.002,
  "max_concentration_ppm": 8000.0,
  "computing_peak_finder_id": "backup_peak_finder"
}
```

**Tests de Validation Hot-Reload** :
- ✅ **Polynômes** : Coefficients modifiables sans interruption
- ✅ **Seuils** : Ajustement dynamique des limites
- ✅ **Source** : Basculement entre PeakFinderNode
- ✅ **Paramètres Sécurité** : Limites min/max en temps réel

### 3.3 DynamicFilterNode (ProcessingNode enrichi) - Support Multi-Fréquences

#### Adaptation basée sur l'état partagé multi-sources
```rust
pub struct DynamicFilterNode {
    id: String,
    base_filter: Box<dyn Filter>,
    fusion_mode: FrequencyFusionMode,
    shared_state: Arc<RwLock<ComputingSharedData>>,
}

#[derive(Debug, Clone)]
pub enum FrequencyFusionMode {
    /// Utilise la fréquence du PeakFinder le plus récent
    MostRecent,
    /// Utilise la fréquence avec la plus forte amplitude
    HighestAmplitude,
    /// Moyenne pondérée par amplitude
    WeightedAverage,
    /// Utilise une fréquence spécifique par node_id
    SelectiveBinding(String),
}

impl ProcessingNode for DynamicFilterNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Lecture de l'état partagé pour obtenir les fréquences détectées
        if let Ok(state) = self.shared_state.try_read() {
            if let Some(target_frequency) = self.calculate_target_frequency(&state)? {
                // Adapter la fréquence centrale du filtre
                self.base_filter.set_center_frequency(target_frequency)?;
            }
        }
        
        // Appliquer le filtrage avec la fréquence adaptée
        self.base_filter.process(input)
    }
    
    fn node_type(&self) -> &str { "dynamic_filter" }
    fn node_id(&self) -> &str { &self.id }
}

impl DynamicFilterNode {
    fn calculate_target_frequency(&self, state: &ComputingSharedData) -> Result<Option<f32>> {
        match self.fusion_mode {
            FrequencyFusionMode::MostRecent => {
                Ok(state.get_latest_peak_result().map(|r| r.frequency))
            }
            FrequencyFusionMode::HighestAmplitude => {
                let best_peak = state.peak_results
                    .values()
                    .max_by(|a, b| a.amplitude.partial_cmp(&b.amplitude).unwrap());
                Ok(best_peak.map(|r| r.frequency))
            }
            FrequencyFusionMode::WeightedAverage => {
                self.calculate_weighted_average_frequency(state)
            }
            FrequencyFusionMode::SelectiveBinding(ref node_id) => {
                Ok(state.get_peak_result(node_id).map(|r| r.frequency))
            }
        }
    }
    
    fn calculate_weighted_average_frequency(&self, state: &ComputingSharedData) -> Result<Option<f32>> {
        let recent_results: Vec<_> = state.peak_results
            .values()
            .filter(|result| {
                result.timestamp.elapsed().unwrap_or_default().as_secs() < 30
            })
            .collect();
            
        if recent_results.is_empty() {
            return Ok(None);
        }
        
        let total_weight: f32 = recent_results.iter().map(|r| r.amplitude).sum();
        
        if total_weight == 0.0 {
            return Ok(None);
        }
        
        let weighted_freq = recent_results.iter()
            .map(|r| r.frequency * r.amplitude)
            .sum::<f32>() / total_weight;
            
        Ok(Some(weighted_freq))
    }
}
```

**Évolutions Multi-Fréquences** :
- **Stratégies de Fusion** : Plusieurs modes pour combiner les fréquences multiples
- **Adaptation Intelligente** : Choix automatique de la meilleure fréquence de référence
- **Pondération par Amplitude** : Priorité aux signaux les plus forts
- **Validation Temporelle** : Utilise uniquement les données récentes et valides

---

## 4. Implémentations Proposées

### 4.1 PeakFinderNode (ComputingNode spécialisé) - Support Multi-Instances

#### Structure et fonctionnalités
```rust
pub struct PeakFinderNode {
    /// Unique identifier for this node - CRITIQUE pour le support multi-instances
    id: String,
    
    /// Shared state for communicating results to other nodes
    shared_state: SharedComputingState,
    
    /// FFT configuration and buffers
    fft_buffer: Vec<Complex<f32>>,
    fft_size: usize,
    sample_rate: u32,
    
    /// Detection parameters
    frequency_range: (f32, f32),  // Bande de recherche
    detection_threshold: f32,
    smoothing_factor: f32,
    
    /// Performance monitoring
    processing_count: u64,
    last_detection_time: Option<SystemTime>,
}

impl ProcessingNode for PeakFinderNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Analyse spectrale sur les données
        if let Some(peak_info) = self.analyze_spectrum(&input)? {
            // Création du résultat avec ID unique
            let peak_result = PeakResult {
                frequency: peak_info.frequency,
                amplitude: peak_info.amplitude,
                concentration_ppm: None,
                timestamp: SystemTime::now(),
            };
            
            // Mise à jour de l'état partagé avec clé unique
            if let Ok(mut state) = self.shared_state.try_write() {
                state.update_peak_result(self.id.clone(), peak_result);
            }
        }
        
        // Transmission des données inchangées
        Ok(input)
    }
    
    fn node_type(&self) -> &str { "computing_peak_finder" }
}
```

**Nouveautés Multi-Instances :**
- **ID Obligatoire** : Chaque instance doit avoir un `id` unique lors de l'instanciation
- **Stockage Cloisonné** : Chaque nœud stocke ses résultats indépendamment
- **Configuration Individuelle** : Chaque instance peut avoir des paramètres différents
  - Bandes de fréquence distinctes (ex: 800-1200 Hz vs 1500-2000 Hz)
  - Seuils de détection adaptés à chaque signal
  - Facteurs de lissage optimisés par usage

#### Fonctionnalités étendues
- **Analyse spectrale FFT** : Détection automatique de la fréquence de résonance par instance
- **Algorithme de détection de pics** : Recherche du maximum local dans la bande configurée
- **Filtrage adaptatif** : Élimination des pics parasites par analyse de cohérence temporelle
- **Suivi temporel indépendant** : Moyenne glissante et historique par nœud
- **Validation croisée** : Possibilité de comparer les résultats entre instances

### 4.2 ConcentrationNode (ComputingNode spécialisé) - IMPLÉMENTATION COMPLÈTE

#### Architecture Multi-Instances avec Polynômes Individuels

**🎯 Objectifs Atteints :**
- ✅ **Instances Multiples** : Chaque ConcentrationNode a son propre ID unique
- ✅ **Polynômes Configurables** : Coefficients individuels par instance
- ✅ **Liaison Sélective** : Paramètre `computing_peak_finder_id` pour source spécifique
- ✅ **Hot-Reload** : Reconfiguration dynamique des polynômes sans redémarrage

#### Structure complète implémentée
```rust
pub struct ConcentrationNode {
    /// Unique identifier for this node
    id: String,

    /// ID of the PeakFinderNode to use as data source
    /// If None, uses the most recent peak data available
    computing_peak_finder_id: Option<String>,

    /// Polynomial coefficients for concentration calculation [a₀, a₁, a₂, a₃, a₄]
    /// Concentration(ppm) = a₀ + a₁·A + a₂·A² + a₃·A³ + a₄·A⁴
    /// where A is the normalized peak amplitude
    polynomial_coefficients: [f64; 5],

    /// Enable temperature compensation for improved accuracy
    temperature_compensation: bool,

    /// Optional identifier for the spectral line being analyzed
    spectral_line_id: Option<String>,

    /// Minimum amplitude threshold for valid concentration calculation
    min_amplitude_threshold: f32,

    /// Maximum concentration limit for safety/validation
    max_concentration_ppm: f32,

    /// Shared state for communicating results to other nodes
    shared_state: Arc<RwLock<ComputingSharedData>>,

    /// Statistics for monitoring performance
    processing_count: u64,
    calculation_count: u64,
    last_calculation_time: Option<SystemTime>,
}

impl ProcessingNode for ConcentrationNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Lecture sélective depuis l'état partagé basée sur computing_peak_finder_id
        let peak_result = match self.shared_state.try_read() {
            Ok(state) => {
                if let Some(source_id) = &self.computing_peak_finder_id {
                    // Lecture depuis un PeakFinder spécifique
                    state.get_peak_result(source_id).cloned()
                } else {
                    // Lecture du résultat le plus récent (comportement automatique)
                    state.get_latest_peak_result().cloned()
                }
            }
            Err(_) => None
        };

        // Calcul de concentration si données disponibles
        if let Some(peak_data) = peak_result {
            if peak_data.amplitude >= self.min_amplitude_threshold {
                let concentration = self.calculate_concentration(peak_data.amplitude);
                self.update_shared_state(&peak_data, concentration);
            }
        }

        // Transmission des données inchangées (pass-through)
        Ok(input)
    }
}
```

#### Calcul Polynomial Avancé - Équation Physique

**Modèle Physique Implémenté** : Relation amplitude-concentration selon la loi de Beer-Lambert modifiée pour la photoacoustique

```mathematica
C(ppm) = a₀ + a₁·A + a₂·A² + a₃·A³ + a₄·A⁴
```

**Où** :
- `A` = amplitude normalisée du pic détecté (0.0 à 1.0)
- `C` = concentration en parties par million (ppm)
- `[a₀, a₁, a₂, a₃, a₄]` = coefficients du polynôme de 4ème degré

**Caractéristiques d'Implémentation** :
- **Validation d'Amplitude** : Seuil minimum configurable (`min_amplitude_threshold`)
- **Limitation de Sécurité** : Concentration maximale configurable (`max_concentration_ppm`)
- **Précision Numérique** : Calculs en double précision (f64) avec conversion finale f32
- **Compensation Thermique** : Support optionnel pour correction de température

#### Avantages Multi-Instances - Cas d'Usage Validés

**🔬 1. Test de Nouveaux Polynômes**
```yaml
processing:
  nodes:
    # Polynôme de référence (actuellement utilisé)
    - id: "concentration_reference"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "primary_peak_finder"
        polynomial_coefficients: [0.0, 0.45, -0.002, 0.0001, 0.0]
        spectral_line_id: "reference_polynomial_v2.1"
        temperature_compensation: true

    # Nouveau polynôme en test (parallèle)
    - id: "concentration_test"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "primary_peak_finder"  # Même source
        polynomial_coefficients: [0.1, 0.52, -0.0025, 0.00015, -0.000001]
        spectral_line_id: "experimental_polynomial_v3.0"
        temperature_compensation: true
        min_amplitude_threshold: 0.002  # Plus restrictif pour tests
```

**Avantages** :
- **Comparaison Temps Réel** : Deux calculs simultanés sur les mêmes données
- **Validation Progressive** : Test de nouveaux modèles sans arrêter la production
- **Analyse de Performance** : Métriques comparatives automatiques

**🎯 2. Calculs Multi-Raies Spectrales**
```yaml
processing:
  nodes:
    # Raie principale du méthane (2ν₃ à ~2100 cm⁻¹)
    - id: "concentration_main_line"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "main_line_detector"
        polynomial_coefficients: [0.0, 0.45, -0.002, 0.0001, 0.0]
        spectral_line_id: "CH4_2v3_main"

    # Raie secondaire pour validation croisée (ν₄ à ~1300 cm⁻¹)
    - id: "concentration_secondary_line"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "secondary_line_detector"
        polynomial_coefficients: [0.05, 0.38, -0.0018, 0.00008, 0.0]
        spectral_line_id: "CH4_v4_secondary"
        temperature_compensation: false  # Différente configuration
```

**Applications Métrologiques** :
- **Redondance Spectrale** : Mesure sur plusieurs raies pour fiabilité
- **Validation Croisée** : Cohérence entre différentes transitions moléculaires
- **Compensation d'Interférences** : Correction des interférences spectrales

**⚡ 3. Positionnement dans le Pipeline - Flexibilité Architecturale**

**Configuration Avant Filtrage** (Recommandée pour analyse large bande) :
```yaml
connections:
- from: differential_detection
  to: concentration_early      # AVANT le filtrage
- from: concentration_early
  to: bandpass_filter
```

**Configuration Après Filtrage** (Optimisée pour SNR) :
```yaml
connections:
- from: bandpass_filter
  to: concentration_late       # APRÈS le filtrage
- from: concentration_late
  to: gain_amplifier
```

**Impacts Techniques Analysés** :

| Position | Avantages | Inconvénients | Cas d'Usage |
|----------|-----------|---------------|-------------|
| **Avant Filtrage** | • Analyse large bande<br>• Détection multi-harmoniques<br>• Moins de distorsion | • Plus de bruit<br>• Calculs sur signal brut | • Test de nouveaux polynômes<br>• Analyse exploratoire |
| **Après Filtrage** | • Meilleur SNR<br>• Signal optimisé<br>• Précision maximale | • Bande limitée<br>• Dépendant du filtre | • Production<br>• Mesures de précision |

#### Configuration Hot-Reload - Paramètres Supportés

**Paramètres Reconfigurables en Temps Réel** :
```json
{
  "polynomial_coefficients": [0.1, 0.52, -0.0025, 0.00015, -0.000001],
  "min_amplitude_threshold": 0.002,
  "max_concentration_ppm": 8000.0,
  "computing_peak_finder_id": "backup_peak_finder"
}
```

**Tests de Validation Hot-Reload** :
- ✅ **Polynômes** : Coefficients modifiables sans interruption
- ✅ **Seuils** : Ajustement dynamique des limites
- ✅ **Source** : Basculement entre PeakFinderNode
- ✅ **Paramètres Sécurité** : Limites min/max en temps réel

### 3.3 DynamicFilterNode (ProcessingNode enrichi) - Support Multi-Fréquences

#### Adaptation basée sur l'état partagé multi-sources
```rust
pub struct DynamicFilterNode {
    id: String,
    base_filter: Box<dyn Filter>,
    fusion_mode: FrequencyFusionMode,
    shared_state: Arc<RwLock<ComputingSharedData>>,
}

#[derive(Debug, Clone)]
pub enum FrequencyFusionMode {
    /// Utilise la fréquence du PeakFinder le plus récent
    MostRecent,
    /// Utilise la fréquence avec la plus forte amplitude
    HighestAmplitude,
    /// Moyenne pondée par amplitude
    WeightedAverage,
    /// Utilise une fréquence spécifique par node_id
    SelectiveBinding(String),
}

impl ProcessingNode for DynamicFilterNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Lecture de l'état partagé pour obtenir les fréquences détectées
        if let Ok(state) = self.shared_state.try_read() {
            if let Some(target_frequency) = self.calculate_target_frequency(&state)? {
                // Adapter la fréquence centrale du filtre
                self.base_filter.set_center_frequency(target_frequency)?;
            }
        }
        
        // Appliquer le filtrage avec la fréquence adaptée
        self.base_filter.process(input)
    }
    
    fn node_type(&self) -> &str { "dynamic_filter" }
    fn node_id(&self) -> &str { &self.id }
}

impl DynamicFilterNode {
    fn calculate_target_frequency(&self, state: &ComputingSharedData) -> Result<Option<f32>> {
        match self.fusion_mode {
            FrequencyFusionMode::MostRecent => {
                Ok(state.get_latest_peak_result().map(|r| r.frequency))
            }
            FrequencyFusionMode::HighestAmplitude => {
                let best_peak = state.peak_results
                    .values()
                    .max_by(|a, b| a.amplitude.partial_cmp(&b.amplitude).unwrap());
                Ok(best_peak.map(|r| r.frequency))
            }
            FrequencyFusionMode::WeightedAverage => {
                self.calculate_weighted_average_frequency(state)
            }
            FrequencyFusionMode::SelectiveBinding(ref node_id) => {
                Ok(state.get_peak_result(node_id).map(|r| r.frequency))
            }
        }
    }
    
    fn calculate_weighted_average_frequency(&self, state: &ComputingSharedData) -> Result<Option<f32>> {
        let recent_results: Vec<_> = state.peak_results
            .values()
            .filter(|result| {
                result.timestamp.elapsed().unwrap_or_default().as_secs() < 30
            })
            .collect();
            
        if recent_results.is_empty() {
            return Ok(None);
        }
        
        let total_weight: f32 = recent_results.iter().map(|r| r.amplitude).sum();
        
        if total_weight == 0.0 {
            return Ok(None);
        }
        
        let weighted_freq = recent_results.iter()
            .map(|r| r.frequency * r.amplitude)
            .sum::<f32>() / total_weight;
            
        Ok(Some(weighted_freq))
    }
}
```

**Évolutions Multi-Fréquences** :
- **Stratégies de Fusion** : Plusieurs modes pour combiner les fréquences multiples
- **Adaptation Intelligente** : Choix automatique de la meilleure fréquence de référence
- **Pondération par Amplitude** : Priorité aux signaux les plus forts
- **Validation Temporelle** : Utilise uniquement les données récentes et valides

---

## 4. Implémentations Proposées

### 4.1 PeakFinderNode (ComputingNode spécialisé) - Support Multi-Instances

#### Structure et fonctionnalités
```rust
pub struct PeakFinderNode {
    /// Unique identifier for this node - CRITIQUE pour le support multi-instances
    id: String,
    
    /// Shared state for communicating results to other nodes
    shared_state: SharedComputingState,
    
    /// FFT configuration and buffers
    fft_buffer: Vec<Complex<f32>>,
    fft_size: usize,
    sample_rate: u32,
    
    /// Detection parameters
    frequency_range: (f32, f32),  // Bande de recherche
    detection_threshold: f32,
    smoothing_factor: f32,
    
    /// Performance monitoring
    processing_count: u64,
    last_detection_time: Option<SystemTime>,
}

impl ProcessingNode for PeakFinderNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Analyse spectrale sur les données
        if let Some(peak_info) = self.analyze_spectrum(&input)? {
            // Création du résultat avec ID unique
            let peak_result = PeakResult {
                frequency: peak_info.frequency,
                amplitude: peak_info.amplitude,
                concentration_ppm: None,
                timestamp: SystemTime::now(),
            };
            
            // Mise à jour de l'état partagé avec clé unique
            if let Ok(mut state) = self.shared_state.try_write() {
                state.update_peak_result(self.id.clone(), peak_result);
            }
        }
        
        // Transmission des données inchangées
        Ok(input)
    }
    
    fn node_type(&self) -> &str { "computing_peak_finder" }
}
```

**Nouveautés Multi-Instances :**
- **ID Obligatoire** : Chaque instance doit avoir un `id` unique lors de l'instanciation
- **Stockage Cloisonné** : Chaque nœud stocke ses résultats indépendamment
- **Configuration Individuelle** : Chaque instance peut avoir des paramètres différents
  - Bandes de fréquence distinctes (ex: 800-1200 Hz vs 1500-2000 Hz)
  - Seuils de détection adaptés à chaque signal
  - Facteurs de lissage optimisés par usage

#### Fonctionnalités étendues
- **Analyse spectrale FFT** : Détection automatique de la fréquence de résonance par instance
- **Algorithme de détection de pics** : Recherche du maximum local dans la bande configurée
- **Filtrage adaptatif** : Élimination des pics parasites par analyse de cohérence temporelle
- **Suivi temporel indépendant** : Moyenne glissante et historique par nœud
- **Validation croisée** : Possibilité de comparer les résultats entre instances

### 4.2 ConcentrationNode (ComputingNode spécialisé) - IMPLÉMENTATION COMPLÈTE

#### Architecture Multi-Instances avec Polynômes Individuels

**🎯 Objectifs Atteints :**
- ✅ **Instances Multiples** : Chaque ConcentrationNode a son propre ID unique
- ✅ **Polynômes Configurables** : Coefficients individuels par instance
- ✅ **Liaison Sélective** : Paramètre `computing_peak_finder_id` pour source spécifique
- ✅ **Hot-Reload** : Reconfiguration dynamique des polynômes sans redémarrage

#### Structure complète implémentée
```rust
pub struct ConcentrationNode {
    /// Unique identifier for this node
    id: String,

    /// ID of the PeakFinderNode to use as data source
    /// If None, uses the most recent peak data available
    computing_peak_finder_id: Option<String>,

    /// Polynomial coefficients for concentration calculation [a₀, a₁, a₂, a₃, a₄]
    /// Concentration(ppm) = a₀ + a₁·A + a₂·A² + a₃·A³ + a₄·A⁴
    /// where A is the normalized peak amplitude
    polynomial_coefficients: [f64; 5],

    /// Enable temperature compensation for improved accuracy
    temperature_compensation: bool,

    /// Optional identifier for the spectral line being analyzed
    spectral_line_id: Option<String>,

    /// Minimum amplitude threshold for valid concentration calculation
    min_amplitude_threshold: f32,

    /// Maximum concentration limit for safety/validation
    max_concentration_ppm: f32,

    /// Shared state for communicating results to other nodes
    shared_state: Arc<RwLock<ComputingSharedData>>,

    /// Statistics for monitoring performance
    processing_count: u64,
    calculation_count: u64,
    last_calculation_time: Option<SystemTime>,
}

impl ProcessingNode for ConcentrationNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Lecture sélective depuis l'état partagé basée sur computing_peak_finder_id
        let peak_result = match self.shared_state.try_read() {
            Ok(state) => {
                if let Some(source_id) = &self.computing_peak_finder_id {
                    // Lecture depuis un PeakFinder spécifique
                    state.get_peak_result(source_id).cloned()
                } else {
                    // Lecture du résultat le plus récent (comportement automatique)
                    state.get_latest_peak_result().cloned()
                }
            }
            Err(_) => None
        };

        // Calcul de concentration si données disponibles
        if let Some(peak_data) = peak_result {
            if peak_data.amplitude >= self.min_amplitude_threshold {
                let concentration = self.calculate_concentration(peak_data.amplitude);
                self.update_shared_state(&peak_data, concentration);
            }
        }

        // Transmission des données inchangées (pass-through)
        Ok(input)
    }
}
```

#### Calcul Polynomial Avancé - Équation Physique

**Modèle Physique Implémenté** : Relation amplitude-concentration selon la loi de Beer-Lambert modifiée pour la photoacoustique

```mathematica
C(ppm) = a₀ + a₁·A + a₂·A² + a₃·A³ + a₄·A⁴
```

**Où** :
- `A` = amplitude normalisée du pic détecté (0.0 à 1.0)
- `C` = concentration en parties par million (ppm)
- `[a₀, a₁, a₂, a₃, a₄]` = coefficients du polynôme de 4ème degré

**Caractéristiques d'Implémentation** :
- **Validation d'Amplitude** : Seuil minimum configurable (`min_amplitude_threshold`)
- **Limitation de Sécurité** : Concentration maximale configurable (`max_concentration_ppm`)
- **Précision Numérique** : Calculs en double précision (f64) avec conversion finale f32
- **Compensation Thermique** : Support optionnel pour correction de température

#### Avantages Multi-Instances - Cas d'Usage Validés

**🔬 1. Test de Nouveaux Polynômes**
```yaml
processing:
  nodes:
    # Polynôme de référence (actuellement utilisé)
    - id: "concentration_reference"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "primary_peak_finder"
        polynomial_coefficients: [0.0, 0.45, -0.002, 0.0001, 0.0]
        spectral_line_id: "reference_polynomial_v2.1"
        temperature_compensation: true

    # Nouveau polynôme en test (parallèle)
    - id: "concentration_test"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "primary_peak_finder"  # Même source
        polynomial_coefficients: [0.1, 0.52, -0.0025, 0.00015, -0.000001]
        spectral_line_id: "experimental_polynomial_v3.0"
        temperature_compensation: true
        min_amplitude_threshold: 0.002  # Plus restrictif pour tests
```

**Avantages** :
- **Comparaison Temps Réel** : Deux calculs simultanés sur les mêmes données
- **Validation Progressive** : Test de nouveaux modèles sans arrêter la production
- **Analyse de Performance** : Métriques comparatives automatiques

**🎯 2. Calculs Multi-Raies Spectrales**
```yaml
processing:
  nodes:
    # Raie principale du méthane (2ν₃ à ~2100 cm⁻¹)
    - id: "concentration_main_line"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "main_line_detector"
        polynomial_coefficients: [0.0, 0.45, -0.002, 0.0001, 0.0]
        spectral_line_id: "CH4_2v3_main"

    # Raie secondaire pour validation croisée (ν₄ à ~1300 cm⁻¹)
    - id: "concentration_secondary_line"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "secondary_line_detector"
        polynomial_coefficients: [0.05, 0.38, -0.0018, 0.00008, 0.0]
        spectral_line_id: "CH4_v4_secondary"
        temperature_compensation: false  # Différente configuration
```

**Applications Métrologiques** :
- **Redondance Spectrale** : Mesure sur plusieurs raies pour fiabilité
- **Validation Croisée** : Cohérence entre différentes transitions moléculaires
- **Compensation d'Interférences** : Correction des interférences spectrales

**⚡ 3. Positionnement dans le Pipeline - Flexibilité Architecturale**

**Configuration Avant Filtrage** (Recommandée pour analyse large bande) :
```yaml
connections:
- from: differential_detection
  to: concentration_early      # AVANT le filtrage
- from: concentration_early
  to: bandpass_filter
```

**Configuration Après Filtrage** (Optimisée pour SNR) :
```yaml
connections:
- from: bandpass_filter
  to: concentration_late       # APRÈS le filtrage
- from: concentration_late
  to: gain_amplifier
```

**Impacts Techniques Analysés** :

| Position | Avantages | Inconvénients | Cas d'Usage |
|----------|-----------|---------------|-------------|
| **Avant Filtrage** | • Analyse large bande<br>• Détection multi-harmoniques<br>• Moins de distorsion | • Plus de bruit<br>• Calculs sur signal brut | • Test de nouveaux polynômes<br>• Analyse exploratoire |
| **Après Filtrage** | • Meilleur SNR<br>• Signal optimisé<br>• Précision maximale | • Bande limitée<br>• Dépendant du filtre | • Production<br>• Mesures de précision |

#### Configuration Hot-Reload - Paramètres Supportés

**Paramètres Reconfigurables en Temps Réel** :
```json
{
  "polynomial_coefficients": [0.1, 0.52, -0.0025, 0.00015, -0.000001],
  "min_amplitude_threshold": 0.002,
  "max_concentration_ppm": 8000.0,
  "computing_peak_finder_id": "backup_peak_finder"
}
```

**Tests de Validation Hot-Reload** :
- ✅ **Polynômes** : Coefficients modifiables sans interruption
- ✅ **Seuils** : Ajustement dynamique des limites
- ✅ **Source** : Basculement entre PeakFinderNode
- ✅ **Paramètres Sécurité** : Limites min/max en temps réel

### 3.3 DynamicFilterNode (ProcessingNode enrichi) - Support Multi-Fréquences

#### Adaptation basée sur l'état partagé multi-sources
```rust
pub struct DynamicFilterNode {
    id: String,
    base_filter: Box<dyn Filter>,
    fusion_mode: FrequencyFusionMode,
    shared_state: Arc<RwLock<ComputingSharedData>>,
}

#[derive(Debug, Clone)]
pub enum FrequencyFusionMode {
    /// Utilise la fréquence du PeakFinder le plus récent
    MostRecent,
    /// Utilise la fréquence avec la plus forte amplitude
    HighestAmplitude,
    /// Moyenne pondée par amplitude
    WeightedAverage,
    /// Utilise une fréquence spécifique par node_id
    SelectiveBinding(String),
}

impl ProcessingNode for DynamicFilterNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Lecture de l'état partagé pour obtenir les fréquences détectées
        if let Ok(state) = self.shared_state.try_read() {
            if let Some(target_frequency) = self.calculate_target_frequency(&state)? {
                // Adapter la fréquence centrale du filtre
                self.base_filter.set_center_frequency(target_frequency)?;
            }
        }
        
        // Appliquer le filtrage avec la fréquence adaptée
        self.base_filter.process(input)
    }
    
    fn node_type(&self) -> &str { "dynamic_filter" }
    fn node_id(&self) -> &str { &self.id }
}

impl DynamicFilterNode {
    fn calculate_target_frequency(&self, state: &ComputingSharedData) -> Result<Option<f32>> {
        match self.fusion_mode {
            FrequencyFusionMode::MostRecent => {
                Ok(state.get_latest_peak_result().map(|r| r.frequency))
            }
            FrequencyFusionMode::HighestAmplitude => {
                let best_peak = state.peak_results
                    .values()
                    .max_by(|a, b| a.amplitude.partial_cmp(&b.amplitude).unwrap());
                Ok(best_peak.map(|r| r.frequency))
            }
            FrequencyFusionMode::WeightedAverage => {
                self.calculate_weighted_average_frequency(state)
            }
            FrequencyFusionMode::SelectiveBinding(ref node_id) => {
                Ok(state.get_peak_result(node_id).map(|r| r.frequency))
            }
        }
    }
    
    fn calculate_weighted_average_frequency(&self, state: &ComputingSharedData) -> Result<Option<f32>> {
        let recent_results: Vec<_> = state.peak_results
            .values()
            .filter(|result| {
                result.timestamp.elapsed().unwrap_or_default().as_secs() < 30
            })
            .collect();
            
        if recent_results.is_empty() {
            return Ok(None);
        }
        
        let total_weight: f32 = recent_results.iter().map(|r| r.amplitude).sum();
        
        if total_weight == 0.0 {
            return Ok(None);
        }
        
        let weighted_freq = recent_results.iter()
            .map(|r| r.frequency * r.amplitude)
            .sum::<f32>() / total_weight;
            
        Ok(Some(weighted_freq))
    }
}
```

**Évolutions Multi-Fréquences** :
- **Stratégies de Fusion** : Plusieurs modes pour combiner les fréquences multiples
- **Adaptation Intelligente** : Choix automatique de la meilleure fréquence de référence
- **Pondération par Amplitude** : Priorité aux signaux les plus forts
- **Validation Temporelle** : Utilise uniquement les données récentes et valides

---

## 4. Implémentations Proposées

### 4.1 PeakFinderNode (ComputingNode spécialisé) - Support Multi-Instances

#### Structure et fonctionnalités
```rust
pub struct PeakFinderNode {
    /// Unique identifier for this node - CRITIQUE pour le support multi-instances
    id: String,
    
    /// Shared state for communicating results to other nodes
    shared_state: SharedComputingState,
    
    /// FFT configuration and buffers
    fft_buffer: Vec<Complex<f32>>,
    fft_size: usize,
    sample_rate: u32,
    
    /// Detection parameters
    frequency_range: (f32, f32),  // Bande de recherche
    detection_threshold: f32,
    smoothing_factor: f32,
    
    /// Performance monitoring
    processing_count: u64,
    last_detection_time: Option<SystemTime>,
}

impl ProcessingNode for PeakFinderNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Analyse spectrale sur les données
        if let Some(peak_info) = self.analyze_spectrum(&input)? {
            // Création du résultat avec ID unique
            let peak_result = PeakResult {
                frequency: peak_info.frequency,
                amplitude: peak_info.amplitude,
                concentration_ppm: None,
                timestamp: SystemTime::now(),
            };
            
            // Mise à jour de l'état partagé avec clé unique
            if let Ok(mut state) = self.shared_state.try_write() {
                state.update_peak_result(self.id.clone(), peak_result);
            }
        }
        
        // Transmission des données inchangées
        Ok(input)
    }
    
    fn node_type(&self) -> &str { "computing_peak_finder" }
}
```

**Nouveautés Multi-Instances :**
- **ID Obligatoire** : Chaque instance doit avoir un `id` unique lors de l'instanciation
- **Stockage Cloisonné** : Chaque nœud stocke ses résultats indépendamment
- **Configuration Individuelle** : Chaque instance peut avoir des paramètres différents
  - Bandes de fréquence distinctes (ex: 800-1200 Hz vs 1500-2000 Hz)
  - Seuils de détection adaptés à chaque signal
  - Facteurs de lissage optimisés par usage

#### Fonctionnalités étendues
- **Analyse spectrale FFT** : Détection automatique de la fréquence de résonance par instance
- **Algorithme de détection de pics** : Recherche du maximum local dans la bande configurée
- **Filtrage adaptatif** : Élimination des pics parasites par analyse de cohérence temporelle
- **Suivi temporel indépendant** : Moyenne glissante et historique par nœud
- **Validation croisée** : Possibilité de comparer les résultats entre instances

### 4.2 ConcentrationNode (ComputingNode spécialisé) - IMPLÉMENTATION COMPLÈTE

#### Architecture Multi-Instances avec Polynômes Individuels

**🎯 Objectifs Atteints :**
- ✅ **Instances Multiples** : Chaque ConcentrationNode a son propre ID unique
- ✅ **Polynômes Configurables** : Coefficients individuels par instance
- ✅ **Liaison Sélective** : Paramètre `computing_peak_finder_id` pour source spécifique
- ✅ **Hot-Reload** : Reconfiguration dynamique des polynômes sans redémarrage

#### Structure complète implémentée
```rust
pub struct ConcentrationNode {
    /// Unique identifier for this node
    id: String,

    /// ID of the PeakFinderNode to use as data source
    /// If None, uses the most recent peak data available
    computing_peak_finder_id: Option<String>,

    /// Polynomial coefficients for concentration calculation [a₀, a₁, a₂, a₃, a₄]
    /// Concentration(ppm) = a₀ + a₁·A + a₂·A² + a₃·A³ + a₄·A⁴
    /// where A is the normalized peak amplitude
    polynomial_coefficients: [f64; 5],

    /// Enable temperature compensation for improved accuracy
    temperature_compensation: bool,

    /// Optional identifier for the spectral line being analyzed
    spectral_line_id: Option<String>,

    /// Minimum amplitude threshold for valid concentration calculation
    min_amplitude_threshold: f32,

    /// Maximum concentration limit for safety/validation
    max_concentration_ppm: f32,

    /// Shared state for communicating results to other nodes
    shared_state: Arc<RwLock<ComputingSharedData>>,

    /// Statistics for monitoring performance
    processing_count: u64,
    calculation_count: u64,
    last_calculation_time: Option<SystemTime>,
}

impl ProcessingNode for ConcentrationNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Lecture sélective depuis l'état partagé basée sur computing_peak_finder_id
        let peak_result = match self.shared_state.try_read() {
            Ok(state) => {
                if let Some(source_id) = &self.computing_peak_finder_id {
                    // Lecture depuis un PeakFinder spécifique
                    state.get_peak_result(source_id).cloned()
                } else {
                    // Lecture du résultat le plus récent (comportement automatique)
                    state.get_latest_peak_result().cloned()
                }
            }
            Err(_) => None
        };

        // Calcul de concentration si données disponibles
        if let Some(peak_data) = peak_result {
            if peak_data.amplitude >= self.min_amplitude_threshold {
                let concentration = self.calculate_concentration(peak_data.amplitude);
                self.update_shared_state(&peak_data, concentration);
            }
        }

        // Transmission des données inchangées (pass-through)
        Ok(input)
    }
}
```

#### Calcul Polynomial Avancé - Équation Physique

**Modèle Physique Implémenté** : Relation amplitude-concentration selon la loi de Beer-Lambert modifiée pour la photoacoustique

```mathematica
C(ppm) = a₀ + a₁·A + a₂·A² + a₃·A³ + a₄·A⁴
```

**Où** :
- `A` = amplitude normalisée du pic détecté (0.0 à 1.0)
- `C` = concentration en parties par million (ppm)
- `[a₀, a₁, a₂, a₃, a₄]` = coefficients du polynôme de 4ème degré

**Caractéristiques d'Implémentation** :
- **Validation d'Amplitude** : Seuil minimum configurable (`min_amplitude_threshold`)
- **Limitation de Sécurité** : Concentration maximale configurable (`max_concentration_ppm`)
- **Précision Numérique** : Calculs en double précision (f64) avec conversion finale f32
- **Compensation Thermique** : Support optionnel pour correction de température

#### Avantages Multi-Instances - Cas d'Usage Validés

**🔬 1. Test de Nouveaux Polynômes**
```yaml
processing:
  nodes:
    # Polynôme de référence (actuellement utilisé)
    - id: "concentration_reference"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "primary_peak_finder"
        polynomial_coefficients: [0.0, 0.45, -0.002, 0.0001, 0.0]
        spectral_line_id: "reference_polynomial_v2.1"
        temperature_compensation: true

    # Nouveau polynôme en test (parallèle)
    - id: "concentration_test"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "primary_peak_finder"  # Même source
        polynomial_coefficients: [0.1, 0.52, -0.0025, 0.00015, -0.000001]
        spectral_line_id: "experimental_polynomial_v3.0"
        temperature_compensation: true
        min_amplitude_threshold: 0.002  # Plus restrictif pour tests
```

**Avantages** :
- **Comparaison Temps Réel** : Deux calculs simultanés sur les mêmes données
- **Validation Progressive** : Test de nouveaux modèles sans arrêter la production
- **Analyse de Performance** : Métriques comparatives automatiques

**🎯 2. Calculs Multi-Raies Spectrales**
```yaml
processing:
  nodes:
    # Raie principale du méthane (2ν₃ à ~2100 cm⁻¹)
    - id: "concentration_main_line"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "main_line_detector"
        polynomial_coefficients: [0.0, 0.45, -0.002, 0.0001, 0.0]
        spectral_line_id: "CH4_2v3_main"

    # Raie secondaire pour validation croisée (ν₄ à ~1300 cm⁻¹)
    - id: "concentration_secondary_line"
      node_type: "computing_concentration"
      parameters:
        computing_peak_finder_id: "secondary_line_detector"
        polynomial_coefficients: [0.05, 0.38, -0.0018, 0.00008, 0.0]
        spectral_line_id: "CH4_v4_secondary"
        temperature_compensation: false  # Différente configuration
```

**Applications Métrologiques** :
- **Redondance Spectrale** : Mesure sur plusieurs raies pour fiabilité
- **Validation Croisée** : Cohérence entre différentes transitions moléculaires
- **Compensation d'Interférences** : Correction des interférences spectrales

**⚡ 3. Positionnement dans le Pipeline - Flexibilité Architecturale**

**Configuration Avant Filtrage** (Recommandée pour analyse large bande) :
```yaml
connections:
- from: differential_detection
  to: concentration_early      # AVANT le filtrage
- from: concentration_early
  to: bandpass_filter
```

**Configuration Après Filtrage** (Optimisée pour SNR) :
```yaml
connections:
- from: bandpass_filter
  to: concentration_late       # APRÈS le filtrage
- from: concentration_late
  to: gain_amplifier
```

**Impacts Techniques Analysés** :

| Position | Avantages | Inconvénients | Cas d'Usage |
|----------|-----------|---------------|-------------|
| **Avant Filtrage** | • Analyse large bande<br>• Détection multi-harmoniques<br>• Moins de distorsion | • Plus de bruit<br>• Calculs sur signal brut | • Test de nouveaux polynômes<br>• Analyse exploratoire |
| **Après Filtrage** | • Meilleur SNR<br>• Signal optimisé<br>• Précision maximale | • Bande limitée<br>• Dépendant du filtre | • Production<br>• Mesures de précision |

#### Configuration Hot-Reload - Paramètres Supportés

**Paramètres Reconfigurables en Temps Réel** :
```json
{
  "polynomial_coefficients": [0.1, 0.52, -0.0025, 0.00015, -0.000001],
  "min_amplitude_threshold": 0.002,
  "max_concentration_ppm": 8000.0,
  "computing_peak_finder_id": "backup_peak_finder"
}
```

**Tests de Validation Hot-Reload** :
- ✅ **Polynômes** : Coefficients modifiables sans interruption
- ✅ **Seuils** : Ajustement dynamique des limites
- ✅ **Source** : Basculement entre PeakFinderNode
- ✅ **Paramètres Sécurité** : Limites min/max en temps réel

### 3.3 DynamicFilterNode (ProcessingNode enrichi) - Support Multi-Fréquences

#### Adaptation basée sur l'état partagé multi-sources
```rust
pub struct DynamicFilterNode {
    id: String,
    base_filter: Box<dyn Filter>,
    fusion_mode: FrequencyFusionMode,
    shared_state: Arc<RwLock<ComputingSharedData>>,
}

#[derive(Debug, Clone)]
pub enum FrequencyFusionMode {
    /// Utilise la fréquence du PeakFinder le plus récent
    MostRecent,
    /// Utilise la fréquence avec la plus forte amplitude
    HighestAmplitude,
    /// Moyenne pondée par amplitude
    WeightedAverage,
    /// Utilise une fréquence spécifique par node_id
    SelectiveBinding(String),
}

impl ProcessingNode for DynamicFilterNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Lecture de l'état partagé pour obtenir les fréquences détectées
        if let Ok(state) = self.shared_state.try_read() {
            if let Some(target_frequency) = self.calculate_target_frequency(&state)? {
                // Adapter la fréquence centrale du filtre
                self.base_filter.set_center_frequency(target_frequency)?;
            }
        }
        
        // Appliquer le filtrage avec la fréquence adaptée
        self.base_filter.process(input)
    }
    
    fn node_type(&self) -> &str { "dynamic_filter" }
    fn node_id(&self) -> &str { &self.id }
}

impl DynamicFilterNode {
    fn calculate_target_frequency(&self, state: &ComputingSharedData) -> Result<Option<f32>> {
        match self.fusion_mode {
            FrequencyFusionMode::MostRecent => {
                Ok(state.get_latest_peak_result().map(|r| r.frequency))
            }
            FrequencyFusionMode::HighestAmplitude => {
                let best_peak = state.peak_results
                    .values()
                    .max_by(|a, b| a.amplitude.partial_cmp(&b.amplitude).unwrap());
                Ok(best_peak.map(|r| r.frequency))
            }
            FrequencyFusionMode::WeightedAverage => {
                self.calculate_weighted_average_frequency(state)
            }
            FrequencyFusionMode::SelectiveBinding(ref node_id) => {
                Ok(state.get_peak_result(node_id).map(|r| r.frequency))
            }
        }
    }
    
    fn calculate_weighted_average_frequency(&self, state: &ComputingSharedData) -> Result<Option<f32>> {
        let recent_results: Vec<_> = state.peak_results
            .values()
            .filter(|result| {
                result.timestamp.elapsed().unwrap_or_default().as_secs() < 30
            })
            .collect();
            
        if recent_results.is_empty() {
            return Ok(None);
        }
        
        let total_weight: f32 = recent_results.iter().map(|r| r.amplitude).sum();
        
        if total_weight == 0.0 {
            return Ok(None);
        }
        
        let weighted_freq = recent_results.iter()
            .map(|r| r.frequency * r.amplitude)
            .sum::<f32>() / total_weight;
            
        Ok(Some(weighted_freq))
    }
}
```

**Évolutions Multi-Fréquences** :
- **Stratégies de Fusion** : Plusieurs modes pour combiner les fréquences multiples
- **Adaptation Intelligente** : Choix automatique de la meilleure fréquence de référence
- **Pondération par Amplitude** : Priorité aux signaux les plus forts
- **Validation Temporelle** : Utilise uniquement les données récentes et valides

---

## 4. Implémentations Proposées

### 4.1 PeakFinderNode (ComputingNode spécialisé) - Support Multi-Instances

#### Structure et fonctionnalités
```rust
pub struct PeakFinderNode {
    /// Unique identifier for this node - CRITIQUE pour le support multi-instances
    id: String,
    
    /// Shared state for communicating results to other nodes
    shared_state: SharedComputingState,
    
    /// FFT configuration and buffers
    fft_buffer: Vec<Complex<f32>>,
    fft_size: usize,
    sample_rate: u32,
    
    /// Detection parameters
    frequency_range: (f32, f32),  // Bande de recherche
    detection_threshold: f32,
    smoothing_factor: f32,
    
    /// Performance monitoring
    processing_count: u64,
    last_detection_time: Option<SystemTime>,
}

impl ProcessingNode for PeakFinderNode {
    fn process(&mut self, input: ProcessingData) -> Result<ProcessingData> {
        // Analyse spectrale sur les données
        if let Some(peak_info) = self.analyze_spectrum(&input)? {
            // Création du résultat avec ID unique
            let peak_result = PeakResult {
                frequency: peak_info.frequency,
                amplitude: peak_info.amplitude,
                concentration_ppm: None,
                timestamp: SystemTime::now(),
            };
            
            // Mise à jour de l'état partagé avec clé unique
            if let Ok(mut state) = self.shared_state.try_write() {
                state.update_peak_result(self.id.clone(), peak_result);
            }
        }
        
        // Transmission des données inchangées
        Ok(input)
    }
    
    fn node_type(&self) -> &str { "computing_peak_finder" }
}
```

**Nouveautés Multi-Instances :**
- **ID Obligatoire** : Chaque instance doit avoir un `id` unique lors de l'instanciation
- **Stockage Cloisonné** : Chaque nœud stocke ses résultats indépendamment
- **Configuration Individuelle** : Chaque instance peut avoir des paramètres différents
  - Bandes de fréquence distinctes (ex: 800-1200 Hz vs 1500-2000 Hz)
  - Seuils de détection adaptés à chaque signal
  - Facteurs de lissage optimisés par usage

#### Fonctionnalités étendues
- **Analyse spectrale FFT** : Détection automatique de la fréquence de résonance par instance
- **Algorithme de détection de pics** : Recherche du maximum local dans la bande configurée
- **Filtrage adaptatif** : Élimination des pics parasites par analyse de cohérence temporelle
- **Suivi temporel indépendant** : Moyenne glissante et historique par nœud
- **Validation croisée** : Possibilité de comparer les résultats entre instances

### 4.2 ConcentrationNode (ComputingNode spécialisé) - IMPLÉMENTATION COMPLÈTE

#### Architecture Multi-Instances avec Polynômes Individuels

**🎯 Objectifs Atteints :**
- ✅ **Instances Multiples** : Chaque ConcentrationNode a son propre ID unique
- ✅ **Polynômes Configurables** : Coefficients individuels par instance
- ✅ **Liaison Sélective** : Paramètre `computing_peak_finder_id` pour source spécifique
- ✅ **Hot-Reload** : Reconfiguration dynamique des polynômes sans redémarrage

#### Structure complète implémentée
```rust
pub struct ConcentrationNode {
    /// Unique identifier for this node
    id: String,

    /// ID of the PeakFinderNode to use as data source
    /// If None, uses the most recent peak data available
    computing_peak_finder_id: Option

✅ **Thread-based Processing** : Traitement asynchrone avec channels mpsc
✅ **Configuration YAML** : Instantiation automatique depuis fichiers de config
✅ **Données Temps Réel** : Transmission des vraies valeurs amplitude/fréquence
✅ **Monitoring Intégré** : Buffers circulaires et métriques de performance
✅ **Threshold Management** : Triggers configurables pour alertes automatiques
✅ **Hot Reload** : Reconfiguration dynamique sans redémarrage
✅ **Multi-Instances** : Support de multiples nœuds avec IDs uniques

### 5.4 Extensions Futures Préparées

L'architecture actuelle facilite l'ajout de nouveaux composants :
- **EmailActionDriver** : Notifications par email
- **DatabaseActionDriver** : Logging vers bases de données  
- **RelayActionDriver** : Contrôle de relais industriels
- **ModbusActionDriver** : Intégration systèmes industriels
- **MQTTActionDriver** : IoT et dispositifs connectés

---
