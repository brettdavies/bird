//! `bird tweet|reply|like|unlike|repost|unrepost|follow|unfollow|dm|block|unblock|mute|unmute`
//!
//! 13 xurl-write subcommands. Each verb has its own `pub fn run_<verb>` here
//! (one file rather than 13 tiny submodules — collapsed into a shared
//! `execute()` helper in U6).

use crate::cli::WriteGuard;
use crate::cli::dispatch::{GuardOutcome, require_confirmation, xurl_write, xurl_write_call};
use crate::error::BirdError;
use crate::output::OutputConfig;

pub fn run_tweet(
    out: &OutputConfig,
    text: String,
    media_id: Option<String>,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
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
            &mut std::io::stderr().lock(),
            None,
        )?,
        GuardOutcome::DryRun
    ) {
        return Ok(());
    }
    let username = config_username;
    xurl_write(cache_only, "tweet", || {
        let mut args = vec!["post", &text];
        let media_owned;
        if let Some(ref id) = media_id {
            media_owned = id.clone();
            args.extend(["--media-id", &media_owned]);
        }
        xurl_write_call(&args, username)
    })
}

pub fn run_reply(
    out: &OutputConfig,
    tweet_id: String,
    text: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
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
            &mut std::io::stderr().lock(),
            None,
        )?,
        GuardOutcome::DryRun
    ) {
        return Ok(());
    }
    let username = config_username;
    xurl_write(cache_only, "reply", || {
        xurl_write_call(&["reply", &tweet_id, &text], username)
    })
}

pub fn run_like(
    out: &OutputConfig,
    tweet_id: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    if matches!(
        require_confirmation(
            "like",
            "POST",
            &format!("https://api.x.com/2/users/me/likes/{}", tweet_id),
            None,
            guard,
            out,
            no_interactive,
            &mut std::io::stderr().lock(),
            None,
        )?,
        GuardOutcome::DryRun
    ) {
        return Ok(());
    }
    let username = config_username;
    xurl_write(cache_only, "like", || {
        xurl_write_call(&["like", &tweet_id], username)
    })
}

pub fn run_unlike(
    out: &OutputConfig,
    tweet_id: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    if matches!(
        require_confirmation(
            "unlike",
            "DELETE",
            &format!("https://api.x.com/2/users/me/likes/{}", tweet_id),
            None,
            guard,
            out,
            no_interactive,
            &mut std::io::stderr().lock(),
            None,
        )?,
        GuardOutcome::DryRun
    ) {
        return Ok(());
    }
    let username = config_username;
    xurl_write(cache_only, "unlike", || {
        xurl_write_call(&["unlike", &tweet_id], username)
    })
}

pub fn run_repost(
    out: &OutputConfig,
    tweet_id: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    if matches!(
        require_confirmation(
            "repost",
            "POST",
            &format!("https://api.x.com/2/users/me/retweets/{}", tweet_id),
            None,
            guard,
            out,
            no_interactive,
            &mut std::io::stderr().lock(),
            None,
        )?,
        GuardOutcome::DryRun
    ) {
        return Ok(());
    }
    let username = config_username;
    xurl_write(cache_only, "repost", || {
        xurl_write_call(&["repost", &tweet_id], username)
    })
}

pub fn run_unrepost(
    out: &OutputConfig,
    tweet_id: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    if matches!(
        require_confirmation(
            "unrepost",
            "DELETE",
            &format!("https://api.x.com/2/users/me/retweets/{}", tweet_id),
            None,
            guard,
            out,
            no_interactive,
            &mut std::io::stderr().lock(),
            None,
        )?,
        GuardOutcome::DryRun
    ) {
        return Ok(());
    }
    let username = config_username;
    xurl_write(cache_only, "unrepost", || {
        xurl_write_call(&["unrepost", &tweet_id], username)
    })
}

pub fn run_follow(
    out: &OutputConfig,
    target: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
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
            &mut std::io::stderr().lock(),
            None,
        )?,
        GuardOutcome::DryRun
    ) {
        return Ok(());
    }
    let username = config_username;
    xurl_write(cache_only, "follow", || {
        xurl_write_call(&["follow", &target], username)
    })
}

pub fn run_unfollow(
    out: &OutputConfig,
    target: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
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
            &mut std::io::stderr().lock(),
            None,
        )?,
        GuardOutcome::DryRun
    ) {
        return Ok(());
    }
    let username = config_username;
    xurl_write(cache_only, "unfollow", || {
        xurl_write_call(&["unfollow", &target], username)
    })
}

pub fn run_dm(
    out: &OutputConfig,
    target: String,
    text: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
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
            &mut std::io::stderr().lock(),
            None,
        )?,
        GuardOutcome::DryRun
    ) {
        return Ok(());
    }
    let username = config_username;
    xurl_write(cache_only, "dm", || {
        xurl_write_call(&["dm", &target, &text], username)
    })
}

pub fn run_block(
    out: &OutputConfig,
    target: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    if matches!(
        require_confirmation(
            "block",
            "POST",
            &format!("https://api.x.com/2/users/me/blocking (target=@{})", target),
            None,
            guard,
            out,
            no_interactive,
            &mut std::io::stderr().lock(),
            None,
        )?,
        GuardOutcome::DryRun
    ) {
        return Ok(());
    }
    let username = config_username;
    xurl_write(cache_only, "block", || {
        xurl_write_call(&["block", &target], username)
    })
}

pub fn run_unblock(
    out: &OutputConfig,
    target: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    if matches!(
        require_confirmation(
            "unblock",
            "DELETE",
            &format!("https://api.x.com/2/users/me/blocking (target=@{})", target),
            None,
            guard,
            out,
            no_interactive,
            &mut std::io::stderr().lock(),
            None,
        )?,
        GuardOutcome::DryRun
    ) {
        return Ok(());
    }
    let username = config_username;
    xurl_write(cache_only, "unblock", || {
        xurl_write_call(&["unblock", &target], username)
    })
}

pub fn run_mute(
    out: &OutputConfig,
    target: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    if matches!(
        require_confirmation(
            "mute",
            "POST",
            &format!("https://api.x.com/2/users/me/muting (target=@{})", target),
            None,
            guard,
            out,
            no_interactive,
            &mut std::io::stderr().lock(),
            None,
        )?,
        GuardOutcome::DryRun
    ) {
        return Ok(());
    }
    let username = config_username;
    xurl_write(cache_only, "mute", || {
        xurl_write_call(&["mute", &target], username)
    })
}

pub fn run_unmute(
    out: &OutputConfig,
    target: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    if matches!(
        require_confirmation(
            "unmute",
            "DELETE",
            &format!("https://api.x.com/2/users/me/muting (target=@{})", target),
            None,
            guard,
            out,
            no_interactive,
            &mut std::io::stderr().lock(),
            None,
        )?,
        GuardOutcome::DryRun
    ) {
        return Ok(());
    }
    let username = config_username;
    xurl_write(cache_only, "unmute", || {
        xurl_write_call(&["unmute", &target], username)
    })
}
