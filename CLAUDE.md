# CLAUDE — Instructions pour Claude Code

## Règles architecturales strictes

- **Pas de logique métier dans `web/`** : calculs signal, seuils, physique → uniquement dans `rust/src/`
- **Pas d'accès hardware dans `visualization/`** : acquisition CPAL, I2C, SPI → uniquement dans `acquisition/` et `thermal_regulation/`
- **Rétrocompatibilité YAML du graphe obligatoire** : tout nouveau paramètre de nœud doit avoir une valeur par défaut ; ne jamais supprimer de clé existante sans migration
- **`cargo test` doit passer** avant tout commit modifiant du code Rust
- **les exemples de code dans la documentation doivent être testés** ils ne peuvent pas utiliser `ignore` , `no_run`ne doit être utilisé que dans des cas très spécifiques (ex : nécessite du hardware) et doit être accompagné d'une explication claire dee la raison dans la docstring
- Le processing graph tourne dans un **thread séparé** du runtime Rocket — ne pas partager d'état mutable sans `Arc<RwLock<_>>`

---

## Commandes de référence

### Rust (depuis `rust/`)

```bash
cargo build
cargo build --release 
cargo test
cargo test --test '*'
cargo tarpaulin --out Html
cargo clippy
cargo fmt
cargo run --bin rust_photoacoustic -- --show-config-schema
cargo run --bin rust_photoacoustic -- --external-web-client https://localhost:5173 # proxy vers Vite dev server en dev avec --base /client (le navigateur doit utiliser https://localhost:8080/client/)
```

### Frontend (depuis `web/`)

```bash
npm install
npm run dev:env
npm test
```

---

## Conventions Rust

- **Async Tokio** partout : ne jamais appeler de code bloquant dans les handlers Rocket  
  → utiliser `tokio::task::spawn_blocking` si un appel bloquant est inévitable
- **Erreurs** :
  - `anyhow::Result` dans les binaires (`main.rs`, handlers)
  - `thiserror` pour les types d'erreur de bibliothèque (modules `processing/`, `acquisition/`, etc.)
- **JSON API** : `serde` avec `#[serde(rename_all = "camelCase")]` sur toutes les structures exposées
- **Nouveau nœud de traitement** :
  1. Créer le fichier dans `processing/nodes/` ou `processing/computing_nodes/`
  2. Implémenter le trait `ProcessingNode` (méthodes `process`, `node_id`, `node_type`, `clone_node`)
  3. Enregistrer le `node_type` dans le parser YAML du graphe (`processing/graph.rs`)
  4. Ajouter des tests unitaires dans le même fichier (`#[cfg(test)]`)
  5. Mettre à jour le JSON Schema config (`resources/config.schema.json`)
- **PyO3** :
  - Acquérir le GIL via `Python::with_gil(|py| { ... })`
  - Initialisation Python unique au démarrage (via `pyo3::prepare_freethreaded_python()`)
  - Feature gate obligatoire : `#[cfg(feature = "python-driver")]`
- **Auth** : utiliser les proc-macros `openapi_protect_get` / `openapi_protect_post` d'`auth-macros/`  
  — ne pas re-implémenter la vérification de scope manuellement
- **Documentation des routes** : docstring au-dessus de chaque handler (Markdown) — elle alimente RapiDoc via `rocket_okapi`

---

## Conventions TypeScript/React

- **Function components + hooks uniquement** — pas de class components
- **React Context** pour tout état global partagé (pas Redux, pas Zustand)
- **HeroUI v3** pour tous les composants UI — ne pas créer de CSS custom sans justification documentée  
  (v3 : `variant="outline"`, `onPress` jamais `onClick`, pas de `radius` prop sur Button)
- **Types stricts** : `strict: true` dans `tsconfig.json`, `any` interdit
- **Hooks** dans `web/src/hooks/`, préfixe `use` obligatoire
- **i18n** : toute string visible par l'utilisateur doit passer par `react-i18next` (`useTranslation`), clés dans les 6 fichiers de `locales/base/`
- **SSE / `useAudioStream`** : gérer la reconnexion automatique avec backoff exponentiel — le serveur peut redémarrer

---

## Sécurité

- Ne **jamais** logger les valeurs de `hmac_secret`, `rs256_private_key`, `key` (TLS), mots de passe
- **OAuth2 PKCE** pour tous les clients publics (SPA) — pas de client_secret côté frontend
- Toutes les routes `/api/*` exigent un JWT Bearer valide avec le scope approprié (`read:api`, `write:api`, `admin:api`)
- Les clés de production (`cert`, `key`, `hmac_secret`) ne sont **jamais** commitées — utiliser des variables d'environnement ou un vault
- Valider côté serveur tous les paramètres de configuration soumis via `POST /api/graph/config` avant application
- Les fichiers WAV d'enregistrement (`record_file`) doivent être dans un répertoire contrôlé — ne pas exposer de chemin arbitraire

---

## Pièges courants

| Piège | Solution |
|---|---|
| Processing graph dans le thread Rocket | Toujours communiquer via `Arc<RwLock<_>>` ou channels Tokio |
| ADS1115 : conflit d'adresse I2C | Adresses 0x48–0x4B selon broche ADDR (GND/VDD/SDA/SCL) |
| SSE : perte de connexion côté React | `useAudioStream` doit implémenter reconnexion auto |
| CORS Rocket en dev | Vérifier `visualization/server/cors.rs` si nouvelles origines |
| Certificats auto-signés | Normaux en dev — `verify_ssl: false` dans les drivers HTTP |
| `cargo build --release` lent | Normal (LTO activé) — utiliser `cargo build` en dev |
| Feature `python-driver` manquante | `libpython3.x-dev` doit être installé sur le système |
| Hot-reload graphe YAML | Le nouveau graphe remplace l'ancien atomiquement — les nœuds `streaming` perdent leur état |
| `frame_size` vs `fft_size` | `frame_size` dans `photoacoustic:` est utilisé comme taille FFT par les computing nodes |
| Modbus désactivé par défaut | `modbus.enabled: false` dans config — activer explicitement si nécessaire |

---

## Processus de contribution

- **Une PR = un objectif fonctionnel** clairement délimité
- **Tests unitaires** obligatoires pour tout nouveau nœud de processing (`#[cfg(test)]` dans le module)
- **Mise à jour du JSON Schema** (`resources/config.schema.json`) si nouveaux paramètres YAML
- **Changelog** (`CHANGELOG.md`) si changement d'API publique (nouvelles routes, changement de contrat JSON)
- **Rétrocompatibilité** : tout champ YAML supprimé nécessite une migration documentée
- Passer `cargo clippy -- -D warnings` et `cargo fmt --check` avant soumission
