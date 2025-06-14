# Audit : possibilité de mise à jour dynamique des nœuds du ProcessingGraph

Ce document détaille, pour chaque type de nœud du `ProcessingGraph`, si une mise à jour dynamique de ses paramètres est possible _sans_ redémarrer le processing (hot-reload), ou si une reconstruction du graphe (et donc un redémarrage du processing consumer) est nécessaire.

---

## 1. Principe général

- Le `ProcessingGraph` est une structure de graphe orienté composée de nœuds (`ProcessingNode`), chaque nœud ayant des paramètres spécifiques.
- Les nœuds sont typés (`node_type`) : input, filter, channel_selector, differential, mixer, record, output, etc.
- La dynamique de modification dépend :
  - de la capacité de chaque nœud à supporter la mutation de ses paramètres à chaud,
  - de la capacité du graphe à accepter l’ajout/retrait/reconnexion de nœuds à chaud (typiquement non supporté, nécessite un rebuild).

---

## 2. Détail par type de nœud

### 2.1 `InputNode`

- **Paramètres** : ID, type de données attendues.
- **Hot-reload ?** : NON (modif. du type d’entrée requiert une reconstruction du graphe).

### 2.2 `FilterNode` (ex : lowpass, highpass, bandpass)

- **Paramètres** : fréquence de coupure, type de filtre, ordre, cible (channel).
- **Hot-reload ?** :  
  - **OUI** si le code implémente la mise à jour des coefficients en temps réel (ex : setter pour la fréquence/ordre, recalcul du filtre).
  - **NON** sinon (reconstruction du nœud nécessaire).
- **Recommandation** : Préférer des setters atomiques pour les paramètres de filtre (sinon, restart processing).

### 2.3 `ChannelSelectorNode`

- **Paramètres** : canal à sélectionner (`ChannelA` / `ChannelB`).
- **Hot-reload ?** : OUI, si setter implémenté (simple valeur atomique, pas de grosse logique).

### 2.4 `DifferentialNode`

- **Paramètres** : type de différentiel, éventuelles constantes.
- **Hot-reload ?** : OUI, si setter prévu, sinon NON.

### 2.5 `MixerNode` / `ChannelMixerNode`

- **Paramètres** : stratégie de mixage, pondérations.
- **Hot-reload ?** : OUI, si setters pour les pondérations ou la stratégie.

### 2.6 `RecordNode`

- **Paramètres** : chemin fichier, taille max, rolling buffer, auto-delete.
- **Hot-reload ?** :  
  - **OUI** pour rolling buffer, taille max, auto-delete, si le node surveille ses propres paramètres.
  - **NON** pour le chemin de fichier (car changement de file handle, nécessite close/reopen, donc un restart du node).
- **Remarque :** Modifier `record_file` nécessite typiquement de fermer et rouvrir un fichier, donc pas hot-reloadable sans code spécifique.

### 2.7 `PhotoacousticOutputNode`

- **Paramètres** : seuils, window size.
- **Hot-reload ?** : Oui pour les seuils et fenêtres d’analyse (si code prévu), car ce sont des paramètres simples.

### 2.8 `Custom/Plugin` nodes

- **Paramètres** : dépend du plugin.
- **Hot-reload ?** : dépend du code.

---

## 3. Synthèse récapitulative

| Node Type              | Hot-reload paramètres ?         | Changement structure (ajout/suppression nœud/connexion) |
|------------------------|:------------------------------:|:-------------------------------------------------------:|
| InputNode              | NON                            | NON                                                     |
| FilterNode             | OUI (coeffs dynamiques)        | NON                                                     |
| ChannelSelectorNode    | OUI                            | NON                                                     |
| DifferentialNode       | OUI                            | NON                                                     |
| MixerNode              | OUI                            | NON                                                     |
| RecordNode             | OUI* (pas pour le fichier)     | NON                                                     |
| OutputNode             | OUI                            | NON                                                     |
| Ajout/Suppression nœud | -                              | NON (rebuild requis)                                    |

(*) Changement du fichier cible nécessite close/reopen, donc un redémarrage du node (ou du graphe).

---

## 4. Remarques sur la dynamique du graphe

- **Modification structurelle du graphe** (ajout, suppression, reconnexion de nœuds) :  
  → **Pas hot-reloadable** dans l’état actuel, nécessite une reconstruction et un restart du `ProcessingConsumer`.

- **Modification des paramètres** :  
  - Si les nœuds exposent des setters thread-safe, le reload dynamique est possible sans restart du graphe.
  - Sinon, il faut retirer et ré-instancier le nœud concerné (donc restart).

---

## 5. Recommandations de conception

- Prévoir, pour chaque node hot-reloadable, des setters thread-safe sur les paramètres dynamiques.
- Documenter pour chaque node la liste des paramètres modifiables à chaud.
- Pour la modification de la structure du graphe (topologie), exposer une API de `reload_graph()` qui remplace le graphe en cours (restart du processing).

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

---

## 7. Pour aller plus loin

- Voir le code source pour confirmer la présence de setters dynamiques pour chaque type de nœud :  
  [processing/nodes.rs](https://github.com/sctg-development/rust-photoacoustic/tree/main/rust/src/processing/nodes.rs)
- Pour vérifier la dynamique du graphe, voir :  
  [processing/graph.rs](https://github.com/sctg-development/rust-photoacoustic/tree/main/rust/src/processing/graph.rs)
- Pour lister tous les types de nœuds disponibles :  
  [config/processing.rs (NodeConfig)](https://github.com/sctg-development/rust-photoacoustic/blob/main/rust/src/config/processing.rs)

---

**_Analyse basée sur un échantillon limité. Voir [GitHub Code Search](https://github.com/sctg-development/rust-photoacoustic/search?q=ProcessingNode) pour le code complet._**