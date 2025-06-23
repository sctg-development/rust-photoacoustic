# Redis Viewer - Guide d'utilisation

`redis_viewer` est un outil CLI simple pour visualiser et surveiller les données Redis stockées par le système photoacoustique.

## Installation et compilation

```bash
cargo build --release --bin redis_viewer
```

## Utilisation de base

### 1. Voir toutes les clés Redis

```bash
# Voir toutes les clés qui commencent par "photoacoustic"
./target/release/redis_viewer --connection-string redis://localhost:6379

# Voir toutes les clés (tous les patterns)
./target/release/redis_viewer --pattern "*"

# Avec formatage JSON automatique
./target/release/redis_viewer --pattern "photoacoustic*" --json
```

### 2. Surveillance en temps réel des clés

```bash
# Surveillance continue (rafraîchissement toutes les 2 secondes)
./target/release/redis_viewer --pattern "photoacoustic*" --watch --json
```

### 3. Écouter un canal pub/sub

```bash
# Écouter un canal spécifique
./target/release/redis_viewer --mode subscribe --channel photoacoustic:realtime:sensor_data

# Écouter avec pattern (équivalent)
./target/release/redis_viewer --mode subscribe --pattern photoacoustic:realtime:sensor_data
```

## Exemples pratiques

### Surveiller les données photoacoustiques en temps réel

```bash
# Mode clés avec surveillance continue
./target/release/redis_viewer \
  --connection-string redis://localhost:6379 \
  --pattern "photoacoustic*" \
  --watch \
  --json \
  --limit 50
```

### Écouter les alertes en temps réel

```bash
# Écouter le canal d'alertes
./target/release/redis_viewer \
  --mode subscribe \
  --channel "photoacoustic:alert"
```

### Voir les dernières données stockées

```bash
# Voir les dernières données avec pattern spécifique
./target/release/redis_viewer \
  --pattern "photoacoustic:latest:*" \
  --json \
  --limit 10
```

### Surveiller un serveur Redis distant

```bash
# Connexion à un serveur Redis distant
./target/release/redis_viewer \
  --connection-string redis://your-redis-server:6379 \
  --pattern "photoacoustic*" \
  --json
```

## Options disponibles

| Option | Description | Valeur par défaut |
|--------|-------------|------------------|
| `-c, --connection-string` | URL de connexion Redis | `redis://localhost:6379` |
| `-m, --mode` | Mode d'opération (`keys` ou `subscribe`) | `keys` |
| `-p, --pattern` | Pattern de recherche ou canal | `photoacoustic*` |
| `--channel` | Canal spécifique à écouter (mode subscribe) | - |
| `-l, --limit` | Limite du nombre de résultats | `100` |
| `-w, --watch` | Surveillance en temps réel | `false` |
| `-j, --json` | Formatage JSON automatique | `false` |

## Types de données affichées

### Pour les clés (mode `keys`)

- **Type de clé** : string, list, set, hash
- **TTL** : Temps de vie de la clé
- **Valeur** : Contenu avec formatage JSON si applicable
- **Métadonnées** : Taille des collections, nombre d'éléments

### Pour pub/sub (mode `subscribe`)

- **Numéro du message**
- **Timestamp** de réception
- **Canal** source
- **Payload** avec formatage JSON automatique

## Dépannage

### Connexion refusée

```bash
# Vérifier que Redis est en marche
docker ps | grep redis

# Ou démarrer Redis
docker run -d -p 6379:6379 redis:latest
```

### Aucune clé trouvée

```bash
# Essayer avec un pattern plus large
./target/release/redis_viewer --pattern "*"

# Vérifier la connexion
redis-cli ping
```

### Données illisibles

```bash
# Utiliser le formatage JSON
./target/release/redis_viewer --json

# Ou limiter les résultats
./target/release/redis_viewer --limit 10
```

## Utilisation avec le système photoacoustique

Selon votre configuration YAML, les données sont stockées avec des clés comme :

- `photoacoustic:display:node_id:timestamp` (mode key-value)
- `photoacoustic:latest:node_id` (dernières valeurs)
- Publications sur `photoacoustic:channel` (mode pub/sub)

Exemples spécifiques :

```bash
# Voir les dernières données de concentration
./target/release/redis_viewer --pattern "photoacoustic:latest:*" --json

# Surveiller les données en temps réel
./target/release/redis_viewer --pattern "photoacoustic:display:*" --watch --json

# Écouter les mises à jour pub/sub
./target/release/redis_viewer --mode subscribe --channel "photoacoustic:realtime"
```
