//! What a response file is filed under: the account a request went out under
//! and the request's method and normalized URL.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

/// Path prefixes whose GETs are never stored or answered: direct messages,
/// usage, and media upload state.
const NEVER_STORED: &[&str] = &["/2/dm_", "/2/usage/", "/2/media/"];

const REQUEST_KEY_PREFIX: &str = "bird-response-v1";
const ACCOUNT_KEY_PREFIX: &str = "bird-account-v1";

/// The length of [`request_key`]'s hex digest.
pub(super) const REQUEST_KEY_LEN: usize = 64;
/// The length of [`account_key`]'s hex digest.
pub(super) const ACCOUNT_KEY_LEN: usize = 16;

/// The account a request went out under: the app and the user, with the user
/// empty for an app-only request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// The app the request was signed with.
    pub app: String,
    /// The user it was sent as; empty for an app-only request.
    pub user: String,
}

impl Account {
    /// The account for a request. A user-context request that names no user
    /// (no requested username and no default user for the app) is `None`: it
    /// is never answered or stored, so it cannot share the app-only key.
    pub fn for_request(
        app: &str,
        app_only: bool,
        requested: Option<&str>,
        default_user: Option<&str>,
    ) -> Option<Account> {
        let user = if app_only {
            ""
        } else {
            requested
                .filter(|u| !u.is_empty())
                .or(default_user.filter(|u| !u.is_empty()))?
        };
        Some(Account {
            app: app.to_string(),
            user: user.to_string(),
        })
    }
}

/// SHA-256 hex over a versioned prefix, the method, and the normalized URL.
pub fn request_key(method: &str, url: &str) -> String {
    digest_hex(&[REQUEST_KEY_PREFIX, method, &normalize_url(url)])
}

/// The first 16 hex digits of SHA-256 over a versioned prefix, the app, and
/// the user.
pub fn account_key(account: &Account) -> String {
    let mut key = digest_hex(&[ACCOUNT_KEY_PREFIX, &account.app, &account.user]);
    key.truncate(ACCOUNT_KEY_LEN);
    key
}

/// Whether a request may be answered from, and stored as, a response file:
/// a GET outside the never-stored families.
pub fn is_answerable(method: &str, url: &str) -> bool {
    if !method.eq_ignore_ascii_case("GET") {
        return false;
    }
    match url::Url::parse(url) {
        Ok(parsed) => !NEVER_STORED.iter().any(|p| parsed.path().starts_with(p)),
        Err(_) => false,
    }
}

/// The URL a key and a stored file are compared by: scheme, host, and path as
/// parsed, query pairs stably sorted by key, every key and value
/// percent-encoded again so a decoded `&` or `=` cannot merge pairs, and
/// values left whole, so `ids` / `usernames` lists keep the order X answers
/// them in.
pub(super) fn normalize_url(url: &str) -> String {
    let Ok(parsed) = url::Url::parse(url) else {
        return url.to_string();
    };
    let mut pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = format!(
        "{}://{}{}",
        parsed.scheme(),
        parsed.host_str().unwrap_or(""),
        parsed.path()
    );
    for (i, (key, value)) in pairs.iter().enumerate() {
        out.push(if i == 0 { '?' } else { '&' });
        out.extend(url::form_urlencoded::byte_serialize(key.as_bytes()));
        out.push('=');
        out.extend(url::form_urlencoded::byte_serialize(value.as_bytes()));
    }
    out
}

/// Whether `name` is a response file's name: a request key plus `.json`.
pub fn is_request_file_name(name: &str) -> bool {
    name.strip_suffix(".json")
        .is_some_and(|stem| is_hex(stem, REQUEST_KEY_LEN))
}

/// Whether `name` is an account directory's name.
pub fn is_account_dir_name(name: &str) -> bool {
    is_hex(name, ACCOUNT_KEY_LEN)
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn digest_hex(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0u8]);
    }
    let mut out = String::with_capacity(REQUEST_KEY_LEN);
    for byte in hasher.finalize() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}
