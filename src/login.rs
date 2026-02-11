//! OAuth2 login: PKCE, local callback server, exchange, store by username.

use crate::auth::{
    exchange_code, fetch_me, make_code_challenge, make_code_verifier, save_stored_tokens,
    StoredTokens, TokenResponse,
};
use crate::config::ResolvedConfig;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for part in query.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            let v = percent_encoding::percent_decode_str(v).decode_utf8_lossy();
            out.insert(k.to_string(), v.to_string());
        }
    }
    out
}

/// Run login flow: start server, open browser, wait for callback, exchange, fetch me, save.
pub async fn run_login(
    client: &reqwest::Client,
    config: ResolvedConfig,
    use_color: bool,
    use_hyperlinks: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client_id = config
        .client_id
        .as_ref()
        .ok_or("client_id required for login (override with X_API_CLIENT_ID if needed)")?;

    let code_verifier = make_code_verifier();
    let code_challenge = make_code_challenge(&code_verifier);
    let state_bytes: Vec<u8> = (0..24).map(|_| rand::random::<u8>()).collect();
    let state_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        &state_bytes[..],
    );

    let authorize_url = crate::auth::build_authorize_url(
        client_id,
        &config.redirect_uri,
        &code_challenge,
        &state_b64,
    );

    let (tx, rx) = oneshot::channel::<Result<(String, String), String>>();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));
    let expected_state = state_b64.clone();

    let addr = SocketAddr::from(([127, 0, 0, 1], 8765));
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("Callback server listening on {}", config.redirect_uri);

    if webbrowser::open(&authorize_url).is_err() {
        let url_display = crate::output::hyperlink(&authorize_url, None, use_hyperlinks);
        let label = crate::output::muted("Open this URL in your browser:", use_color);
        eprintln!("{}\n{}", label, url_display);
    }

    let tx_clone = Arc::clone(&tx);
    let expected_state_clone = expected_state.clone();
    let server = async move {
        if let Ok((stream, _)) = listener.accept().await {
            let mut reader = BufReader::new(stream);
            let mut first_line = String::new();
            let _ = reader.read_line(&mut first_line).await;
            let body: String = if let Some(path_query) = first_line.trim().strip_prefix("GET ") {
                let path_query = path_query.split_whitespace().next().unwrap_or("");
                let (path, query) = path_query.split_once('?').unwrap_or((path_query, ""));
                let params = parse_query(query);
                let state = params.get("state").cloned().unwrap_or_default();
                let code = params.get("code").cloned();
                if path == "/callback" && state == *expected_state_clone {
                    match &code {
                        Some(c) => {
                            let _ = tx_clone
                                .lock()
                                .ok()
                                .and_then(|mut g| g.take())
                                .map(|t| t.send(Ok((c.clone(), state))));
                            "<html><body>Authorized. You can close this tab.</body></html>"
                                .to_string()
                        }
                        None => {
                            let err = params
                                .get("error")
                                .cloned()
                                .unwrap_or_else(|| "unknown".into());
                            let _ = tx_clone
                                .lock()
                                .ok()
                                .and_then(|mut g| g.take())
                                .map(|t| t.send(Err(format!("error={}", err))));
                            "<html><body>Authorization failed. Check the terminal for details.</body></html>".to_string()
                        }
                    }
                } else {
                    let _ = tx_clone
                        .lock()
                        .ok()
                        .and_then(|mut g| g.take())
                        .map(|t| t.send(Err("state mismatch".into())));
                    "<html><body>State mismatch. Please try again.</body></html>".to_string()
                }
            } else {
                "<html><body>Bad request.</body></html>".to_string()
            };
            let mut line = String::new();
            while reader.read_line(&mut line).await.is_ok() && !line.trim().is_empty() {
                line.clear();
            }
            let mut stream = reader.into_inner();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    };

    tokio::spawn(server);
    let (code, _) = tokio::time::timeout(std::time::Duration::from_secs(120), rx)
        .await
        .map_err(|_| "login timed out after 120s — no callback received (is a browser available?)")?
        .map_err(|_| "callback channel closed")??;

    let token: TokenResponse = exchange_code(
        client,
        client_id,
        config.client_secret.as_deref(),
        &config.redirect_uri,
        &code,
        &code_verifier,
    )
    .await?;

    let username = fetch_me(client, &token.access_token).await?;

    config.ensure_config_dir()?;
    let mut stored =
        crate::auth::load_stored_tokens(&config.tokens_path).unwrap_or_else(StoredTokens::new);
    let expires_at = StoredTokens::expires_at(token.expires_in);
    stored.add_account(
        username.clone(),
        crate::auth::OAuth2Account {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_at_secs: expires_at,
        },
    );
    save_stored_tokens(&config.tokens_path, &stored)?;

    println!(
        "{}",
        crate::output::success(&format!("Logged in as @{}", username), use_color)
    );
    Ok(())
}
