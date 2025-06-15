\
<!-- filepath: /home/coder/rust-photoacoustic/docs/AUDIT_IMPACT_RELOAD_DAEMON.md -->
# Audit d\'Impact : Rechargement de Configuration par Composant Daemon

Ce document détaille, pour chaque composant principal (démon/service) démarré et géré au sein du projet `rust-photoacoustic`, l\'impact d\'une modification de la configuration. Il évalue si un rechargement à chaud (hot-reload) des paramètres est possible ou si un redémarrage complet du composant est requis. Cette analyse est cruciale pour implémenter une gestion de configuration dynamique efficace et minimiser les interruptions de service.

_Basé sur l’implémentation typique des démons, la structure de configuration `Config` et les objectifs de la configuration dynamique._

---

## 1. Principes Généraux d\'Analyse

Pour chaque composant, l\'analyse considère :

-   **Paramètres de Configuration Utilisés** : Quelles sections/valeurs de la `Config` globale sont lues et utilisées par le composant.
-   **Moment de Lecture de la Configuration** : La configuration est-elle lue uniquement au démarrage, périodiquement, ou sur notification ?
-   **Nature des Paramètres** :
    -   **Structurels** : Paramètres qui définissent l\'infrastructure du composant (ex: port d\'écoute, fichier de périphérique, type de pipeline). Leur modification nécessite souvent un redémarrage.
    -   **Comportementaux/Opérationnels** : Paramètres qui modifient le comportement en cours d\'exécution (ex: seuils, intervalles, niveaux de log). Souvent candidats au hot-reload.
-   **Capacité de Hot-Reload Intrinsèque** : Le code du composant est-il conçu pour détecter et appliquer les changements de configuration sans redémarrer ? (ex: via des setters, des canaux de notification comme `tokio::sync::watch`, ou relecture de `Arc<RwLock<Config>>`).

---

## 2. Analyse Détaillée par Composant

### 2.1 Visualization Web Server (Ex: Rocket)

-   **Paramètres de Config Utilisés** :
    -   `config.visualization.enabled`: Activation du serveur.
    -   `config.visualization.address`, `config.visualization.port`: Adresse et port d\'écoute.
    -   `config.visualization.tls`: Configuration TLS (certificats, clés).
    -   `config.security.hmac_secret`, `config.session.*`: Paramètres de sécurité, gestion de session.
    -   Autres paramètres spécifiques à l\'interface utilisateur ou aux fonctionnalités exposées.
-   **Analyse de la Capacité de Hot-Reload** :
    -   **NON** pour `address`, `port`, `tls` (certificats) : Ces paramètres sont typiquement liés au `bind` du socket serveur au démarrage de Rocket. Leur modification nécessite un redémarrage complet du serveur Rocket.
    -   **OUI (Potentiel)** pour `hmac_secret`, paramètres de session, options d\'affichage/fonctionnalité : Si le code est structuré pour relire ces valeurs depuis `Arc<RwLock<Config>>` à chaque requête pertinente ou est notifié d\'un changement (ex: via un `State<Arc<RwLock<Config>>>` et relecture, ou un canal `watch`).
    -   **NON** pour `enabled` (si le changement est de `true` à `false` pour arrêter, ou `false` à `true` pour démarrer) : Géré par le `DaemonManager` qui démarrera/arrêtera le service.
-   **Conclusion** :
    -   **Redémarrage Indispensable** pour les changements d\'adresse, port, configuration TLS.
    -   **Hot-Reload Possible** pour les paramètres de sécurité (ex: rotation de clé HMAC si gérée), options de session, et autres paramètres de runtime, *si le code est adapté*.
-   **Stratégie de Mise à Jour Recommandée** :
    -   Le `DaemonManager` doit redémarrer le serveur Rocket si `address`, `port`, ou `tls` changent.
    -   Pour les autres paramètres, le serveur peut être notifié ou relire la configuration.

### 2.2 Data Acquisition Daemon

-   **Paramètres de Config Utilisés** :
    -   `config.acquisition.enabled`: Activation.
    -   `config.acquisition.device_name`, `config.acquisition.sample_rate`, `config.acquisition.channels`, `config.acquisition.format`: Paramètres du périphérique audio/source de données.
    -   `config.acquisition.buffer_size`: Taille des tampons.
    -   `config.acquisition.polling_interval` (si applicable) : Intervalle d\'interrogation.
    -   `config.acquisition.gain_db` (si applicable) : Gain appliqué.
-   **Analyse de la Capacité de Hot-Reload** :
    -   **NON** pour `device_name`, `sample_rate`, `channels`, `format` : Changer la source de données ou ses caractéristiques fondamentales (format, taux d\'échantillonnage) nécessite généralement de fermer le handle du périphérique actuel et d\'en ouvrir un nouveau, ce qui équivaut à un redémarrage du cœur de l\'acquisition.
    -   **OUI (Potentiel)** pour `buffer_size` (si modifiable sans réallouer toute la chaîne), `polling_interval`, `gain_db` : Ces paramètres peuvent être ajustés dynamiquement si le démon est conçu pour les relire et les appliquer à chaque cycle ou sur notification.
    -   **NON** pour `enabled` : Géré par le `DaemonManager`.
-   **Conclusion** :
    -   **Redémarrage Indispensable** pour les changements de périphérique, format, taux d\'échantillonnage, nombre de canaux.
    -   **Hot-Reload Possible** pour `buffer_size` (avec précautions), `polling_interval`, `gain_db`, *si le code est adapté*.
-   **Stratégie de Mise à Jour Recommandée** :
    -   Le `DaemonManager` doit redémarrer le démon d\'acquisition si les paramètres structurels du périphérique changent.
    -   Les paramètres opérationnels peuvent être mis à jour via notification/relecture.

### 2.3 Processing Consumer Daemon (Gestionnaire du `ProcessingGraph`)

-   **Paramètres de Config Utilisés** :
    -   `config.processing.enabled`: Activation.
    -   `config.processing.graph_definition`: La structure complète du `ProcessingGraph` (nœuds, connexions, paramètres des nœuds).
    -   Paramètres spécifiques au comportement global du consumer (ex: taille de file d\'attente d\'entrée).
-   **Analyse de la Capacité de Hot-Reload** :
    -   **NON** pour des changements majeurs dans `graph_definition` (ajout/suppression de nœuds, changement de connexions, changement de type de nœud nécessitant une ré-instanciation) : Cela requiert une reconstruction complète du `ProcessingGraph` et donc un redémarrage du consumer. Voir `AUDIT_PROCESSINGGRAPH_NODES_HOT_RELOAD.md`.
    -   **PARTIEL (via les nœuds)** pour les modifications de paramètres *internes* aux nœuds du `ProcessingGraph` qui supportent eux-mêmes le hot-reload (ex: fréquence de coupure d\'un `FilterNode`). Le `ProcessingConsumer` doit être capable de propager ces changements aux nœuds concernés ou les nœuds doivent s\'abonner aux changements.
    -   **✅ OUI - IMPLÉMENTÉ** pour les paramètres du `GainNode` : Le `GainNode` supporte désormais le hot-reload complet de son paramètre `gain_db` via la méthode `update_config()`. Le `ProcessingConsumer` peut propager les changements de configuration directement au nœud sans redémarrage.
    -   **✅ OUI - IMPLÉMENTÉ** pour les paramètres du `ChannelSelectorNode` : Le `ChannelSelectorNode` supporte le hot-reload de son paramètre `target_channel` (ChannelA/ChannelB) permettant de changer dynamiquement le canal sélectionné.
    -   **✅ OUI - IMPLÉMENTÉ** pour les paramètres du `ChannelMixerNode` : Le `ChannelMixerNode` supporte le hot-reload de sa `mix_strategy` (Add, Subtract, Average, Weighted) permettant de changer dynamiquement la stratégie de mixage des canaux.
    -   **✅ OUI - IMPLÉMENTÉ** pour les paramètres des filtres numériques : `LowpassFilter`, `HighpassFilter`, et `BandpassFilter` supportent le hot-reload de leurs paramètres principaux (`cutoff_freq`, `center_freq`, `bandwidth`, `sample_rate`, `order`) avec validation complète et recalcul automatique des coefficients.
    -   **✅ OUI - IMPLÉMENTÉ** pour les paramètres du `FilterNode` : Le `FilterNode` supporte le hot-reload de son paramètre `target_channel` (ChannelA, ChannelB, Both) permettant de rediriger dynamiquement l'application du filtre.
    -   **⚠️ PRÉPARÉ** pour le `DifferentialNode` : Infrastructure `update_config()` en place mais pas de paramètres hot-reloadables actuellement. Nécessite reconstruction du nœud pour tout changement.
    -   **NON** pour `enabled` : Géré par le `DaemonManager`.
-   **Conclusion** :
    -   **Redémarrage Indispensable** pour toute modification de la topologie du graphe ou des types de nœuds incompatibles avec le hot-reload.
    -   **Hot-Reload Possible (Délégué aux Nœuds)** pour les paramètres internes des nœuds qui le supportent. Le `ProcessingConsumer` agit comme un orchestrateur.
    -   **✅ Hot-Reload Entièrement Supporté** pour les paramètres du `GainNode` - aucun redémarrage requis.
    -   **✅ Hot-Reload Entièrement Supporté** pour les paramètres du `ChannelSelectorNode` et `ChannelMixerNode` - aucun redémarrage requis.
    -   **✅ Hot-Reload Entièrement Supporté** pour les paramètres des filtres numériques (`LowpassFilter`, `HighpassFilter`, `BandpassFilter`) - aucun redémarrage requis.
    -   **✅ Hot-Reload Entièrement Supporté** pour les paramètres du `FilterNode` (`target_channel`) - aucun redémarrage requis.
-   **Stratégie de Mise à Jour Recommandée** :
    -   Le `DaemonManager` redémarre le `ProcessingConsumer` si la structure du graphe change fondamentalement.
    -   Pour les changements de paramètres de nœuds, le `ProcessingConsumer` (ou les nœuds directement) doit gérer le rechargement. Une notification du `DaemonManager` au `ProcessingConsumer` peut initier ce processus.
    -   **Pour le `GainNode`** : Simple notification au `ProcessingConsumer` qui peut appeler `node.update_config()` avec les nouveaux paramètres. Aucune interruption de service requise.
    -   **Pour le `ChannelSelectorNode` et `ChannelMixerNode`** : Même approche - notification au `ProcessingConsumer` qui appelle `node.update_config()`. Changements appliqués instantanément sans interruption.
    -   **Pour les filtres numériques** : Notification au `ProcessingConsumer` qui peut mettre à jour les filtres via leurs méthodes `update_config()`. Les coefficients sont recalculés automatiquement pour le `BandpassFilter`.
    -   **Pour le `FilterNode`** : Mise à jour du paramètre `target_channel` via `node.update_config()` pour rediriger l'application du filtre sans interruption.

### 2.4 Modbus Server

-   **Paramètres de Config Utilisés** :
    -   `config.modbus.enabled`: Activation.
    -   `config.modbus.address`, `config.modbus.port`: Adresse et port d\'écoute TCP/IP ou paramètres série.
    -   `config.modbus.registers_mapping`: Définition des registres Modbus, leur type, et comment ils sont liés aux données internes du système.
    -   `config.modbus.polling_period_ms` (si le serveur Modbus lit activement des données internes à une certaine fréquence).
-   **Analyse de la Capacité de Hot-Reload** :
    -   **NON** pour `address`, `port` : Nécessite un re-bind du socket, donc redémarrage du serveur Modbus.
    -   **PARTIEL/OUI** pour `registers_mapping` :
        -   Changer l\'adresse d\'un registre ou son type : Potentiellement OUI, si la logique interne peut remapper dynamiquement.
        -   Ajouter/Supprimer des registres : Potentiellement OUI, mais peut être complexe.
        -   Changer la source de données d\'un registre : OUI, si la logique de lecture est flexible.
    -   **OUI** pour `polling_period_ms` : Facilement ajustable dynamiquement.
    -   **NON** pour `enabled` : Géré par le `DaemonManager`.
-   **Conclusion** :
    -   **Redémarrage Indispensable** pour les changements d\'adresse/port.
    -   **Hot-Reload Possible (avec complexité variable)** pour `registers_mapping`. Les changements simples (ex: source de données d\'un registre existant) sont plus faciles que les changements structurels du mapping.
    -   **Hot-Reload Possible** pour `polling_period_ms`.
-   **Stratégie de Mise à Jour Recommandée** :
    -   Le `DaemonManager` redémarre le serveur Modbus si `address` ou `port` changent.
    -   Pour `registers_mapping`, une notification peut déclencher une re-configuration interne. Les modifications complexes peuvent nécessiter un redémarrage par prudence.

### 2.5 Record Consumer (Enregistrement de Données)

-   **Paramètres de Config Utilisés** :
    -   `config.photoacoustic.record_consumer.enabled` (ou un chemin similaire) : Activation.
    -   `config.photoacoustic.record_consumer.output_directory`: Dossier de destination des fichiers.
    -   `config.photoacoustic.record_consumer.file_format`: Format des fichiers (ex: WAV, CSV, binaire personnalisé).
    -   `config.photoacoustic.record_consumer.max_file_size_mb`, `config.photoacoustic.record_consumer.max_duration_s`: Critères de segmentation des fichiers.
    -   `config.photoacoustic.record_consumer.rolling_buffer_config`: Configuration pour l\'enregistrement en continu avec écrasement.
-   **Analyse de la Capacité de Hot-Reload** :
    -   **NON** pour `output_directory`, `file_format` (si le changement implique une logique d\'écriture différente ou la fermeture/ouverture de fichiers) : Changer le lieu ou le format fondamental de l\'enregistrement nécessite souvent de finaliser le fichier courant et d\'en ouvrir un nouveau, ce qui s\'apparente à un redémarrage de la logique d\'enregistrement.
    -   **OUI** pour `max_file_size_mb`, `max_duration_s`, paramètres de `rolling_buffer_config` (ex: durée à conserver) : Ces paramètres peuvent être lus et appliqués pour les *nouveaux* segments de fichiers ou pour la gestion du buffer existant, si le code est prévu pour.
    -   **NON** pour `enabled` : Géré par le `DaemonManager`.
-   **Conclusion** :
    -   **Redémarrage (ou réinitialisation majeure de la logique d\'enregistrement) Indispensable** pour les changements de `output_directory` ou `file_format`.
    -   **Hot-Reload Possible** pour les paramètres de segmentation et de gestion du buffer, *si le code est adapté*.
-   **Stratégie de Mise à Jour Recommandée** :
    -   Le `DaemonManager` devrait notifier le Record Consumer. Celui-ci pourrait avoir une logique interne pour finaliser l\'enregistrement en cours, puis se reconfigurer et redémarrer l\'enregistrement si `output_directory` ou `file_format` changent. Pour les autres paramètres, il peut les appliquer dynamiquement.

### 2.6 Heartbeat Monitoring

-   **Paramètres de Config Utilisés** :
    -   Généralement peu de configuration dynamique, peut-être un intervalle ou des endpoints à surveiller.
    -   `config.heartbeat.interval_s`: Intervalle d\'émission ou de vérification.
    -   `config.heartbeat.targets`: Liste des cibles à notifier ou vérifier.
-   **Analyse de la Capacité de Hot-Reload** :
    -   **OUI** pour `interval_s`, `targets` : Ces paramètres sont généralement faciles à mettre à jour dynamiquement.
-   **Conclusion** :
    -   **Hot-Reload Généralement Possible** pour la plupart des paramètres.
    -   Un redémarrage est rarement justifié sauf en cas de refonte majeure de sa logique.
-   **Stratégie de Mise à Jour Recommandée** :
    -   Le composant peut relire sa configuration sur notification ou périodiquement.

---

## 3. Tableau Récapitulatif de l\'Impact

| Composant                 | Paramètres Structurels (Restart Souvent Requis)                                  | Paramètres Comportementaux (Hot-Reload Possible si Prévu)                                     | Géré par DaemonManager (Enable/Disable) |
|---------------------------|-----------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------|-----------------------------------------|
| **Visualization Server**  | `address`, `port`, `tls` (certs)                                                  | Sécurité (HMAC, session), options UI                                                         | Oui                                     |
| **Data Acquisition**      | `device_name`, `sample_rate`, `channels`, `format`                                | `buffer_size` (partiel), `polling_interval`, `gain_db`                                       | Oui                                     |
| **Processing Consumer**   | Structure du `ProcessingGraph` (nœuds, connexions, types incompatibles)           | Paramètres internes des nœuds du graphe (si le nœud supporte le hot-reload), **`GainNode.gain_db` ✅**, **`ChannelSelectorNode.target_channel` ✅**, **`ChannelMixerNode.mix_strategy` ✅**, **Filtres numériques (cutoff_freq, center_freq, bandwidth, etc.) ✅**, **`FilterNode.target_channel` ✅**  | Oui                                     |
| **Modbus Server**         | `address`, `port`                                                                 | `registers_mapping` (partiel/complexe), `polling_period_ms`                                  | Oui                                     |
| **Record Consumer**       | `output_directory`, `file_format`                                                 | `max_file_size_mb`, `max_duration_s`, `rolling_buffer_config`                                | Oui                                     |
| **Heartbeat Monitoring**  | (Rarement des changements structurels majeurs)                                    | `interval_s`, `targets`                                                                      | Oui (si on peut le désactiver)          |

---

## 4. Recommandations Générales pour le `DaemonManager`

1.  **Analyse Différentielle** :
    *   Lors d\'une mise à jour de configuration, le `DaemonManager` devrait, si possible, comparer l\'ancienne et la nouvelle configuration pour identifier précisément les sections modifiées.
    *   Ceci permet de cibler uniquement les démons affectés.

2.  **Logique de Décision Basée sur l\'Audit** :
    *   Le `DaemonManager` doit implémenter une logique qui, pour chaque démon affecté par un changement :
        *   Consulte cet audit (`AUDIT_IMPACT_RELOAD_DAEMON.md`) pour déterminer si les paramètres modifiés nécessitent un redémarrage ou si un hot-reload est envisageable.
        *   Prend en compte la capacité réelle de hot-reload du code du démon.

3.  **Orchestration des Actions** :
    *   **Redémarrage** : Arrêter proprement le démon, puis le relancer avec la nouvelle configuration (ou en s\'assurant qu\'il lira la nouvelle configuration depuis `Arc<RwLock<Config>>` au redémarrage).
    *   **Hot-Reload** : Envoyer une notification au démon concerné (ex: via un canal `tokio::sync::watch`, `mpsc`, ou autre mécanisme IPC asynchrone). Le démon est alors responsable d\'appliquer les changements.

4.  **Gestion des Dépendances** :
    *   Si des démons ont des dépendances entre eux, l\'ordre de redémarrage ou de notification peut être important.

5.  **Journalisation et Retours d\'État** :
    *   Journaliser toutes les actions entreprises (quel démon redémarré/notifié, pour quels changements).
    *   L\'API `/api/config` devrait idéalement retourner un statut indiquant si la configuration a été appliquée et si des redémarrages ont été initiés.

6.  **Priorité à la Stabilité** :
    *   En cas de doute sur la capacité d\'un composant à gérer un hot-reload pour un changement donné, privilégier un redémarrage contrôlé pour garantir la stabilité.

---

## 5. Diagramme de Flux de Décision du `DaemonManager`

```mermaid
graph TD
    API_UPDATE[API /api/config reçoit nouvelle Config] --> |1. Valide & Écrit dans Arc<RwLock<Config>>| SHARED_CONFIG[Arc<RwLock<Config>>]
    SHARED_CONFIG --> |2. Notifie| DM[DaemonManager]
    
    DM --> |3. Compare ancienne/nouvelle Config| DIFF_ANALYSIS{Analyse des Différences}
    
    subgraph Pour Chaque Service Affecté
        DIFF_ANALYSIS --> |Service X affecté| CHECK_AUDIT_X{Consulte AUDIT_IMPACT_RELOAD_DAEMON.md pour Service X}
        CHECK_AUDIT_X --> |Paramètres modifiés nécessitent restart?| NEED_RESTART_X{Restart Requis?}
        NEED_RESTART_X -- Oui --> |4a. Arrête Service X| STOP_X[Arrêt Service X]
        STOP_X --> |Relance Service X| START_X[Lancement Service X avec nouvelle Config]
        START_X --> SERVICE_X_RUNNING[Service X Opérationnel]
        
        NEED_RESTART_X -- Non --> CHECK_HOT_RELOAD_X{Service X supporte Hot-Reload pour ces params?}
        CHECK_HOT_RELOAD_X -- Oui --> |4b. Notifie Service X| NOTIFY_X[Notification Hot-Reload à Service X]
        NOTIFY_X --> SERVICE_X_RELOADED[Service X Recharge à Chaud]
        SERVICE_X_RELOADED --> SERVICE_X_RUNNING
        
        CHECK_HOT_RELOAD_X -- Non (ou incertain) --> |Fallback: Prudence| STOP_X_FALLBACK[Arrêt Service X (Fallback)]
        STOP_X_FALLBACK --> START_X_FALLBACK[Lancement Service X (Fallback)]
        START_X_FALLBACK --> SERVICE_X_RUNNING
    end

    DM --> |5. Journalise actions & retourne statut à l'API| LOG_STATUS[Journalisation & Statut]
```

---
