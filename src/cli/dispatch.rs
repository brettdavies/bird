//! Dispatcher helpers shared by the CLI runner.
//!
//! Pulled out of `src/main.rs` so the per-command modules in `cli::commands`
//! and the layered runner entrypoint can call them without forcing every
//! consumer to depend on the binary crate root.

use crate::cli::commands;
use crate::cli::{CacheAction, Command, OutputFlags, WatchlistCommand, WriteGuard};
use crate::config::ResolvedConfig;
use crate::db;
use crate::error::BirdError;
use crate::output::{self, OutputConfig};
use crate::requirements;
use crate::schema;
use std::collections::HashMap;
use std::io::{IsTerminal, Write};

/// Top-level dispatcher: routes a parsed `Command` to its per-command module.
///
/// Pre-dispatched commands (`Completions`, `Skill`, `Schema`, `Doctor`,
/// `Watchlist::Add`/`Remove`/`List`) are handled by the runner before this
/// is called; their arms here are unreachable but kept as `Ok(())` returns
/// for exhaustiveness — the dispatcher never panics on a stray
/// pre-dispatched variant slipping through.
///
/// `stdout` and `stderr` writers are threaded through here so each
/// per-command module can pass them to its handler. The `stderr` writer
/// reaches handlers' diagnostic sites and `cost::display_cost`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    command: Command,
    config: ResolvedConfig,
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    cache_only: bool,
    no_interactive: bool,
    list_flags: ListFlags,
) -> Result<(), BirdError> {
    match command {
        Command::Login { headless } => commands::login::run(
            client,
            out,
            stdout,
            stderr,
            headless,
            config.username.as_deref(),
            config.app.as_deref(),
        ),
        Command::Me {
            common: OutputFlags { pretty },
        } => commands::reads::run_me(client, out, stdout, stderr, pretty),
        Command::Get {
            path,
            param,
            query,
            header,
            common: OutputFlags { pretty },
        } => commands::reads::run_get(
            client,
            out,
            stdout,
            stderr,
            path,
            param,
            query,
            header,
            pretty,
            &list_flags,
        ),
        Command::Bookmarks {
            common: OutputFlags { pretty },
        } => commands::bookmarks::run(client, out, stdout, stderr, pretty, &list_flags),
        Command::Profile {
            username,
            common: OutputFlags { pretty },
        } => commands::profile::run(client, out, stdout, stderr, username, pretty),
        Command::Search {
            query,
            common: OutputFlags { pretty },
            sort,
            min_likes,
            max_results,
            pages,
        } => commands::search::run(
            client,
            out,
            stdout,
            stderr,
            query,
            pretty,
            sort,
            min_likes,
            max_results,
            pages,
            &list_flags,
        ),
        Command::Thread {
            tweet_id,
            common: OutputFlags { pretty },
            max_pages,
        } => commands::thread::run(client, out, stdout, stderr, tweet_id, pretty, max_pages),
        Command::Post {
            path,
            param,
            query,
            body,
            header,
            common: OutputFlags { pretty },
            guard,
        } => commands::raw_write::run_post(
            client,
            out,
            stdout,
            stderr,
            path,
            param,
            query,
            body,
            header,
            pretty,
            guard,
            no_interactive,
        ),
        Command::Put {
            path,
            param,
            query,
            body,
            header,
            common: OutputFlags { pretty },
            guard,
        } => commands::raw_write::run_put(
            client,
            out,
            stdout,
            stderr,
            path,
            param,
            query,
            body,
            header,
            pretty,
            guard,
            no_interactive,
        ),
        Command::Delete {
            path,
            param,
            query,
            header,
            common: OutputFlags { pretty },
            guard,
        } => commands::raw_write::run_delete(
            client,
            out,
            stdout,
            stderr,
            path,
            param,
            query,
            header,
            pretty,
            guard,
            no_interactive,
        ),
        Command::Watchlist {
            action,
            common: OutputFlags { pretty },
        } => match action {
            WatchlistCommand::Fetch => commands::watchlist::run_fetch(
                client,
                out,
                stdout,
                stderr,
                &config,
                pretty,
                &list_flags,
            ),
            // Pre-dispatched in main(); unreachable here.
            WatchlistCommand::Add { .. }
            | WatchlistCommand::Remove { .. }
            | WatchlistCommand::List => Ok(()),
        },
        Command::Usage {
            since,
            local,
            common: OutputFlags { pretty },
        } => commands::usage::run(client, out, stdout, stderr, since, local, pretty),
        Command::Tweet {
            text,
            media_id,
            guard,
        } => commands::writes::run_tweet(
            client,
            out,
            stdout,
            text,
            media_id,
            guard,
            cache_only,
            no_interactive,
            config.username.as_deref(),
        ),
        Command::Reply {
            tweet_id,
            text,
            guard,
        } => commands::writes::run_reply(
            client,
            out,
            stdout,
            tweet_id,
            text,
            guard,
            cache_only,
            no_interactive,
            config.username.as_deref(),
        ),
        Command::Like { tweet_id, guard } => commands::writes::run_like(
            client,
            out,
            stdout,
            tweet_id,
            guard,
            cache_only,
            no_interactive,
            config.username.as_deref(),
        ),
        Command::Unlike { tweet_id, guard } => commands::writes::run_unlike(
            client,
            out,
            stdout,
            tweet_id,
            guard,
            cache_only,
            no_interactive,
            config.username.as_deref(),
        ),
        Command::Repost { tweet_id, guard } => commands::writes::run_repost(
            client,
            out,
            stdout,
            tweet_id,
            guard,
            cache_only,
            no_interactive,
            config.username.as_deref(),
        ),
        Command::Unrepost { tweet_id, guard } => commands::writes::run_unrepost(
            client,
            out,
            stdout,
            tweet_id,
            guard,
            cache_only,
            no_interactive,
            config.username.as_deref(),
        ),
        Command::Follow {
            username: target,
            guard,
        } => commands::writes::run_follow(
            client,
            out,
            stdout,
            target,
            guard,
            cache_only,
            no_interactive,
            config.username.as_deref(),
        ),
        Command::Unfollow {
            username: target,
            guard,
        } => commands::writes::run_unfollow(
            client,
            out,
            stdout,
            target,
            guard,
            cache_only,
            no_interactive,
            config.username.as_deref(),
        ),
        Command::Dm {
            username: target,
            text,
            guard,
        } => commands::writes::run_dm(
            client,
            out,
            stdout,
            target,
            text,
            guard,
            cache_only,
            no_interactive,
            config.username.as_deref(),
        ),
        Command::Block {
            username: target,
            guard,
        } => commands::writes::run_block(
            client,
            out,
            stdout,
            target,
            guard,
            cache_only,
            no_interactive,
            config.username.as_deref(),
        ),
        Command::Unblock {
            username: target,
            guard,
        } => commands::writes::run_unblock(
            client,
            out,
            stdout,
            target,
            guard,
            cache_only,
            no_interactive,
            config.username.as_deref(),
        ),
        Command::Mute {
            username: target,
            guard,
        } => commands::writes::run_mute(
            client,
            out,
            stdout,
            target,
            guard,
            cache_only,
            no_interactive,
            config.username.as_deref(),
        ),
        Command::Unmute {
            username: target,
            guard,
        } => commands::writes::run_unmute(
            client,
            out,
            stdout,
            target,
            guard,
            cache_only,
            no_interactive,
            config.username.as_deref(),
        ),
        Command::Cache { action } => {
            commands::cache::run(client, out, stdout, stderr, action, no_interactive)
        }
        // Pre-dispatched by the runner before `run` is invoked
        // (`Completions`, `Skill`, `Schema`, `Doctor`); arms kept for
        // exhaustiveness so a stray variant never panics.
        Command::Doctor { .. }
        | Command::Completions { .. }
        | Command::Skill { .. }
        | Command::Schema { .. } => Ok(()),
    }
}

/// Parse a `KEY=VALUE` parameter vector into a map.
pub fn parse_param_vec(param: &[String]) -> HashMap<String, String> {
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
pub fn command_needs_xurl(cmd: &Command, stdin_is_tty: bool, no_interactive: bool) -> bool {
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
        // Login spawns xurl directly (`xurl auth oauth2` or headless OAuth2),
        // so it must surface the detailed resolution error from the runner's
        // xurl gate rather than falling through to a generic install-hint
        // message inside the handler.
        Command::Login { .. } => true,
        // Pre-dispatched in main() / runner short-circuits before `run` —
        // never reach the dispatcher's xurl gate.
        Command::Completions { .. } | Command::Schema { .. } | Command::Skill { .. } => false,
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
pub fn default_auth_type(command_name: &str) -> requirements::AuthType {
    requirements::requirements_for_command(command_name)
        .and_then(|r| r.accepted.first().copied())
        .unwrap_or(requirements::AuthType::OAuth2User)
}

/// `true` when the dispatched command is one that ultimately hits the X API
/// (and therefore should receive the `XURL_APP` migration warning). Mirrors
/// `command_needs_xurl` without the guard-resolution gating, since the warn
/// fires before the runner figures out whether a write will actually run.
pub fn command_is_api_hitting(cmd: &Command) -> bool {
    match cmd {
        Command::Cache { .. }
        | Command::Watchlist { .. }
        | Command::Completions { .. }
        | Command::Schema { .. }
        | Command::Skill { .. }
        | Command::Doctor { .. } => false,
        Command::Login { .. }
        | Command::Me { .. }
        | Command::Get { .. }
        | Command::Bookmarks { .. }
        | Command::Profile { .. }
        | Command::Search { .. }
        | Command::Thread { .. }
        | Command::Usage { .. }
        | Command::Delete { .. }
        | Command::Post { .. }
        | Command::Put { .. }
        | Command::Tweet { .. }
        | Command::Reply { .. }
        | Command::Like { .. }
        | Command::Unlike { .. }
        | Command::Repost { .. }
        | Command::Unrepost { .. }
        | Command::Follow { .. }
        | Command::Unfollow { .. }
        | Command::Dm { .. }
        | Command::Block { .. }
        | Command::Unblock { .. }
        | Command::Mute { .. }
        | Command::Unmute { .. } => true,
    }
}

/// Validate the `--auth` flag value against the resolved command. Rejects
/// `none` for commands whose `auth_matrix::supported_auth(method,
/// template)` returns a non-empty scheme list, and rejects unknown values
/// outright. The wire vocabulary mirrors xurl's: `app`, `oauth1`, `oauth2`,
/// `none`.
pub fn validate_auth_value(value: &str, cmd: &Command) -> Result<(), String> {
    let normalized = value.to_ascii_lowercase();
    match normalized.as_str() {
        "app" | "oauth1" | "oauth2" => Ok(()),
        "none" => {
            if command_requires_auth(cmd) {
                Err(
                    "--auth none is not accepted for this command; supported schemes: app, oauth1, oauth2".to_string()
                )
            } else {
                Ok(())
            }
        }
        _ => Err(format!(
            "--auth value must be one of: app, oauth1, oauth2, none (got: {value:?})"
        )),
    }
}

/// `true` when the command's `requirements_for_command` table lists any
/// auth scheme other than `AuthType::None`. PR3's U13 dissolves
/// requirements.rs and shifts this query to `auth_matrix::supported_auth`;
/// the contract surfaced to `validate_auth_value` stays identical.
fn command_requires_auth(cmd: &Command) -> bool {
    let name = match cmd {
        Command::Me { .. } => "me",
        Command::Get { .. } => "get",
        Command::Post { .. } => "post",
        Command::Put { .. } => "put",
        Command::Delete { .. } => "delete",
        Command::Bookmarks { .. } => "bookmarks",
        Command::Profile { .. } => "profile",
        Command::Search { .. } => "search",
        Command::Thread { .. } => "thread",
        Command::Tweet { .. } => "tweet",
        Command::Reply { .. } => "reply",
        Command::Like { .. } => "like",
        Command::Unlike { .. } => "unlike",
        Command::Repost { .. } => "repost",
        Command::Unrepost { .. } => "unrepost",
        Command::Follow { .. } => "follow",
        Command::Unfollow { .. } => "unfollow",
        Command::Dm { .. } => "dm",
        Command::Block { .. } => "block",
        Command::Unblock { .. } => "unblock",
        Command::Mute { .. } => "mute",
        Command::Unmute { .. } => "unmute",
        Command::Usage { .. } => "usage",
        Command::Login { .. }
        | Command::Cache { .. }
        | Command::Watchlist { .. }
        | Command::Completions { .. }
        | Command::Schema { .. }
        | Command::Skill { .. }
        | Command::Doctor { .. } => return false,
    };
    requirements::requirements_for_command(name)
        .map(|reqs| {
            reqs.accepted
                .iter()
                .any(|at| !matches!(at, requirements::AuthType::None))
        })
        .unwrap_or(false)
}

/// Outcome of a write-guard check (--dry-run / --force / TTY confirmation).
pub enum GuardOutcome {
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
///
/// `stdout` receives the dry-run envelope or human "Would …" line when
/// `guard.dry_run` is set. `prompt_writer` receives the confirmation prompt
/// text (binary passes `stderr.lock()`). `answer_reader`, when `Some`,
/// supplies the user's answer (tests pass a canned closure); when `None`,
/// the function falls back to `stdin().read_line`.
#[allow(clippy::too_many_arguments)]
pub fn require_confirmation(
    verb: &str,
    method: &str,
    target: &str,
    body: Option<&serde_json::Value>,
    guard: WriteGuard,
    out: &OutputConfig,
    no_interactive: bool,
    stdout: &mut dyn Write,
    prompt_writer: &mut dyn Write,
    answer_reader: Option<Box<dyn FnOnce() -> std::io::Result<String>>>,
) -> Result<GuardOutcome, BirdError> {
    if guard.dry_run {
        emit_dry_run(verb, method, target, body, out, stdout).map_err(|e| {
            BirdError::general(
                "dry-run",
                Box::<dyn std::error::Error + Send + Sync>::from(e),
            )
        })?;
        return Ok(GuardOutcome::DryRun);
    }
    if guard.force {
        return Ok(GuardOutcome::Proceed);
    }
    // When the caller supplies an `answer_reader`, it knows how to obtain a
    // confirmation answer (canned closure in tests; bespoke prompt loop in
    // future callers). Skip the TTY check in that case so the library function
    // stays callable from non-interactive contexts that still want the prompt
    // semantics. The binary continues to pass `None`, which preserves the
    // historical `stdin().read_line` + `IsTerminal` gating.
    if answer_reader.is_none() {
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
    } else if no_interactive {
        return Err(BirdError::usage(
            "requires-confirmation",
            format!(
                "destructive operation '{}' requires --force, --yes, or interactive confirmation",
                verb
            ),
        ));
    }
    let _ = write!(
        prompt_writer,
        "About to {} {} {}. Proceed? [y/N] ",
        verb, method, target
    );
    let _ = prompt_writer.flush();
    let read_result = match answer_reader {
        Some(f) => f(),
        None => {
            let mut line = String::new();
            std::io::stdin().read_line(&mut line).map(|_| line)
        }
    };
    let line = match read_result {
        Ok(s) => s,
        Err(_) => {
            return Err(BirdError::usage(
                "requires-confirmation",
                "failed to read confirmation from stdin".to_string(),
            ));
        }
    };
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

/// Emit the dry-run envelope (JSON) or human lines (text) onto `stdout`.
pub fn emit_dry_run(
    verb: &str,
    method: &str,
    target: &str,
    body: Option<&serde_json::Value>,
    out: &OutputConfig,
    stdout: &mut dyn Write,
) -> std::io::Result<()> {
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
            return writeln!(stdout, "{}", line);
        }
    }
    writeln!(stdout, "Would {}: {} {}", verb, method, target)?;
    if let Some(b) = body {
        writeln!(stdout, "Body: {}", b)?;
    }
    writeln!(stdout, "(--dry-run; no request sent)")
}

/// Build the effective absolute URL for dry-run preview output.
/// Mirrors the path/query handling in `raw::run_raw` without performing any
/// transport-layer work. Returns `None` when the URL cannot be assembled, in
/// which case callers should fall back to the raw input path.
pub fn build_dry_run_url(
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
pub fn clamp_limit(requested: Option<u32>, default: u32, ceiling: u32) -> (u32, bool) {
    match requested {
        Some(n) if n > ceiling => (ceiling, true),
        Some(0) => (default, false),
        Some(n) => (n, false),
        None => (default, false),
    }
}

/// Call xurl for a write command and print the JSON result to `stdout`.
///
/// Routes through `BirdClient`'s transport so the resolved xurl path and
/// timeout flow through a single chokepoint.
pub fn xurl_write_call(
    client: &crate::db::BirdClient,
    stdout: &mut dyn Write,
    args: &[&str],
    username: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut full_args: Vec<String> = Vec::new();
    if let Some(u) = username {
        full_args.push("-u".into());
        full_args.push(u.into());
    }
    full_args.extend(args.iter().map(|s| (*s).to_string()));
    let json = client.transport_request(&full_args)?;
    writeln!(stdout, "{}", serde_json::to_string(&json)?)?;
    Ok(())
}

/// Guard + dispatch for write commands: reject --cache-only, then run the closure.
pub fn xurl_write(
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
pub struct ListFlags {
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{CacheAction, Command, OutputFlags, WriteGuard};

    fn cfg() -> OutputConfig {
        OutputConfig {
            format: crate::output::OutputFormat::Text,
            use_color: false,
            quiet: true,
            raw: false,
        }
    }

    #[test]
    fn validate_auth_accepts_app_oauth1_oauth2() {
        let me = Command::Me {
            common: OutputFlags::default(),
        };
        assert!(validate_auth_value("app", &me).is_ok());
        assert!(validate_auth_value("oauth1", &me).is_ok());
        assert!(validate_auth_value("oauth2", &me).is_ok());
        // Case-insensitive normalization.
        assert!(validate_auth_value("OAuth2", &me).is_ok());
    }

    #[test]
    fn validate_auth_rejects_unknown_value() {
        let me = Command::Me {
            common: OutputFlags::default(),
        };
        let err = validate_auth_value("bearer", &me).expect_err("bearer is xurl-side legacy");
        assert!(err.contains("app, oauth1, oauth2, none"));
    }

    #[test]
    fn validate_auth_none_rejected_for_command_with_required_auth() {
        let tweet = Command::Tweet {
            text: "hello".into(),
            media_id: None,
            guard: WriteGuard::default(),
        };
        let err = validate_auth_value("none", &tweet).expect_err("tweet requires auth");
        assert!(err.contains("not accepted"));
    }

    #[test]
    fn validate_auth_none_allowed_for_local_only_command() {
        let cache = Command::Cache {
            action: CacheAction::Stats {
                common: OutputFlags::default(),
            },
        };
        assert!(validate_auth_value("none", &cache).is_ok());
    }

    #[test]
    fn command_is_api_hitting_skips_diagnostic_commands() {
        let cache = Command::Cache {
            action: CacheAction::Stats {
                common: OutputFlags::default(),
            },
        };
        assert!(!command_is_api_hitting(&cache));
        let doctor = Command::Doctor {
            command: None,
            common: OutputFlags::default(),
        };
        assert!(!command_is_api_hitting(&doctor));
    }

    #[test]
    fn command_is_api_hitting_true_for_writes_and_reads() {
        let me = Command::Me {
            common: OutputFlags::default(),
        };
        assert!(command_is_api_hitting(&me));
        let tweet = Command::Tweet {
            text: "hi".into(),
            media_id: None,
            guard: WriteGuard::default(),
        };
        assert!(command_is_api_hitting(&tweet));
    }

    // U4.1: command_needs_xurl on Cache::Stats returns false.
    #[test]
    fn command_needs_xurl_cache_stats_returns_false() {
        let cmd = Command::Cache {
            action: CacheAction::Stats {
                common: OutputFlags::default(),
            },
        };
        assert!(!command_needs_xurl(&cmd, true, false));
    }

    // U4.2: command_needs_xurl on Tweet returns false for --dry-run, true otherwise.
    #[test]
    fn command_needs_xurl_tweet_respects_dry_run() {
        let dry = Command::Tweet {
            text: "hi".to_string(),
            media_id: None,
            guard: WriteGuard {
                force: false,
                dry_run: true,
            },
        };
        assert!(!command_needs_xurl(&dry, true, false));

        let go = Command::Tweet {
            text: "hi".to_string(),
            media_id: None,
            guard: WriteGuard {
                force: true,
                dry_run: false,
            },
        };
        assert!(command_needs_xurl(&go, true, false));
    }

    // U4.3a: require_confirmation with "yes" closure returns Proceed and writes the prompt.
    #[test]
    fn require_confirmation_yes_returns_proceed() {
        let out = cfg();
        let guard = WriteGuard {
            force: false,
            dry_run: false,
        };
        let mut stdout: Vec<u8> = Vec::new();
        let mut buf: Vec<u8> = Vec::new();
        let res = require_confirmation(
            "like",
            "POST",
            "https://api.x.com/2/users/me/likes/1",
            None,
            guard,
            &out,
            false,
            &mut stdout,
            &mut buf,
            Some(Box::new(|| Ok("yes\n".to_string()))),
        );
        assert!(matches!(res, Ok(GuardOutcome::Proceed)));
        let prompt = String::from_utf8(buf).expect("prompt is utf-8");
        assert!(
            prompt.contains("About to like POST https://api.x.com/2/users/me/likes/1"),
            "prompt should describe the action, got {prompt:?}"
        );
    }

    // U4.3b: require_confirmation with "n" closure returns user-aborted usage error.
    #[test]
    fn require_confirmation_no_returns_user_aborted() {
        let out = cfg();
        let guard = WriteGuard {
            force: false,
            dry_run: false,
        };
        let mut stdout: Vec<u8> = Vec::new();
        let mut buf: Vec<u8> = Vec::new();
        let res = require_confirmation(
            "like",
            "POST",
            "https://api.x.com/2/users/me/likes/1",
            None,
            guard,
            &out,
            false,
            &mut stdout,
            &mut buf,
            Some(Box::new(|| Ok("n\n".to_string()))),
        );
        match res {
            Err(BirdError::Usage { error_id, .. }) => {
                assert_eq!(error_id, "user-aborted");
            }
            _ => panic!("expected user-aborted usage error"),
        }
    }

    // U4.4: require_confirmation with --dry-run returns DryRun without invoking the reader.
    #[test]
    fn require_confirmation_dry_run_returns_dry_run_without_reader() {
        let out = cfg();
        let guard = WriteGuard {
            force: false,
            dry_run: true,
        };
        let mut stdout: Vec<u8> = Vec::new();
        let mut buf: Vec<u8> = Vec::new();
        let panic_reader: Box<dyn FnOnce() -> std::io::Result<String>> =
            Box::new(|| panic!("reader must not be invoked for --dry-run"));
        let res = require_confirmation(
            "like",
            "POST",
            "https://api.x.com/2/users/me/likes/1",
            None,
            guard,
            &out,
            false,
            &mut stdout,
            &mut buf,
            Some(panic_reader),
        );
        assert!(matches!(res, Ok(GuardOutcome::DryRun)));
    }

    // U4.5: require_confirmation with --no-interactive returns requires-confirmation usage error.
    #[test]
    fn require_confirmation_no_interactive_errors() {
        let out = cfg();
        let guard = WriteGuard {
            force: false,
            dry_run: false,
        };
        let mut stdout: Vec<u8> = Vec::new();
        let mut buf: Vec<u8> = Vec::new();
        let res = require_confirmation(
            "like",
            "POST",
            "https://api.x.com/2/users/me/likes/1",
            None,
            guard,
            &out,
            true,
            &mut stdout,
            &mut buf,
            None,
        );
        match res {
            Err(BirdError::Usage { error_id, .. }) => {
                assert_eq!(error_id, "requires-confirmation");
            }
            _ => panic!("expected requires-confirmation usage error"),
        }
    }

    // U4.6: build_dry_run_url returns the expected absolute URL.
    #[test]
    fn build_dry_run_url_assembles_absolute_url() {
        let params: HashMap<String, String> = HashMap::new();
        let query: Vec<String> = Vec::new();
        let url = build_dry_run_url("/2/users/me", &params, &query);
        assert_eq!(url.as_deref(), Some("https://api.x.com/2/users/me"));
    }

    // U4.7: clamp_limit ceiling + default behavior.
    #[test]
    fn clamp_limit_enforces_ceiling_and_default() {
        assert_eq!(clamp_limit(Some(50_000), 1000, 10_000), (10_000, true));
        assert_eq!(clamp_limit(None, 1000, 10_000), (1000, false));
        assert_eq!(clamp_limit(Some(500), 1000, 10_000), (500, false));
        assert_eq!(clamp_limit(Some(0), 1000, 10_000), (1000, false));
    }
}
