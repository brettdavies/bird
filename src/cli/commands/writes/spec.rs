//! Per-verb spec for the `writes::execute` helper.
//!
//! The verb-specific fields a builder fills in live in [`WriteSpec`]; the
//! runtime environment (guard, output config, flags, username) is carried
//! separately by the runner so each builder stays small.

/// Structured description of the X API call each write verb should make
/// when dispatched through the embedded xurl client. The variant captures
/// enough verb-specific shape that the dispatcher can run any required
/// pre-call resolution (`/2/users/me` for the caller's id,
/// `/2/users/by/username/{}` for a target's id) and then construct the
/// final request without each per-verb builder having to do so itself.
#[derive(Clone, Debug)]
pub enum EmbeddedWriteCall {
    /// `POST /2/tweets`. No prerequisite resolution.
    TweetCreate {
        text: String,
        media_id: Option<String>,
    },
    /// `POST /2/tweets` with `reply.in_reply_to_tweet_id`.
    Reply { parent_id: String, text: String },
    /// `POST /2/users/{me_id}/likes` with `{"tweet_id": ...}`. Needs `/me`.
    Like { tweet_id: String },
    /// `DELETE /2/users/{me_id}/likes/{tweet_id}`. Needs `/me`.
    Unlike { tweet_id: String },
    /// `POST /2/users/{me_id}/retweets` with `{"tweet_id": ...}`. Needs `/me`.
    Repost { tweet_id: String },
    /// `DELETE /2/users/{me_id}/retweets/{tweet_id}`. Needs `/me`.
    Unrepost { tweet_id: String },
    /// `POST /2/users/{me_id}/following` with `{"target_user_id": ...}`.
    /// Needs `/me` plus `/users/by/username/{target}` for the user id.
    Follow { target_username: String },
    /// `DELETE /2/users/{me_id}/following/{target_id}`.
    Unfollow { target_username: String },
    /// `POST /2/users/{me_id}/muting`.
    Mute { target_username: String },
    /// `DELETE /2/users/{me_id}/muting/{target_id}`.
    Unmute { target_username: String },
    /// `POST /2/users/{me_id}/blocking`.
    Block { target_username: String },
    /// `DELETE /2/users/{me_id}/blocking/{target_id}`.
    Unblock { target_username: String },
    /// `POST /2/dm_conversations/with/{target_id}/messages` with
    /// `{"text": ...}`. Needs `/users/by/username/{target}`.
    Dm {
        target_username: String,
        text: String,
    },
}

/// Verb-specific inputs that vary per xurl-write subcommand.
pub struct WriteSpec {
    /// Verb name used in prompts, error envelopes, and dispatcher diagnostics
    /// (e.g. `"tweet"`, `"like"`, `"unfollow"`).
    pub verb: &'static str,
    /// HTTP method shown in the confirmation prompt (`"POST"` or `"DELETE"`).
    pub method: &'static str,
    /// Target URL shown in the confirmation prompt; not used for transport.
    pub url_for_prompt: String,
    /// JSON body shown in the prompt (and dry-run envelope); `None` for
    /// DELETE-shaped verbs that send no body.
    pub body: Option<serde_json::Value>,
    /// Structured shape the embedded transport dispatcher consumes so the
    /// call can be reconstructed without parsing `url_for_prompt` (which is
    /// a display string, not a real API URL for many verbs).
    pub embedded_call: EmbeddedWriteCall,
}
