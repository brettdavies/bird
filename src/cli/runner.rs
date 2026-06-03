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
//! Per R7, the only Plan-1 macro change made here is that the Tier-1
//! `Completions` short-circuit routes `clap_complete::generate` through the
//! runner's `stdout` writer rather than `std::io::stdout()`. The remaining
//! `out_println!` / `out_print!` / `diag!` sites continue to write to global
//! handles; Plan 2 addresses them.

#![doc(hidden)]

use crate::cli::argv::{explicit_output_from_argv, output_from_argv};
use crate::cli::clap_errors::clap_error_to_bird;
use crate::cli::dispatch::{
    GuardOutcome, ListFlags, clamp_limit, command_needs_xurl, require_confirmation,
};
use crate::cli::{Cli, Command, SkillAction, WatchlistCommand};
use crate::config::{ArgOverrides, EnvOverrides, ResolvedConfig, ResolvedPaths};
use crate::error::BirdError;
use crate::output::{OutputConfig, OutputFormat};
use crate::{
    db, diag, doctor, out_print, out_println, output, schema, schema_print, skill_install,
    transport, watchlist,
};
use clap::Parser;
use std::ffi::OsString;
use std::io::{IsTerminal, Write};
use std::process::ExitCode;

/// Curated top-level examples block — embedded so `--examples` works on every host.
const TOP_LEVEL_EXAMPLES: &str = include_str!("../../examples/top-level.txt");

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
            Ok(line) => out_println!("{}", line),
            Err(_) => out_println!("{}", TOP_LEVEL_EXAMPLES),
        }
    } else {
        out_print!("{}", TOP_LEVEL_EXAMPLES);
    }
    ExitCode::SUCCESS
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
            let err = BirdError::config(e);
            let _ = writeln!(stderr, "{}", err.message());
            return ExitCode::from(err.exit_code());
        }
    };
    let env = EnvOverrides::from_env();
    run_with_paths(args, stdout, stderr, paths, env)
}

/// Worker entrypoint. Owns the full dispatch pipeline against caller-supplied
/// paths and env. Tests call this directly with `TempDir`-backed paths.
///
/// Today most output still routes through the `out_println!` / `out_print!` /
/// `diag!` macros (global handles); Plan 2 migrates them to the injected
/// writers. The clap Tier-1 `Completions` branch is the one Plan-1 exception
/// and writes to the `stdout` parameter directly.
pub fn run_with_paths<I, S>(
    args: I,
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
    let explicit_output = explicit_output_from_argv(&argv);

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
        return print_examples(&cfg);
    }

    // try_parse_from routes clap errors through the JSON-aware envelope
    // formatter. Reading from the caller-supplied iterator (not
    // `std::env::args`) keeps the library pure.
    let cli = match Cli::try_parse_from(args_os.iter()) {
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
                        Ok(line) => out_println!("{}", line),
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

    // Apply --timeout to the xurl transport layer. U8 wraps TIMEOUT_OVERRIDE
    // in OnceLock<Mutex<Option<u64>>> per KTD-5; the deferred R22 follow-up
    // threads the value through Transport::request and drops the static.
    transport::set_timeout_secs(cli.timeout);

    // --- Meta-commands: need nothing beyond parsed args ---
    if let Command::Completions { shell } = &cli.command {
        use clap::CommandFactory;
        // R7: route completions through the runner's stdout so library
        // consumers capture all output (AE1).
        clap_complete::generate(*shell, &mut Cli::command(), "bird", stdout);
        return ExitCode::SUCCESS;
    }

    if let Command::Skill { action } = &cli.command {
        let (host, all, dry_run) = match *action {
            SkillAction::Install { host, all, dry_run }
            | SkillAction::Update { host, all, dry_run } => (host, all, dry_run),
        };
        let home = match skill_install::resolve_home() {
            Ok(h) => h,
            Err(e) => {
                let err = BirdError::from_source("skill", Box::new(e));
                output::print_error(&err, &out);
                return ExitCode::from(err.exit_code());
            }
        };
        return match skill_install::run(&out, stdout, host, dry_run, all, &home) {
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
    // env.username is the X_API_USERNAME snapshot; validate at runner time to
    // preserve the same warn-and-drop behavior the inline read used to have.
    let env_username = env
        .username
        .clone()
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

    let config = match ResolvedConfig::load_with_paths(overrides, paths.clone(), env.clone()) {
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
        match doctor::run_doctor(&client, &out, stdout, *pretty, scope, use_emoji) {
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
        let result = match action {
            WatchlistCommand::Add { username } => {
                watchlist::run_watchlist_add(&config, &out, username).map_err(BirdError::config)
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
                    &mut std::io::stderr().lock(),
                    None,
                ) {
                    Ok(GuardOutcome::DryRun) => Ok(()),
                    Ok(GuardOutcome::Proceed) => {
                        watchlist::run_watchlist_remove(&config, &out, username)
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
    match crate::cli::dispatch::run(
        cli.command,
        config,
        &mut client,
        &out,
        stdout,
        stderr,
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
            store_path: tmp,
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
        let exit = run_with_paths(["bird", "--help"], &mut stdout, &mut stderr, paths, env);
        assert!(
            exit_eq(exit, ExitCode::SUCCESS),
            "--help should exit 0, got {:?}",
            exit
        );
        // Plan 1 note: --help currently bypasses the runner's writers (clap's
        // `e.print()`). Plan 2 U11 will tighten the writer assertion.
    }

    #[test]
    fn run_with_paths_bogus_flag_returns_two() {
        let paths = test_paths();
        let env = EnvOverrides::default();
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        let exit = run_with_paths(["bird", "--bogus"], &mut stdout, &mut stderr, paths, env);
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
        let exit = run_with_paths(["bird", "--version"], &mut stdout, &mut stderr, paths, env);
        assert!(
            exit_eq(exit, ExitCode::SUCCESS),
            "--version should exit 0, got {:?}",
            exit
        );
    }
}
