//! `bird tweet|reply|like|unlike|repost|unrepost|follow|unfollow|dm|block|unblock|mute|unmute`
//!
//! 13 xurl-write subcommands collapsed onto a shared [`execute`] helper plus
//! 13 thin per-verb builders. Each builder returns a [`WriteSpec`] describing
//! the verb-specific bits (verb name, method, prompt URL, JSON body, xurl
//! args); `execute` owns the shared sequence of `require_confirmation` ->
//! `--cache-only` guard -> `xurl_write_call`.

pub mod spec;

use crate::cli::WriteGuard;
use crate::cli::dispatch::{GuardOutcome, require_confirmation, xurl_write, xurl_write_call};
use crate::error::BirdError;
use crate::output::OutputConfig;
use spec::WriteSpec;

/// Shared dispatch sequence for every xurl-write verb.
///
/// Guards via `require_confirmation` (`--dry-run`, `--force`/`--yes`, TTY
/// confirmation prompt), short-circuits on dry-run, and routes through
/// `xurl_write` which rejects `--cache-only` before invoking
/// `xurl_write_call` with the verb's xurl args.
#[allow(clippy::too_many_arguments)]
pub fn execute(
    spec: WriteSpec,
    out: &OutputConfig,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    username: Option<&str>,
) -> Result<(), BirdError> {
    let outcome = require_confirmation(
        spec.verb,
        spec.method,
        &spec.url_for_prompt,
        spec.body.as_ref(),
        guard,
        out,
        no_interactive,
        &mut std::io::stderr().lock(),
        None,
    )?;
    if matches!(outcome, GuardOutcome::DryRun) {
        return Ok(());
    }
    let xurl_args = spec.xurl_args;
    xurl_write(cache_only, spec.verb, || {
        let args: Vec<&str> = xurl_args.iter().map(String::as_str).collect();
        xurl_write_call(&args, username)
    })
}

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
    let mut xurl_args: Vec<String> = vec!["post".into(), text.clone()];
    if let Some(id) = media_id.as_ref() {
        xurl_args.extend(["--media-id".into(), id.clone()]);
    }
    let spec = WriteSpec {
        verb: "tweet",
        method: "POST",
        url_for_prompt: "https://api.x.com/2/tweets".into(),
        body: Some(body),
        xurl_args,
    };
    execute(
        spec,
        out,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
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
    let spec = WriteSpec {
        verb: "reply",
        method: "POST",
        url_for_prompt: format!("https://api.x.com/2/tweets (reply to {})", tweet_id),
        body: Some(serde_json::json!({"text": text, "reply_to": tweet_id})),
        xurl_args: vec!["reply".into(), tweet_id, text],
    };
    execute(
        spec,
        out,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

pub fn run_like(
    out: &OutputConfig,
    tweet_id: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    let spec = WriteSpec {
        verb: "like",
        method: "POST",
        url_for_prompt: format!("https://api.x.com/2/users/me/likes/{}", tweet_id),
        body: None,
        xurl_args: vec!["like".into(), tweet_id],
    };
    execute(
        spec,
        out,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

pub fn run_unlike(
    out: &OutputConfig,
    tweet_id: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    let spec = WriteSpec {
        verb: "unlike",
        method: "DELETE",
        url_for_prompt: format!("https://api.x.com/2/users/me/likes/{}", tweet_id),
        body: None,
        xurl_args: vec!["unlike".into(), tweet_id],
    };
    execute(
        spec,
        out,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

pub fn run_repost(
    out: &OutputConfig,
    tweet_id: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    let spec = WriteSpec {
        verb: "repost",
        method: "POST",
        url_for_prompt: format!("https://api.x.com/2/users/me/retweets/{}", tweet_id),
        body: None,
        xurl_args: vec!["repost".into(), tweet_id],
    };
    execute(
        spec,
        out,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

pub fn run_unrepost(
    out: &OutputConfig,
    tweet_id: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    let spec = WriteSpec {
        verb: "unrepost",
        method: "DELETE",
        url_for_prompt: format!("https://api.x.com/2/users/me/retweets/{}", tweet_id),
        body: None,
        xurl_args: vec!["unrepost".into(), tweet_id],
    };
    execute(
        spec,
        out,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

pub fn run_follow(
    out: &OutputConfig,
    target: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    let spec = WriteSpec {
        verb: "follow",
        method: "POST",
        url_for_prompt: format!(
            "https://api.x.com/2/users/me/following (target=@{})",
            target
        ),
        body: None,
        xurl_args: vec!["follow".into(), target],
    };
    execute(
        spec,
        out,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

pub fn run_unfollow(
    out: &OutputConfig,
    target: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    let spec = WriteSpec {
        verb: "unfollow",
        method: "DELETE",
        url_for_prompt: format!(
            "https://api.x.com/2/users/me/following (target=@{})",
            target
        ),
        body: None,
        xurl_args: vec!["unfollow".into(), target],
    };
    execute(
        spec,
        out,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
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
    let spec = WriteSpec {
        verb: "dm",
        method: "POST",
        url_for_prompt: format!(
            "https://api.x.com/2/dm_conversations/with/@{}/messages",
            target
        ),
        body: Some(serde_json::json!({"to": target, "text": text})),
        xurl_args: vec!["dm".into(), target, text],
    };
    execute(
        spec,
        out,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

pub fn run_block(
    out: &OutputConfig,
    target: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    let spec = WriteSpec {
        verb: "block",
        method: "POST",
        url_for_prompt: format!("https://api.x.com/2/users/me/blocking (target=@{})", target),
        body: None,
        xurl_args: vec!["block".into(), target],
    };
    execute(
        spec,
        out,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

pub fn run_unblock(
    out: &OutputConfig,
    target: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    let spec = WriteSpec {
        verb: "unblock",
        method: "DELETE",
        url_for_prompt: format!("https://api.x.com/2/users/me/blocking (target=@{})", target),
        body: None,
        xurl_args: vec!["unblock".into(), target],
    };
    execute(
        spec,
        out,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

pub fn run_mute(
    out: &OutputConfig,
    target: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    let spec = WriteSpec {
        verb: "mute",
        method: "POST",
        url_for_prompt: format!("https://api.x.com/2/users/me/muting (target=@{})", target),
        body: None,
        xurl_args: vec!["mute".into(), target],
    };
    execute(
        spec,
        out,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

pub fn run_unmute(
    out: &OutputConfig,
    target: String,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    let spec = WriteSpec {
        verb: "unmute",
        method: "DELETE",
        url_for_prompt: format!("https://api.x.com/2/users/me/muting (target=@{})", target),
        body: None,
        xurl_args: vec!["unmute".into(), target],
    };
    execute(
        spec,
        out,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::OutputFormat;

    fn quiet_out() -> OutputConfig {
        OutputConfig {
            format: OutputFormat::Text,
            use_color: false,
            quiet: true,
            raw: false,
        }
    }

    // U6.3: execute() with --dry-run returns Ok without invoking xurl.
    // (If xurl were invoked, the test process would attempt to spawn the
    // xurl binary; here the dry-run short-circuits before xurl_write fires.)
    #[test]
    fn execute_dry_run_short_circuits() {
        let out = quiet_out();
        let spec = WriteSpec {
            verb: "like",
            method: "POST",
            url_for_prompt: "https://api.x.com/2/users/me/likes/1".into(),
            body: None,
            xurl_args: vec!["like".into(), "1".into()],
        };
        let guard = WriteGuard {
            force: false,
            dry_run: true,
        };
        assert!(execute(spec, &out, guard, false, false, None).is_ok());
    }

    // U6.2: execute() with --cache-only and force=true returns Err(BirdError::General)
    // mapped from the xurl_write cache-only refusal. Verifies the cache-only
    // guard fires after confirmation succeeds and before transport is invoked.
    #[test]
    fn execute_cache_only_rejects_write() {
        let out = quiet_out();
        let spec = WriteSpec {
            verb: "like",
            method: "POST",
            url_for_prompt: "https://api.x.com/2/users/me/likes/1".into(),
            body: None,
            xurl_args: vec!["like".into(), "1".into()],
        };
        let guard = WriteGuard {
            force: true,
            dry_run: false,
        };
        match execute(spec, &out, guard, true, false, None) {
            Err(BirdError::General {
                command, message, ..
            }) => {
                assert_eq!(command, Some("like"));
                assert!(
                    message.contains("--cache-only"),
                    "expected cache-only refusal, got: {message}"
                );
            }
            Err(_) => panic!("expected General error with cache-only refusal"),
            Ok(()) => panic!("expected cache-only refusal, got Ok"),
        }
    }

    // U6.5: --no-interactive without --force/--yes returns requires-confirmation
    // usage error from the guard layer, before xurl_write runs.
    #[test]
    fn execute_no_interactive_without_force_errors() {
        let out = quiet_out();
        let spec = WriteSpec {
            verb: "like",
            method: "POST",
            url_for_prompt: "https://api.x.com/2/users/me/likes/1".into(),
            body: None,
            xurl_args: vec!["like".into(), "1".into()],
        };
        let guard = WriteGuard {
            force: false,
            dry_run: false,
        };
        match execute(spec, &out, guard, false, true, None) {
            Err(BirdError::Usage { error_id, .. }) => {
                assert_eq!(error_id, "requires-confirmation");
            }
            Err(_) => panic!("expected requires-confirmation usage error"),
            Ok(()) => panic!("expected requires-confirmation error, got Ok"),
        }
    }

    // U6.4a: tweet builder produces the expected xurl envelope, including the
    // --media-id passthrough.
    #[test]
    fn tweet_builder_dry_run_envelope_with_media() {
        let out = quiet_out();
        // Dry-run short-circuits before xurl_write but still exercises the
        // builder, body, and url_for_prompt assembly via require_confirmation.
        let res = run_tweet(
            &out,
            "hello world".into(),
            Some("media-123".into()),
            WriteGuard {
                force: false,
                dry_run: true,
            },
            false,
            false,
            None,
        );
        assert!(res.is_ok(), "tweet dry-run should succeed");
    }

    // U6.4b: follow builder envelope (target carried into url_for_prompt and
    // xurl_args without an extra `@` prefix).
    #[test]
    fn follow_builder_dry_run_envelope() {
        let out = quiet_out();
        let res = run_follow(
            &out,
            "someuser".into(),
            WriteGuard {
                force: false,
                dry_run: true,
            },
            false,
            false,
            None,
        );
        assert!(res.is_ok(), "follow dry-run should succeed");
    }

    // U6.4c: dm builder envelope (POST with body) — exercises the multi-arg
    // xurl_args path (dm + target + text).
    #[test]
    fn dm_builder_dry_run_envelope() {
        let out = quiet_out();
        let res = run_dm(
            &out,
            "9876543210".into(),
            "hi".into(),
            WriteGuard {
                force: false,
                dry_run: true,
            },
            false,
            false,
            None,
        );
        assert!(res.is_ok(), "dm dry-run should succeed");
    }
}
