//! CLI argument definitions (clap derive structs and enums).
//!
//! Pure data structures with no runtime behavior. Command dispatch lives in main.rs.

use crate::output::{ColorMode, OutputFormat};
use crate::skill_install::Host;
use clap::Parser;

/// Default reqwest/xurl timeout in seconds when `--timeout` is not provided.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Parser, Debug)]
#[command(name = "bird", about = "X API CLI", version)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Username for multi-user token selection (maps to xurl -u).
    #[arg(long, short = 'u', global = true)]
    pub username: Option<String>,

    /// Output format (text, json, jsonl, ndjson). Defaults to json when piped.
    #[arg(long, short = 'o', global = true, value_enum, env = "BIRD_OUTPUT")]
    pub output: Option<OutputFormat>,

    /// Shorthand for `--output json`.
    #[arg(long, global = true, conflicts_with_all = ["output", "jsonl"])]
    pub json: bool,

    /// Shorthand for `--output jsonl`.
    #[arg(long, global = true, conflicts_with = "output")]
    pub jsonl: bool,

    /// Color mode: auto (default), always, never.
    #[arg(
        long,
        global = true,
        value_enum,
        default_value = "auto",
        env = "BIRD_COLOR"
    )]
    pub color: ColorMode,

    /// Deprecated alias for `--color never` (plain output, no color).
    #[arg(long, global = true, hide = true)]
    pub plain: bool,

    /// Deprecated alias for `--color never`.
    #[arg(long, global = true, hide = true)]
    pub no_color: bool,

    /// Suppress informational stderr output (keep only fatal errors).
    #[arg(
        long,
        short = 'q',
        global = true,
        env = "BIRD_QUIET",
        value_parser = clap::builder::FalseyValueParser::new(),
    )]
    pub quiet: bool,

    /// Increase verbosity (repeatable: -v info, -vv debug, -vvv trace).
    #[arg(
        long,
        short = 'v',
        global = true,
        action = clap::ArgAction::Count,
        env = "BIRD_VERBOSE",
    )]
    pub verbose: u8,

    /// Network timeout in seconds (default 30). Applies to xurl subprocesses.
    #[arg(long, global = true, env = "BIRD_TIMEOUT", default_value_t = DEFAULT_TIMEOUT_SECS)]
    pub timeout: u64,

    /// Disable interactive prompts (refuse anything that would block on stdin).
    #[arg(long, global = true, env = "BIRD_NO_INTERACTIVE")]
    pub no_interactive: bool,

    /// Emit pipe-safe, undecorated text. Ignored in JSON modes.
    #[arg(long, global = true)]
    pub raw: bool,

    /// Print curated examples block and exit.
    #[arg(long, global = true)]
    pub examples: bool,

    /// Bypass store read, still write response to store.
    #[arg(long, global = true)]
    pub refresh: bool,

    /// Disable entity store entirely (no read, no write).
    #[arg(long, global = true)]
    pub no_cache: bool,

    /// Only serve from local store; never make API requests.
    #[arg(long, global = true)]
    pub cache_only: bool,
}

#[derive(clap::Subcommand, Debug)]
pub(crate) enum Command {
    /// Authenticate via xurl (OAuth2 PKCE browser flow).
    Login {
        #[command(flatten)]
        headless: crate::login::HeadlessAuthArgs,
    },

    /// Show current user (GET /2/users/me).
    Me {
        /// Human-readable output.
        #[arg(long)]
        pretty: bool,
    },

    /// GET request to path (e.g. /2/users/me or /2/users/{id}/bookmarks with -p id=123).
    Get {
        path: String,
        #[arg(long, short = 'p', value_name = "KEY=VALUE", num_args = 1..)]
        param: Vec<String>,
        #[arg(long, value_name = "KEY=VALUE", num_args = 1..)]
        query: Vec<String>,
        #[arg(long)]
        pretty: bool,
    },

    /// POST request to path.
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

    /// PUT request to path.
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

    /// List bookmarks for the current user (paginated, max_results=100).
    Bookmarks {
        #[arg(long)]
        pretty: bool,
    },

    /// Look up a user profile by username.
    Profile {
        /// X/Twitter username (with or without @).
        username: String,
        /// Pretty-print JSON output.
        #[arg(long)]
        pretty: bool,
    },

    /// Search recent tweets (GET /2/tweets/search/recent).
    Search {
        /// Search query (X API search syntax).
        query: String,

        /// Pretty-print JSON output.
        #[arg(long)]
        pretty: bool,

        /// Sort results: recent (default), likes.
        #[arg(long, default_value = "recent")]
        sort: String,

        /// Minimum like count threshold.
        #[arg(long)]
        min_likes: Option<u64>,

        /// Maximum results per page (10-100, default: 100).
        #[arg(long)]
        max_results: Option<u32>,

        /// Number of pages to fetch (1-10, default: 1).
        #[arg(long)]
        pages: Option<u32>,
    },

    /// Reconstruct a conversation thread from a tweet.
    Thread {
        /// Tweet ID (root tweet or any reply in the thread).
        tweet_id: String,
        /// Pretty-print JSON output.
        #[arg(long)]
        pretty: bool,
        /// Maximum number of search result pages (default: 10, max: 25).
        #[arg(long, default_value = "10")]
        max_pages: u32,
    },

    /// DELETE request to path.
    Delete {
        path: String,
        #[arg(long, short = 'p', value_name = "KEY=VALUE", num_args = 1..)]
        param: Vec<String>,
        #[arg(long, value_name = "KEY=VALUE", num_args = 1..)]
        query: Vec<String>,
        #[arg(long)]
        pretty: bool,
    },

    /// Monitor users: check recent activity, manage watchlist.
    Watchlist {
        #[command(subcommand)]
        action: WatchlistCommand,
        /// Pretty-print JSON output.
        #[arg(long)]
        pretty: bool,
    },

    /// View API usage and costs.
    Usage {
        /// Show usage since this date (YYYY-MM-DD; default: 30 days ago).
        #[arg(long)]
        since: Option<String>,
        /// Show only local estimates (skip API).
        #[arg(long)]
        local: bool,
        /// Pretty-print output.
        #[arg(long)]
        pretty: bool,
    },

    /// Post a tweet (via xurl).
    Tweet {
        /// Tweet text.
        text: String,
        /// Media ID to attach.
        #[arg(long)]
        media_id: Option<String>,
    },

    /// Reply to a tweet (via xurl).
    Reply {
        /// Tweet ID to reply to.
        tweet_id: String,
        /// Reply text.
        text: String,
    },

    /// Like a tweet (via xurl).
    Like {
        /// Tweet ID to like.
        tweet_id: String,
    },

    /// Unlike a tweet (via xurl).
    Unlike {
        /// Tweet ID to unlike.
        tweet_id: String,
    },

    /// Repost (retweet) a tweet (via xurl).
    Repost {
        /// Tweet ID to repost.
        tweet_id: String,
    },

    /// Undo a repost (via xurl).
    Unrepost {
        /// Tweet ID to unrepost.
        tweet_id: String,
    },

    /// Follow a user (via xurl).
    Follow {
        /// Username to follow.
        username: String,
    },

    /// Unfollow a user (via xurl).
    Unfollow {
        /// Username to unfollow.
        username: String,
    },

    /// Send a direct message (via xurl).
    Dm {
        /// Username to message.
        username: String,
        /// Message text.
        text: String,
    },

    /// Block a user (via xurl).
    Block {
        /// Username to block.
        username: String,
    },

    /// Unblock a user (via xurl).
    Unblock {
        /// Username to unblock.
        username: String,
    },

    /// Mute a user (via xurl).
    Mute {
        /// Username to mute.
        username: String,
    },

    /// Unmute a user (via xurl).
    Unmute {
        /// Username to unmute.
        username: String,
    },

    /// Show what is available: xurl status, commands, and entity store health.
    Doctor {
        /// Scope report to this command only (e.g. me, bookmarks, get).
        command: Option<String>,
        #[arg(long)]
        pretty: bool,
    },

    /// Manage the HTTP response cache.
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },

    /// Generate shell completions.
    Completions {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Manage the bird agent-skill bundle (install for Claude Code, etc.)
    #[command(after_help = "Examples:
  bird skill install
  bird skill install --host claude-code --dry-run
  bird skill install --all")]
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },

    /// Print a JSON Schema document for one of bird's output shapes.
    #[command(after_help = "Examples:
  bird schema
  bird schema --list
  bird schema bookmarks
  bird schema bookmarks --output json")]
    Schema {
        /// Schema name to print. Omit to print the universal success envelope.
        name: Option<String>,
        /// List all available schema names instead of printing a schema.
        #[arg(long)]
        list: bool,
    },
}

#[derive(clap::Subcommand, Debug, Clone, Copy)]
pub(crate) enum SkillAction {
    /// Install the bird skill bundle into a host's canonical skills directory
    Install {
        /// Target host (default: claude-code). Mutually exclusive with --all
        #[arg(long, value_enum, conflicts_with = "all")]
        host: Option<Host>,

        /// Install into every supported host in one invocation
        #[arg(long)]
        all: bool,

        /// Print the planned destination without writing
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(clap::Subcommand, Debug)]
pub(crate) enum CacheAction {
    /// Delete all cache entries.
    Clear,
    /// Show cache status (JSON default, --pretty for human-readable).
    Stats {
        #[arg(long)]
        pretty: bool,
    },
}

#[derive(clap::Subcommand, Debug)]
pub(crate) enum WatchlistCommand {
    /// Check recent activity for all watched users.
    Check,
    /// Add a user to the watchlist.
    Add {
        /// X/Twitter username (with or without @).
        username: String,
    },
    /// Remove a user from the watchlist.
    Remove {
        /// X/Twitter username to remove.
        username: String,
    },
    /// Show the current watchlist.
    List,
}

impl Cli {
    /// Resolve the effective color mode honoring deprecated `--plain` and `--no-color` aliases.
    pub fn effective_color(&self) -> ColorMode {
        if self.plain || self.no_color {
            ColorMode::Never
        } else {
            self.color
        }
    }

    /// Resolve the effective output format honoring `--json` / `--jsonl` shorthand flags.
    pub fn effective_output(&self) -> Option<OutputFormat> {
        if self.json {
            Some(OutputFormat::Json)
        } else if self.jsonl {
            Some(OutputFormat::Jsonl)
        } else {
            self.output
        }
    }
}
