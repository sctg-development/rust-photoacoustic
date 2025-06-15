\
<!-- filepath: /home/coder/rust-photoacoustic/docs/AUDIT_PROCESSINGGRAPH_NODES_HOT_RELOAD.md -->
# Audit : possibilité de mise à jour dynamique des nœuds du ProcessingGraph

Ce document détaille, pour chaque type de nœud du `ProcessingGraph`, si une mise à jour dynamique de ses paramètres est possible _sans_ redémarrer le processing (hot-reload), ou si une reconstruction du graphe (et donc un redémarrage du processing consumer) est nécessaire. Ce document adopte une approche systématique pour évaluer la capacité de hot-reload de chaque type de nœud, afin de guider l'implémentation d'une configuration dynamique robuste.

---

## 1. Principe général

- Le `ProcessingGraph` est une structure de graphe orienté composée de nœuds (`ProcessingNode`), chaque nœud ayant des paramètres spécifiques.
- Les nœuds sont typés (`node_type`) : input, filter, channel_selector, differential, mixer, record, output, etc.
- La dynamique de modification dépend :
  - de la capacité de chaque nœud à supporter la mutation de ses paramètres à chaud,
  - de la capacité du graphe à accepter l’ajout/retrait/reconnexion de nœuds à chaud (typiquement non supporté, nécessite un rebuild).

---

## 2. Détail par type de nœud

### 2.1 `InputNode`

- **Description** : Nœud d'entrée du graphe, fournissant les données initiales.
- **Paramètres principaux** : `id` (identifiant unique), `node_type` (type de nœud, implicitement "input"), `data_format` (format des données attendues, e.g., PCM, float, nombre de canaux).
- **Analyse de la capacité de Hot-Reload** :
    - `id`: NON. Un changement d'ID équivaut à remplacer le nœud.
    - `data_format`: NON. Modifier le format des données (e.g., passer de mono à stéréo, changer la quantification) impacte toute la chaîne de traitement et nécessite une reconfiguration des nœuds dépendants et potentiellement du graphe.
- **Conclusion sur le Hot-Reload du Nœud** : NON SUPPORTÉ.
- **Stratégie de mise à jour recommandée** : Toute modification des paramètres fondamentaux d'un `InputNode` impose une reconstruction du graphe.

### 2.2 `FilterNode` (ex : lowpass, highpass, bandpass) ✅ **IMPLÉMENTÉ**

- **Description** : Nœud appliquant un filtre fréquentiel aux données.
- **Paramètres principaux** : `filter_type` (lowpass, highpass, etc.), `cutoff_frequency` (ou `center_freq`, `bandwidth`), `order` (ordre du filtre), `sample_rate`, `target_channel` (canal(aux) affecté(s)).
- **Analyse de la capacité de Hot-Reload** :
    - `filter_type`: NON. Changer le type de filtre (e.g., de lowpass à bandpass) modifie fondamentalement l'algorithme et l'état interne du filtre. Nécessite une ré-instanciation du nœud.
    - **`cutoff_frequency`, `center_freq`, `bandwidth`, `order`, `sample_rate`: OUI - IMPLÉMENTÉ**. Ces paramètres peuvent être modifiés dynamiquement via les méthodes `update_config()` des filtres individuels. Les coefficients du filtre sont automatiquement recalculés après chaque mise à jour.
    - **`target_channel`: OUI - IMPLÉMENTÉ**. Le ciblage du canal peut être ajusté dynamiquement via la méthode `update_config()` du `FilterNode`.
- **Conclusion sur le Hot-Reload du Nœud** : **LARGEMENT SUPPORTÉ**.
- **Stratégie de mise à jour recommandée** : **IMPLÉMENTÉE** pour la plupart des paramètres.
    - **Pour `cutoff_frequency`, `center_freq`, `bandwidth`, `order`, `sample_rate`, `target_channel`: IMPLÉMENTÉ** - Setters thread-safe avec recalcul automatique des coefficients.
    - Pour `filter_type`: Nécessite la reconstruction du nœud (et potentiellement du graphe si les connexions sont affectées).
- **Implémentation actuelle** :
    ```rust
    // Pour les filtres individuels (LowpassFilter, HighpassFilter, BandpassFilter)
    impl LowpassFilter {
        pub fn update_config(&mut self, parameters: &serde_json::Value) -> anyhow::Result<bool> {
            let mut updated = false;
            // Met à jour cutoff_freq, sample_rate, order
            // Validation des paramètres (fréquence < Nyquist, order > 0, etc.)
            // Pas de recalcul nécessaire pour LowpassFilter (stateless)
            Ok(updated)
        }
    }
    
    impl BandpassFilter {
        pub fn update_config(&mut self, parameters: &serde_json::Value) -> anyhow::Result<bool> {
            let mut updated = false;
            // Met à jour center_freq, bandwidth, sample_rate, order (doit être pair)
            // Validation des paramètres
            if updated {
                self.compute_coefficients(); // Recalcule les coefficients biquad
            }
            Ok(updated)
        }
    }
    
    // Pour le FilterNode
    impl ProcessingNode for FilterNode {
        fn update_config(&mut self, parameters: &serde_json::Value) -> anyhow::Result<bool> {
            let mut updated = false;
            // Met à jour target_channel si fourni
            if let Some(target) = parameters.get("target_channel") {
                // Validation et mise à jour du ChannelTarget
            }
            // Note: mise à jour des paramètres du filtre sous-jacent nécessiterait
            // l'ajout d'update_config au trait Filter
            Ok(updated)
        }
    }
    ```

### 2.3 `ChannelSelectorNode` ✅ **IMPLÉMENTÉ**

- **Description** : Nœud sélectionnant un canal spécifique à partir d'un flux dual-channel.
- **Paramètres principaux** : `target_channel` (canal à sélectionner: ChannelA ou ChannelB).
- **Analyse de la capacité de Hot-Reload** :
    - `target_channel`: **OUI - IMPLÉMENTÉ**. La sélection du canal peut être modifiée dynamiquement via la méthode `update_config()`. Le nœud accepte "ChannelA" ou "ChannelB" (Note: "Both" n'est pas valide pour ce nœud car il produit une sortie SingleChannel).
- **Conclusion sur le Hot-Reload du Nœud** : **ENTIÈREMENT SUPPORTÉ**.
- **Stratégie de mise à jour recommandée** : **IMPLÉMENTÉE** - Changement direct du paramètre `target_channel` sans état interne complexe à réinitialiser.
- **Implémentation actuelle** :
    ```rust
    impl ProcessingNode for ChannelSelectorNode {
        fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
            if let Value::Object(params) = parameters {
                if let Some(channel_value) = params.get("target_channel") {
                    match channel_value.as_str() {
                        Some("ChannelA") => {
                            self.target_channel = ChannelTarget::ChannelA;
                            return Ok(true); // Hot-reload successful
                        }
                        Some("ChannelB") => {
                            self.target_channel = ChannelTarget::ChannelB;
                            return Ok(true); // Hot-reload successful
                        }
                        // ... validation and error handling
                    }
                }
            }
            Ok(false) // No matching parameters found
        }
    }
    ```

### 2.4 `DifferentialNode` ⚠️ **STRUCTURE PRÉPARÉE**

- **Description** : Nœud calculant une différence (e.g., entre canaux, ou par rapport à une valeur).
- **Paramètres principaux** : `calculator_type` (type de calculateur différentiel), paramètres spécifiques au calculateur utilisé.
- **Analyse de la capacité de Hot-Reload** :
    - **État actuel** : La méthode `update_config()` est implémentée mais retourne `false` (pas de hot-reload supporté).
    - **Raison** : Le `DifferentialCalculator` actuel (`SimpleDifferential`) n'a pas de paramètres configurables.
    - **Possibilités futures** : Extension pour supporter différents types de calculateurs (weighted, adaptive) avec leurs paramètres spécifiques.
- **Conclusion sur le Hot-Reload du Nœud** : **NON SUPPORTÉ ACTUELLEMENT** (structure préparée pour futures améliorations).
- **Stratégie de mise à jour recommandée** : **PRÉPARÉE** - Infrastructure en place pour ajouter le hot-reload quand des calculateurs configurables seront disponibles.
- **Implémentation actuelle** :
    ```rust
    impl ProcessingNode for DifferentialNode {
        fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
            if let Value::Object(_params) = parameters {
                // Currently no hot-reloadable parameters
                // Future: support for different calculator types and their parameters
            } else {
                anyhow::bail!("Parameters must be a JSON object");
            }
            Ok(false) // No hot-reload support currently - requires node reconstruction
        }
    }
    ```

### 2.5 `MixerNode` / `ChannelMixerNode` ✅ **IMPLÉMENTÉ**

- **Description** : Nœud combinant deux canaux audio avec différentes stratégies.
- **Paramètres principaux** : `mix_strategy` (Add, Subtract, Average, ou Weighted avec a_weight/b_weight).
- **Analyse de la capacité de Hot-Reload** :
    - `mix_strategy`: **OUI - IMPLÉMENTÉ**. La stratégie de mixage peut être modifiée dynamiquement via la méthode `update_config()`. Supporte toutes les stratégies : Add, Subtract, Average, et Weighted avec des poids personnalisés.
- **Conclusion sur le Hot-Reload du Nœud** : **ENTIÈREMENT SUPPORTÉ**.
- **Stratégie de mise à jour recommandée** : **IMPLÉMENTÉE** - Changement direct de la `mix_strategy` sans interruption du traitement.
- **Implémentation actuelle** :
    ```rust
    impl ProcessingNode for ChannelMixerNode {
        fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
            if let Value::Object(params) = parameters {
                if let Some(strategy_value) = params.get("mix_strategy") {
                    match strategy_value {
                        Value::String(s) => match s.as_str() {
                            "Add" => { self.mix_strategy = MixStrategy::Add; return Ok(true); }
                            "Subtract" => { self.mix_strategy = MixStrategy::Subtract; return Ok(true); }
                            "Average" => { self.mix_strategy = MixStrategy::Average; return Ok(true); }
                            // ... error handling
                        },
                        Value::Object(obj) => {
                            // Handle Weighted strategy with a_weight and b_weight
                            if let (Some(a), Some(b)) = (obj.get("a_weight"), obj.get("b_weight")) {
                                self.mix_strategy = MixStrategy::Weighted { 
                                    a_weight: a.as_f64().unwrap() as f32,
                                    b_weight: b.as_f64().unwrap() as f32 
                                };
                                return Ok(true);
                            }
                        }
                    }
                }
            }
            Ok(false)
        }
    }
    ```

### 2.6 `RecordNode`

- **Description** : Nœud enregistrant les données audio dans un fichier.
- **Paramètres principaux** : `output_path` (chemin du fichier), `file_format` (format du fichier), `max_size`, `rolling_buffer_enabled`, `auto_delete_old`.
- **Analyse de la capacité de Hot-Reload** :
    - `output_path`, `file_format`: NON. Changer le fichier de destination ou son format implique de fermer le handle de fichier actuel, potentiellement finaliser l'écriture, puis d'en ouvrir un nouveau. Ceci est une opération disruptive qui s'apparente à un redémarrage du mécanisme d'enregistrement du nœud.
    - `max_size`, `rolling_buffer_enabled`, `auto_delete_old`: OUI. Ces paramètres de gestion du fichier (taille, roulement, suppression) peuvent souvent être ajustés dynamiquement si le nœud est conçu pour vérifier ces valeurs périodiquement ou sur notification.
- **Conclusion sur le Hot-Reload du Nœud** : PARTIELLEMENT SUPPORTÉ.
- **Stratégie de mise à jour recommandée** :
    - Pour `max_size`, `rolling_buffer_enabled`, `auto_delete_old`: Setters thread-safe ou relecture périodique de la configuration.
    - Pour `output_path`, `file_format`: Nécessite une réinitialisation de la logique d'enregistrement du nœud (équivalent à un redémarrage du service d'enregistrement interne au nœud). Une gestion propre impliquerait `close_current_file()`, `reconfigure(...)`, `open_new_file()`.

### 2.7 `PhotoacousticOutputNode` (ou `OutputNode` générique)

- **Description** : Nœud final du graphe, souvent pour l'analyse ou l'envoi des résultats.
- **Paramètres principaux** : `analysis_thresholds` (seuils d'analyse), `window_size` (taille de fenêtre pour analyse), `output_target` (e.g., bus de message, callback).
- **Analyse de la capacité de Hot-Reload** :
    - `analysis_thresholds`, `window_size`: OUI. Ces paramètres numériques utilisés pour l'analyse peuvent typiquement être mis à jour dynamiquement via des setters.
    - `output_target`: NON. Changer la destination de sortie (e.g., d'un bus de message à un autre, ou changer le type de callback) est une modification structurelle.
- **Conclusion sur le Hot-Reload du Nœud** : PARTIELLEMENT SUPPORTÉ.
- **Stratégie de mise à jour recommandée** :
    - Pour `analysis_thresholds`, `window_size`: Setters thread-safe.
    - Pour `output_target`: Reconstruction du nœud.

### 2.8 `GainNode` ✅ **IMPLÉMENTÉ**

- **Description** : Nœud appliquant un gain (amplification/atténuation) aux signaux audio.
- **Paramètres principaux** : `gain_db` (gain en décibels), `linear_gain` (facteur de gain linéaire calculé).
- **Analyse de la capacité de Hot-Reload** :
    - `gain_db`: **OUI - IMPLÉMENTÉ**. Le gain en décibels peut être modifié dynamiquement via la méthode `update_config()`. Le nœud recalcule automatiquement le facteur de gain linéaire correspondant.
    - `linear_gain`: Calculé automatiquement à partir de `gain_db`, pas modifiable directement.
- **Conclusion sur le Hot-Reload du Nœud** : **ENTIÈREMENT SUPPORTÉ**.
- **Stratégie de mise à jour recommandée** : **IMPLÉMENTÉE** - Utilisation de `Arc<RwLock<f32>>` pour un accès thread-safe au paramètre `gain_db`.
- **Implémentation actuelle** :
    ```rust
    impl ProcessingNode for GainNode {
        fn update_config(&mut self, parameters: &serde_json::Value) -> Result<bool> {
            if let Some(gain_value) = parameters.get("gain_db") {
                if let Some(new_gain_db) = gain_value.as_f64() {
                    let mut gain_guard = self.gain_db.write().unwrap();
                    *gain_guard = new_gain_db as f32;
                    debug!("GainNode '{}': Updated gain_db to {} dB", self.id, new_gain_db);
                    return Ok(true); // Hot-reload successful
                }
            }
            Ok(false) // No matching parameters found
        }
    }
    ```

### 2.8 `Custom/PluginNode`

- **Description** : Nœud défini par l'utilisateur ou un plugin externe.
- **Paramètres principaux** : Spécifiques au plugin.
- **Analyse de la capacité de Hot-Reload** : Dépend entièrement de l'implémentation du plugin. Le plugin doit explicitement documenter et supporter le hot-reload de ses paramètres.
- **Conclusion sur le Hot-Reload du Nœud** : DÉPEND DU PLUGIN.
- **Stratégie de mise à jour recommandée** : Se référer à la documentation du plugin. Encourager les développeurs de plugins à implémenter des setters thread-safe pour les paramètres configurables et à documenter clairement les capacités de hot-reload.

---

## 3. Synthèse récapitulative

| Type de Nœud        | Hot-Reload Paramètres? | Exemples de Paramètres Hot-Reloadable | Exemples de Paramètres NON Hot-Reloadable | Modification Structurelle du Graphe (ajout/suppression nœud/connexion) |
|---------------------|:----------------------:|:--------------------------------------|:------------------------------------------|:----------------------------------------------------------------------:|
| InputNode           | NON                    | -                                     | `id`, `data_format`                       | NON (rebuild requis)                                                  |
| **FilterNode**      | **LARGEMENT ✅**       | **`cutoff_freq`, `center_freq`, `bandwidth`, `order`, `sample_rate`, `target_channel`** | `filter_type`                             | NON (rebuild requis)                                                  |
| **ChannelSelectorNode** | **OUI ✅**            | **`target_channel`**                  | -                                         | NON (rebuild requis)                                                  |
| DifferentialNode    | NON ⚠️                 | -                                     | `calculator_type`, paramètres calculateur | NON (rebuild requis)                                                  |
| **MixerNode**       | **OUI ✅**            | **`mix_strategy`** (toutes variantes) | -                                         | NON (rebuild requis)                                                  |
| RecordNode          | PARTIEL                | `max_size`, `rolling_buffer`          | `output_path`, `file_format`              | NON (rebuild requis)                                                  |
| OutputNode          | PARTIEL                | `analysis_thresholds`, `window_size`  | `output_target`                           | NON (rebuild requis)                                                  |
| **GainNode**        | **OUI ✅**            | **`gain_db`**                         | -                                         | NON (rebuild requis)                                                  |
| Custom/PluginNode   | DÉPEND DU PLUGIN       | Selon implémentation                  | Selon implémentation                      | NON (rebuild requis)                                                  |
| Ajout/Suppression nœud/connexion | -                      | -                                     | -                                         | NON (rebuild requis)                                                  |

(*) Changement du fichier cible nécessite close/reopen, donc un redémarrage du node (ou du graphe).

---

## 4. Remarques sur la dynamique du graphe

- **Modification structurelle du graphe** (ajout, suppression, reconnexion de nœuds, changement de type de nœud incompatible avec hot-reload) :
  → **Pas hot-reloadable** dans la plupart des implémentations standards, nécessite une reconstruction complète et un restart du `ProcessingConsumer` (ou du gestionnaire de graphe).

- **Modification des paramètres internes des nœuds** :
  - Si les nœuds exposent des setters thread-safe pour les paramètres concernés, et que le gestionnaire de graphe peut propager ces changements, le reload dynamique est possible sans restart du graphe.
  - Sinon, il faut au minimum retirer et ré-instancier le nœud concerné. Si cette granularité n'est pas gérée, un restart complet du graphe est nécessaire.

---

## 5. Recommandations de conception

1.  **Setters Thread-Safe** : Pour chaque paramètre identifié comme hot-reloadable au sein d'un nœud, implémenter des setters thread-safe.
2.  **Documentation Claire** : Documenter précisément pour chaque type de nœud :
    *   La liste de ses paramètres configurables.
    *   Pour chaque paramètre, s'il est modifiable à chaud.
    *   L'impact d'une modification (hot-reload transparent, nécessité de réinitialiser l'état interne du nœud, etc.).
3.  **API de Gestion du Graphe** :
    *   Pour la modification de la *structure* du graphe (topologie, ajout/suppression de nœuds, changement de type de nœud incompatible avec hot-reload) : Exposer une API de type `reload_graph(new_graph_config)` qui reconstruit et redémarre le `ProcessingConsumer` ou le graphe de manière contrôlée.
    *   Pour la modification des *paramètres* d'un nœud : Envisager un mécanisme où le `ProcessingConsumer` peut notifier un nœud spécifique d'un changement de sa configuration, ou le nœud lui-même s'abonne aux changements pertinents de la configuration globale.
4.  **Granularité des Mises à Jour** : Distinguer clairement les mises à jour qui peuvent être appliquées à un nœud individuel de celles qui nécessitent une action sur l'ensemble du graphe.
5.  **Notification et État** : Prévoir un mécanisme de notification pour informer le système (et potentiellement l'utilisateur) du succès ou de l'échec d'une tentative de mise à jour de configuration, et de l'action entreprise (hot-reload, redémarrage partiel/complet).

---

## 6. Exemple de setter dynamique (Rust)

```rust
impl FilterNode {
    pub fn set_cutoff_frequency(&mut self, freq: f32) {
        self.filter.set_cutoff(freq);
        // Recalcule les coefficients du filtre en temps réel
    }
}
```
