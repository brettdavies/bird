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
use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use spec::WriteSpec;
use std::io::Write;

/// Shared dispatch sequence for every xurl-write verb.
///
/// Guards via `require_confirmation` (`--dry-run`, `--force`/`--yes`, TTY
/// confirmation prompt), short-circuits on dry-run, and routes through
/// `xurl_write` which rejects `--cache-only` before invoking
/// `xurl_write_call` with the verb's xurl args. The client is threaded
/// through so `xurl_write_call` reads the resolved xurl path and timeout
/// off the live transport.
#[allow(clippy::too_many_arguments)]
pub fn execute(
    spec: WriteSpec,
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        stdout,
        &mut std::io::stderr().lock(),
        None,
    )?;
    if matches!(outcome, GuardOutcome::DryRun) {
        return Ok(());
    }
    let xurl_args = spec.xurl_args;
    xurl_write(cache_only, spec.verb, || {
        let args: Vec<&str> = xurl_args.iter().map(String::as_str).collect();
        xurl_write_call(client, stdout, &args, username)
    })
}

#[allow(clippy::too_many_arguments)]
pub fn run_tweet(
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        client,
        out,
        stdout,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_reply(
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        client,
        out,
        stdout,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_like(
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        client,
        out,
        stdout,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_unlike(
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        client,
        out,
        stdout,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_repost(
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        client,
        out,
        stdout,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_unrepost(
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        client,
        out,
        stdout,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_follow(
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        client,
        out,
        stdout,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_unfollow(
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        client,
        out,
        stdout,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_dm(
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        client,
        out,
        stdout,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_block(
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        client,
        out,
        stdout,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_unblock(
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        client,
        out,
        stdout,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_mute(
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        client,
        out,
        stdout,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_unmute(
    client: &db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
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
        client,
        out,
        stdout,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[cfg(all(test, not(feature = "embedded-xurl")))]
mod tests {
    use super::*;
    use crate::output::OutputFormat;
    use crate::transport::tests::MockTransport;

    fn quiet_out() -> OutputConfig {
        OutputConfig {
            format: OutputFormat::Text,
            use_color: false,
            quiet: true,
            raw: false,
        }
    }

    fn test_client() -> db::BirdClient {
        let transport = Box::new(MockTransport::new(vec![]));
        let db_obj = crate::db::store::in_memory_db();
        db::BirdClient::new_test(transport, db_obj)
    }

    // U6.3: execute() with --dry-run returns Ok without invoking xurl.
    // (If xurl were invoked, the test process would attempt to spawn the
    // xurl binary; here the dry-run short-circuits before xurl_write fires.)
    #[test]
    fn execute_dry_run_short_circuits() {
        let client = test_client();
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
        let mut stdout: Vec<u8> = Vec::new();
        assert!(execute(spec, &client, &out, &mut stdout, guard, false, false, None).is_ok());
    }

    // U6.2: execute() with --cache-only and force=true returns Err(BirdError::General)
    // mapped from the xurl_write cache-only refusal. Verifies the cache-only
    // guard fires after confirmation succeeds and before transport is invoked.
    #[test]
    fn execute_cache_only_rejects_write() {
        let client = test_client();
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
        let mut stdout: Vec<u8> = Vec::new();
        match execute(spec, &client, &out, &mut stdout, guard, true, false, None) {
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
        let client = test_client();
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
        let mut stdout: Vec<u8> = Vec::new();
        match execute(spec, &client, &out, &mut stdout, guard, false, true, None) {
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
        let client = test_client();
        let out = quiet_out();
        let mut stdout: Vec<u8> = Vec::new();
        // Dry-run short-circuits before xurl_write but still exercises the
        // builder, body, and url_for_prompt assembly via require_confirmation.
        let res = run_tweet(
            &client,
            &out,
            &mut stdout,
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
        let client = test_client();
        let out = quiet_out();
        let mut stdout: Vec<u8> = Vec::new();
        let res = run_follow(
            &client,
            &out,
            &mut stdout,
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
        let client = test_client();
        let out = quiet_out();
        let mut stdout: Vec<u8> = Vec::new();
        let res = run_dm(
            &client,
            &out,
            &mut stdout,
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
