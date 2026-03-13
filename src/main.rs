//! bird — X API CLI. Subcommands: login, me; raw get/post/put/delete.

mod bookmarks;
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
mod types;
mod usage;
mod watchlist;

use clap::CommandFactory;
use clap::FromArgMatches;
use config::{ArgOverrides, ResolvedConfig};
use std::collections::HashMap;
use std::io::IsTerminal;
use std::process::ExitCode;

/// Structured error for the CLI. Each variant maps to a distinct exit code.
enum BirdError {
    /// Configuration error (exit code 78 — EX_CONFIG)
    Config(Box<dyn std::error::Error + Send + Sync>),
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
            BirdError::Command { .. } => 1,
        }
    }

    fn print(&self, use_color: bool) {
        match self {
            BirdError::Config(e) => {
                eprintln!("{}{}", output::error("config failed: ", use_color), e);
            }
            BirdError::Command { name, source } => {
                let prefix = format!("{} failed: ", name);
                eprintln!("{}{}", output::error(&prefix, use_color), source);
            }
        }
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

    /// Account name for multi-account token selection (maps to xurl -u)
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
    /// Authenticate via xurl (OAuth2 PKCE browser flow)
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
        /// Sync actual usage from X API (requires Bearer token via xurl)
        #[arg(long)]
        sync: bool,
        /// Pretty-print output
        #[arg(long)]
        pretty: bool,
    },

    /// Post a tweet (via xurl)
    Tweet {
        /// Tweet text
        text: String,
        /// Media ID to attach
        #[arg(long)]
        media_id: Option<String>,
    },

    /// Reply to a tweet (via xurl)
    Reply {
        /// Tweet ID to reply to
        tweet_id: String,
        /// Reply text
        text: String,
    },

    /// Like a tweet (via xurl)
    Like {
        /// Tweet ID to like
        tweet_id: String,
    },

    /// Unlike a tweet (via xurl)
    Unlike {
        /// Tweet ID to unlike
        tweet_id: String,
    },

    /// Repost (retweet) a tweet (via xurl)
    Repost {
        /// Tweet ID to repost
        tweet_id: String,
    },

    /// Undo a repost (via xurl)
    Unrepost {
        /// Tweet ID to unrepost
        tweet_id: String,
    },

    /// Follow a user (via xurl)
    Follow {
        /// Username to follow
        username: String,
    },

    /// Unfollow a user (via xurl)
    Unfollow {
        /// Username to unfollow
        username: String,
    },

    /// Send a direct message (via xurl)
    Dm {
        /// Username to message
        username: String,
        /// Message text
        text: String,
    },

    /// Block a user (via xurl)
    Block {
        /// Username to block
        username: String,
    },

    /// Unblock a user (via xurl)
    Unblock {
        /// Username to unblock
        username: String,
    },

    /// Mute a user (via xurl)
    Mute {
        /// Username to mute
        username: String,
    },

    /// Unmute a user (via xurl)
    Unmute {
        /// Username to unmute
        username: String,
    },

    /// Show what is available: xurl status, commands, and entity store health
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
    account: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut full_args: Vec<&str> = Vec::new();
    if let Some(acct) = account {
        full_args.extend(["-u", acct]);
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
    f().map_err(|e| BirdError::Command { name, source: e })
}

fn run(
    command: Command,
    config: ResolvedConfig,
    client: &mut db::BirdClient,
    use_color: bool,
    cache_only: bool,
) -> Result<(), BirdError> {
    match command {
        Command::Login => {
            // Delegate to xurl for OAuth2 authentication
            transport::xurl_passthrough(&["auth", "oauth2"]).map_err(|e| BirdError::Command {
                name: "login",
                source: e,
            })?;
            // Verify login and clear store
            if let Some(Ok(count)) = client.db_clear() {
                if count > 0 {
                    eprintln!("[store] Cleared {} stored entries after login.", count);
                }
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
                &auth_type,
            )
            .map_err(|e| BirdError::Command {
                name: "me",
                source: e,
            })?;
        }
        Command::Bookmarks { pretty } => {
            bookmarks::run_bookmarks(client, pretty, use_color).map_err(|e| {
                BirdError::Command {
                    name: "bookmarks",
                    source: e,
                }
            })?;
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
                &auth_type,
            )
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
            let auth_type = default_auth_type("search");
            let opts = search::SearchOpts {
                query: &query,
                pretty,
                sort: &sort,
                min_likes,
                max_results: max_results.unwrap_or(100).clamp(10, 100),
                pages: pages.unwrap_or(1).clamp(1, 10),
            };
            search::run_search(client, opts, use_color, &auth_type).map_err(|e| {
                BirdError::Command {
                    name: "search",
                    source: e,
                }
            })?;
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
                &auth_type,
            )
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
            let auth_type = default_auth_type("get");
            raw::run_raw(
                client, "GET", &path, &params, &query, None, pretty, use_color, &auth_type,
            )
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
                &auth_type,
            )
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
                &auth_type,
            )
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
            let auth_type = default_auth_type("delete");
            raw::run_raw(
                client, "DELETE", &path, &params, &query, None, pretty, use_color, &auth_type,
            )
            .map_err(|e| BirdError::Command {
                name: "delete",
                source: e,
            })?;
        }
        Command::Watchlist { action, pretty } => match action {
            WatchlistCommand::Check => {
                let auth_type = default_auth_type("watchlist_check");
                watchlist::run_watchlist_check(client, &config, pretty, use_color, &auth_type)
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
            usage::run_usage(client, since.as_deref(), sync, pretty).map_err(|e| {
                BirdError::Command {
                    name: "usage",
                    source: e,
                }
            })?;
        }
        // -- Write commands (xurl passthrough) --
        Command::Tweet { text, media_id } => {
            let account = config.username.as_deref();
            xurl_write(cache_only, "tweet", || {
                let mut args = vec!["post", &text];
                let media_owned;
                if let Some(ref id) = media_id {
                    media_owned = id.clone();
                    args.extend(["--media-id", &media_owned]);
                }
                xurl_write_call(&args, account)
            })?;
        }
        Command::Reply { tweet_id, text } => {
            let account = config.username.as_deref();
            xurl_write(cache_only, "reply", || {
                xurl_write_call(&["reply", &tweet_id, &text], account)
            })?;
        }
        Command::Like { tweet_id } => {
            let account = config.username.as_deref();
            xurl_write(cache_only, "like", || xurl_write_call(&["like", &tweet_id], account))?;
        }
        Command::Unlike { tweet_id } => {
            let account = config.username.as_deref();
            xurl_write(cache_only, "unlike", || xurl_write_call(&["unlike", &tweet_id], account))?;
        }
        Command::Repost { tweet_id } => {
            let account = config.username.as_deref();
            xurl_write(cache_only, "repost", || xurl_write_call(&["repost", &tweet_id], account))?;
        }
        Command::Unrepost { tweet_id } => {
            let account = config.username.as_deref();
            xurl_write(cache_only, "unrepost", || xurl_write_call(&["unrepost", &tweet_id], account))?;
        }
        Command::Follow { username } => {
            let account = config.username.as_deref();
            xurl_write(cache_only, "follow", || xurl_write_call(&["follow", &username], account))?;
        }
        Command::Unfollow { username } => {
            let account = config.username.as_deref();
            xurl_write(cache_only, "unfollow", || xurl_write_call(&["unfollow", &username], account))?;
        }
        Command::Dm { username, text } => {
            let account = config.username.as_deref();
            xurl_write(cache_only, "dm", || xurl_write_call(&["dm", &username, &text], account))?;
        }
        Command::Block { username } => {
            let account = config.username.as_deref();
            xurl_write(cache_only, "block", || xurl_write_call(&["block", &username], account))?;
        }
        Command::Unblock { username } => {
            let account = config.username.as_deref();
            xurl_write(cache_only, "unblock", || xurl_write_call(&["unblock", &username], account))?;
        }
        Command::Mute { username } => {
            let account = config.username.as_deref();
            xurl_write(cache_only, "mute", || xurl_write_call(&["mute", &username], account))?;
        }
        Command::Unmute { username } => {
            let account = config.username.as_deref();
            xurl_write(cache_only, "unmute", || xurl_write_call(&["unmute", &username], account))?;
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

fn main() -> ExitCode {
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

    // Fail-fast if xurl is not installed
    if let Err(e) = transport::resolve_xurl_path() {
        let err = BirdError::Config(e);
        err.print(use_color);
        return ExitCode::from(err.exit_code());
    }

    let overrides = ArgOverrides {
        username: cli.account.or_else(|| std::env::var("X_API_USERNAME").ok()),
    };

    let config = match ResolvedConfig::load(overrides) {
        Ok(c) => c,
        Err(e) => {
            let err = BirdError::Config(e);
            err.print(use_color);
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
    );

    match run(cli.command, config, &mut client, use_color, cli.cache_only) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            e.print(use_color);
            ExitCode::from(e.exit_code())
        }
    }
}
