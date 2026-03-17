//! bird — X API CLI. Subcommands: login, me; raw get/post/put/delete.

mod bookmarks;
mod cli;
mod config;
mod cost;
mod db;
mod doctor;
mod fields;
mod output;
mod profile;
mod raw;
mod requirements;
mod schema;
mod search;
mod thread;
mod transport;
mod usage;
mod watchlist;

use clap::CommandFactory;
use clap::FromArgMatches;
use cli::{CacheAction, Cli, Command, WatchlistCommand};
use config::{ArgOverrides, ResolvedConfig};
use output::{OutputConfig, OutputFormat};
use std::collections::HashMap;
use std::io::IsTerminal;
use std::process::ExitCode;

/// Structured error for the CLI. Each variant maps to a distinct exit code.
enum BirdError {
    /// Configuration error (exit code 78 — EX_CONFIG)
    Config(Box<dyn std::error::Error + Send + Sync>),
    /// Authentication error (exit code 77)
    Auth(Box<dyn std::error::Error + Send + Sync>),
    /// Command execution error — API, network, I/O (exit code 1)
    Command {
        name: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl BirdError {
    fn exit_code(&self) -> u8 {
        match self {
            BirdError::Config(_) => 78,
            BirdError::Auth(_) => 77,
            BirdError::Command { .. } => 1,
        }
    }

    fn print(&self, out: &OutputConfig) {
        if out.format == OutputFormat::Json {
            self.print_json();
        } else {
            self.print_text(out.use_color);
        }
    }

    fn print_text(&self, use_color: bool) {
        match self {
            BirdError::Config(e) => {
                eprintln!("{}{}", output::error("config failed: ", use_color), e);
            }
            BirdError::Auth(e) => {
                eprintln!("{}{}", output::error("auth failed: ", use_color), e);
            }
            BirdError::Command { name, source } => {
                let prefix = format!("{} failed: ", name);
                eprintln!("{}{}", output::error(&prefix, use_color), source);
            }
        }
    }

    fn print_json(&self) {
        let mut json = serde_json::json!({
            "error": output::sanitize_for_stderr(&self.message(), 500),
            "kind": self.kind(),
            "code": self.exit_code(),
        });
        if let BirdError::Command { name, source } = self {
            json["command"] = serde_json::Value::String((*name).to_string());
            if let Some(xurl_err) = source.downcast_ref::<transport::XurlError>()
                && let transport::XurlError::Api { status, .. } = xurl_err
                && *status > 0
            {
                json["status"] = serde_json::json!(status);
            }
        }
        eprintln!("{}", json);
    }

    fn kind(&self) -> &'static str {
        match self {
            BirdError::Config(_) => "config",
            BirdError::Auth(_) => "auth",
            BirdError::Command { .. } => "command",
        }
    }

    fn message(&self) -> String {
        match self {
            BirdError::Config(e) | BirdError::Auth(e) => e.to_string(),
            BirdError::Command { source, .. } => source.to_string(),
        }
    }
}

/// Centralized error mapping: detects XurlError::Auth and maps to BirdError::Auth,
/// otherwise wraps in BirdError::Command. Used by all command dispatch closures.
fn map_cmd_error(name: &'static str, e: Box<dyn std::error::Error + Send + Sync>) -> BirdError {
    if let Some(xurl_err) = e.downcast_ref::<transport::XurlError>()
        && matches!(xurl_err, transport::XurlError::Auth(_))
    {
        return BirdError::Auth(e);
    }
    BirdError::Command { name, source: e }
}

fn use_color_from_cli(plain: bool, no_color: bool) -> bool {
    let stderr_tty = std::io::stderr().is_terminal();
    let no_color_env = std::env::var("NO_COLOR").is_ok();
    let term_dumb = std::env::var("TERM").as_deref() == Ok("dumb");
    let default_on = stderr_tty && !no_color_env && !term_dumb;
    default_on && !plain && !no_color
}

fn parse_param_vec(param: &[String]) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for p in param {
        if let Some((k, v)) = p.split_once('=') {
            m.insert(k.to_string(), v.to_string());
        }
    }
    m
}

/// Resolve the default auth type for a command name using requirements.rs.
/// Returns the first accepted auth type for the command.
fn default_auth_type(command_name: &str) -> requirements::AuthType {
    requirements::requirements_for_command(command_name)
        .and_then(|r| r.accepted.first().copied())
        .unwrap_or(requirements::AuthType::OAuth2User)
}

/// Call xurl for a write command and print the JSON result.
fn xurl_write_call(
    args: &[&str],
    username: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut full_args: Vec<&str> = Vec::new();
    if let Some(u) = username {
        full_args.extend(["-u", u]);
    }
    full_args.extend_from_slice(args);
    let json = transport::xurl_call(&full_args)?;
    println!("{}", serde_json::to_string(&json)?);
    Ok(())
}

/// Guard + dispatch for write commands: reject --cache-only, then run the closure.
fn xurl_write(
    cache_only: bool,
    name: &'static str,
    f: impl FnOnce() -> Result<(), Box<dyn std::error::Error + Send + Sync>>,
) -> Result<(), BirdError> {
    if cache_only {
        return Err(BirdError::Command {
            name,
            source: "write commands require network access; remove --cache-only".into(),
        });
    }
    f().map_err(|e| map_cmd_error(name, e))
}

fn run(
    command: Command,
    config: ResolvedConfig,
    client: &mut db::BirdClient,
    out: &OutputConfig,
    cache_only: bool,
) -> Result<(), BirdError> {
    let use_color = out.use_color;
    let quiet = out.suppress_diag();
    match command {
        Command::Login => {
            // Delegate to xurl for OAuth2 authentication
            transport::xurl_passthrough(&["auth", "oauth2"])
                .map_err(|e| map_cmd_error("login", e))?;
            // Verify login and clear store
            if let Some(Ok(count)) = client.db_clear()
                && count > 0
            {
                diag!(
                    quiet,
                    "[store] Cleared {} stored entries after login.",
                    count
                );
            }
        }
        Command::Me { pretty } => {
            let params = HashMap::new();
            let auth_type = default_auth_type("me");
            raw::run_raw(
                client,
                "GET",
                "/2/users/me",
                &params,
                &[],
                None,
                pretty,
                use_color,
                quiet,
                &auth_type,
            )
            .map_err(|e| map_cmd_error("me", e))?;
        }
        Command::Bookmarks { pretty } => {
            bookmarks::run_bookmarks(client, pretty, use_color, quiet)
                .map_err(|e| map_cmd_error("bookmarks", e))?;
        }
        Command::Profile { username, pretty } => {
            let auth_type = default_auth_type("profile");
            profile::run_profile(
                client,
                profile::ProfileOpts {
                    username: &username,
                    pretty,
                },
                use_color,
                quiet,
                &auth_type,
            )
            .map_err(|e| map_cmd_error("profile", e))?;
        }
        Command::Search {
            query,
            pretty,
            sort,
            min_likes,
            max_results,
            pages,
        } => {
            let auth_type = default_auth_type("search");
            let opts = search::SearchOpts {
                query: &query,
                pretty,
                sort: &sort,
                min_likes,
                max_results: max_results.unwrap_or(100).clamp(10, 100),
                pages: pages.unwrap_or(1).clamp(1, 10),
            };
            search::run_search(client, opts, use_color, quiet, &auth_type)
                .map_err(|e| map_cmd_error("search", e))?;
        }
        Command::Thread {
            tweet_id,
            pretty,
            max_pages,
        } => {
            let auth_type = default_auth_type("thread");
            thread::run_thread(
                client,
                thread::ThreadOpts {
                    tweet_id: &tweet_id,
                    pretty,
                    max_pages,
                },
                use_color,
                quiet,
                &auth_type,
            )
            .map_err(|e| map_cmd_error("thread", e))?;
        }
        Command::Get {
            path,
            param,
            query,
            pretty,
        } => {
            let params = parse_param_vec(&param);
            let auth_type = default_auth_type("get");
            raw::run_raw(
                client, "GET", &path, &params, &query, None, pretty, use_color, quiet, &auth_type,
            )
            .map_err(|e| map_cmd_error("get", e))?;
        }
        Command::Post {
            path,
            param,
            query,
            body,
            pretty,
        } => {
            let params = parse_param_vec(&param);
            let auth_type = default_auth_type("post");
            raw::run_raw(
                client,
                "POST",
                &path,
                &params,
                &query,
                body.as_deref(),
                pretty,
                use_color,
                quiet,
                &auth_type,
            )
            .map_err(|e| map_cmd_error("post", e))?;
        }
        Command::Put {
            path,
            param,
            query,
            body,
            pretty,
        } => {
            let params = parse_param_vec(&param);
            let auth_type = default_auth_type("put");
            raw::run_raw(
                client,
                "PUT",
                &path,
                &params,
                &query,
                body.as_deref(),
                pretty,
                use_color,
                quiet,
                &auth_type,
            )
            .map_err(|e| map_cmd_error("put", e))?;
        }
        Command::Delete {
            path,
            param,
            query,
            pretty,
        } => {
            let params = parse_param_vec(&param);
            let auth_type = default_auth_type("delete");
            raw::run_raw(
                client, "DELETE", &path, &params, &query, None, pretty, use_color, quiet,
                &auth_type,
            )
            .map_err(|e| map_cmd_error("delete", e))?;
        }
        Command::Watchlist { action, pretty } => match action {
            WatchlistCommand::Check => {
                let auth_type = default_auth_type("watchlist_check");
                watchlist::run_watchlist_check(
                    client, &config, pretty, use_color, quiet, &auth_type,
                )
                .map_err(|e| map_cmd_error("watchlist", e))?;
            }
            WatchlistCommand::Add { username } => {
                watchlist::run_watchlist_add(&config, &username, quiet)
                    .map_err(BirdError::Config)?;
            }
            WatchlistCommand::Remove { username } => {
                watchlist::run_watchlist_remove(&config, &username, quiet)
                    .map_err(BirdError::Config)?;
            }
            WatchlistCommand::List => {
                watchlist::run_watchlist_list(&config, pretty, quiet)
                    .map_err(|e| map_cmd_error("watchlist", e))?;
            }
        },
        Command::Usage {
            since,
            sync,
            pretty,
        } => {
            usage::run_usage(client, since.as_deref(), sync, pretty, quiet)
                .map_err(|e| map_cmd_error("usage", e))?;
        }
        // -- Write commands (xurl passthrough) --
        Command::Tweet { text, media_id } => {
            let username = config.username.as_deref();
            xurl_write(cache_only, "tweet", || {
                let mut args = vec!["post", &text];
                let media_owned;
                if let Some(ref id) = media_id {
                    media_owned = id.clone();
                    args.extend(["--media-id", &media_owned]);
                }
                xurl_write_call(&args, username)
            })?;
        }
        Command::Reply { tweet_id, text } => {
            let username = config.username.as_deref();
            xurl_write(cache_only, "reply", || {
                xurl_write_call(&["reply", &tweet_id, &text], username)
            })?;
        }
        Command::Like { tweet_id } => {
            let username = config.username.as_deref();
            xurl_write(cache_only, "like", || {
                xurl_write_call(&["like", &tweet_id], username)
            })?;
        }
        Command::Unlike { tweet_id } => {
            let username = config.username.as_deref();
            xurl_write(cache_only, "unlike", || {
                xurl_write_call(&["unlike", &tweet_id], username)
            })?;
        }
        Command::Repost { tweet_id } => {
            let username = config.username.as_deref();
            xurl_write(cache_only, "repost", || {
                xurl_write_call(&["repost", &tweet_id], username)
            })?;
        }
        Command::Unrepost { tweet_id } => {
            let username = config.username.as_deref();
            xurl_write(cache_only, "unrepost", || {
                xurl_write_call(&["unrepost", &tweet_id], username)
            })?;
        }
        Command::Follow { username: target } => {
            let username = config.username.as_deref();
            xurl_write(cache_only, "follow", || {
                xurl_write_call(&["follow", &target], username)
            })?;
        }
        Command::Unfollow { username: target } => {
            let username = config.username.as_deref();
            xurl_write(cache_only, "unfollow", || {
                xurl_write_call(&["unfollow", &target], username)
            })?;
        }
        Command::Dm {
            username: target,
            text,
        } => {
            let username = config.username.as_deref();
            xurl_write(cache_only, "dm", || {
                xurl_write_call(&["dm", &target, &text], username)
            })?;
        }
        Command::Block { username: target } => {
            let username = config.username.as_deref();
            xurl_write(cache_only, "block", || {
                xurl_write_call(&["block", &target], username)
            })?;
        }
        Command::Unblock { username: target } => {
            let username = config.username.as_deref();
            xurl_write(cache_only, "unblock", || {
                xurl_write_call(&["unblock", &target], username)
            })?;
        }
        Command::Mute { username: target } => {
            let username = config.username.as_deref();
            xurl_write(cache_only, "mute", || {
                xurl_write_call(&["mute", &target], username)
            })?;
        }
        Command::Unmute { username: target } => {
            let username = config.username.as_deref();
            xurl_write(cache_only, "unmute", || {
                xurl_write_call(&["unmute", &target], username)
            })?;
        }
        Command::Doctor { .. } => {
            unreachable!("doctor is handled before the xurl gate in main()")
        }
        Command::Completions { .. } => {
            unreachable!("completions is handled before config init in main()")
        }
        Command::Cache { action } => match action {
            CacheAction::Clear => match client.db_clear() {
                Some(Ok(count)) => {
                    let stats = client.db_stats().and_then(|r| r.ok());
                    let size_str =
                        stats.map_or("0.0".to_string(), |s| format!("{:.1}", s.size_mb()));
                    diag!(
                        quiet,
                        "Cleared {} stored entities ({} MB).",
                        count,
                        size_str
                    );
                }
                Some(Err(e)) => {
                    return Err(BirdError::Command {
                        name: "cache",
                        source: format!("failed to clear store: {}", e).into(),
                    });
                }
                None => {
                    diag!(quiet, "Store is not available.");
                }
            },
            CacheAction::Stats { pretty } => match client.db_stats() {
                Some(Ok(stats)) => {
                    let path = client
                        .db_path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    if pretty {
                        println!("Store: {}", path);
                        println!(
                            "Size:  {:.1} MB / {:.0} MB limit",
                            stats.size_mb(),
                            stats.max_size_mb()
                        );
                        println!("Tweets: {}", stats.tweet_count);
                        println!("Users:  {}", stats.user_count);
                        println!("Raw:    {}", stats.raw_response_count);
                    } else {
                        let json = serde_json::json!({
                            "path": path,
                            "size_mb": (stats.size_mb() * 10.0).round() / 10.0,
                            "max_size_mb": stats.max_size_mb() as u64,
                            "tweets": stats.tweet_count,
                            "users": stats.user_count,
                            "raw_responses": stats.raw_response_count,
                            "healthy": stats.healthy(),
                        });
                        println!(
                            "{}",
                            serde_json::to_string(&json).map_err(|e| BirdError::Command {
                                name: "cache",
                                source: e.into(),
                            })?
                        );
                    }
                }
                Some(Err(e)) => {
                    return Err(BirdError::Command {
                        name: "cache",
                        source: format!("failed to read store stats: {}", e).into(),
                    });
                }
                None => {
                    diag!(quiet, "Store is not available.");
                }
            },
        },
    }
    Ok(())
}

fn main() -> ExitCode {
    // Restore default SIGPIPE handling so piped commands exit cleanly.
    // Without this, Rust masks SIGPIPE and all writes to closed pipes panic.
    // The `libc` crate is already a dependency.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("bird=info".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cmd = Cli::command().color(output::color_choice_for_clap());
    let matches = cmd.get_matches();
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(c) => c,
        Err(e) => {
            e.exit();
        }
    };

    let use_color = use_color_from_cli(cli.plain, cli.no_color);

    // Resolve output format: explicit flag > env var > auto-detect from stderr TTY
    let output_format = cli.output.unwrap_or_else(|| {
        if std::io::stderr().is_terminal() {
            OutputFormat::Text
        } else {
            OutputFormat::Json
        }
    });
    let out = OutputConfig {
        format: output_format,
        use_color,
        quiet: cli.quiet,
    };

    // --- Meta-commands: need nothing beyond parsed args ---
    if let Command::Completions { shell } = &cli.command {
        clap_complete::generate(*shell, &mut Cli::command(), "bird", &mut std::io::stdout());
        return ExitCode::SUCCESS;
    }

    // --- Username validation + config + DB init (no xurl needed) ---

    // Validate --username if provided (strips @, checks charset)
    let cli_username = match cli.username {
        Some(ref raw) => match schema::validate_username(raw) {
            Ok(clean) => Some(clean.to_string()),
            Err(e) => {
                let err = BirdError::Config(format!("--username: {}", e).into());
                err.print(&out);
                return ExitCode::from(err.exit_code());
            }
        },
        None => None,
    };
    // X_API_USERNAME is lowest priority (below config file)
    let env_username =
        std::env::var("X_API_USERNAME")
            .ok()
            .and_then(|u| match schema::validate_username(&u) {
                Ok(s) => Some(s.to_string()),
                Err(e) => {
                    diag!(
                        out.suppress_diag(),
                        "[config] warning: X_API_USERNAME invalid, ignoring: {}",
                        e
                    );
                    None
                }
            });
    let overrides = ArgOverrides {
        username: cli_username,
        env_username,
    };

    let config = match ResolvedConfig::load(overrides) {
        Ok(c) => c,
        Err(e) => {
            let err = BirdError::Config(e);
            err.print(&out);
            return ExitCode::from(err.exit_code());
        }
    };

    let transport = Box::new(transport::XurlTransport);
    let cache_opts = db::CacheOpts {
        no_store: cli.no_cache || !config.cache_enabled,
        refresh: cli.refresh,
        cache_only: cli.cache_only,
    };
    let mut client = db::BirdClient::new(
        transport,
        &config.cache_path,
        cache_opts,
        config.cache_max_size_mb,
        config.username.clone(),
        out.suppress_diag(),
    );

    // --- Diagnostic commands: need config/DB but not xurl ---
    if let Command::Doctor { command, pretty } = &cli.command {
        let scope = command.as_deref();
        let use_emoji = use_color && *pretty;
        match doctor::run_doctor(
            &client,
            *pretty,
            scope,
            use_color,
            use_emoji,
            out.suppress_diag(),
        ) {
            Ok(()) => return ExitCode::SUCCESS,
            Err(e) => {
                let err = BirdError::Command {
                    name: "doctor",
                    source: e,
                };
                err.print(&out);
                return ExitCode::from(err.exit_code());
            }
        }
    }

    // --- Local watchlist commands: need config/DB but not xurl ---
    if let Command::Watchlist { ref action, pretty } = cli.command
        && !matches!(action, WatchlistCommand::Check)
    {
        let quiet = out.suppress_diag();
        let result = match action {
            WatchlistCommand::Add { username } => {
                watchlist::run_watchlist_add(&config, username, quiet).map_err(BirdError::Config)
            }
            WatchlistCommand::Remove { username } => {
                watchlist::run_watchlist_remove(&config, username, quiet).map_err(BirdError::Config)
            }
            WatchlistCommand::List => watchlist::run_watchlist_list(&config, pretty, quiet)
                .map_err(|e| map_cmd_error("watchlist", e)),
            WatchlistCommand::Check => unreachable!(),
        };
        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                e.print(&out);
                ExitCode::from(e.exit_code())
            }
        };
    }

    // --- xurl gate: only for API commands ---
    if let Err(e) = transport::resolve_xurl_path() {
        let err = BirdError::Config(e);
        err.print(&out);
        return ExitCode::from(err.exit_code());
    }

    match run(cli.command, config, &mut client, &out, cli.cache_only) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            e.print(&out);
            ExitCode::from(e.exit_code())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bird_error_exit_codes() {
        assert_eq!(
            BirdError::Config("test".into()).exit_code(),
            78,
            "Config errors should exit 78"
        );
        assert_eq!(
            BirdError::Auth("test".into()).exit_code(),
            77,
            "Auth errors should exit 77"
        );
        assert_eq!(
            BirdError::Command {
                name: "test",
                source: "test".into(),
            }
            .exit_code(),
            1,
            "Command errors should exit 1"
        );
    }

    #[test]
    fn map_cmd_error_detects_auth() {
        let auth_err: Box<dyn std::error::Error + Send + Sync> =
            Box::new(transport::XurlError::Auth("unauthorized".to_string()));
        let mapped = map_cmd_error("test", auth_err);
        assert_eq!(
            mapped.exit_code(),
            77,
            "XurlError::Auth should map to exit 77"
        );
    }

    #[test]
    fn map_cmd_error_preserves_command_for_non_auth() {
        let api_err: Box<dyn std::error::Error + Send + Sync> = Box::new(
            transport::XurlError::Process("connection failed".to_string()),
        );
        let mapped = map_cmd_error("profile", api_err);
        assert_eq!(
            mapped.exit_code(),
            1,
            "Non-auth XurlError should map to exit 1"
        );
    }
}
