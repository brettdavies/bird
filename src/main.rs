//! bird — X API CLI. Subcommands: login, me; raw get/post/put/delete.

mod bookmarks;
mod cli;
mod config;
mod cost;
mod db;
mod doctor;
mod error;
mod fields;
mod login;
mod output;
mod profile;
mod raw;
mod requirements;
mod schema;
mod schema_print;
mod search;
mod skill_install;
mod thread;
mod transport;
mod usage;
mod watchlist;

use clap::Parser;
use cli::{CacheAction, Cli, Command, SkillAction, WatchlistCommand};
use config::{ArgOverrides, ResolvedConfig};
use error::BirdError;
use output::{OutputConfig, OutputFormat};
use std::collections::HashMap;
use std::io::IsTerminal;
use std::process::ExitCode;

fn parse_param_vec(param: &[String]) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for p in param {
        if let Some((k, v)) = p.split_once('=') {
            m.insert(k.to_string(), v.to_string());
        }
    }
    m
}

/// Returns true if this command will spawn xurl during normal execution.
/// Skips the eager xurl presence check for:
///   - Local-only commands (Cache, Watchlist Add/Remove/List)
///   - `--dry-run` invocations (we print the would-be call and exit)
///   - Destructive commands without `--force`/`--yes` in non-TTY context —
///     `require_confirmation` will refuse first with a usage error, no xurl needed
fn command_needs_xurl(cmd: &Command, stdin_is_tty: bool, no_interactive: bool) -> bool {
    use cli::{CacheAction, WatchlistCommand, WriteGuard};
    // For write-guarded commands: if guard would refuse (no force/yes, no dry-run,
    // and the caller can't confirm interactively), xurl is never invoked.
    let guard_proceeds = |g: &WriteGuard| -> bool {
        if g.dry_run {
            return false;
        }
        if g.force {
            return true;
        }
        stdin_is_tty && !no_interactive
    };
    match cmd {
        // Local-only: never call xurl.
        Command::Cache { action } => match action {
            CacheAction::Clear { guard } => guard_proceeds(guard),
            CacheAction::Stats { .. } => false,
        },
        Command::Watchlist { action, .. } => matches!(action, WatchlistCommand::Fetch),
        // Pre-dispatched in main(); never reach run().
        Command::Login { .. }
        | Command::Completions { .. }
        | Command::Schema { .. }
        | Command::Skill { .. } => false,
        // Diagnostic: doctor probes xurl itself but should not gate on absence.
        Command::Doctor { .. } => false,
        // Write commands — gate fires only when the command will actually run.
        Command::Delete { guard, .. }
        | Command::Post { guard, .. }
        | Command::Put { guard, .. }
        | Command::Tweet { guard, .. }
        | Command::Reply { guard, .. }
        | Command::Like { guard, .. }
        | Command::Unlike { guard, .. }
        | Command::Repost { guard, .. }
        | Command::Unrepost { guard, .. }
        | Command::Follow { guard, .. }
        | Command::Unfollow { guard, .. }
        | Command::Dm { guard, .. }
        | Command::Block { guard, .. }
        | Command::Unblock { guard, .. }
        | Command::Mute { guard, .. }
        | Command::Unmute { guard, .. } => guard_proceeds(guard),
        // Read-only network commands with no guard.
        Command::Me { .. }
        | Command::Get { .. }
        | Command::Bookmarks { .. }
        | Command::Profile { .. }
        | Command::Search { .. }
        | Command::Thread { .. }
        | Command::Usage { .. } => true,
    }
}

/// Resolve the default auth type for a command name using requirements.rs.
/// Returns the first accepted auth type for the command.
fn default_auth_type(command_name: &str) -> requirements::AuthType {
    requirements::requirements_for_command(command_name)
        .and_then(|r| r.accepted.first().copied())
        .unwrap_or(requirements::AuthType::OAuth2User)
}

/// Outcome of a write-guard check (--dry-run / --force / TTY confirmation).
enum GuardOutcome {
    /// The user supplied `--dry-run`; the would-be request was emitted and the
    /// command should exit without invoking the API.
    DryRun,
    /// Confirmation satisfied (via `--force`/`--yes` or interactive `y`);
    /// the command should proceed.
    Proceed,
}

/// Apply the `--force` / `--yes` / `--dry-run` policy for a mutating command.
///
/// Order of evaluation: `--dry-run` short-circuits (prints the would-be effect
/// and returns `GuardOutcome::DryRun`). Otherwise, when neither `--force` nor
/// `--yes` is set, we either prompt on a TTY or return a `requires-confirmation`
/// usage error.
fn require_confirmation(
    verb: &str,
    method: &str,
    target: &str,
    body: Option<&serde_json::Value>,
    guard: crate::cli::WriteGuard,
    out: &OutputConfig,
    no_interactive: bool,
) -> Result<GuardOutcome, BirdError> {
    if guard.dry_run {
        emit_dry_run(verb, method, target, body, out);
        return Ok(GuardOutcome::DryRun);
    }
    if guard.force {
        return Ok(GuardOutcome::Proceed);
    }
    let stdin_tty = std::io::stdin().is_terminal();
    if !stdin_tty || no_interactive {
        return Err(BirdError::usage(
            "requires-confirmation",
            format!(
                "destructive operation '{}' requires --force, --yes, or interactive confirmation",
                verb
            ),
        ));
    }
    eprint!("About to {} {} {}. Proceed? [y/N] ", verb, method, target);
    use std::io::Write;
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return Err(BirdError::usage(
            "requires-confirmation",
            "failed to read confirmation from stdin".to_string(),
        ));
    }
    let trimmed = line.trim();
    if trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes") {
        Ok(GuardOutcome::Proceed)
    } else {
        Err(BirdError::usage(
            "user-aborted",
            "aborted by user (confirmation declined)".to_string(),
        ))
    }
}

/// Emit the dry-run envelope (JSON) or human line (text) on stdout.
fn emit_dry_run(
    verb: &str,
    method: &str,
    target: &str,
    body: Option<&serde_json::Value>,
    out: &OutputConfig,
) {
    if out.format.is_json() {
        let mut would = serde_json::json!({
            "method": method,
            "url": target,
        });
        if let Some(b) = body {
            would["body"] = b.clone();
        }
        let data = serde_json::json!({"dry_run": true, "would": would, "verb": verb});
        let meta = serde_json::json!({});
        if let Ok(line) = output::success_envelope_string(&data, &meta) {
            crate::out_println!("{}", line);
            return;
        }
    }
    crate::out_println!("Would {}: {} {}", verb, method, target);
    if let Some(b) = body {
        crate::out_println!("Body: {}", b);
    }
    crate::out_println!("(--dry-run; no request sent)");
}

/// Build the effective absolute URL for dry-run preview output.
/// Mirrors the path/query handling in `raw::run_raw` without performing any
/// transport-layer work. Returns `None` when the URL cannot be assembled, in
/// which case callers should fall back to the raw input path.
fn build_dry_run_url(
    path: &str,
    params: &HashMap<String, String>,
    query: &[String],
) -> Option<String> {
    let resolved = schema::resolve_path(path, params).ok()?;
    let mut url = url::Url::parse(&format!("https://api.x.com{}", resolved)).ok()?;
    for q in query {
        if let Some((k, v)) = q.split_once('=') {
            url.query_pairs_mut().append_pair(k, v);
        }
    }
    Some(url.to_string())
}

/// Clamp a list `--limit` value to a per-command ceiling, mirroring U4 plan
/// semantics (default 100, max 1000). Returns the clamped value plus a flag
/// indicating whether the requested limit was reduced.
fn clamp_limit(requested: Option<u32>, default: u32, ceiling: u32) -> (u32, bool) {
    match requested {
        Some(n) if n > ceiling => (ceiling, true),
        Some(0) => (default, false),
        Some(n) => (n, false),
        None => (default, false),
    }
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
    crate::out_println!("{}", serde_json::to_string(&json)?);
    Ok(())
}

/// Guard + dispatch for write commands: reject --cache-only, then run the closure.
fn xurl_write(
    cache_only: bool,
    name: &'static str,
    f: impl FnOnce() -> Result<(), Box<dyn std::error::Error + Send + Sync>>,
) -> Result<(), BirdError> {
    if cache_only {
        return Err(BirdError::general(
            name,
            "write commands require network access; remove --cache-only".into(),
        ));
    }
    f().map_err(|e| BirdError::from_source(name, e))
}

/// List-pagination options resolved from the global `--limit` / `--cursor`
/// flags. Threaded through `run()` so per-subcommand handlers don't need
/// to re-parse the global CLI struct.
#[derive(Clone, Debug, Default)]
struct ListFlags {
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

fn run(
    command: Command,
    config: ResolvedConfig,
    client: &mut db::BirdClient,
    out: &OutputConfig,
    cache_only: bool,
    no_interactive: bool,
    list_flags: ListFlags,
) -> Result<(), BirdError> {
    let use_color = out.use_color;
    let quiet = out.suppress_diag();
    match command {
        Command::Login { headless } => {
            if headless.no_browser {
                login::run_oauth2_authenticate_headless(out, config.username.as_deref())
                    .map_err(|e| BirdError::from_source("login", e))?;
            } else {
                // Delegate to xurl for OAuth2 authentication (browser-launching flow)
                transport::xurl_passthrough(&["auth", "oauth2"])
                    .map_err(|e| BirdError::from_source("login", e))?;
            }
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
            .map_err(|e| BirdError::from_source("me", e))?;
        }
        Command::Bookmarks { pretty } => {
            let (limit, _) = clamp_limit(list_flags.limit, 100, 1000);
            bookmarks::run_bookmarks(
                client,
                bookmarks::BookmarkOpts {
                    pretty,
                    limit,
                    cursor: list_flags.cursor.as_deref(),
                },
                use_color,
                quiet,
            )
            .map_err(|e| BirdError::from_source("bookmarks", e))?;
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
            .map_err(|e| BirdError::from_source("profile", e))?;
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
            // `--limit` is the canonical agent-facing flag; `--max-results` is kept
            // as the per-page Twitter-API knob. When both are set, `--limit` wins.
            let resolved_max = list_flags.limit.or(max_results);
            let opts = search::SearchOpts {
                query: &query,
                pretty,
                sort: &sort,
                min_likes,
                max_results: resolved_max.unwrap_or(100).clamp(10, 100),
                pages: pages.unwrap_or(1).clamp(1, 10),
                cursor: list_flags.cursor.as_deref(),
            };
            search::run_search(client, opts, use_color, quiet, &auth_type)
                .map_err(|e| BirdError::from_source("search", e))?;
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
            .map_err(|e| BirdError::from_source("thread", e))?;
        }
        Command::Get {
            path,
            param,
            query,
            pretty,
        } => {
            let params = parse_param_vec(&param);
            let mut query = query;
            if let Some(ref tok) = list_flags.cursor {
                query.push(format!("pagination_token={}", tok));
            }
            if let Some(n) = list_flags.limit {
                let (clamped, _) = clamp_limit(Some(n), 100, 1000);
                query.push(format!("max_results={}", clamped));
            }
            let auth_type = default_auth_type("get");
            raw::run_raw(
                client, "GET", &path, &params, &query, None, pretty, use_color, quiet, &auth_type,
            )
            .map_err(|e| BirdError::from_source("get", e))?;
        }
        Command::Post {
            path,
            param,
            query,
            body,
            pretty,
            guard,
        } => {
            let params = parse_param_vec(&param);
            let target = build_dry_run_url(&path, &params, &query)
                .unwrap_or_else(|| format!("https://api.x.com{}", path));
            let body_json = body.as_deref().and_then(|s| serde_json::from_str(s).ok());
            match require_confirmation(
                "POST",
                "POST",
                &target,
                body_json.as_ref(),
                guard,
                out,
                no_interactive,
            )? {
                GuardOutcome::DryRun => return Ok(()),
                GuardOutcome::Proceed => {}
            }
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
            .map_err(|e| BirdError::from_source("post", e))?;
        }
        Command::Put {
            path,
            param,
            query,
            body,
            pretty,
            guard,
        } => {
            let params = parse_param_vec(&param);
            let target = build_dry_run_url(&path, &params, &query)
                .unwrap_or_else(|| format!("https://api.x.com{}", path));
            let body_json = body.as_deref().and_then(|s| serde_json::from_str(s).ok());
            match require_confirmation(
                "PUT",
                "PUT",
                &target,
                body_json.as_ref(),
                guard,
                out,
                no_interactive,
            )? {
                GuardOutcome::DryRun => return Ok(()),
                GuardOutcome::Proceed => {}
            }
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
            .map_err(|e| BirdError::from_source("put", e))?;
        }
        Command::Delete {
            path,
            param,
            query,
            pretty,
            guard,
        } => {
            let params = parse_param_vec(&param);
            let target = build_dry_run_url(&path, &params, &query)
                .unwrap_or_else(|| format!("https://api.x.com{}", path));
            match require_confirmation(
                "delete",
                "DELETE",
                &target,
                None,
                guard,
                out,
                no_interactive,
            )? {
                GuardOutcome::DryRun => return Ok(()),
                GuardOutcome::Proceed => {}
            }
            let auth_type = default_auth_type("delete");
            raw::run_raw(
                client, "DELETE", &path, &params, &query, None, pretty, use_color, quiet,
                &auth_type,
            )
            .map_err(|e| BirdError::from_source("delete", e))?;
        }
        Command::Watchlist { action, pretty } => match action {
            WatchlistCommand::Fetch => {
                let (limit, _) = clamp_limit(list_flags.limit, 100, 1000);
                let auth_type = default_auth_type("watchlist_check");
                watchlist::run_watchlist_check(
                    client,
                    &config,
                    watchlist::CheckOpts {
                        pretty,
                        limit,
                        cursor: list_flags.cursor.as_deref(),
                    },
                    use_color,
                    quiet,
                    &auth_type,
                )
                .map_err(|e| BirdError::from_source("watchlist", e))?;
            }
            WatchlistCommand::Add { username } => {
                watchlist::run_watchlist_add(&config, &username, quiet)
                    .map_err(BirdError::config)?;
            }
            WatchlistCommand::Remove { username, guard } => {
                let target = format!("watchlist:@{}", username);
                match require_confirmation(
                    "remove",
                    "LOCAL",
                    &target,
                    None,
                    guard,
                    out,
                    no_interactive,
                )? {
                    GuardOutcome::DryRun => return Ok(()),
                    GuardOutcome::Proceed => {}
                }
                watchlist::run_watchlist_remove(&config, &username, quiet)
                    .map_err(BirdError::config)?;
            }
            WatchlistCommand::List => {
                let (limit, _) = clamp_limit(list_flags.limit, 1000, 10_000);
                watchlist::run_watchlist_list(
                    &config,
                    pretty,
                    quiet,
                    Some(limit),
                    list_flags.cursor.as_deref(),
                )
                .map_err(|e| BirdError::from_source("watchlist", e))?;
            }
        },
        Command::Usage {
            since,
            local,
            pretty,
        } => {
            usage::run_usage(client, since.as_deref(), local, pretty, quiet)
                .map_err(|e| BirdError::from_source("usage", e))?;
        }
        // -- Write commands (xurl passthrough) --
        Command::Tweet {
            text,
            media_id,
            guard,
        } => {
            let body = serde_json::json!({"text": text, "media_id": media_id});
            if matches!(
                require_confirmation(
                    "tweet",
                    "POST",
                    "https://api.x.com/2/tweets",
                    Some(&body),
                    guard,
                    out,
                    no_interactive,
                )?,
                GuardOutcome::DryRun
            ) {
                return Ok(());
            }
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
        Command::Reply {
            tweet_id,
            text,
            guard,
        } => {
            let body = serde_json::json!({"text": text, "reply_to": tweet_id});
            if matches!(
                require_confirmation(
                    "reply",
                    "POST",
                    &format!("https://api.x.com/2/tweets (reply to {})", tweet_id),
                    Some(&body),
                    guard,
                    out,
                    no_interactive,
                )?,
                GuardOutcome::DryRun
            ) {
                return Ok(());
            }
            let username = config.username.as_deref();
            xurl_write(cache_only, "reply", || {
                xurl_write_call(&["reply", &tweet_id, &text], username)
            })?;
        }
        Command::Like { tweet_id, guard } => {
            if matches!(
                require_confirmation(
                    "like",
                    "POST",
                    &format!("https://api.x.com/2/users/me/likes/{}", tweet_id),
                    None,
                    guard,
                    out,
                    no_interactive,
                )?,
                GuardOutcome::DryRun
            ) {
                return Ok(());
            }
            let username = config.username.as_deref();
            xurl_write(cache_only, "like", || {
                xurl_write_call(&["like", &tweet_id], username)
            })?;
        }
        Command::Unlike { tweet_id, guard } => {
            if matches!(
                require_confirmation(
                    "unlike",
                    "DELETE",
                    &format!("https://api.x.com/2/users/me/likes/{}", tweet_id),
                    None,
                    guard,
                    out,
                    no_interactive,
                )?,
                GuardOutcome::DryRun
            ) {
                return Ok(());
            }
            let username = config.username.as_deref();
            xurl_write(cache_only, "unlike", || {
                xurl_write_call(&["unlike", &tweet_id], username)
            })?;
        }
        Command::Repost { tweet_id, guard } => {
            if matches!(
                require_confirmation(
                    "repost",
                    "POST",
                    &format!("https://api.x.com/2/users/me/retweets/{}", tweet_id),
                    None,
                    guard,
                    out,
                    no_interactive,
                )?,
                GuardOutcome::DryRun
            ) {
                return Ok(());
            }
            let username = config.username.as_deref();
            xurl_write(cache_only, "repost", || {
                xurl_write_call(&["repost", &tweet_id], username)
            })?;
        }
        Command::Unrepost { tweet_id, guard } => {
            if matches!(
                require_confirmation(
                    "unrepost",
                    "DELETE",
                    &format!("https://api.x.com/2/users/me/retweets/{}", tweet_id),
                    None,
                    guard,
                    out,
                    no_interactive,
                )?,
                GuardOutcome::DryRun
            ) {
                return Ok(());
            }
            let username = config.username.as_deref();
            xurl_write(cache_only, "unrepost", || {
                xurl_write_call(&["unrepost", &tweet_id], username)
            })?;
        }
        Command::Follow {
            username: target,
            guard,
        } => {
            if matches!(
                require_confirmation(
                    "follow",
                    "POST",
                    &format!(
                        "https://api.x.com/2/users/me/following (target=@{})",
                        target
                    ),
                    None,
                    guard,
                    out,
                    no_interactive,
                )?,
                GuardOutcome::DryRun
            ) {
                return Ok(());
            }
            let username = config.username.as_deref();
            xurl_write(cache_only, "follow", || {
                xurl_write_call(&["follow", &target], username)
            })?;
        }
        Command::Unfollow {
            username: target,
            guard,
        } => {
            if matches!(
                require_confirmation(
                    "unfollow",
                    "DELETE",
                    &format!(
                        "https://api.x.com/2/users/me/following (target=@{})",
                        target
                    ),
                    None,
                    guard,
                    out,
                    no_interactive,
                )?,
                GuardOutcome::DryRun
            ) {
                return Ok(());
            }
            let username = config.username.as_deref();
            xurl_write(cache_only, "unfollow", || {
                xurl_write_call(&["unfollow", &target], username)
            })?;
        }
        Command::Dm {
            username: target,
            text,
            guard,
        } => {
            let body = serde_json::json!({"to": target, "text": text});
            if matches!(
                require_confirmation(
                    "dm",
                    "POST",
                    &format!(
                        "https://api.x.com/2/dm_conversations/with/@{}/messages",
                        target
                    ),
                    Some(&body),
                    guard,
                    out,
                    no_interactive,
                )?,
                GuardOutcome::DryRun
            ) {
                return Ok(());
            }
            let username = config.username.as_deref();
            xurl_write(cache_only, "dm", || {
                xurl_write_call(&["dm", &target, &text], username)
            })?;
        }
        Command::Block {
            username: target,
            guard,
        } => {
            if matches!(
                require_confirmation(
                    "block",
                    "POST",
                    &format!("https://api.x.com/2/users/me/blocking (target=@{})", target),
                    None,
                    guard,
                    out,
                    no_interactive,
                )?,
                GuardOutcome::DryRun
            ) {
                return Ok(());
            }
            let username = config.username.as_deref();
            xurl_write(cache_only, "block", || {
                xurl_write_call(&["block", &target], username)
            })?;
        }
        Command::Unblock {
            username: target,
            guard,
        } => {
            if matches!(
                require_confirmation(
                    "unblock",
                    "DELETE",
                    &format!("https://api.x.com/2/users/me/blocking (target=@{})", target),
                    None,
                    guard,
                    out,
                    no_interactive,
                )?,
                GuardOutcome::DryRun
            ) {
                return Ok(());
            }
            let username = config.username.as_deref();
            xurl_write(cache_only, "unblock", || {
                xurl_write_call(&["unblock", &target], username)
            })?;
        }
        Command::Mute {
            username: target,
            guard,
        } => {
            if matches!(
                require_confirmation(
                    "mute",
                    "POST",
                    &format!("https://api.x.com/2/users/me/muting (target=@{})", target),
                    None,
                    guard,
                    out,
                    no_interactive,
                )?,
                GuardOutcome::DryRun
            ) {
                return Ok(());
            }
            let username = config.username.as_deref();
            xurl_write(cache_only, "mute", || {
                xurl_write_call(&["mute", &target], username)
            })?;
        }
        Command::Unmute {
            username: target,
            guard,
        } => {
            if matches!(
                require_confirmation(
                    "unmute",
                    "DELETE",
                    &format!("https://api.x.com/2/users/me/muting (target=@{})", target),
                    None,
                    guard,
                    out,
                    no_interactive,
                )?,
                GuardOutcome::DryRun
            ) {
                return Ok(());
            }
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
        Command::Skill { .. } => {
            unreachable!("skill is handled before config init in main()")
        }
        Command::Schema { .. } => {
            unreachable!("schema is handled before config init in main()")
        }
        Command::Cache { action } => match action {
            CacheAction::Clear { guard } => {
                let target = client
                    .db_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "<store>".to_string());
                match require_confirmation(
                    "clear",
                    "LOCAL",
                    &target,
                    None,
                    guard,
                    out,
                    no_interactive,
                )? {
                    GuardOutcome::DryRun => return Ok(()),
                    GuardOutcome::Proceed => {}
                }
                match client.db_clear() {
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
                        return Err(BirdError::general(
                            "cache",
                            format!("failed to clear store: {}", e).into(),
                        ));
                    }
                    None => {
                        diag!(quiet, "Store is not available.");
                    }
                }
            }
            CacheAction::Stats { pretty } => match client.db_stats() {
                Some(Ok(stats)) => {
                    let path = client
                        .db_path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let data = serde_json::json!({
                        "path": path,
                        "size_mb": (stats.size_mb() * 10.0).round() / 10.0,
                        "max_size_mb": stats.max_size_mb() as u64,
                        "tweets": stats.tweet_count,
                        "users": stats.user_count,
                        "raw_responses": stats.raw_response_count,
                        "healthy": stats.healthy(),
                    });
                    if pretty {
                        crate::out_println!("Store: {}", path);
                        crate::out_println!(
                            "Size:  {:.1} MB / {:.0} MB limit",
                            stats.size_mb(),
                            stats.max_size_mb()
                        );
                        crate::out_println!("Tweets: {}", stats.tweet_count);
                        crate::out_println!("Users:  {}", stats.user_count);
                        crate::out_println!("Raw:    {}", stats.raw_response_count);
                    } else if out.is_raw_text() {
                        // --raw text: one key=value per line, pipe-safe.
                        crate::out_println!("path={}", path);
                        crate::out_println!("size_mb={:.1}", stats.size_mb());
                        crate::out_println!("max_size_mb={:.0}", stats.max_size_mb());
                        crate::out_println!("tweets={}", stats.tweet_count);
                        crate::out_println!("users={}", stats.user_count);
                        crate::out_println!("raw_responses={}", stats.raw_response_count);
                        crate::out_println!("healthy={}", stats.healthy());
                    } else {
                        let meta = serde_json::json!({});
                        let line = output::success_envelope_string(&data, &meta).map_err(|e| {
                            BirdError::general(
                                "cache",
                                Box::<dyn std::error::Error + Send + Sync>::from(e),
                            )
                        })?;
                        crate::out_println!("{}", line);
                    }
                }
                Some(Err(e)) => {
                    return Err(BirdError::general(
                        "cache",
                        format!("failed to read store stats: {}", e).into(),
                    ));
                }
                None => {
                    let data = serde_json::json!({"healthy": false});
                    let meta = serde_json::json!({"status": "store-unavailable"});
                    if !pretty && !out.is_raw_text() {
                        let line = output::success_envelope_string(&data, &meta).map_err(|e| {
                            BirdError::general(
                                "cache",
                                Box::<dyn std::error::Error + Send + Sync>::from(e),
                            )
                        })?;
                        crate::out_println!("{}", line);
                    } else {
                        diag!(quiet, "Store is not available.");
                    }
                }
            },
        },
    }
    Ok(())
}

/// Pre-scan argv for an EXPLICIT output flag (`--output json`, `--output=json`,
/// `-o json`, `--json`, `--jsonl`). Returns `None` if no explicit flag is set
/// (caller may then consult env vars or auto-detect from TTY).
fn explicit_output_from_argv(argv: &[String]) -> Option<OutputFormat> {
    let mut i = 0;
    while i < argv.len() {
        let a = argv[i].as_str();
        if a == "--json" {
            return Some(OutputFormat::Json);
        }
        if a == "--jsonl" {
            return Some(OutputFormat::Jsonl);
        }
        if (a == "-o" || a == "--output")
            && let Some(v) = argv.get(i + 1)
            && let Some(f) = parse_output_value(v)
        {
            return Some(f);
        }
        if let Some(rest) = a.strip_prefix("--output=")
            && let Some(f) = parse_output_value(rest)
        {
            return Some(f);
        }
        if let Some(rest) = a.strip_prefix("-o=")
            && let Some(f) = parse_output_value(rest)
        {
            return Some(f);
        }
        i += 1;
    }
    None
}

/// Pre-scan argv plus env for the format to use when emitting the envelope on
/// clap parse failures. Falls back to TTY auto-detection.
fn output_from_argv(argv: &[String]) -> OutputFormat {
    if let Some(f) = explicit_output_from_argv(argv) {
        return f;
    }
    if let Ok(env) = std::env::var("BIRD_OUTPUT")
        && let Some(f) = parse_output_value(&env)
    {
        return f;
    }
    if std::io::stderr().is_terminal() {
        OutputFormat::Text
    } else {
        OutputFormat::Json
    }
}

/// Curated top-level examples block — embedded so `--examples` works on every host.
const TOP_LEVEL_EXAMPLES: &str = include_str!("../examples/top-level.txt");

/// Emit the curated top-level examples block and exit zero. JSON mode wraps the
/// parsed example invocations in `{"data": [...], "meta": {...}}`.
fn print_examples(out: &OutputConfig) -> ExitCode {
    if out.format.is_json() {
        let qualified: Vec<String> = TOP_LEVEL_EXAMPLES
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim_start();
                trimmed.strip_prefix("bird ").map(|rest| {
                    // Strip trailing `# comment` so machine consumers see the bare command.
                    let cmd = rest.split('#').next().unwrap_or(rest).trim_end();
                    format!("bird {}", cmd)
                })
            })
            .filter(|s| !s.is_empty())
            .collect();
        let data = serde_json::json!(qualified);
        let meta = serde_json::json!({"count": qualified.len()});
        match output::success_envelope_string(&data, &meta) {
            Ok(line) => crate::out_println!("{}", line),
            Err(_) => crate::out_println!("{}", TOP_LEVEL_EXAMPLES),
        }
    } else {
        crate::out_print!("{}", TOP_LEVEL_EXAMPLES);
    }
    ExitCode::SUCCESS
}

fn parse_output_value(v: &str) -> Option<OutputFormat> {
    match v {
        "json" => Some(OutputFormat::Json),
        "jsonl" => Some(OutputFormat::Jsonl),
        "ndjson" => Some(OutputFormat::Ndjson),
        "text" => Some(OutputFormat::Text),
        _ => None,
    }
}

/// Convert a clap parse error to a `BirdError::Usage` (for non-help cases) or
/// route help/version to stdout directly. Returns `None` when the error was a
/// help/version display (program should exit 0).
fn clap_error_to_bird(err: &clap::Error) -> Option<BirdError> {
    use clap::error::ErrorKind;
    match err.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => None,
        _ => {
            let error_id = match err.kind() {
                ErrorKind::UnknownArgument => "unknown-argument",
                ErrorKind::MissingRequiredArgument => "missing-required-argument",
                ErrorKind::MissingSubcommand => "missing-subcommand",
                ErrorKind::InvalidSubcommand => "invalid-subcommand",
                ErrorKind::InvalidValue => "invalid-value",
                ErrorKind::TooManyValues => "too-many-values",
                ErrorKind::TooFewValues => "too-few-values",
                ErrorKind::ArgumentConflict => "argument-conflict",
                ErrorKind::NoEquals => "missing-equals",
                ErrorKind::ValueValidation => "invalid-value",
                _ => "invalid-arguments",
            };
            Some(BirdError::usage(error_id, err.to_string()))
        }
    }
}

fn main() -> ExitCode {
    // Restore default SIGPIPE handling so piped commands exit cleanly.
    // Without this, Rust masks SIGPIPE and all writes to closed pipes panic.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    // Pre-scan argv so clap parse failures know what envelope to emit.
    let argv: Vec<String> = std::env::args().collect();
    let argv_output = output_from_argv(&argv);
    let explicit_output = explicit_output_from_argv(&argv);

    // `--examples` is a global help-style flag: print the curated examples block
    // and exit zero, even when no subcommand is supplied. This must short-circuit
    // before `Cli::try_parse` so a missing subcommand does not turn it into a
    // usage error.
    if argv.iter().any(|a| a == "--examples") {
        let fmt = argv_output;
        let cfg = OutputConfig {
            format: fmt,
            use_color: output::use_color_auto() && !fmt.is_json(),
            quiet: false,
            raw: false,
        };
        return print_examples(&cfg);
    }

    // Initialize tracing with default level; verbosity is applied after parse below.
    let default_directive = "bird=info"
        .parse::<tracing_subscriber::filter::Directive>()
        .expect("invariant: 'bird=info' is a valid tracing directive");
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(default_directive),
        )
        .with_writer(std::io::stderr)
        .init();

    // try_parse routes clap errors through the JSON-aware envelope formatter.
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => match clap_error_to_bird(&e) {
            None => {
                // Help/version display: only when the user EXPLICITLY requested JSON
                // (via `--json`, `--jsonl`, or `--output {json,jsonl}`) do we wrap the
                // help/version text in a success envelope. Auto-detected pipe mode
                // keeps the plain clap output so naive `bird --help | grep` still works.
                let wrap_in_envelope = explicit_output.is_some_and(|f| f.is_json());
                if wrap_in_envelope {
                    let body = e.to_string();
                    let kind = match e.kind() {
                        clap::error::ErrorKind::DisplayVersion => "version",
                        _ => "help",
                    };
                    let data = serde_json::json!({
                        kind: body.trim(),
                    });
                    let meta = serde_json::json!({"format": "text"});
                    match output::success_envelope_string(&data, &meta) {
                        Ok(line) => crate::out_println!("{}", line),
                        Err(_) => {
                            let _ = e.print();
                        }
                    }
                } else {
                    let _ = e.print();
                }
                return ExitCode::SUCCESS;
            }
            Some(bird_err) => {
                let fmt = argv_output;
                let cfg = OutputConfig {
                    format: fmt,
                    use_color: output::use_color_auto() && !fmt.is_json(),
                    quiet: false,
                    raw: false,
                };
                output::print_error(&bird_err, &cfg);
                return ExitCode::from(bird_err.exit_code());
            }
        },
    };

    let color_mode = cli.effective_color();
    let use_color = output::resolve_color(color_mode);
    let raw = cli.raw;

    // Resolve output format: explicit flag > env var > auto-detect from stderr TTY.
    let output_format = cli.effective_output().unwrap_or_else(|| {
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
        raw,
    };

    // Apply --timeout to the xurl transport layer.
    transport::set_timeout_secs(cli.timeout);

    // --- Meta-commands: need nothing beyond parsed args ---
    if let Command::Completions { shell } = &cli.command {
        use clap::CommandFactory;
        clap_complete::generate(*shell, &mut Cli::command(), "bird", &mut std::io::stdout());
        return ExitCode::SUCCESS;
    }

    if let Command::Skill { action } = &cli.command {
        let (host, all, dry_run) = match *action {
            SkillAction::Install { host, all, dry_run }
            | SkillAction::Update { host, all, dry_run } => (host, all, dry_run),
        };
        return match skill_install::run(host, dry_run, all) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                let err = BirdError::from_source("skill", e);
                output::print_error(&err, &out);
                ExitCode::from(err.exit_code())
            }
        };
    }

    if let Command::Schema { name, list } = &cli.command {
        return match schema_print::run(name.as_deref(), *list, &out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                output::print_error(&err, &out);
                ExitCode::from(err.exit_code())
            }
        };
    }

    // --- Username validation + config + DB init (no xurl needed) ---

    let cli_username = match cli.username {
        Some(ref raw) => match schema::validate_username(raw) {
            Ok(clean) => Some(clean.to_string()),
            Err(e) => {
                let err = BirdError::config(format!("--username: {}", e));
                output::print_error(&err, &out);
                return ExitCode::from(err.exit_code());
            }
        },
        None => None,
    };
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
            let err = BirdError::config(e);
            output::print_error(&err, &out);
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
                let err = BirdError::general("doctor", e);
                output::print_error(&err, &out);
                return ExitCode::from(err.exit_code());
            }
        }
    }

    // --- Local watchlist commands: need config/DB but not xurl ---
    if let Command::Watchlist { ref action, pretty } = cli.command
        && !matches!(action, WatchlistCommand::Fetch)
    {
        let quiet = out.suppress_diag();
        let result = match action {
            WatchlistCommand::Add { username } => {
                watchlist::run_watchlist_add(&config, username, quiet).map_err(BirdError::config)
            }
            WatchlistCommand::Remove { username, guard } => {
                let target = format!("watchlist:@{}", username);
                match require_confirmation(
                    "remove",
                    "LOCAL",
                    &target,
                    None,
                    *guard,
                    &out,
                    cli.no_interactive,
                ) {
                    Ok(GuardOutcome::DryRun) => Ok(()),
                    Ok(GuardOutcome::Proceed) => {
                        watchlist::run_watchlist_remove(&config, username, quiet)
                            .map_err(BirdError::config)
                    }
                    Err(e) => Err(e),
                }
            }
            WatchlistCommand::List => {
                let (limit, _) = clamp_limit(cli.limit, 1000, 10_000);
                watchlist::run_watchlist_list(
                    &config,
                    pretty,
                    quiet,
                    Some(limit),
                    cli.cursor.as_deref(),
                )
                .map_err(|e| BirdError::from_source("watchlist", e))
            }
            WatchlistCommand::Fetch => unreachable!(),
        };
        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                output::print_error(&e, &out);
                ExitCode::from(e.exit_code())
            }
        };
    }

    // --- xurl gate: only for commands that actually spawn xurl ---
    // Skip when:
    //   * The command is local-only (Cache, Watchlist Add/Remove/List)
    //   * --cache-only is set (no network)
    //   * The command's guard is --dry-run (we print the would-be call and exit)
    let stdin_is_tty = std::io::stdin().is_terminal();
    if command_needs_xurl(&cli.command, stdin_is_tty, cli.no_interactive)
        && !cli.cache_only
        && let Err(e) = transport::resolve_xurl_path()
    {
        let err = BirdError::config(e);
        output::print_error(&err, &out);
        return ExitCode::from(err.exit_code());
    }

    let list_flags = ListFlags {
        limit: cli.limit,
        cursor: cli.cursor.clone(),
    };
    match run(
        cli.command,
        config,
        &mut client,
        &out,
        cli.cache_only,
        cli.no_interactive,
        list_flags,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            output::print_error(&e, &out);
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
            BirdError::config("test").exit_code(),
            78,
            "Config errors should exit 78"
        );
        let auth = BirdError::from_source("test", Box::new(transport::XurlError::Auth("x".into())));
        assert_eq!(auth.exit_code(), 77, "Auth errors should exit 77");
        assert_eq!(
            BirdError::general("test", "test".into()).exit_code(),
            1,
            "Command errors should exit 1"
        );
        assert_eq!(
            BirdError::usage("bad", "test").exit_code(),
            2,
            "Usage errors should exit 2"
        );
    }

    #[test]
    fn map_cmd_error_detects_auth() {
        let auth_err: Box<dyn std::error::Error + Send + Sync> =
            Box::new(transport::XurlError::Auth("unauthorized".to_string()));
        let mapped = BirdError::from_source("test", auth_err);
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
        let mapped = BirdError::from_source("profile", api_err);
        assert_eq!(
            mapped.exit_code(),
            1,
            "Non-auth XurlError should map to exit 1"
        );
    }

    #[test]
    fn output_from_argv_detects_json_flag() {
        let argv = vec!["bird".to_string(), "--json".to_string(), "me".to_string()];
        assert_eq!(output_from_argv(&argv), OutputFormat::Json);
    }

    #[test]
    fn output_from_argv_detects_jsonl_flag() {
        let argv = vec![
            "bird".to_string(),
            "bookmarks".to_string(),
            "--jsonl".to_string(),
        ];
        assert_eq!(output_from_argv(&argv), OutputFormat::Jsonl);
    }

    #[test]
    fn output_from_argv_detects_output_separate_value() {
        let argv = vec![
            "bird".to_string(),
            "--output".to_string(),
            "json".to_string(),
            "me".to_string(),
        ];
        assert_eq!(output_from_argv(&argv), OutputFormat::Json);
    }

    #[test]
    fn output_from_argv_detects_output_equals_value() {
        let argv = vec![
            "bird".to_string(),
            "--output=jsonl".to_string(),
            "bookmarks".to_string(),
        ];
        assert_eq!(output_from_argv(&argv), OutputFormat::Jsonl);
    }

    #[test]
    fn output_from_argv_detects_short_o_value() {
        let argv = vec![
            "bird".to_string(),
            "-o".to_string(),
            "json".to_string(),
            "me".to_string(),
        ];
        assert_eq!(output_from_argv(&argv), OutputFormat::Json);
    }
}
