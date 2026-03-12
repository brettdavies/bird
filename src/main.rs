//! bird — X API CLI. Subcommands: login, me; raw get/post/put/delete.

mod auth;
mod bookmarks;
mod config;
mod cost;
mod db;
mod doctor;
mod fields;
mod login;
mod output;
mod profile;
mod raw;
mod requirements;
mod schema;
mod search;
mod thread;
mod types;
mod usage;
mod watchlist;

use clap::CommandFactory;
use clap::FromArgMatches;
use config::{ArgOverrides, ResolvedConfig};
use std::collections::HashMap;
use std::io::IsTerminal;
use std::process::ExitCode;
use std::time::Duration;

/// Structured error for the CLI. Each variant carries the command name and maps to a distinct exit code.
enum BirdError {
    /// Configuration error (exit code 78 — EX_CONFIG)
    Config(Box<dyn std::error::Error + Send + Sync>),
    /// Auth error — no valid credentials for the command (exit code 77 — EX_NOPERM)
    Auth(requirements::AuthRequiredError),
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

    fn print(&self, use_color: bool) {
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
}

impl From<requirements::AuthRequiredError> for BirdError {
    fn from(e: requirements::AuthRequiredError) -> Self {
        BirdError::Auth(e)
    }
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

    /// Account name for multi-account token selection (matches stored token key)
    #[arg(long, global = true)]
    account: Option<String>,

    /// Plain output (no color, no hyperlinks; script-friendly)
    #[arg(long, global = true)]
    plain: bool,

    /// Disable ANSI colors (or set NO_COLOR)
    #[arg(long, global = true)]
    no_color: bool,

    /// Bypass store read, still write response to store
    #[arg(long, global = true)]
    refresh: bool,

    /// Disable entity store entirely (no read, no write)
    #[arg(long, global = true)]
    no_cache: bool,

    /// Only serve from local store; never make API requests
    #[arg(long, global = true)]
    cache_only: bool,
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

    /// Look up a user profile by username
    Profile {
        /// X/Twitter username (with or without @)
        username: String,
        /// Pretty-print JSON output
        #[arg(long)]
        pretty: bool,
    },

    /// Search recent tweets (GET /2/tweets/search/recent)
    Search {
        /// Search query (X API search syntax)
        query: String,

        /// Pretty-print JSON output
        #[arg(long)]
        pretty: bool,

        /// Sort results: recent (default), likes
        #[arg(long, default_value = "recent")]
        sort: String,

        /// Minimum like count threshold
        #[arg(long)]
        min_likes: Option<u64>,

        /// Maximum results per page (10-100, default: 100)
        #[arg(long)]
        max_results: Option<u32>,

        /// Number of pages to fetch (1-10, default: 1)
        #[arg(long)]
        pages: Option<u32>,
    },

    /// Reconstruct a conversation thread from a tweet
    Thread {
        /// Tweet ID (root tweet or any reply in the thread)
        tweet_id: String,
        /// Pretty-print JSON output
        #[arg(long)]
        pretty: bool,
        /// Maximum number of search result pages (default: 10, max: 25)
        #[arg(long, default_value = "10")]
        max_pages: u32,
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

    /// Monitor accounts: check recent activity, manage watchlist
    Watchlist {
        #[command(subcommand)]
        action: WatchlistCommand,
        /// Pretty-print JSON output
        #[arg(long)]
        pretty: bool,
    },

    /// View API usage and costs
    Usage {
        /// Show usage since this date (YYYY-MM-DD; default: 30 days ago)
        #[arg(long)]
        since: Option<String>,
        /// Sync actual usage from X API (requires Bearer token)
        #[arg(long)]
        sync: bool,
        /// Pretty-print output
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

    /// Manage the HTTP response cache
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
}

#[derive(clap::Subcommand)]
enum CacheAction {
    /// Delete all cache entries
    Clear,
    /// Show cache status (JSON default, --pretty for human-readable)
    Stats {
        #[arg(long)]
        pretty: bool,
    },
}

#[derive(clap::Subcommand)]
enum WatchlistCommand {
    /// Check recent activity for all watched accounts
    Check,
    /// Add an account to the watchlist
    Add {
        /// X/Twitter username (with or without @)
        username: String,
    },
    /// Remove an account from the watchlist
    Remove {
        /// X/Twitter username to remove
        username: String,
    },
    /// Show the current watchlist
    List,
}

async fn run(
    command: Command,
    config: ResolvedConfig,
    client: &mut db::BirdClient,
    use_color: bool,
    use_hyperlinks: bool,
) -> Result<(), BirdError> {
    match command {
        Command::Login => {
            login::run_login(client.http(), config, use_color, use_hyperlinks)
                .await
                .map_err(|e| BirdError::Command {
                    name: "login",
                    source: e,
                })?;
            // Security: clear store after re-auth to prevent stale data from previous context
            if let Some(Ok(count)) = client.db_clear() {
                if count > 0 {
                    eprintln!("[store] Cleared {} stored entries after login.", count);
                }
            }
        }
        Command::Me { pretty } => {
            let params = HashMap::new();
            raw::run_raw(
                client,
                &config,
                "GET",
                "/2/users/me",
                &params,
                &[],
                None,
                pretty,
                use_color,
            )
            .await
            .map_err(|e| BirdError::Command {
                name: "me",
                source: e,
            })?;
        }
        Command::Bookmarks { pretty } => {
            bookmarks::run_bookmarks(client, &config, pretty, use_color)
                .await
                .map_err(|e| BirdError::Command {
                    name: "bookmarks",
                    source: e,
                })?;
        }
        Command::Profile { username, pretty } => {
            profile::run_profile(
                client,
                &config,
                profile::ProfileOpts {
                    username: &username,
                    pretty,
                },
                use_color,
            )
            .await
            .map_err(|e| BirdError::Command {
                name: "profile",
                source: e,
            })?;
        }
        Command::Search {
            query,
            pretty,
            sort,
            min_likes,
            max_results,
            pages,
        } => {
            let opts = search::SearchOpts {
                query: &query,
                pretty,
                sort: &sort,
                min_likes,
                max_results: max_results.unwrap_or(100).clamp(10, 100),
                pages: pages.unwrap_or(1).clamp(1, 10),
            };
            search::run_search(client, &config, opts, use_color)
                .await
                .map_err(|e| BirdError::Command {
                    name: "search",
                    source: e,
                })?;
        }
        Command::Thread {
            tweet_id,
            pretty,
            max_pages,
        } => {
            thread::run_thread(
                client,
                &config,
                thread::ThreadOpts {
                    tweet_id: &tweet_id,
                    pretty,
                    max_pages,
                },
                use_color,
            )
            .await
            .map_err(|e| BirdError::Command {
                name: "thread",
                source: e,
            })?;
        }
        Command::Get {
            path,
            param,
            query,
            pretty,
        } => {
            let params = parse_param_vec(&param);
            raw::run_raw(
                client, &config, "GET", &path, &params, &query, None, pretty, use_color,
            )
            .await
            .map_err(|e| BirdError::Command {
                name: "get",
                source: e,
            })?;
        }
        Command::Post {
            path,
            param,
            query,
            body,
            pretty,
        } => {
            let params = parse_param_vec(&param);
            raw::run_raw(
                client,
                &config,
                "POST",
                &path,
                &params,
                &query,
                body.as_deref(),
                pretty,
                use_color,
            )
            .await
            .map_err(|e| BirdError::Command {
                name: "post",
                source: e,
            })?;
        }
        Command::Put {
            path,
            param,
            query,
            body,
            pretty,
        } => {
            let params = parse_param_vec(&param);
            raw::run_raw(
                client,
                &config,
                "PUT",
                &path,
                &params,
                &query,
                body.as_deref(),
                pretty,
                use_color,
            )
            .await
            .map_err(|e| BirdError::Command {
                name: "put",
                source: e,
            })?;
        }
        Command::Delete {
            path,
            param,
            query,
            pretty,
        } => {
            let params = parse_param_vec(&param);
            raw::run_raw(
                client, &config, "DELETE", &path, &params, &query, None, pretty, use_color,
            )
            .await
            .map_err(|e| BirdError::Command {
                name: "delete",
                source: e,
            })?;
        }
        Command::Watchlist { action, pretty } => match action {
            WatchlistCommand::Check => {
                watchlist::run_watchlist_check(client, &config, pretty, use_color)
                    .await
                    .map_err(|e| BirdError::Command {
                        name: "watchlist",
                        source: e,
                    })?;
            }
            WatchlistCommand::Add { username } => {
                watchlist::run_watchlist_add(&config, &username).map_err(BirdError::Config)?;
            }
            WatchlistCommand::Remove { username } => {
                watchlist::run_watchlist_remove(&config, &username).map_err(BirdError::Config)?;
            }
            WatchlistCommand::List => {
                watchlist::run_watchlist_list(&config, pretty).map_err(|e| BirdError::Command {
                    name: "watchlist",
                    source: e,
                })?;
            }
        },
        Command::Usage {
            since,
            sync,
            pretty,
        } => {
            usage::run_usage(client, &config, since.as_deref(), sync, pretty)
                .await
                .map_err(|e| BirdError::Command {
                    name: "usage",
                    source: e,
                })?;
        }
        Command::Doctor { command, pretty } => {
            let scope = command.as_deref();
            let use_emoji = use_color && pretty;
            doctor::run_doctor(&config, client, pretty, scope, use_color, use_emoji).map_err(
                |e| BirdError::Command {
                    name: "doctor",
                    source: e,
                },
            )?;
        }
        Command::Cache { action } => match action {
            CacheAction::Clear => match client.db_clear() {
                Some(Ok(count)) => {
                    let stats = client.db_stats().and_then(|r| r.ok());
                    let size_str =
                        stats.map_or("0.0".to_string(), |s| format!("{:.1}", s.size_mb()));
                    eprintln!("Cleared {} stored entities ({} MB).", count, size_str);
                }
                Some(Err(e)) => {
                    return Err(BirdError::Command {
                        name: "cache",
                        source: format!("failed to clear store: {}", e).into(),
                    });
                }
                None => {
                    eprintln!("Store is not available.");
                }
            },
            CacheAction::Stats { pretty } => match client.db_stats() {
                Some(Ok(stats)) => {
                    if pretty {
                        let path = client
                            .db_path()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "unknown".to_string());
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
                        let path = client
                            .db_path()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "unknown".to_string());
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
                    eprintln!("Store is not available.");
                }
            },
        },
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
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
    let use_hyperlinks = use_color && std::io::stderr().is_terminal();

    let overrides = ArgOverrides {
        client_id: cli
            .client_id
            .or_else(|| std::env::var("X_API_CLIENT_ID").ok()),
        client_secret: cli
            .client_secret
            .or_else(|| std::env::var("X_API_CLIENT_SECRET").ok()),
        access_token: cli
            .access_token
            .or_else(|| std::env::var("X_API_ACCESS_TOKEN").ok()),
        refresh_token: cli
            .refresh_token
            .or_else(|| std::env::var("X_API_REFRESH_TOKEN").ok()),
        bearer_token: std::env::var("X_API_BEARER_TOKEN").ok(),
        username: cli.account.or_else(|| std::env::var("X_API_USERNAME").ok()),
        oauth1_consumer_key: None,
        oauth1_consumer_secret: None,
        oauth1_access_token: None,
        oauth1_access_token_secret: None,
    };

    let config = match ResolvedConfig::load(overrides) {
        Ok(c) => c,
        Err(e) => {
            let err = BirdError::Config(e);
            err.print(use_color);
            return ExitCode::from(err.exit_code());
        }
    };

    let http_client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client");

    let cache_opts = db::CacheOpts {
        no_store: cli.no_cache || !config.cache_enabled,
        refresh: cli.refresh,
        cache_only: cli.cache_only,
    };
    let mut client = db::BirdClient::new(
        http_client,
        &config.cache_path,
        cache_opts,
        config.cache_max_size_mb,
    );

    match run(cli.command, config, &mut client, use_color, use_hyperlinks).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            e.print(use_color);
            ExitCode::from(e.exit_code())
        }
    }
}
