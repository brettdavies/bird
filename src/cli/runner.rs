//! Layered entrypoints for in-process invocation.
//!
//! - [`run_argv`]: the binary entrypoint. Reads `args_os`, locks std stdout/stderr.
//! - [`run`]: convenience wrapper for library consumers — loads paths/env from
//!   the process and delegates to [`run_with_paths`].
//! - [`run_with_paths`]: the real worker. Tests call this directly with
//!   `TempDir`-backed [`ResolvedPaths`] and explicit [`EnvOverrides`].
//!
//! The library never calls `process::exit`; it returns [`ExitCode`] to the caller.
//!
//! All stdout writes in this module route through the runner-injected
//! `stdout` writer: the `Completions` short-circuit, the `--examples`
//! short-circuit (`print_examples`), and the JSON-wrapped clap help/version
//! envelope branch. Diagnostic writes route through the injected `stderr`
//! writer or — for `BirdClient`/`BirdDb` internal sites — through the
//! `Arc<Mutex<dyn Write + Send>>` handle constructed at runner-entry.

#![doc(hidden)]

use crate::cli::argv::{explicit_output_from_argv, output_from_argv};
use crate::cli::clap_errors::clap_error_to_bird;
use crate::cli::dispatch::command_is_api_hitting;
use crate::cli::dispatch::{
    GuardOutcome, ListFlags, clamp_limit, require_confirmation, validate_auth_value,
};
use crate::cli::{Cli, Command, OutputFlags, SkillAction, WatchlistCommand};
use crate::config::{ArgOverrides, EnvOverrides, ResolvedConfig, ResolvedPaths};
use crate::error::BirdError;
use crate::output::envelope::Reason;
use crate::output::hints::HintContext;
use crate::output::{OutputConfig, OutputFormat};
use crate::{db, doctor, output, schema, schema_print, skill_install, watchlist};
use clap::Parser;
use std::ffi::OsString;
use std::io::{BufRead, IsTerminal, Write};
use std::process::ExitCode;

/// Curated top-level examples block — embedded so `--examples` works on every host.
const TOP_LEVEL_EXAMPLES: &str = include_str!("../../examples/top-level.txt");

/// Emit the curated top-level examples block and exit zero. JSON mode prints the
/// parsed example invocations as `{"status":"ok","data":[...],"meta":{...}}`.
fn print_examples(out: &OutputConfig, stdout: &mut dyn Write) -> ExitCode {
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
        let count = qualified.len();
        let payload = serde_json::json!({"data": qualified, "meta": {"count": count}});
        let _ = out.print_ok(stdout, payload);
    } else {
        let _ = write!(stdout, "{}", TOP_LEVEL_EXAMPLES);
    }
    ExitCode::SUCCESS
}

/// `bird <command path> --help` for the deepest subcommand `argv` names.
///
/// Words before the first subcommand are skipped (values of global flags);
/// after it, the first word that is not a nested subcommand ends the path.
fn invoked_help(argv: &[String]) -> String {
    use clap::CommandFactory;
    let mut cmd = Cli::command();
    let mut path = vec!["bird".to_string()];
    for arg in argv.iter().skip(1) {
        if arg == "--" {
            break;
        }
        if arg.starts_with('-') {
            continue;
        }
        match cmd.find_subcommand(arg) {
            Some(sub) => {
                path.push(sub.get_name().to_string());
                let next = sub.clone();
                cmd = next;
            }
            None if path.len() == 1 => continue,
            None => break,
        }
    }
    path.push("--help".to_string());
    path.join(" ")
}

/// Binary entrypoint. Reads `std::env::args_os`, locks std stdout/stderr, and
/// delegates to [`run`]. Returns [`ExitCode`]; the binary converts to a process
/// exit.
pub fn run_argv() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let mut stdout_lock = stdout.lock();
    let mut stderr_lock = stderr.lock();
    run(args, &mut stdout_lock, &mut stderr_lock)
}

/// Library-consumer entrypoint. Loads [`ResolvedPaths`] and [`EnvOverrides`]
/// from the process environment, then delegates to [`run_with_paths`]. Consumers
/// that need to inject paths (tests, embeddors) should call [`run_with_paths`]
/// directly.
pub fn run<I, S>(args: I, stdout: &mut dyn Write, stderr: &mut dyn Write) -> ExitCode
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
{
    let paths = match ResolvedPaths::from_env() {
        Ok(p) => p,
        Err(e) => {
            // Fatal pre-config path: no OutputConfig has been constructed
            // yet, so route through the BirdError::print chokepoint per
            // Plan 2 R18 instead of bypassing the format selection.
            let err = BirdError::config(e);
            err.print();
            return ExitCode::from(err.exit_code());
        }
    };
    let env = EnvOverrides::from_env();
    run_with_paths(
        args,
        &mut std::io::stdin().lock(),
        stdout,
        stderr,
        paths,
        env,
    )
}

/// Worker entrypoint. Owns the full dispatch pipeline against caller-supplied
/// paths and env. Tests call this directly with `TempDir`-backed paths.
///
/// `stdin` is read only by `bird login --no-browser`, for the redirect URL;
/// tests pass `std::io::empty()`.
///
/// stdout flows through the injected `stdout` writer for every short-circuit
/// branch (`Completions`, `--examples`, the JSON-wrapped help/version
/// envelope) and through dispatch into the per-command handlers. Diagnostic
/// output flows through the injected `stderr` writer (handler params) or
/// through the `Arc<Mutex<dyn Write + Send>>` handle on `BirdClient` /
/// `BirdDb` for internal sites — both bind to the process stderr in the
/// binary entrypoint, both can be redirected for library callers.
pub fn run_with_paths<I, S>(
    args: I,
    stdin: &mut dyn BufRead,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    paths: ResolvedPaths,
    env: EnvOverrides,
) -> ExitCode
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
{
    // Materialize argv once for the pre-parse scans (clap also needs to consume
    // it from a clone via `try_parse_from`).
    let args_os: Vec<OsString> = args.into_iter().map(|s| s.into()).collect();
    let argv: Vec<String> = args_os
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let argv_output = output_from_argv(&argv);
    let mut hint_ctx = HintContext {
        snapshot: None,
        invocation: argv.clone(),
        help: invoked_help(&argv),
        structured: argv_output.is_json(),
    };

    // `--examples` is a global help-style flag: print the curated examples block
    // and exit zero, even when no subcommand is supplied. Must short-circuit
    // before `Cli::try_parse_from` so a missing subcommand does not turn it
    // into a usage error.
    if argv.iter().any(|a| a == "--examples") {
        let fmt = argv_output;
        let cfg = OutputConfig {
            format: fmt,
            use_color: output::use_color_auto() && !fmt.is_json(),
            quiet: false,
            raw: false,
        };
        return print_examples(&cfg, stdout);
    }

    // try_parse_from routes clap errors through the JSON-aware envelope
    // formatter. Reading from the caller-supplied iterator (not
    // `std::env::args`) keeps the library pure.
    let cli = match Cli::try_parse_from(args_os.iter()) {
        Ok(c) => c,
        Err(e) => match clap_error_to_bird(&e) {
            None => {
                // Help and version print plain unless JSON was asked for by
                // flag, where they take the ok envelope so a JSON caller (and
                // anc's `--help --output json` probe) still reads one document.
                let explicit = explicit_output_from_argv(&argv).filter(|f| f.is_json());
                match explicit {
                    Some(format) => {
                        let key = match e.kind() {
                            clap::error::ErrorKind::DisplayVersion => "version",
                            _ => "help",
                        };
                        let cfg = OutputConfig {
                            format,
                            use_color: false,
                            quiet: false,
                            raw: argv.iter().any(|a| a == "--raw"),
                        };
                        let doc = output::envelope::Envelope::Ok {
                            payload: serde_json::json!({"data": { key: e.render().to_string().trim() }}),
                        };
                        let value = serde_json::to_value(doc).unwrap_or_default();
                        let _ = cfg.write_structured(stdout, &value);
                    }
                    None => {
                        let _ = write!(stdout, "{}", e.render());
                    }
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
                let _ = cfg.print_error(stderr, &bird_err, &hint_ctx);
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
    hint_ctx.structured = out.format.is_json();

    // --- Meta-commands: need nothing beyond parsed args ---
    if let Command::Completions { shell } = &cli.command {
        use clap::CommandFactory;
        // R7: route completions through the runner's stdout so library
        // consumers capture all output (AE1).
        clap_complete::generate(*shell, &mut Cli::command(), "bird", stdout);
        return ExitCode::SUCCESS;
    }

    if let Command::Skill { action } = &cli.command {
        let home = env.xdk.home.as_deref();
        let code = match *action {
            SkillAction::Install { host, all, dry_run } => {
                skill_install::run_install_multi(host, all, dry_run, home, &out, stdout)
            }
            SkillAction::Update { host, all, dry_run } => {
                skill_install::run_update_multi(host, all, dry_run, home, &out, stdout)
            }
        };
        return ExitCode::from((code.clamp(0, 255)) as u8);
    }

    if let Command::Schema { name, list, all } = &cli.command {
        return match schema_print::run(name.as_deref(), *list, *all, &out, stdout) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                let _ = out.print_error(stderr, &err, &hint_ctx);
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
                let _ = out.print_error(stderr, &err, &hint_ctx);
                return ExitCode::from(err.exit_code());
            }
        },
        None => None,
    };
    // env.username is the X_API_USERNAME snapshot; validate at runner time to
    // preserve the same warn-and-drop behavior the inline read used to have.
    let env_username = env
        .username
        .clone()
        .and_then(|u| match schema::validate_username(&u) {
            Ok(s) => Some(s.to_string()),
            Err(e) => {
                if !out.suppress_diag() {
                    writeln!(
                        stderr,
                        "[config] warning: X_API_USERNAME invalid, ignoring: {}",
                        e
                    )
                    .ok();
                }
                None
            }
        });
    let overrides = ArgOverrides {
        username: cli_username,
        env_username,
    };

    let mut config = match ResolvedConfig::load_with_paths(overrides, paths.clone(), env.clone()) {
        Ok(c) => c,
        Err(e) => {
            let err = BirdError::config(e);
            let _ = out.print_error(stderr, &err, &hint_ctx);
            return ExitCode::from(err.exit_code());
        }
    };
    config.app = cli.app.clone();

    // Construct the xdk-rs client from the captured environment and token
    // store, honoring the bird-side `--app` / `BIRD_APP` resolution. bird
    // does not consult `XURL_APP` (R10/R11); the XURL_APP migration warn fires
    // elsewhere in the runner when the user has `XURL_APP` set but not
    // `BIRD_APP`. A construction failure is captured as a stub impl returning
    // the original error on every call so non-API commands keep working.
    let xurl_client: Box<dyn crate::xurl_client::XurlClient + Send> = {
        let (cfg, auth) = crate::xurl_client::open_store(
            &config.xdk_env,
            config.token_store.as_deref(),
            config.app.as_deref(),
        );
        // Taken before `auth` moves into the client: a credential failure's
        // recovery step is chosen from the store this run loaded. The
        // snapshot holds presence flags and names, never a secret.
        hint_ctx.snapshot = Some(xdk::store::snapshot::StoreSnapshot::new(
            &auth.token_store,
            config.app.as_deref().unwrap_or_default(),
            config
                .xdk_env
                .client_id
                .as_deref()
                .is_some_and(|value| !value.is_empty()),
        ));
        hint_ctx.structured = out.format.is_json();
        match crate::xurl_client::EmbeddedClient::new(&cfg, auth, cli.timeout) {
            Ok(client) => Box::new(client),
            Err(e) => Box::new(crate::xurl_client::ConstructionStub::new(e.to_string())),
        }
    };

    let cache_opts = db::CacheOpts {
        no_store: cli.no_cache || !config.cache_enabled,
        refresh: cli.refresh,
        cache_only: cli.cache_only,
    };
    // BirdClient takes an Arc-shared writer (KTD-2). The runner's local
    // `&mut dyn Write` stderr param cannot be cloned/shared into an Arc, so we
    // construct a separate handle bound to the process stderr. Diagnostic
    // sites that fire inside `BirdClient` / `BirdDb` route through this
    // handle, not the runner's `stderr` borrow — capturing them in tests
    // requires the Plan 2 U11 signature change.
    let client_stderr: std::sync::Arc<std::sync::Mutex<dyn std::io::Write + Send>> =
        std::sync::Arc::new(std::sync::Mutex::new(std::io::stderr()));
    let mut client = db::BirdClient::new(
        xurl_client,
        &config.cache_path,
        cache_opts,
        config.cache_max_size_mb,
        config.username.clone(),
        out.suppress_diag(),
        client_stderr,
    );

    // --- Diagnostic commands: need config/DB but not xurl ---
    if let Command::Doctor {
        command,
        common: OutputFlags { pretty },
    } = &cli.command
    {
        let scope = command.as_deref();
        let pretty = *pretty;
        let use_emoji = use_color && pretty;
        match doctor::run_doctor(&client, &config, &out, stdout, pretty, scope, use_emoji) {
            Ok(()) => return ExitCode::SUCCESS,
            Err(e) => {
                let err = BirdError::general("doctor", e);
                let _ = out.print_error(stderr, &err, &hint_ctx);
                return ExitCode::from(err.exit_code());
            }
        }
    }

    // --- Local watchlist commands: need config/DB but not xurl ---
    if let Command::Watchlist {
        ref action,
        common: OutputFlags { pretty },
    } = cli.command
        && !matches!(action, WatchlistCommand::Fetch)
    {
        let result = match action {
            WatchlistCommand::Add { username } => {
                watchlist::run_watchlist_add(&config, &out, stdout, stderr, username)
                    .map_err(BirdError::config)
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
                    stdout,
                    stderr,
                    None,
                ) {
                    Ok(GuardOutcome::DryRun | GuardOutcome::Declined) => Ok(()),
                    Ok(GuardOutcome::Proceed) => {
                        watchlist::run_watchlist_remove(&config, &out, stdout, stderr, username)
                            .map_err(BirdError::config)
                    }
                    Err(e) => Err(e),
                }
            }
            WatchlistCommand::List => {
                let (limit, _) = clamp_limit(cli.limit, 1000, 10_000);
                watchlist::run_watchlist_list(
                    &config,
                    &out,
                    stdout,
                    stderr,
                    pretty,
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
                let _ = out.print_error(stderr, &e, &hint_ctx);
                ExitCode::from(e.exit_code())
            }
        };
    }

    // R10/R11: XURL_APP migration warn. Fires only on API-hitting commands
    // so `bird --help`, `bird completions`, `bird examples`, `bird cache`,
    // and the watchlist local ops stay clean.
    let needs_xurl_for_warn = command_is_api_hitting(&cli.command);
    if needs_xurl_for_warn
        && cli.app.is_none()
        && std::env::var_os("XURL_APP").is_some_and(|v| !v.is_empty())
        && !out.suppress_diag()
    {
        writeln!(
            stderr,
            "warning: XURL_APP is set but bird does not read it; set BIRD_APP or pass --app to pin the app selection"
        )
        .ok();
    }

    // R12: reject `--auth none` at parse time for commands whose
    // `auth_matrix::supported_auth` returns a non-empty scheme list.
    if let Some(ref a) = cli.auth
        && let Err(usage) = validate_auth_value(a, &cli.command)
    {
        let err = BirdError::usage(Reason::AuthMethodMismatch, usage);
        let _ = out.print_error(stderr, &err, &hint_ctx);
        return ExitCode::from(err.exit_code());
    }

    let list_flags = ListFlags {
        limit: cli.limit,
        cursor: cli.cursor.clone(),
    };
    match crate::cli::dispatch::run(
        cli.command,
        config,
        &mut client,
        &out,
        stdin,
        stdout,
        stderr,
        cli.cache_only,
        cli.no_interactive,
        list_flags,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = out.print_error(stderr, &e, &hint_ctx);
            ExitCode::from(e.exit_code())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_paths() -> ResolvedPaths {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tmp: PathBuf =
            std::env::temp_dir().join(format!("bird-runner-test-{}-{}", std::process::id(), nanos));
        ResolvedPaths {
            config_dir: tmp.clone(),
            token_store: Some(tmp.join("auth.yml")),
        }
    }

    // ExitCode does not impl PartialEq on stable; compare via Debug format,
    // which renders `ExitCode(unix_exit_status(N))` deterministically.
    fn exit_eq(actual: ExitCode, expected: ExitCode) -> bool {
        format!("{:?}", actual) == format!("{:?}", expected)
    }

    #[test]
    fn run_with_paths_help_returns_zero() {
        let paths = test_paths();
        let env = EnvOverrides::default();
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        let exit = run_with_paths(
            ["bird", "--help"],
            &mut std::io::empty(),
            &mut stdout,
            &mut stderr,
            paths,
            env,
        );
        assert!(
            exit_eq(exit, ExitCode::SUCCESS),
            "--help should exit 0, got {:?}",
            exit
        );
        // --help routes through clap's `e.print()`, which writes to the
        // process stdout directly rather than the runner's injected stdout
        // writer; the captured-content surface is therefore empty for
        // --help/--version in in-process tests.
    }

    #[test]
    fn run_with_paths_bogus_flag_returns_two() {
        let paths = test_paths();
        let env = EnvOverrides::default();
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        let exit = run_with_paths(
            ["bird", "--bogus"],
            &mut std::io::empty(),
            &mut stdout,
            &mut stderr,
            paths,
            env,
        );
        assert!(
            exit_eq(exit, ExitCode::from(2)),
            "bogus flag should exit 2, got {:?}",
            exit
        );
    }

    #[test]
    fn run_with_paths_version_returns_zero() {
        let paths = test_paths();
        let env = EnvOverrides::default();
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        let exit = run_with_paths(
            ["bird", "--version"],
            &mut std::io::empty(),
            &mut stdout,
            &mut stderr,
            paths,
            env,
        );
        assert!(
            exit_eq(exit, ExitCode::SUCCESS),
            "--version should exit 0, got {:?}",
            exit
        );
    }
}
