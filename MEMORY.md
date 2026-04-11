# MEMORY — rust-photoacoustic

## Identité du projet

- **Nom** : rust-photoacoustic (LaserSmartApiServer/0.1.0)
- **Repo** : github.com/sctg-development/rust-photoacoustic
- **Auteur** : Ronan Le Meillat / SCTG Development
- **Licence** : SCTG-Non-Commercial-1.0
- **Objectif** : Analyseur de gaz industriel par spectroscopie photoacoustique laser différentielle (LPAS) — sensibilité ppb
- **Workspace** : Cargo workspace (`rust/`, `rust/auth-macros/`) + app web (`web/`)

---

## Architecture bipartite

```
rust/   → Backend Rust : acquisition audio, traitement signal, API REST, auth OAuth2, thermique
web/    → Frontend React : visualisation seule, aucune logique métier
```

- Le backend Rocket sert le frontend compilé depuis `/client/` (route statique embedded)
- `--external-web-client <URL>` : proxy vers Vite dev server en développement
- Les deux parties communiquent uniquement via l'API REST et SSE

---

## Modules Rust (`rust/src/`)

| Module | Rôle |
|---|---|
| `acquisition/` | Sources audio : microphone CPAL, fichier WAV, mock, simulateur physique universel |
| `preprocessing/` | Filtres numériques : Butterworth bandpass/lowpass/highpass, gain, channel mixing |
| `spectral/` | FFT (RealFFT), analyse spectrale, fenêtrage (Hann, Blackman, Hamming) |
| `processing/` | Moteur de graphe : nœuds, consommateurs, résultats, computing nodes |
| `visualization/` | Serveur Rocket, API REST OpenAPI, auth OAuth2/OIDC, SSE streaming |
| `thermal_regulation/` | Régulation PID thermique, drivers I2C, simulation thermique |
| `modbus/` | Serveur Modbus TCP (port 5502 par défaut) |
| `daemon/` | Daemon d'acquisition temps réel, boucle de traitement |
| `config/` | Structures de config YAML : `acquisition`, `processing`, `visualization`, `modbus`, `thermal_regulation`, `access`, `generix`, `photoacoustic` |
| `photoacoustic/` | Logique métier PA : source simulée universelle avec physique Helmholtz |
| `utility/` | Utilitaires transversaux |
| `auth-macros/` | Proc-macros Rust pour `openapi_protect_get` et `openapi_protect_post` avec JWT |

---

## Processing graph engine

Graphe reconfigurable de nœuds défini en YAML. Thread séparé du runtime Rocket (Tokio).  
Hot-reload du graphe YAML sans redémarrage possible.

### Nœuds disponibles (`processing/nodes/`)

| Fichier / Nœud | `node_type` YAML |
|---|---|
| `input.rs` — InputNode | `input` |
| `filter.rs` — ButterworthFilter | `filter` (butter_bandpass, butter_lowpass, butter_highpass) |
| `differential.rs` — ChannelMixer | `channel_mixer` (add, subtract) |
| `gain.rs` — GainNode | `gain` |
| `record.rs` — RecordNode | `record` |
| `streaming.rs` — StreamingNode | `streaming` |
| `channel.rs` — ChannelNode | interne |
| `python.rs` — PythonNode | `python` (PyO3, script_path, auto_reload) |
| `output.rs` — OutputNode | `output` |

### Computing nodes (`processing/computing_nodes/`)

| Fichier / Nœud | `node_type` YAML |
|---|---|
| `peak_finder.rs` — PeakFinderNode | `computing_peak_finder` |
| `concentration.rs` — ConcentrationNode | `computing_concentration` |
| `universal_action.rs` — ActionUniversalNode | `action_universal` |

### Action drivers (`processing/computing_nodes/action_drivers/`)

- `https_callback` — POST JSON vers URL webhook
- `redis` — Key-value ou Pub/Sub vers Redis (modes `key_value` / `pubsub`)
- Kafka (partiel)

### Trait central

```rust
pub trait ProcessingNode: Send + Sync {
    fn process(&mut self, data: ProcessingData) -> Result<ProcessingData>;
    fn node_id(&self) -> &str;
    fn node_type(&self) -> &str;
    fn clone_node(&self) -> Box<dyn ProcessingNode>;
    // + méthodes optionnelles pour shared state, stats, etc.
}
```

---

## API REST & streaming (Rocket)

Base URL : `https://<host>:8080`  
Docs interactives : `/rapidoc` et `/api/doc/openapi.json`

### Routes principales

| Méthode | Route | Scope requis | Description |
|---|---|---|---|
| GET | `/api/config` | `admin:api` | Configuration complète |
| GET | `/api/config.schema.json` | `admin:api` | JSON Schema config |
| GET | `/api/config/visualization/output` | `read:api` | Output items |
| GET | `/api/graph` | `read:api` | Graphe de traitement courant |
| GET | `/api/graph-statistics` | `read:api` | Stats du graphe |
| POST | `/api/graph/config` | `write:api` | Mettre à jour le graphe |
| GET | `/api/thermal` | `read:api` | Données thermiques |
| GET | `/api/computing` | `read:api` | Résultats computing nodes |
| GET | `/api/profile` | `read:api` | Profil utilisateur |
| GET | `/api/data` | `read:api` | Données brutes |
| GET | `/api/status` | public | Status serveur |

### Auth

- OAuth2 PKCE pour clients publics (SPA)
- JWT Bearer obligatoire sur toutes les routes `/api/*` (sauf `/api/status`)
- Proc-macro `openapi_protect_get` / `openapi_protect_post` : injecte automatiquement la vérification du scope JWT et la doc OpenAPI
- OIDC : `/.well-known/openid-configuration`, `/.well-known/jwks.json`
- Introspection : `/introspect`

### SSE (Server-Sent Events)

- Audio brut et spectral : `EventStream` Rocket via `visualization/streaming/audio.rs`
- `StreamingNodeRegistry` : registre partagé des nœuds `streaming` du graphe
- Le client React doit gérer la reconnexion automatique

### Configuration CORS

`rocket_cors` configuré dans `visualization/server/cors.rs`.

---

## Frontend — pages & hooks (`web/src/`)

### Pages (`pages/`)

| Fichier | Contenu |
|---|---|
| `index.tsx` | Dashboard principal, concentration en temps réel |
| `audio.tsx` | Visualisation audio et spectrale (FFT) |
| `thermal.tsx` | Courbes de régulation thermique PID |
| `graph.tsx` | Éditeur/visualiseur du graphe de traitement |
| `local.tsx` | Données locales |
| `blog.tsx` | Documentation embarquée |
| `404.tsx` | Page d'erreur |

### Hooks (`hooks/`)

| Fichier | Rôle |
|---|---|
| `useAudioStream.ts` | Connexion SSE, reconstruction signal, FFT client (~2000 lignes) |
| `use-processing-graph.ts` | Fetch, mise à jour et abonnement au graphe de traitement |
| `use-theme.ts` | Gestion thème clair/sombre |

### Autres répertoires

- `authentication/` : flux OIDC/Auth0 (generix provider)
- `components/` : composants UI réutilisables
- `contexts/` : React Context pour état global
- `types/` : types TypeScript strict
- `utilities/` : helpers
- `styles/` : CSS global minimal
- `layouts/` : layouts de page

### i18n

6 langues : `en-US`, `fr-FR`, `es-ES`, `zh-CN`, `ar-SA` (RTL), `he-IL` (RTL)  
Librairie : `react-i18next`, fichiers dans `web/src/locales/base/`

### Auth

OIDC via provider "generix" (configurable dans `generix:` YAML).  
En prod pointe sur le serveur Rocket lui-même.

---

## Hardware Laser+Smart

Schéma Altium dans `hardware/6C47543F-*/`

| Composant | Rôle | Interface |
|---|---|---|
| ATmega32U4 | Microcontrôleur principal, USB-HID | USB |
| 4× ADS1115 | ADC 16-bit, 860 SPS, adresses 0x48–0x4B selon broche ADDR | I2C |
| LTC2641 | DAC 12-bit | SPI |
| AD9833 | DDS signal generator, résolution 0.004 Hz, sortie sinus/carré/triangle | SPI |
| MCP23017 (remplacé CAT9555 0x20) | GPIO expander 16 bits | I2C |
| REF5040 | Référence de tension 4.096V | — |
| PCA9685 (0x40) | Contrôleur PWM 16 canaux (régulation thermique) | I2C |

---

## Paramètres physiques de référence

| Paramètre | Valeur |
|---|---|
| Fréquence d'excitation laser | 2000–2100 Hz (configurable, ex. 2000.8 Hz) |
| Fréquence d'échantillonnage | 48 000 Hz |
| Taille FFT (frame_size) | 4096 ou 8192 pts (configurable) |
| Latence cible | < 10 ms |
| Sensibilité | ppb (parties par milliard) |
| Précision thermique (PID) | ±0.1°C |
| Canaux microphones | 2 (configuration différentielle) |
| Modulation laser | Pulsée (~20 Hz) ou continue |

---

## Intégrations externes

| Service | Protocole | Config YAML |
|---|---|---|
| Modbus TCP | Port configurable (défaut 5502) | `modbus:` |
| Redis | TCP/TLS (`redis://` ou `rediss://`) | `action_universal.driver.type: redis` |
| Kafka | (intégration partielle) | action driver |
| Python | PyO3, Python 3.10+, GIL géré | `node_type: python`, feature `python-driver` |
| Webhooks | HTTPS POST JSON | `action_universal.driver.type: https_callback` |

---

## Configuration YAML

Fichier : `config.yaml` (ou `--config` CLI)  
Schéma JSON : `rust/src/resources/config.schema.json`  
Commande : `cargo run -- --show-config-schema`

### Sections principales

```yaml
visualization:    # Serveur HTTP/HTTPS, port, TLS, JWT secrets, OAuth2 clients
acquisition:      # Activation, intervalle d'acquisition
photoacoustic:    # Source audio (device/fichier/simulé), fréquence, FFT, simulation physique
processing:       # Graphe de nœuds (nodes + connections), buffer, performance
modbus:           # Activation, port, adresse
thermal_regulation: # Régulateurs PID, bus I2C, drivers
access:           # Utilisateurs, clients OAuth2, durée JWT
generix:          # Provider OIDC (authority, client_id, scope, redirect_uri...)
```

### TLS

- Dev : auto-généré au démarrage
- Prod : certificat base64 dans les champs `cert`/`key` du YAML
- `hmac_secret` : clé HS256 pour JWT
- `rs256_private_key` / `rs256_public_key` : clés RS256 en base64

---

## Fonctionnalités build

```toml
[features]
default = ["python-driver"]
python-driver = ["pyo3", "pythonize"]   # Nœud Python PyO3
static = ["pyo3"]                        # Build statique musl
```

---

## Docker dans `rust/docker/` 

- `Dockerfile.trixie` : image de base Debian Trixie (13) pour Raspberry Pi OS 64-bit