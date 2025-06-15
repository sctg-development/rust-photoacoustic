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

### 2.2 `FilterNode` (ex : lowpass, highpass, bandpass)

- **Description** : Nœud appliquant un filtre fréquentiel aux données.
- **Paramètres principaux** : `filter_type` (lowpass, highpass, etc.), `cutoff_frequency` (ou `low_cutoff`, `high_cutoff`), `order` (ordre du filtre), `target_channel` (canal(aux) affecté(s)).
- **Analyse de la capacité de Hot-Reload** :
    - `filter_type`: NON. Changer le type de filtre (e.g., de lowpass à bandpass) modifie fondamentalement l'algorithme et l'état interne du filtre. Nécessite une ré-instanciation du nœud.
    - `cutoff_frequency`, `order`: OUI. Ces paramètres peuvent être modifiés dynamiquement si le nœud est conçu pour recalculer ses coefficients de filtre internes (e.g., via des setters thread-safe) sans interrompre le flux de données.
    - `target_channel`: OUI. Le ciblage du canal peut souvent être ajusté dynamiquement.
- **Conclusion sur le Hot-Reload du Nœud** : PARTIELLEMENT SUPPORTÉ.
- **Stratégie de mise à jour recommandée** :
    - Pour `cutoff_frequency`, `order`, `target_channel`: Implémenter des setters thread-safe permettant la mise à jour dynamique.
    - Pour `filter_type`: Nécessite la reconstruction du nœud (et potentiellement du graphe si les connexions sont affectées).

### 2.3 `ChannelSelectorNode`

- **Description** : Nœud sélectionnant un ou plusieurs canaux spécifiques à partir d'un flux multi-canaux.
- **Paramètres principaux** : `selected_channels` (liste des canaux à conserver/sélectionner).
- **Analyse de la capacité de Hot-Reload** :
    - `selected_channels`: OUI. La sélection des canaux peut typiquement être modifiée dynamiquement par un setter, car cela implique souvent un changement d'index ou de mapping simple.
- **Conclusion sur le Hot-Reload du Nœud** : SUPPORTÉ.
- **Stratégie de mise à jour recommandée** : Implémenter un setter thread-safe pour `selected_channels`.

### 2.4 `DifferentialNode`

- **Description** : Nœud calculant une différence (e.g., entre canaux, ou par rapport à une valeur).
- **Paramètres principaux** : `mode` (type de différentiel: e.g., `ChannelA - ChannelB`, `ChannelA - Constant`), `constant_value` (si applicable).
- **Analyse de la capacité de Hot-Reload** :
    - `mode`: NON. Changer le mode de calcul différentiel peut impliquer une logique différente et un nombre d'entrées différent. Nécessite ré-instanciation.
    - `constant_value`: OUI. Si le mode utilise une constante, cette valeur peut être mise à jour dynamiquement via un setter.
- **Conclusion sur le Hot-Reload du Nœud** : PARTIELLEMENT SUPPORTÉ.
- **Stratégie de mise à jour recommandée** :
    - Pour `constant_value`: Setter thread-safe.
    - Pour `mode`: Reconstruction du nœud.

### 2.5 `MixerNode` / `ChannelMixerNode`

- **Description** : Nœud combinant plusieurs canaux ou signaux.
- **Paramètres principaux** : `mix_strategy` (e.g., addition, moyenne), `channel_weights` (pondérations pour chaque canal d'entrée).
- **Analyse de la capacité de Hot-Reload** :
    - `mix_strategy`: NON. Changer la stratégie de mixage (e.g., d'une simple addition à une moyenne pondérée complexe) peut nécessiter une refonte de la logique interne.
    - `channel_weights`: OUI. Les pondérations des canaux sont typiquement des coefficients qui peuvent être ajustés dynamiquement via des setters.
- **Conclusion sur le Hot-Reload du Nœud** : PARTIELLEMENT SUPPORTÉ.
- **Stratégie de mise à jour recommandée** :
    - Pour `channel_weights`: Setters thread-safe.
    - Pour `mix_strategy`: Reconstruction du nœud.

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
| FilterNode          | PARTIEL                | `cutoff_frequency`, `order`           | `filter_type`                             | NON (rebuild requis)                                                  |
| ChannelSelectorNode | OUI                    | `selected_channels`                   | -                                         | NON (rebuild requis)                                                  |
| DifferentialNode    | PARTIEL                | `constant_value`                      | `mode`                                    | NON (rebuild requis)                                                  |
| MixerNode           | PARTIEL                | `channel_weights`                     | `mix_strategy`                            | NON (rebuild requis)                                                  |
| RecordNode          | PARTIEL                | `max_size`, `rolling_buffer`          | `output_path`, `file_format`              | NON (rebuild requis)                                                  |
| OutputNode          | PARTIEL                | `analysis_thresholds`, `window_size`  | `output_target`                           | NON (rebuild requis)                                                  |
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
