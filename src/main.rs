//! bird — X API CLI. Subcommands: login, me; raw get/post/put/delete.

use bird::cli::argv::{explicit_output_from_argv, output_from_argv};
use bird::cli::clap_errors::clap_error_to_bird;
use bird::cli::dispatch::{
    GuardOutcome, ListFlags, clamp_limit, command_needs_xurl, require_confirmation,
};
use bird::cli::{Cli, Command, SkillAction, WatchlistCommand};
use bird::config::{ArgOverrides, ResolvedConfig};
use bird::error::BirdError;
use bird::output::{OutputConfig, OutputFormat};
use bird::{
    db, diag, doctor, out_print, out_println, output, schema, schema_print, skill_install,
    transport, watchlist,
};
use clap::Parser;
use std::io::IsTerminal;
use std::process::ExitCode;

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
            Ok(line) => out_println!("{}", line),
            Err(_) => out_println!("{}", TOP_LEVEL_EXAMPLES),
        }
    } else {
        out_print!("{}", TOP_LEVEL_EXAMPLES);
    }
    ExitCode::SUCCESS
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
        let home = match skill_install::resolve_home() {
            Ok(h) => h,
            Err(e) => {
                let err = BirdError::from_source("skill", Box::new(e));
                output::print_error(&err, &out);
                return ExitCode::from(err.exit_code());
            }
        };
        return match skill_install::run(host, dry_run, all, &home) {
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
                    &mut std::io::stderr().lock(),
                    None,
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
    match bird::cli::dispatch::run(
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
}
