# Étude de Refactorisation : Hot-Reload de `AccessConfig`

**Date** : 11 avril 2026  
**Contexte** : Analyse préalable dans `audit_config_dynamique.md` §6.2  
**Objectif** : Permettre le rechargement à chaud de la configuration d'accès utilisateurs/OAuth2 sans redémarrage du serveur web.

---

## 1. Cartographie Complète de l'Existant

### 1.1 Structures de données impliquées

```
Config (Arc<RwLock<Config>>)
└── access: AccessConfig
    ├── users: Vec<User>        { user, pass (bcrypt), permissions, email, name }
    ├── clients: Vec<Client>    { client_id, default_scope, allowed_callbacks }
    ├── duration: Option<i64>   (heures de validité des tokens)
    └── iss: Option<String>     (claim "iss" des JWT)
```

### 1.2 Sites de consommation de `AccessConfig` — état actuel

| Composant | Fichier | Source lue | Comportement actuel |
|---|---|---|---|
| `OxideState.access_config` | `oauth2/state.rs` | Clone au démarrage (dans `from_config`) | **FIGÉ** |
| `OxideState.registrar` (ClientMap) | `oauth2/state.rs` | Construit depuis `access_config.clients` au démarrage | **FIGÉ** |
| `OxideState.issuer` (JwtIssuer) | `oauth2/state.rs` | `iss` et `duration` baked-in au démarrage | **FIGÉ** |
| Login handler `/login` | `oauth2/handlers.rs:198` | `&state.access_config` (OxideState figé) | **FIGÉ** |
| `AuthenticatedUser` guard | `api_auth.rs:185` | `get_config_from_request()` → figment Rocket | **FIGÉ** |
| `AccessConfig` request guard | `config/access.rs:225` | `oxide_state.access_config.clone()` (OxideState figé) | **FIGÉ** |
| `Arc<JwtValidator>` validate() | `jwt/validator.rs:340` | `self.access_config.clients` (construit au démarrage) | **FIGÉ** (audience validation) |
| `Arc<JwtValidator>` get_user_info() | `jwt/validator.rs:400` | Paramètre passé par l'appelant | **DYNAMIQUE si l'appelant passe du live** |
| `OAuthBearer` guard | `auth/guards/bearer.rs:155` | `config.read().await.access.clone()` | ✅ **LIVE** |
| OIDC openid-configuration | `oidc.rs:110-114` | `state.access_config.clone()` (OxideState figé) | **FIGÉ** |

### 1.3 Découverte critique : `JwtValidator.access_config` a deux usages distincts

Le champ `access_config: AccessConfig` stocké dans `JwtValidator` est utilisé dans **deux méthodes avec des rôles différents** :

**Usage 1** — `validate()` : audience JWT (liste des `client_id` valides)
```rust
// jwt/validator.rs — validate()
let audiences: Vec<String> = self
    .access_config       // ← utilise SELF.access_config
    .clients
    .iter()
    .map(|client| client.client_id.clone())
    .collect();
validation.set_audience(&audience_refs);
```

**Usage 2** — `get_user_info()` : lookup de l'utilisateur dans la liste
```rust
// jwt/validator.rs — get_user_info()
pub fn get_user_info(&self, token: &str, access_config: AccessConfig) -> Result<UserSysInfo> {
    let user = access_config    // ← utilise le PARAMÈTRE, pas self.access_config
        .users
        .iter()
        .find(|u| u.user == claims.sub)
        ...
```

**Conséquence** : `self.access_config` dans `Arc<JwtValidator>` contrôle quels `client_id` sont acceptés dans les JWT. C'est figé au démarrage. Toute modification de `access_config.clients` n'est pas prise en compte par la validation d'audience dans ce guard.

### 1.4 Tableau de faisabilité par champ (état actuel précis)

| Champ | Via `OAuthBearer` | Via `AuthenticatedUser` | Via login `/login` | Via OAuth flow |
|---|---|---|---|---|
| `users` — mot de passe | ✅ Live (jointure via paramètre `get_user_info`) | ❌ Figment figé | ❌ OxideState figé | N/A |
| `users` — permissions | ✅ Live (jointure live sur `users`) | ❌ Figment figé | N/A | N/A |
| `clients` — client_id (audience JWT) | ❌ `self.access_config` figé dans JwtValidator per-request | ❌ `Arc<JwtValidator>` figé | N/A | ❌ ClientMap figé |
| `clients` — allowed_callbacks | ❌ N/A | ❌ N/A | N/A | ❌ ClientMap figé |
| `duration` — durée des tokens | N/A | N/A | N/A | ❌ JwtIssuer figé |
| `iss` — issuer | N/A | N/A | N/A | ❌ JwtIssuer figé |

---

## 2. Stratégie de Refactorisation Phasée

### Principe directeur

**Source de vérité unique** : `Arc<tokio::sync::RwLock<Config>>` déjà en place.  
Tous les composants doivent lire (directement ou via un `Arc` partagé) depuis cette source au lieu d'utiliser des copies figées.

**Modèle existant à répliquer** : `OAuthBearer` guard dans `bearer.rs` est la **référence** d'une implémentation correcte.

### 2.1 Phase 1 — Correction immédiate, sans impact architectural (< 1 jour)

Ces changements ne nécessitent aucune restructuration. Ils corrigent les lectures figées du figment et d'`OxideState.access_config` pour les remplacer par des lectures live.

#### 1a. `AuthenticatedUser` guard — `api_auth.rs`

**Problème** : `get_config_from_request()` lit depuis le figment Rocket (figé au démarrage).

```rust
// AVANT — api_auth.rs::AuthenticatedUser::from_request()
let access_config = get_config_from_request(request);
let state = request
    .rocket()
    .state::<Arc<JwtValidator>>()
    .expect("JwtValidator not configured");
let user_info = match state.get_user_info(&token, access_config) {
```

```rust
// APRÈS — lire depuis Arc<RwLock<Config>>
let config_state = request
    .rocket()
    .state::<Arc<RwLock<Config>>>()
    .expect("Config not managed");
let access_config = config_state.read().await.access.clone();
let state = request
    .rocket()
    .state::<Arc<JwtValidator>>()
    .expect("JwtValidator not configured");
let user_info = match state.get_user_info(&token, access_config) {
```

**Impact** : `users` (mots de passe + permissions) rechargés live pour `AuthenticatedUser`. Note : `clients` (audience) reste figé dans `Arc<JwtValidator>` — voir Phase 3.

**Import à ajouter** :
```rust
use tokio::sync::RwLock;
use crate::config::Config;
use std::sync::Arc;
```

#### 1b. `AccessConfig` request guard — `config/access.rs`

**Problème** : lit depuis `oxide_state.access_config` (figé dans OxideState).

```rust
// AVANT
impl<'r> FromRequest<'r> for AccessConfig {
    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match request.guard::<&State<OxideState>>().await {
            Outcome::Success(oxide_state) => Outcome::Success(oxide_state.access_config.clone()),
            ...
        }
    }
}
```

```rust
// APRÈS — lire depuis Arc<RwLock<Config>>
impl<'r> FromRequest<'r> for AccessConfig {
    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match request.guard::<&State<Arc<RwLock<Config>>>>().await {
            Outcome::Success(config) => {
                Outcome::Success(config.read().await.access.clone())
            }
            Outcome::Error((status, _)) => Outcome::Error((status, "Missing config state")),
            Outcome::Forward(status) => Outcome::Forward(status),
        }
    }
}
```

**Import à ajouter/modifier dans `access.rs`** :
```rust
use crate::config::Config;  // déjà présent en tant que module parent
use tokio::sync::RwLock;
use std::sync::Arc;
// Supprimer l'import OxideState si non utilisé ailleurs dans ce fichier
```

### 2.2 Phase 2 — Login handler live (demi-journée)

#### 2a. Handler `login` — `oauth2/handlers.rs`

**Problème** : `&state.access_config` lit depuis l'OxideState figé. Handler actuellement **synchrone (fn login)** — il faut le rendre **async**.

```rust
// AVANT
#[post("/login", data = "<form>")]
pub fn login(
    form: Form<AuthForm>,
    state: &State<OxideState>,
    cookies: &CookieJar<'_>,
) -> Result<OAuthResponse, OAuthFailure> {
    let access_config = &state.access_config;
    if let Some(user) = validate_user(&form.username, &form.password, access_config) {
```

```rust
// APRÈS — handler async, config live
#[post("/login", data = "<form>")]
pub async fn login(
    form: Form<AuthForm>,
    state: &State<OxideState>,
    config: &State<Arc<RwLock<Config>>>,
    cookies: &CookieJar<'_>,
) -> Result<OAuthResponse, OAuthFailure> {
    let access_config = config.read().await.access.clone();
    if let Some(user) = validate_user(&form.username, &form.password, &access_config) {
```

**Import à ajouter dans `handlers.rs`** :
```rust
use crate::config::Config;
use tokio::sync::RwLock;
use std::sync::Arc;
```

**Note** : La signature Rocket détectera automatiquement le nouveau paramètre `config: &State<Arc<RwLock<Config>>>` car `config.clone()` est déjà géré via `.manage(config.clone())` dans `build_rocket()`.

#### 2b. OIDC openid-configuration — `oidc.rs`

**Problème** : `generate_openid_configuration` lit `state.access_config.iss` depuis OxideState figé.

```rust
// AVANT — oidc.rs
let access_config = state.access_config.clone();
issuer: access_config.iss.unwrap_or("LaserSmartServer".to_string()),
```

```rust
// APRÈS — lire depuis Config partagé
// Passer config en paramètre de la fonction openid_configuration
#[get("/.well-known/openid-configuration")]
pub async fn openid_configuration(
    connection: ConnectionInfo<'_>,
    state: &State<OxideState>,
    config: &State<Arc<RwLock<Config>>>,
) -> Json<OpenIdConfiguration> {
    let access_config = config.read().await.access.clone();
    let base_url = format!("{}://{}", connection.scheme, connection.base_url_with_port);
    Json(generate_openid_configuration_from_config(&base_url, state, &access_config))
}
```

### 2.3 Phase 3 — Mise à jour de `OxideState.access_config` (1-2 jours)

Cette phase traite le cœur du problème : `OxideState` est un état Rocket statique (`&State<OxideState>` = référence immutable). Pour que ses parties dynamiques soient mutables, elles doivent utiliser l'interior mutability déjà en place (`Arc<Mutex<>>`) ou être remplacées par `Arc<RwLock<>>`.

#### Analyse de la structure OxideState

```rust
pub struct OxideState {
    registrar: Arc<Mutex<ClientMap>>,              // ← déjà Arc<Mutex>, MUTABLE si on a l'Arc
    authorizer: Arc<Mutex<AuthMap<...>>>,          // ← idem
    pub issuer: Arc<Mutex<JwtIssuer>>,             // ← déjà Arc<Mutex>, MUTABLE
    pub hmac_secret: String,                       // ← ne change pas (clé HMAC = config TLS)
    pub rs256_private_key: String,                 // ← ne change pas  
    pub rs256_public_key: String,                  // ← ne change pas
    pub access_config: AccessConfig,               // ← PROBLÈME : valeur plain, non mutable
    pub generix_config: GenerixConfig,             // ← hors scope
}
```

**`registrar` et `issuer`** sont déjà wrappés dans `Arc<Mutex<>>` → ils peuvent être mutés via `lock().unwrap()` même depuis une référence `&OxideState`. ✅

**`access_config`** est une valeur plain → elle ne peut pas être mise à jour via `&OxideState`. ❌

#### Changement de type : `access_config` devient `Arc<RwLock<AccessConfig>>`

```rust
// oauth2/state.rs — MODIFICATION DE LA STRUCT
pub struct OxideState {
    registrar: Arc<Mutex<ClientMap>>,
    authorizer: Arc<Mutex<AuthMap<RandomGenerator>>>,
    pub issuer: Arc<Mutex<JwtIssuer>>,
    pub hmac_secret: String,
    pub rs256_private_key: String,
    pub rs256_public_key: String,
    pub access_config: Arc<RwLock<AccessConfig>>,  // ← CHANGÉ
    pub generix_config: GenerixConfig,
}
```

**Propagation dans `Clone`** :
```rust
impl Clone for OxideState {
    fn clone(&self) -> Self {
        OxideState {
            registrar: Arc::clone(&self.registrar),
            authorizer: Arc::clone(&self.authorizer),
            issuer: Arc::clone(&self.issuer),
            hmac_secret: self.hmac_secret.clone(),
            rs256_private_key: self.rs256_private_key.clone(),
            rs256_public_key: self.rs256_public_key.clone(),
            access_config: Arc::clone(&self.access_config),  // ← Arc::clone, pas deep clone
            generix_config: self.generix_config.clone(),
        }
    }
}
```

**Propagation dans `from_config()`** :
```rust
OxideState {
    // ...
    access_config: Arc::new(RwLock::new(access_config)),  // ← wrappé
    // ...
}
```

**Propagation dans `preconfigured()`** :
```rust
OxideState {
    // ...
    access_config: Arc::new(RwLock::new(AccessConfig::default())),
    // ...
}
```

**Sites d'utilisation qui doivent être async** :

| Fichier | Utilisation actuelle | Utilisation après |
|---|---|---|
| `handlers.rs:198` | `&state.access_config` | `state.access_config.read().await` |
| `config/access.rs:230` | `oxide_state.access_config.clone()` | `oxide_state.access_config.read().await.clone()` |
| `oidc.rs:111` | `state.access_config.clone()` | `state.access_config.read().await.clone()` |

**Avantage clé** : La Phase 2 (handlers live) est simplifiable. Au lieu de passer `config: &State<Arc<RwLock<Config>>>` aux handlers, ils peuvent directement lire `state.access_config.read().await` — une seule source de vérité dans l'OxideState live.

#### Ajout de méthodes de mise à jour dans OxideState

```rust
impl OxideState {
    /// Met à jour la configuration d'accès en live (users + clients + duration + iss)
    pub async fn update_access_config(&self, new_config: AccessConfig) {
        // 1. Mettre à jour access_config (users, iss, duration)
        *self.access_config.write().await = new_config.clone();

        // 2. Reconstruire le ClientMap (OAuth2 clients)
        self.rebuild_registrar(&new_config);

        // 3. Mettre à jour JwtIssuer (duration + iss)
        self.update_jwt_issuer(&new_config);
    }

    /// Reconstruit la liste des clients OAuth2 autorisés
    fn rebuild_registrar(&self, access_config: &AccessConfig) {
        let mut client_map: Vec<oxide_auth::primitives::prelude::Client> = vec![];
        for client in &access_config.clients {
            let mut oauth_client = oxide_auth::primitives::prelude::Client::public(
                client.client_id.as_str(),
                oxide_auth::primitives::registrar::RegisteredUrl::Semantic(
                    client.allowed_callbacks[0].parse::<url::Url>().unwrap(),
                ),
                client.default_scope.parse::<oxide_auth::primitives::prelude::Scope>().unwrap(),
            );
            for callback in &client.allowed_callbacks[1..] {
                oauth_client = oauth_client.with_additional_redirect_uris(vec![
                    oxide_auth::primitives::registrar::RegisteredUrl::Semantic(
                        callback.parse().unwrap(),
                    ),
                ]);
            }
            client_map.push(oauth_client);
        }
        let new_registrar: oxide_auth::primitives::prelude::ClientMap =
            client_map.into_iter().collect();
        *self.registrar.lock().unwrap() = new_registrar;
    }

    /// Met à jour la durée de validité et l'issuer des JWT émis
    fn update_jwt_issuer(&self, access_config: &AccessConfig) {
        let mut issuer = self.issuer.lock().unwrap();
        issuer.with_issuer(
            access_config
                .iss
                .clone()
                .unwrap_or_else(|| "LaserSmartServer".to_string()),
        );
        let duration_secs = access_config.duration.unwrap_or(86400);
        issuer.valid_for(chrono::Duration::seconds(duration_secs));
    }
}
```

### 2.4 Phase 4 — Mise à jour de `Arc<JwtValidator>` pour les audiences (1 jour)

**Problème spécifique** : Le `Arc<JwtValidator>` en état Rocket utilise `self.access_config.clients` pour valider les `aud` claims des JWT dans `validate()`. Ce champ est figé.

**Deux sous-options** :

#### Option A : Supprimer l'audience validation stricte dans `JwtValidator`

Dans `validate()`, remplacer la validation d'audience stricte par une validation souple ou la désactiver si `access_config.clients` est vide :

```rust
// jwt/validator.rs — validate() — modification
let audiences: Vec<String> = self
    .access_config
    .clients
    .iter()
    .map(|client| client.client_id.clone())
    .collect();

// Si la liste d'audiences est vide ou si le champ expected_audience est configuré,
// utiliser expected_audience en priorité
if let Some(ref expected) = self.expected_audience {
    validation.set_audience(&[expected.as_str()]);
} else if !audiences.is_empty() {
    let audience_refs: Vec<&str> = audiences.iter().map(|s| s.as_str()).collect();
    validation.set_audience(&audience_refs);
} else {
    validation.validate_aud = false;  // Pas de validation d'audience si pas de config
}
```

Cette option est **déjà partiellement applicable** : `init_jwt_validator` configure `expected_audience = "LaserSmartClient"` via `.with_audience("LaserSmartClient")`. Si `expected_audience` est défini, la liste `clients` est ignorée → changement de `clients` n'affecte plus la validation d'audience.

**Impact** : Aucune mise à jour dynamique de `Arc<JwtValidator>` requise.

#### Option B : Reconstruire `Arc<JwtValidator>` lors d'un changement de `clients`

Stocker le `Arc<RwLock<Arc<JwtValidator>>>` dans Rocket state au lieu de `Arc<JwtValidator>` :

```rust
// builder.rs
let jwt_validator = Arc::new(RwLock::new(Arc::new(init_jwt_validator(...))));
rocket.manage(jwt_validator.clone());
```

Les guards accèdent alors à :
```rust
let validator_lock = request.rocket().state::<Arc<RwLock<Arc<JwtValidator>>>>()...;
let validator = validator_lock.read().await.clone();
validator.get_user_info(token, access_config)
```

Et le monitoring met à jour :
```rust
let new_validator = Arc::new(init_jwt_validator(hmac, rs256, new_access_config));
*validator_arc_rwlock.write().await = new_validator;
```

**Recommandation** : Commencer par l'Option A (plus simple, déjà quasiment en place). L'Option B si la liste de clients doit changer dynamiquement en production.

### 2.5 Phase 5 — Câblage du monitoring dans le daemon (demi-journée)

Le daemon possède `launch_daemon.rs::check_and_apply_config_changes()` avec une section `"access"` déjà reconnue mais sans handler effectif :

```rust
// launch_daemon.rs — état actuel
"access" => {
    warn!("Access configuration changes require daemon restart to take effect");
}
```

**Après la Phase 3**, le daemon doit détenir une référence à `OxideState` (ou à l'`Arc<RwLock<AccessConfig>>` partagé). Le plus propre est de partager l'`Arc<RwLock<AccessConfig>>` via un `Arc::clone` :

```rust
// Struct Daemon — AJOUT de champ
pub struct Daemon {
    config: Arc<RwLock<Config>>,
    running: Arc<AtomicBool>,
    // ... autres champs ...
    oxide_state: Option<Arc<OxideState>>,  // référence pour le hot-reload
}
```

> **Problème** : Rocket consomme `OxideState` via `.manage(oxide_state)`.  
> `.manage()` prend ownership → Rocket stocke le `T`, pas un `Arc<T>`.  
> Pour partager un `Arc`, il faut passer `oxide_state.clone()` ET `.manage(oxide_state)`.

**Solution** : Passer `Arc<OxideState>` à Rocket :

```rust
// build_rocket() — appelle .manage(Arc::new(oxide_state))
let oxide_state = Arc::new(OxideState::from_config(&config).await);
let oxide_state_ref = Arc::clone(&oxide_state); // pour le daemon
rocket.manage(oxide_state)  // Rocket gère Arc<OxideState>
```

Les handlers Rocket doivent alors utiliser `&State<Arc<OxideState>>` et déréférencer : `state.as_ref()`.

Ou, plus simplement : **partager seulement l'`Arc<RwLock<AccessConfig>>`** sans mettre `OxideState` dans un `Arc` :

```rust
// Dans build_rocket() — créer l'Arc partagé AVANT OxideState
let live_access_config: Arc<RwLock<AccessConfig>> = Arc::new(RwLock::new(
    config.read().await.access.clone()
));

// OxideState reçoit un clone de cet Arc (pas une copie de AccessConfig)
let oxide_state = OxideState::from_config_with_live_access(&config, Arc::clone(&live_access_config)).await;

// Le daemon reçoit un clone pour le monitoring
daemon.live_access_config = Some(Arc::clone(&live_access_config));

// Rocket gère à la fois OxideState et le live_access_config
rocket.manage(oxide_state)
      .manage(live_access_config);  // accessible aux guards comme Option<TwoStep>
```

**Handler de monitoring** :
```rust
// launch_daemon.rs — check_and_apply_config_changes()
"access" => {
    let new_access_config = self.config.read().await.access.clone();
    if let Some(live_ac) = &self.live_access_config {
        // Met à jour la référence partagée
        *live_ac.write().await = new_access_config.clone();
        info!("AccessConfig hot-reloaded: {} users, {} clients",
              new_access_config.users.len(), new_access_config.clients.len());
    }
    // Si OxideState est partagé via Arc, rebuilder le registrar
    if let Some(oxide) = &self.oxide_state_ref {
        oxide.rebuild_registrar(&new_access_config);
        oxide.update_jwt_issuer(&new_access_config);
    }
}
```

---

## 3. Impact par Fichier — Récapitulatif Complet

### Fichiers à modifier

| Fichier | Phase | Changements |
|---|---|---|
| `src/visualization/auth/oauth2/state.rs` | 3 | `access_config: Arc<RwLock<AccessConfig>>`, +`update_access_config()`, +`rebuild_registrar()`, +`update_jwt_issuer()` |
| `src/visualization/api_auth.rs` | 1a | Remplacer `get_config_from_request()` par lecture `Arc<RwLock<Config>>` |
| `src/config/access.rs` | 1b | `FromRequest` pour `AccessConfig` → lire depuis `Arc<RwLock<Config>>` |
| `src/visualization/auth/oauth2/handlers.rs` | 2a | `login` → `async fn`, +paramètre config ou lecture via `state.access_config.read().await` |
| `src/visualization/oidc.rs` | 2b | `openid_configuration` → lire `iss` depuis `state.access_config.read().await` |
| `src/visualization/server/builder.rs` | 5 | Créer `live_access_config` Arc partagé, passer aux 2 constructions |
| `src/daemon/launch_daemon.rs` | 5 | Ajouter `oxide_state_ref`, handler `"access"` → appel `update_access_config` |
| `src/visualization/auth/jwt/validator.rs` | 4 (option A) | Prioriser `expected_audience` sur `access_config.clients` dans `validate()` |

### Fichiers non impactés

| Fichier | Raison |
|---|---|
| `auth/guards/bearer.rs` | Déjà correct — modèle de référence |
| `auth/jwt/issuer.rs` | Les méthodes `with_issuer`, `valid_for` existent déjà |
| `auth/jwt/token_map.rs` | Pas de dépendance à `AccessConfig` |
| `auth/jwt/mod.rs` | `init_jwt_validator` reste inchangé |
| `auth/oauth2/auth.rs` | `validate_user` est une pure function sans état |
| `auth/oauth2/consent.rs` | Pas de dépendance à `AccessConfig` |
| `auth/oauth2/forms.rs` | Session cookie — indépendant de `AccessConfig` |
| `visualization/introspection.rs` | Utilise `state.issuer` (déjà `Arc<Mutex>`) |

---

## 4. Ordre d'Exécution Recommandé et Tests

### Ordre optimal (du moins risqué au plus risqué)

```
Phase 1a → Phase 1b → Tests unitaires/intégration
    ↓
Phase 2a → Phase 2b → Tests login/OIDC
    ↓
Phase 3 (state.rs + sites d'utilisation) → Tests OAuth complets
    ↓
Phase 4 (validator audiences) → Tests de validation JWT
    ↓
Phase 5 (daemon monitoring) → Tests de hot-reload end-to-end
```

### Tests à écrire

#### Phase 1 — Tests unitaires

```rust
#[cfg(test)]
mod tests {
    // Test : modifier Config dans Arc<RwLock>, vérifier que le guard voit le changement
    #[tokio::test]
    async fn test_authenticated_user_sees_live_users() {
        let config = Arc::new(RwLock::new(Config::default()));
        // Construire un faux request avec le config en state
        // Émettre un token pour user "alice"
        // Supprimer "alice" de config.access.users
        // Vérifier que get_user_info() retourne "User not found"
    }
}
```

#### Phase 3 — Tests de update_access_config

```rust
#[tokio::test]
async fn test_oxide_state_update_access_config() {
    let initial_config = AccessConfig::default();
    let oxide_state = OxideState::from_config(&make_test_config(initial_config)).await;

    let mut new_config = AccessConfig::default();
    new_config.users.push(User { user: "bob".to_string(), ... });
    oxide_state.update_access_config(new_config.clone()).await;

    let current = oxide_state.access_config.read().await;
    assert_eq!(current.users.len(), 2);
}

#[test]
fn test_rebuild_registrar() {
    let mut config = AccessConfig::default();
    config.clients.push(Client { client_id: "new_client".to_string(), ... });
    oxide_state.rebuild_registrar(&config);
    // Vérifier que le nouveau client est reconnu par l'OAuth flow
}
```

#### Phase 5 — Test de hot-reload end-to-end

```rust
// Test d'intégration : cycle complet
// 1. Démarrer le daemon
// 2. Obtenir un token pour "alice"
// 3. Modifier config (supprimer "alice", ajouter "charlie")
// 4. POST /api/graph/config (ou modifier le fichier YAML + signal HUP)
// 5. Attendre le polling (< 2 secondes)
// 6. Vérifier que le token d'alice est rejeté par OAuthBearer
// 7. Émettre un token pour "charlie" → doit fonctionner
```

---

## 5. Risques et Mitigations

| Risque | Probabilité | Impact | Mitigation |
|---|---|---|---|
| Tokens existants invalides après suppression d'un client | Élevée | Moyen | Warning dans les logs + documentation — les tokens restent signés mais l'audience sera rejetée si `clients` change |
| Deadlock sur `RwLock<AccessConfig>` si un handler tient le lock trop longtemps | Faible | Élevé | Toujours cloner immédiatement : `config.read().await.clone()` — ne jamais garder la guard `read()` pendant une opération async |
| `OxideState.registrar` (Mutex) bloqué pendant rebuild | Très faible | Moyen | Le rebuild ne prend que quelques µs ; le Mutex est libéré dès que la nouvelle `ClientMap` est construite |
| Tokens émis avec l'ancien `iss` rejetés après changement | Moyen | Faible | `iss` change rarement ; les tokens pré-existants expirent naturellement (`duration`) |
| `cargo test` échoue après changement de signature `login` (sync → async) | Élevée | Faible | Prévisible — corriger les tests en même temps que le changement via `async fn` |
| Contention sur `RwLock<AccessConfig>` sous forte charge | Très faible | Très faible | Les lectures sont `read()` → non bloquantes entre elles ; `write()` n'arrive que lors d'un reload |

---

## 6. Résumé Exécutif

### Ce qui marche déjà

Le guard `OAuthBearer` (`bearer.rs`) fait déjà correctement le hot-reload des **utilisateurs** (mots de passe + permissions) car il construit un `JwtValidator` per-request depuis `Arc<RwLock<Config>>`. C'est le **modèle** à répliquer.

### Ce qui doit changer

1. **Phase 1** (correction rapide, ~2h) : `AuthenticatedUser` guard et `AccessConfig` request guard → lire depuis `Arc<RwLock<Config>>` au lieu du figment et d'OxideState.

2. **Phase 2** (~2h) : Rendre le handler `login` async et le nourrir depuis `state.access_config.read().await` (post-Phase 3) ou depuis `config` (intermédiaire).

3. **Phase 3** (changement architectural, ~8h) : Passer `OxideState.access_config` à `Arc<RwLock<AccessConfig>>` + ajouter les méthodes de mise à jour (`rebuild_registrar`, `update_jwt_issuer`). C'est le pivot qui rend tous les autres composants mutables.

4. **Phase 4** (~2h) : Corriger la validation d'audience dans `JwtValidator.validate()` pour qu'elle utilise `expected_audience` en priorité sur `access_config.clients` — évitant un second problème figé.

5. **Phase 5** (~4h) : Câbler le monitoring du daemon — ajouter la référence `Arc<OxideState>` ou `Arc<RwLock<AccessConfig>>` dans `Daemon` et implémenter le handler `"access"` dans `check_and_apply_config_changes()`.

### Effort total estimé

| Phase | Effort | Risque | Gain |
|---|---|---|---|
| 1 (guards immédiats) | ~2h | Faible | `users` live via `AuthenticatedUser` |
| 2 (handlers login/OIDC) | ~2h | Faible | Login live |
| 3 (`Arc<RwLock>` OxideState) | ~8h | Moyen | `clients`, `duration`, `iss` live |
| 4 (audiences JwtValidator) | ~2h | Faible | `clients` live pour validation JWT |
| 5 (daemon monitoring) | ~4h | Moyen | Pipeline hot-reload complet |
| **Total** | **~18h** | | **100% des champs AccessConfig hot-reloadables** |

### Priorité recommandée

Commencer par les **Phases 1 + 2** (ROI immédiat sur `users`) qui sont les cas d'usage les plus fréquents (changement de mot de passe, ajout d'utilisateur). Les Phases 3+4+5 (changement de clients OAuth2) sont moins urgentes car la liste de clients change rarement.

---

*Document de travail — à maintenir à jour à mesure que les phases sont implémentées.*
