//! CLI argument definitions (clap derive structs and enums).
//!
//! Pure data structures with no runtime behavior. Command dispatch lives in main.rs.

pub mod argv;
pub mod clap_errors;
pub mod commands;
pub mod dispatch;
pub mod runner;

pub use runner::{run, run_argv, run_with_paths};

use crate::output::{ColorMode, OutputFormat};
use crate::skill_install::Host;
use clap::{Args, Parser};

/// Default reqwest/xurl timeout in seconds when `--timeout` is not provided.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Parser, Debug)]
#[command(
    name = "bird",
    about = "X API CLI",
    version,
    after_help = include_str!("../../examples/top-level.txt")
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Username for multi-user token selection (maps to xurl -u).
    #[arg(long, short = 'u', global = true)]
    pub username: Option<String>,

    /// Output format (text, json, jsonl, ndjson). Defaults to json when piped.
    #[arg(long, short = 'o', global = true, value_enum, env = "BIRD_OUTPUT")]
    pub output: Option<OutputFormat>,

    /// Shorthand for `--output json`.
    #[arg(
        long,
        global = true,
        conflicts_with_all = ["output", "jsonl"],
        env = "BIRD_JSON",
        value_parser = clap::builder::FalseyValueParser::new(),
    )]
    pub json: bool,

    /// Shorthand for `--output jsonl`.
    #[arg(
        long,
        global = true,
        conflicts_with = "output",
        env = "BIRD_JSONL",
        value_parser = clap::builder::FalseyValueParser::new(),
    )]
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

    /// Maximum number of results to return on list-style commands (default 100, ceiling 1000).
    #[arg(long, global = true, value_name = "N")]
    pub limit: Option<u32>,

    /// Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`).
    #[arg(long, global = true, value_name = "TOKEN", alias = "page")]
    pub cursor: Option<String>,
}

/// Confirmation + dry-run guard shared by every mutating subcommand.
///
/// `--force` / `--yes` are aliases (both accepted; `-f` short form binds to
/// `force`). `--dry-run` short-circuits before any HTTP call, prints the
/// would-be request, and exits 0.
#[derive(Args, Debug, Clone, Copy, Default)]
pub struct WriteGuard {
    /// Skip the interactive confirmation prompt (alias: --yes).
    #[arg(long, short = 'f', alias = "yes", global = false)]
    pub force: bool,

    /// Validate inputs and print the would-be request, then exit without calling the API.
    #[arg(long, global = false)]
    pub dry_run: bool,
}

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Authenticate via xurl (OAuth2 PKCE browser flow).
    #[command(after_help = include_str!("../../examples/login.txt"))]
    Login {
        #[command(flatten)]
        headless: crate::login::HeadlessAuthArgs,
    },

    /// Show current user (GET /2/users/me).
    #[command(after_help = include_str!("../../examples/me.txt"))]
    Me {
        /// Human-readable output.
        #[arg(long)]
        pretty: bool,
    },

    /// GET request to path (e.g. /2/users/me or /2/users/{id}/bookmarks with -p id=123).
    #[command(after_help = include_str!("../../examples/get.txt"))]
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
    #[command(after_help = include_str!("../../examples/post.txt"))]
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
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// PUT request to path.
    #[command(after_help = include_str!("../../examples/put.txt"))]
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
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// List bookmarks for the current user (paginated, max_results=100).
    #[command(after_help = include_str!("../../examples/bookmarks.txt"))]
    Bookmarks {
        #[arg(long)]
        pretty: bool,
    },

    /// Look up a user profile by username.
    #[command(after_help = include_str!("../../examples/profile.txt"))]
    Profile {
        /// X/Twitter username (with or without @).
        username: String,
        /// Pretty-print JSON output.
        #[arg(long)]
        pretty: bool,
    },

    /// Search recent tweets (GET /2/tweets/search/recent).
    #[command(after_help = include_str!("../../examples/search.txt"))]
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
    #[command(after_help = include_str!("../../examples/thread.txt"))]
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
    #[command(after_help = include_str!("../../examples/delete.txt"))]
    Delete {
        path: String,
        #[arg(long, short = 'p', value_name = "KEY=VALUE", num_args = 1..)]
        param: Vec<String>,
        #[arg(long, value_name = "KEY=VALUE", num_args = 1..)]
        query: Vec<String>,
        #[arg(long)]
        pretty: bool,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Monitor users: check recent activity, manage watchlist.
    #[command(after_help = include_str!("../../examples/watchlist.txt"))]
    Watchlist {
        #[command(subcommand)]
        action: WatchlistCommand,
        /// Pretty-print JSON output.
        #[arg(long)]
        pretty: bool,
    },

    /// View API usage and costs.
    #[command(after_help = include_str!("../../examples/usage.txt"))]
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
    #[command(after_help = include_str!("../../examples/tweet.txt"))]
    Tweet {
        /// Tweet text.
        text: String,
        /// Media ID to attach.
        #[arg(long)]
        media_id: Option<String>,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Reply to a tweet (via xurl).
    #[command(after_help = include_str!("../../examples/reply.txt"))]
    Reply {
        /// Tweet ID to reply to.
        tweet_id: String,
        /// Reply text.
        text: String,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Like a tweet (via xurl).
    #[command(after_help = include_str!("../../examples/like.txt"))]
    Like {
        /// Tweet ID to like.
        tweet_id: String,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Unlike a tweet (via xurl).
    #[command(after_help = include_str!("../../examples/unlike.txt"))]
    Unlike {
        /// Tweet ID to unlike.
        tweet_id: String,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Repost (retweet) a tweet (via xurl).
    #[command(after_help = include_str!("../../examples/repost.txt"))]
    Repost {
        /// Tweet ID to repost.
        tweet_id: String,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Undo a repost (via xurl).
    #[command(after_help = include_str!("../../examples/unrepost.txt"))]
    Unrepost {
        /// Tweet ID to unrepost.
        tweet_id: String,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Follow a user (via xurl).
    #[command(after_help = include_str!("../../examples/follow.txt"))]
    Follow {
        /// Username to follow.
        username: String,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Unfollow a user (via xurl).
    #[command(after_help = include_str!("../../examples/unfollow.txt"))]
    Unfollow {
        /// Username to unfollow.
        username: String,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Send a direct message (via xurl).
    #[command(after_help = include_str!("../../examples/dm.txt"))]
    Dm {
        /// Username to message.
        username: String,
        /// Message text.
        text: String,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Block a user (via xurl).
    #[command(after_help = include_str!("../../examples/block.txt"))]
    Block {
        /// Username to block.
        username: String,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Unblock a user (via xurl).
    #[command(after_help = include_str!("../../examples/unblock.txt"))]
    Unblock {
        /// Username to unblock.
        username: String,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Mute a user (via xurl).
    #[command(after_help = include_str!("../../examples/mute.txt"))]
    Mute {
        /// Username to mute.
        username: String,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Unmute a user (via xurl).
    #[command(after_help = include_str!("../../examples/unmute.txt"))]
    Unmute {
        /// Username to unmute.
        username: String,
        #[command(flatten)]
        guard: WriteGuard,
    },

    /// Show what is available: xurl status, commands, and entity store health.
    #[command(after_help = include_str!("../../examples/doctor.txt"))]
    Doctor {
        /// Scope report to this command only (e.g. me, bookmarks, get).
        command: Option<String>,
        #[arg(long)]
        pretty: bool,
    },

    /// Manage the HTTP response cache.
    #[command(after_help = include_str!("../../examples/cache.txt"))]
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },

    /// Generate shell completions.
    #[command(after_help = include_str!("../../examples/completions.txt"))]
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
pub enum SkillAction {
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
    /// Update the installed bird skill bundle to the embedded version
    #[command(alias = "upgrade")]
    Update {
        /// Target host (default: claude-code). Mutually exclusive with --all
        #[arg(long, value_enum, conflicts_with = "all")]
        host: Option<Host>,

        /// Update every supported host in one invocation
        #[arg(long)]
        all: bool,

        /// Print the planned destination without writing
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum CacheAction {
    /// Delete all cache entries.
    #[command(after_help = include_str!("../../examples/cache-clear.txt"))]
    Clear {
        #[command(flatten)]
        guard: WriteGuard,
    },
    /// Show cache status (JSON default, --pretty for human-readable).
    #[command(after_help = include_str!("../../examples/cache-stats.txt"))]
    Stats {
        #[arg(long)]
        pretty: bool,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum WatchlistCommand {
    /// Fetch recent activity for all watched users.
    #[command(
        alias = "check",
        after_help = include_str!("../../examples/watchlist-check.txt"),
    )]
    Fetch,
    /// Add a user to the watchlist.
    #[command(after_help = include_str!("../../examples/watchlist-add.txt"))]
    Add {
        /// X/Twitter username (with or without @).
        username: String,
    },
    /// Remove a user from the watchlist.
    #[command(after_help = include_str!("../../examples/watchlist-remove.txt"))]
    Remove {
        /// X/Twitter username to remove.
        username: String,
        #[command(flatten)]
        guard: WriteGuard,
    },
    /// Show the current watchlist.
    #[command(after_help = include_str!("../../examples/watchlist-list.txt"))]
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
