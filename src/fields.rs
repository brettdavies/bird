//! Canonical X API v2 field sets. Single source of truth for all commands.
//!
//! Every command should use these constants (or the helper functions) instead of
//! defining its own field strings. This ensures consistent entity shapes across
//! the codebase and enables Plan 2's entity store to decompose any response.

/// Post object fields: the union bird's search, thread, watchlist, and
/// bookmarks reads need, plus what entity-store decomposition reads. Every
/// member is one X's 2.168 spec lists under `post.fields`; `author_id`,
/// `in_reply_to_user_id`, and referenced posts are expansions there.
pub const POST_FIELDS: &str = "\
    attachments,\
    conversation_id,\
    created_at,\
    entities,\
    public_metrics,\
    text";

/// User object fields. Union of fields used across profile, search, thread, and watchlist.
pub const USER_FIELDS: &str = "\
    created_at,\
    description,\
    location,\
    name,\
    profile_image_url,\
    public_metrics,\
    url,\
    username,\
    verified";

/// Media object fields for expanded media attachments.
pub const MEDIA_FIELDS: &str = "\
    height,\
    media_key,\
    preview_image_url,\
    type,\
    url,\
    width";

/// Expansion set: author user objects, the replied-to user, referenced posts,
/// and media attachments, spelled as xdk-rs's own post reads send them.
pub const EXPANSIONS: &str = "\
    attachments.media_keys,\
    author_id,\
    in_reply_to_user_id,\
    referenced_posts";

/// Build the standard query parameter pairs for post-centric endpoints.
/// Includes post.fields, user.fields, media.fields, and expansions.
pub fn post_query_params() -> Vec<(&'static str, &'static str)> {
    vec![
        ("post.fields", POST_FIELDS),
        ("user.fields", USER_FIELDS),
        ("media.fields", MEDIA_FIELDS),
        ("expansions", EXPANSIONS),
    ]
}

/// Build query parameter pairs for user-centric endpoints (e.g. profile lookup).
/// Only includes user.fields since the primary object is already a user.
pub fn user_query_params() -> Vec<(&'static str, &'static str)> {
    vec![("user.fields", USER_FIELDS)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify field strings are well-formed CSV: no leading/trailing whitespace,
    /// no trailing commas, no empty segments, no duplicate fields.
    fn assert_valid_field_string(name: &str, fields: &str) {
        assert!(!fields.is_empty(), "{name} must not be empty");
        assert_eq!(
            fields,
            fields.trim(),
            "{name} has leading/trailing whitespace"
        );
        assert!(!fields.ends_with(','), "{name} has trailing comma");
        assert!(!fields.starts_with(','), "{name} has leading comma");

        let segments: Vec<&str> = fields.split(',').collect();
        for (i, seg) in segments.iter().enumerate() {
            assert_eq!(
                *seg,
                seg.trim(),
                "{name} segment {i} has whitespace: '{seg}'"
            );
            assert!(!seg.is_empty(), "{name} has empty segment at position {i}");
        }

        // Check for duplicates
        let mut seen = std::collections::HashSet::new();
        for seg in &segments {
            assert!(seen.insert(*seg), "{name} has duplicate field: '{seg}'");
        }

        // Check alphabetical ordering (our convention)
        let mut sorted = segments.clone();
        sorted.sort();
        assert_eq!(
            segments, sorted,
            "{name} fields are not in alphabetical order"
        );
    }

    #[test]
    fn post_fields_well_formed() {
        assert_valid_field_string("POST_FIELDS", POST_FIELDS);
    }

    #[test]
    fn user_fields_well_formed() {
        assert_valid_field_string("USER_FIELDS", USER_FIELDS);
    }

    #[test]
    fn media_fields_well_formed() {
        assert_valid_field_string("MEDIA_FIELDS", MEDIA_FIELDS);
    }

    #[test]
    fn expansions_well_formed() {
        assert_valid_field_string("EXPANSIONS", EXPANSIONS);
    }

    /// The expansions xdk-rs's `read_post` sends in
    /// `crates/xdk/src/api/shortcuts.rs` at the pinned commit
    /// (`author_id,in_reply_to_user_id,referenced_posts`) are all among
    /// bird's; xdk-rs builds them in a private builder, so the literals are
    /// copied here.
    #[test]
    fn expansions_cover_xdk_read_post() {
        let ours: Vec<&str> = EXPANSIONS.split(',').collect();
        for expansion in ["author_id", "in_reply_to_user_id", "referenced_posts"] {
            assert!(ours.contains(&expansion), "missing {expansion}");
        }
        for legacy in ["referenced_tweets", "referenced_tweets.id"] {
            assert!(!ours.contains(&legacy), "legacy {legacy}");
        }
    }

    #[test]
    fn post_query_params_has_all_keys() {
        let params = post_query_params();
        let keys: Vec<&str> = params.iter().map(|(k, _)| *k).collect();
        assert!(keys.contains(&"post.fields"));
        assert!(!keys.contains(&"tweet.fields"));
        assert!(keys.contains(&"user.fields"));
        assert!(keys.contains(&"media.fields"));
        assert!(keys.contains(&"expansions"));
    }

    #[test]
    fn user_query_params_has_user_fields() {
        let params = user_query_params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].0, "user.fields");
    }
}
