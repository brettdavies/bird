//! bird — X API CLI. Subcommands: login, me; raw get/post/put/delete.

mod auth;
mod config;
mod doctor;
mod output;
mod requirements;
mod login;
mod bookmarks;
mod raw;
mod schema;

use clap::CommandFactory;
use clap::FromArgMatches;
use config::{ArgOverrides, ResolvedConfig};
use reqwest_oauth1::OAuthClientProvider;
use std::collections::HashMap;
use std::io::IsTerminal;
use std::process::ExitCode;

fn use_color_from_cli(plain: bool, no_color: bool) -> bool {
    let stderr_tty = std::io::stderr().is_terminal();
    let no_color_env = std::env::var("NO_COLOR").is_ok();
    let term_dumb = std::env::var("TERM").as_deref() == Ok("dumb");
    let default_on = stderr_tty && !no_color_env && !term_dumb;
    default_on && !plain && !no_color
}

fn eprint_command_error(cmd: &str, e: &dyn std::error::Error, use_color: bool) {
    let prefix = format!("{} failed: ", cmd);
    eprintln!("{}{}", output::error(&prefix, use_color), e);
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

#[derive(clap::Parser)]
#[command(name = "bird", about = "X API CLI", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// OAuth2 client ID (overrides config and env)
    #[arg(long, global = true)]
    client_id: Option<String>,

    /// OAuth2 client secret (overrides config and env)
    #[arg(long, global = true)]
    client_secret: Option<String>,

    /// Access token (overrides config and env)
    #[arg(long, global = true)]
    access_token: Option<String>,

    /// Refresh token (overrides config and env)
    #[arg(long, global = true)]
    refresh_token: Option<String>,

    /// Username for multi-account selection
    #[arg(long, global = true)]
    username: Option<String>,

    /// Plain output (no color, no hyperlinks; script-friendly)
    #[arg(long, global = true)]
    plain: bool,

    /// Disable ANSI colors (or set NO_COLOR)
    #[arg(long, global = true)]
    no_color: bool,
}

#[derive(clap::Subcommand)]
enum Command {
    /// OAuth2 login: open browser, store tokens by username
    Login,

    /// Show current user (GET /2/users/me)
    Me {
        /// Human-readable output
        #[arg(long)]
        pretty: bool,
    },

    /// GET request to path (e.g. /2/users/me or /2/users/{id}/bookmarks with -p id=123)
    Get {
        path: String,
        #[arg(long, short = 'p', value_name = "KEY=VALUE", num_args = 1..)]
        param: Vec<String>,
        #[arg(long, value_name = "KEY=VALUE", num_args = 1..)]
        query: Vec<String>,
        #[arg(long)]
        pretty: bool,
    },

    /// POST request to path
    Post {
        path: String,
        #[arg(long, short = 'p', value_name = "KEY=VALUE", num_args = 1..)]
        param: Vec<String>,
        #[arg(long, value_name = "KEY=VALUE", num_args = 1..)]
        query: Vec<String>,
        #[arg(long, value_name = "JSON")]
        body: Option<String>,
        #[arg(long)]
        pretty: bool,
    },

    /// PUT request to path
    Put {
        path: String,
        #[arg(long, short = 'p', value_name = "KEY=VALUE", num_args = 1..)]
        param: Vec<String>,
        #[arg(long, value_name = "KEY=VALUE", num_args = 1..)]
        query: Vec<String>,
        #[arg(long, value_name = "JSON")]
        body: Option<String>,
        #[arg(long)]
        pretty: bool,
    },

    /// List bookmarks for the current user (paginated, max_results=100)
    Bookmarks {
        #[arg(long)]
        pretty: bool,
    },

    /// DELETE request to path
    Delete {
        path: String,
        #[arg(long, short = 'p', value_name = "KEY=VALUE", num_args = 1..)]
        param: Vec<String>,
        #[arg(long, value_name = "KEY=VALUE", num_args = 1..)]
        query: Vec<String>,
        #[arg(long)]
        pretty: bool,
    },

    /// Show what is available: auth state, effective config, and which commands can run (JSON by default; --pretty for human summary). Optional command name for scoped report (e.g. bird doctor me).
    Doctor {
        /// Scope report to this command only (e.g. me, bookmarks, get)
        command: Option<String>,
        #[arg(long)]
        pretty: bool,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("bird=info".parse().unwrap()))
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
    let overrides = ArgOverrides {
        client_id: cli.client_id.or_else(|| std::env::var("X_API_CLIENT_ID").ok()),
        client_secret: cli.client_secret.or_else(|| std::env::var("X_API_CLIENT_SECRET").ok()),
        access_token: cli.access_token.or_else(|| std::env::var("X_API_ACCESS_TOKEN").ok()),
        refresh_token: cli.refresh_token.or_else(|| std::env::var("X_API_REFRESH_TOKEN").ok()),
        bearer_token: std::env::var("X_API_BEARER_TOKEN").ok(),
        username: cli.username.or_else(|| std::env::var("X_API_USERNAME").ok()),
        oauth1_consumer_key: None,
        oauth1_consumer_secret: None,
        oauth1_access_token: None,
        oauth1_access_token_secret: None,
    };

    let use_color = use_color_from_cli(cli.plain, cli.no_color);
    let use_hyperlinks = use_color && std::io::stderr().is_terminal();

    let config = match ResolvedConfig::load(overrides) {
        Ok(c) => c,
        Err(e) => {
            eprint_command_error("config", e.as_ref(), use_color);
            return ExitCode::from(1);
        }
    };

    match cli.command {
        Command::Login => {
            if let Err(e) = login::run_login(config, use_color, use_hyperlinks).await {
                eprint_command_error("login", e.as_ref(), use_color);
                return ExitCode::from(1);
            }
        }
        Command::Me { pretty } => {
            if let Err(e) = run_me(&config, pretty).await {
                eprint_command_error("me", e.as_ref(), use_color);
                return ExitCode::from(1);
            }
        }
        Command::Bookmarks { pretty } => {
            if let Err(e) = bookmarks::run_bookmarks(&config, pretty).await {
                eprint_command_error("bookmarks", e.as_ref(), use_color);
                return ExitCode::from(1);
            }
        }
        Command::Get { path, param, query, pretty } => {
            let params = parse_param_vec(&param);
            if let Err(e) = raw::run_raw(&config, "GET", &path, &params, &query, None, pretty).await {
                eprint_command_error("get", e.as_ref(), use_color);
                return ExitCode::from(1);
            }
        }
        Command::Post { path, param, query, body, pretty } => {
            let params = parse_param_vec(&param);
            if let Err(e) = raw::run_raw(&config, "POST", &path, &params, &query, body.as_deref(), pretty).await {
                eprint_command_error("post", e.as_ref(), use_color);
                return ExitCode::from(1);
            }
        }
        Command::Put { path, param, query, body, pretty } => {
            let params = parse_param_vec(&param);
            if let Err(e) = raw::run_raw(&config, "PUT", &path, &params, &query, body.as_deref(), pretty).await {
                eprint_command_error("put", e.as_ref(), use_color);
                return ExitCode::from(1);
            }
        }
        Command::Delete { path, param, query, pretty } => {
            let params = parse_param_vec(&param);
            if let Err(e) = raw::run_raw(&config, "DELETE", &path, &params, &query, None, pretty).await {
                eprint_command_error("delete", e.as_ref(), use_color);
                return ExitCode::from(1);
            }
        }
        Command::Doctor { command, pretty } => {
            let scope = command.as_deref();
            let use_emoji = use_color && pretty;
            if let Err(e) = doctor::run_doctor(&config, pretty, scope, use_color, use_emoji) {
                eprint_command_error("doctor", e.as_ref(), use_color);
                return ExitCode::from(1);
            }
        }
    }

    ExitCode::SUCCESS
}

async fn run_me(
    config: &ResolvedConfig,
    pretty: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = auth::resolve_token_for_command(config, "me").await?;
    let client = reqwest::Client::new();
    let (status, text) = match token {
        auth::CommandToken::Bearer(access) => {
            let res = client
                .get("https://api.x.com/2/users/me")
                .header("Authorization", format!("Bearer {}", access))
                .send()
                .await?;
            (res.status(), res.text().await?)
        }
        auth::CommandToken::OAuth1 => {
            let ck = config.oauth1_consumer_key.as_ref().unwrap();
            let cs = config.oauth1_consumer_secret.as_ref().unwrap();
            let at = config.oauth1_access_token.as_ref().unwrap();
            let ats = config.oauth1_access_token_secret.as_ref().unwrap();
            let secrets = reqwest_oauth1::Secrets::new(ck.as_str(), cs.as_str()).token(at.as_str(), ats.as_str());
            let res = client
                .oauth1(secrets)
                .get("https://api.x.com/2/users/me")
                .send()
                .await?;
            (res.status(), res.text().await?)
        }
    };
    if !status.is_success() {
        return Err(format!("GET /2/users/me failed {}: {}", status, text).into());
    }
    let json: serde_json::Value = serde_json::from_str(&text)?;
    if pretty {
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("{}", serde_json::to_string(&json)?);
    }
    Ok(())
}
