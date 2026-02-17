//! Auth layer: token resolution (args > config file > env > default), OAuth2 login, refresh.

use crate::config::{ResolvedConfig, AUTHORIZE_URL, TOKEN_URL};
use crate::requirements::{self, requirements_for_command, AuthType as ReqAuthType};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const SCOPES: &str = "tweet.read users.read bookmark.read offline.access";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredTokens {
    #[serde(default)]
    pub accounts: HashMap<String, OAuth2Account>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OAuth2Account {
    pub access_token: String,
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at_secs: Option<u64>,
}

impl std::fmt::Debug for OAuth2Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuth2Account")
            .field("access_token", &"[REDACTED]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("expires_at_secs", &self.expires_at_secs)
            .finish()
    }
}

/// Resolve effective OAuth2 access token: args > config > env > stored (by username).
/// Returns (access_token, optionally refresh_token for refresh).
pub fn resolve_oauth2_token(
    config: &ResolvedConfig,
    stored: Option<&StoredTokens>,
) -> Option<(String, Option<String>)> {
    if let Some(t) = &config.access_token {
        let refresh = config.refresh_token.clone();
        return Some((t.clone(), refresh));
    }
    if let Ok(t) = std::env::var("X_API_ACCESS_TOKEN") {
        let refresh = std::env::var("X_API_REFRESH_TOKEN").ok();
        return Some((t, refresh));
    }
    let stored = stored?;
    let username = config
        .username
        .as_deref()
        .or_else(|| stored.accounts.keys().next().map(String::as_str))?;
    let acc = stored.accounts.get(username)?;
    let refresh = acc.refresh_token.clone();
    Some((acc.access_token.clone(), refresh))
}

/// Resolve bearer (app-only) token: args > config > env.
pub fn resolve_bearer_token(config: &ResolvedConfig) -> Option<String> {
    config
        .bearer_token
        .clone()
        .or_else(|| std::env::var("X_API_BEARER_TOKEN").ok())
}

/// PKCE code_verifier (43-128 chars, unreserved).
pub fn make_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

/// PKCE S256 code_challenge = base64url(SHA256(verifier)).
pub fn make_code_challenge(verifier: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(verifier.as_bytes());
    base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        &digest[..],
    )
}

/// Build authorize URL for OAuth2 PKCE.
pub fn build_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    code_challenge: &str,
    state: &str,
) -> String {
    let params = [
        ("response_type", "code"),
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("scope", SCOPES),
        ("state", state),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
    ];
    let q: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{}?{}", AUTHORIZE_URL, q)
}

fn url_encode(s: &str) -> String {
    percent_encoding::percent_encode(s.as_bytes(), percent_encoding::NON_ALPHANUMERIC).to_string()
}

#[derive(Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
}

impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResponse")
            .field("access_token", &"[REDACTED]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

/// Post form body to TOKEN_URL with optional Basic auth (confidential client) or client_id in body (public client).
async fn post_token(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: Option<&str>,
    body: &[(&str, &str)],
    error_label: &str,
) -> Result<TokenResponse, Box<dyn std::error::Error + Send + Sync>> {
    let req = client
        .post(TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded");
    let req = if let Some(secret) = client_secret {
        let creds = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!("{}:{}", client_id, secret),
        );
        req.header("Authorization", format!("Basic {}", creds))
            .body(
                body.iter()
                    .filter(|(k, _)| *k != "client_id")
                    .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
                    .collect::<Vec<_>>()
                    .join("&"),
            )
    } else {
        req.body(
            body.iter()
                .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
                .collect::<Vec<_>>()
                .join("&"),
        )
    };
    let res = req.send().await?;
    let status = res.status();
    let text = res.text().await?;
    if !status.is_success() {
        return Err(format!("{} failed {}: {}", error_label, status, text).into());
    }
    let token: TokenResponse = serde_json::from_str(&text)?;
    Ok(token)
}

/// Exchange authorization code for tokens.
pub async fn exchange_code(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: Option<&str>,
    redirect_uri: &str,
    code: &str,
    code_verifier: &str,
) -> Result<TokenResponse, Box<dyn std::error::Error + Send + Sync>> {
    let body: Vec<(&str, &str)> = vec![
        ("code", code),
        ("grant_type", "authorization_code"),
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("code_verifier", code_verifier),
    ];
    post_token(client, client_id, client_secret, &body, "token exchange").await
}

/// Refresh OAuth2 access token.
pub async fn refresh_access_token(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: Option<&str>,
    refresh_token: &str,
) -> Result<TokenResponse, Box<dyn std::error::Error + Send + Sync>> {
    let body: Vec<(&str, &str)> = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
    ];
    post_token(client, client_id, client_secret, &body, "refresh").await
}

/// Fetch current user (GET /2/users/me) to get username.
pub async fn fetch_me(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let res = client
        .get("https://api.x.com/2/users/me")
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;
    let status = res.status();
    let text = res.text().await?;
    if !status.is_success() {
        return Err(format!("GET /2/users/me failed {}: {}", status, text).into());
    }
    let json: serde_json::Value = serde_json::from_str(&text)?;
    let username = json
        .get("data")
        .and_then(|d| d.get("username"))
        .and_then(|u| u.as_str())
        .ok_or("no data.username in response")?;
    Ok(username.to_string())
}

pub fn load_stored_tokens(path: &Path) -> Option<StoredTokens> {
    let s = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).ok()
}

pub fn save_stored_tokens(path: &Path, tokens: &StoredTokens) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = serde_json::to_string_pretty(tokens)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(s.as_bytes())?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, s)
    }
}

impl StoredTokens {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    pub fn add_account(&mut self, username: String, account: OAuth2Account) {
        self.accounts.insert(username, account);
    }

    pub fn expires_at(expires_in_secs: Option<u64>) -> Option<u64> {
        let secs = expires_in_secs?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
        Some(now + secs)
    }
}

impl Default for StoredTokens {
    fn default() -> Self {
        Self::new()
    }
}

/// Token for authenticating a request: either Bearer (OAuth2 or app-only) or OAuth 1.0a (caller uses reqwest_oauth1).
#[derive(Clone)]
pub enum CommandToken {
    Bearer { token: String, auth_type: ReqAuthType },
    OAuth1,
}

// Custom Debug impl to redact bearer token (every other secret-carrying struct already does this).
impl std::fmt::Debug for CommandToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandToken::Bearer { auth_type, .. } => f
                .debug_struct("Bearer")
                .field("token", &"[REDACTED]")
                .field("auth_type", auth_type)
                .finish(),
            CommandToken::OAuth1 => write!(f, "OAuth1"),
        }
    }
}

/// Resolve a valid token for a command, trying accepted auth types in priority order:
/// OAuth2User (richest user context) → OAuth1 (user context, no expiry) → Bearer (app-only fallback).
/// Skips auth methods that aren't configured to avoid wasted work (e.g., no `ensure_access_token()` disk I/O).
pub async fn resolve_token_for_command(
    client: &reqwest::Client,
    config: &ResolvedConfig,
    command_name: &str,
) -> Result<CommandToken, requirements::AuthRequiredError> {
    let reqs = match requirements_for_command(command_name) {
        Some(r) => r,
        None => return Err(requirements::auth_required_error(command_name)),
    };

    // 1. OAuth2User (preferred — user context, richest data)
    if reqs.accepted.contains(&ReqAuthType::OAuth2User) {
        if has_oauth2_available(config) {
            if let Ok(t) = ensure_access_token(client, config).await {
                return Ok(CommandToken::Bearer {
                    token: t,
                    auth_type: ReqAuthType::OAuth2User,
                });
            }
            // Full auth failed (expired + no refresh) — fall through
        }
    }

    // 2. OAuth1 (user context, no expiry)
    if reqs.accepted.contains(&ReqAuthType::OAuth1) {
        if has_oauth1_available(config) {
            return Ok(CommandToken::OAuth1);
        }
    }

    // 3. Bearer (app-only, fallback)
    if reqs.accepted.contains(&ReqAuthType::Bearer) {
        if let Some(t) = resolve_bearer_token(config) {
            return Ok(CommandToken::Bearer {
                token: t,
                auth_type: ReqAuthType::Bearer,
            });
        }
    }

    Err(requirements::auth_required_error(command_name))
}

/// Quick check: is there any OAuth2 credential source available?
/// Does NOT load tokens.json, does NOT check expiry, does NOT refresh.
pub fn has_oauth2_available(config: &ResolvedConfig) -> bool {
    config.access_token.is_some()
        || std::env::var("X_API_ACCESS_TOKEN").is_ok()
        || config.tokens_path.exists()
}

/// Quick check: are all four OAuth1 credentials available?
pub fn has_oauth1_available(config: &ResolvedConfig) -> bool {
    config.oauth1_consumer_key.is_some()
        && config.oauth1_consumer_secret.is_some()
        && config.oauth1_access_token.is_some()
        && config.oauth1_access_token_secret.is_some()
}

/// Resolve a valid OAuth2 access token, refreshing if we have refresh_token and token is expired.
pub async fn ensure_access_token(
    client: &reqwest::Client,
    config: &ResolvedConfig,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let stored = load_stored_tokens(&config.tokens_path);
    let (access, refresh_opt) = resolve_oauth2_token(config, stored.as_ref())
        .ok_or("no access token (run bird login or set X_API_ACCESS_TOKEN)")?;

    let stored_path = &config.tokens_path;
    let mut tokens = stored.unwrap_or_default();
    let username = config
        .username
        .clone()
        .or_else(|| tokens.accounts.keys().next().cloned());
    let expires_at = username
        .as_ref()
        .and_then(|u| tokens.accounts.get(u))
        .and_then(|a| a.expires_at_secs);
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let expired = expires_at.map(|e| now_secs >= e).unwrap_or(false);

    if let (true, Some(ref refresh_token)) = (expired, &refresh_opt) {
        let client_id = config
            .client_id
            .as_ref()
            .ok_or("client_id required to refresh")?;
        let refreshed = refresh_access_token(
            client,
            client_id,
            config.client_secret.as_deref(),
            refresh_token,
        )
        .await?;
        if let Some(ref u) = username {
            if let Some(acc) = tokens.accounts.get_mut(u) {
                acc.access_token = refreshed.access_token.clone();
                acc.refresh_token = refreshed.refresh_token.or(acc.refresh_token.clone());
                acc.expires_at_secs = StoredTokens::expires_at(refreshed.expires_in);
            }
            save_stored_tokens(stored_path, &tokens)?;
        }
        return Ok(refreshed.access_token);
    }

    Ok(access)
}
