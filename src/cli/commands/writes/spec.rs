//! Per-verb spec for the `writes::execute` helper.
//!
//! The verb-specific fields a builder fills in live in [`WriteSpec`]; the
//! runtime environment (guard, output config, flags, username) is carried
//! separately by the runner so each builder stays small.

/// Verb-specific inputs that vary per xurl-write subcommand.
pub struct WriteSpec {
    /// Verb name used in prompts, error envelopes, and the xurl subcommand
    /// (e.g. `"tweet"`, `"like"`, `"unfollow"`).
    pub verb: &'static str,
    /// HTTP method shown in the confirmation prompt (`"POST"` or `"DELETE"`).
    pub method: &'static str,
    /// Target URL shown in the confirmation prompt; not used for transport.
    pub url_for_prompt: String,
    /// JSON body shown in the prompt (and dry-run envelope); `None` for
    /// DELETE-shaped verbs that send no body.
    pub body: Option<serde_json::Value>,
    /// Actual CLI args passed to `xurl_write_call`.
    pub xurl_args: Vec<String>,
}
