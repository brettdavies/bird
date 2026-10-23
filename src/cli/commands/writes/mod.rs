//! `bird tweet|reply|like|unlike|repost|unrepost|follow|unfollow|dm|block|unblock|mute|unmute`
//!
//! 13 xurl-write subcommands collapsed onto a shared [`execute`] helper plus
//! 13 thin per-verb builders. Each builder returns a [`WriteSpec`] describing
//! the verb-specific bits (verb name, method, prompt URL, JSON body, embedded
//! call payload); `execute` owns the shared sequence of `require_confirmation`
//! -> `--cache-only` guard -> `execute_embedded_write`.

pub mod spec;

use crate::cli::WriteGuard;
use crate::cli::dispatch::{GuardOutcome, require_confirmation, xurl_write};
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::store;
use spec::WriteSpec;
use std::io::Write;

/// Shared dispatch sequence for every xurl-write verb.
///
/// Guards via `require_confirmation` (`--dry-run`, `--force`/`--yes`, TTY
/// confirmation prompt), short-circuits on dry-run, and routes through
/// `xurl_write` which rejects `--cache-only` before invoking
/// `execute_embedded_write` with the verb's typed payload.
#[allow(clippy::too_many_arguments)]
pub fn execute(
    spec: WriteSpec,
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
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
    if matches!(outcome, GuardOutcome::DryRun | GuardOutcome::Declined) {
        return Ok(());
    }
    let verb = spec.verb;
    let embedded_call = spec.embedded_call;
    xurl_write(cache_only, verb, move || {
        let auth = crate::cli::auth_scheme::AuthType::OAuth2User;
        let ctx = store::RequestContext {
            auth_type: &auth,
            username,
        };
        let outcome = client.execute_embedded_write(embedded_call, &ctx)?;
        for estimate in &outcome.costs {
            crate::cost::display_cost(out, stderr, estimate);
        }
        writeln!(stdout, "{}", serde_json::to_string(&outcome.json)?)?;
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub fn run_tweet(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    text: String,
    media_id: Option<String>,
    guard: WriteGuard,
    cache_only: bool,
    no_interactive: bool,
    config_username: Option<&str>,
) -> Result<(), BirdError> {
    let body = serde_json::json!({"text": text, "media_id": media_id});
    let spec = WriteSpec {
        verb: "tweet",
        method: "POST",
        url_for_prompt: "https://api.x.com/2/tweets".into(),
        body: Some(body),
        embedded_call: spec::EmbeddedWriteCall::TweetCreate { text, media_id },
    };
    execute(
        spec,
        client,
        out,
        stdout,
        stderr,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_reply(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
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
        embedded_call: spec::EmbeddedWriteCall::Reply {
            parent_id: tweet_id,
            text,
        },
    };
    execute(
        spec,
        client,
        out,
        stdout,
        stderr,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_like(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
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
        embedded_call: spec::EmbeddedWriteCall::Like { tweet_id },
    };
    execute(
        spec,
        client,
        out,
        stdout,
        stderr,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_unlike(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
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
        embedded_call: spec::EmbeddedWriteCall::Unlike { tweet_id },
    };
    execute(
        spec,
        client,
        out,
        stdout,
        stderr,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_repost(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
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
        embedded_call: spec::EmbeddedWriteCall::Repost { tweet_id },
    };
    execute(
        spec,
        client,
        out,
        stdout,
        stderr,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_unrepost(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
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
        embedded_call: spec::EmbeddedWriteCall::Unrepost { tweet_id },
    };
    execute(
        spec,
        client,
        out,
        stdout,
        stderr,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_follow(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
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
        embedded_call: spec::EmbeddedWriteCall::Follow {
            target_username: target,
        },
    };
    execute(
        spec,
        client,
        out,
        stdout,
        stderr,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_unfollow(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
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
        embedded_call: spec::EmbeddedWriteCall::Unfollow {
            target_username: target,
        },
    };
    execute(
        spec,
        client,
        out,
        stdout,
        stderr,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_dm(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
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
        embedded_call: spec::EmbeddedWriteCall::Dm {
            target_username: target,
            text,
        },
    };
    execute(
        spec,
        client,
        out,
        stdout,
        stderr,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_block(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
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
        embedded_call: spec::EmbeddedWriteCall::Block {
            target_username: target,
        },
    };
    execute(
        spec,
        client,
        out,
        stdout,
        stderr,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_unblock(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
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
        embedded_call: spec::EmbeddedWriteCall::Unblock {
            target_username: target,
        },
    };
    execute(
        spec,
        client,
        out,
        stdout,
        stderr,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_mute(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
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
        embedded_call: spec::EmbeddedWriteCall::Mute {
            target_username: target,
        },
    };
    execute(
        spec,
        client,
        out,
        stdout,
        stderr,
        guard,
        cache_only,
        no_interactive,
        config_username,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_unmute(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
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
        embedded_call: spec::EmbeddedWriteCall::Unmute {
            target_username: target,
        },
    };
    execute(
        spec,
        client,
        out,
        stdout,
        stderr,
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
    use crate::store::CacheOpts;
    use crate::xurl_client::mock::MockXurlClient;
    use std::io;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    fn quiet_out() -> OutputConfig {
        OutputConfig {
            format: OutputFormat::Text,
            use_color: false,
            quiet: true,
            raw: false,
        }
    }

    fn test_client() -> store::BirdClient {
        store::BirdClient::new(
            Box::new(MockXurlClient::new()),
            store::StoreSetup::under(Path::new("/nonexistent/writes-test")),
            CacheOpts {
                no_store: true,
                ..CacheOpts::default()
            },
            None,
            true,
            Arc::new(Mutex::new(io::sink())),
        )
    }

    fn like_spec() -> WriteSpec {
        WriteSpec {
            verb: "like",
            method: "POST",
            url_for_prompt: "https://api.x.com/2/users/me/likes/1".into(),
            body: None,
            embedded_call: spec::EmbeddedWriteCall::Like {
                tweet_id: "1".into(),
            },
        }
    }

    /// `--dry-run` returns Ok without invoking xurl. If xurl were invoked,
    /// the test would hit the empty MockXurlClient queue and panic; dry-run
    /// short-circuits before xurl_write fires.
    #[test]
    fn execute_dry_run_short_circuits() {
        let mut client = test_client();
        let out = quiet_out();
        let guard = WriteGuard {
            force: false,
            dry_run: true,
        };
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        assert!(
            execute(
                like_spec(),
                &mut client,
                &out,
                &mut stdout,
                &mut stderr,
                guard,
                false,
                false,
                None
            )
            .is_ok()
        );
    }

    /// `--cache-only` with `force=true` returns `BirdError::General` from
    /// the xurl_write cache-only refusal — the guard fires after confirmation
    /// succeeds and before transport is invoked.
    #[test]
    fn execute_cache_only_rejects_write() {
        let mut client = test_client();
        let out = quiet_out();
        let guard = WriteGuard {
            force: true,
            dry_run: false,
        };
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        match execute(
            like_spec(),
            &mut client,
            &out,
            &mut stdout,
            &mut stderr,
            guard,
            true,
            false,
            None,
        ) {
            Err(err) => {
                assert_eq!(err.text_prefix(), "like failed: ");
                assert!(
                    err.message().contains("--cache-only"),
                    "expected cache-only refusal, got: {}",
                    err.message()
                );
            }
            Ok(()) => panic!("expected cache-only refusal, got Ok"),
        }
    }

    /// `--no-interactive` without `--force`/`--yes` returns the
    /// confirmation-required error from the guard layer, before xurl_write
    /// runs.
    #[test]
    fn execute_no_interactive_without_force_errors() {
        let mut client = test_client();
        let out = quiet_out();
        let guard = WriteGuard {
            force: false,
            dry_run: false,
        };
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        match execute(
            like_spec(),
            &mut client,
            &out,
            &mut stdout,
            &mut stderr,
            guard,
            false,
            true,
            None,
        ) {
            Err(err) => assert_eq!(err.error_id(), "confirmation-required"),
            Ok(()) => panic!("expected confirmation-required error, got Ok"),
        }
    }

    /// tweet builder exercise: the builder, body, and url_for_prompt
    /// assembly run via `require_confirmation` even when the dry-run guard
    /// short-circuits before xurl_write.
    #[test]
    fn tweet_builder_dry_run_envelope_with_media() {
        let mut client = test_client();
        let out = quiet_out();
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        let res = run_tweet(
            &mut client,
            &out,
            &mut stdout,
            &mut stderr,
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

    /// follow builder exercise (target carried into url_for_prompt without
    /// an extra `@` prefix).
    #[test]
    fn follow_builder_dry_run_envelope() {
        let mut client = test_client();
        let out = quiet_out();
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        let res = run_follow(
            &mut client,
            &out,
            &mut stdout,
            &mut stderr,
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

    /// dm builder exercise (POST with body) — multi-field envelope.
    #[test]
    fn dm_builder_dry_run_envelope() {
        let mut client = test_client();
        let out = quiet_out();
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        let res = run_dm(
            &mut client,
            &out,
            &mut stdout,
            &mut stderr,
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

    /// A sent write prints one cost line per request: the `/2/users/me`
    /// lookup, then the like at its write rate.
    #[test]
    fn executed_write_prints_each_request_cost() {
        let mock = MockXurlClient::new();
        mock.push_value("send_request", serde_json::json!({"data": {"id": "42"}}));
        mock.push_value("send_request", serde_json::json!({"data": {"liked": true}}));
        let mut client = store::BirdClient::new(
            Box::new(mock),
            store::StoreSetup::under(Path::new("/nonexistent/writes-test")),
            CacheOpts {
                no_store: true,
                ..CacheOpts::default()
            },
            None,
            true,
            Arc::new(Mutex::new(io::sink())),
        );
        let out = OutputConfig {
            quiet: false,
            ..quiet_out()
        };
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        execute(
            like_spec(),
            &mut client,
            &out,
            &mut stdout,
            &mut stderr,
            WriteGuard {
                force: true,
                dry_run: false,
            },
            false,
            false,
            None,
        )
        .expect("like must send");

        assert_eq!(
            String::from_utf8(stderr).expect("utf8"),
            "[cost] ~$0.0100 (1 user, cache miss)\n\
             [cost] ~$0.0150 (User Interaction: Create)\n"
        );
    }
}
