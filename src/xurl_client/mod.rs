//! Bird's seam over xurl-rs v2.0.0.
//!
//! Defines a trait that mirrors the typed shortcut surface bird consumes from
//! xurl plus a generic `send_request` for the `bird raw` and the bird
//! block/unblock verbs (which have no typed shortcuts upstream). The trait
//! exists so tests can substitute a hand-rolled fake at the bird/xurl
//! boundary while production calls flow through `xurl::api::ApiClient`.
//!
//! The trip-point in the brainstorm R18 is ~15 methods — this trait ships at
//! 18 (17 typed + 1 generic) and the next bird command added past PR3 fires
//! the typed-adapter revisit.

// Mirrors xurl-rs's own decision to keep `XurlError` unboxed in `Result`
// returns (`#[allow(clippy::result_large_err)]` upstream at the enum). Boxing
// at every bird seam method would pay an allocation per error path for no
// downstream gain — the type is the type.
#![allow(clippy::result_large_err)]

use xurl::api::{
    ApiClient, ApiResponse, CallOptions, DmEvent, FollowingResult, LikedResult, MutingResult,
    RequestOptions, RetweetedResult, Tweet, UsageData, User,
};
use xurl::error::Result as XurlResult;

#[cfg(test)]
pub mod mock;

/// Typed seam over xurl's shortcut surface plus the generic request entry
/// point. Production impl is `xurl::api::ApiClient`; tests substitute
/// `fake::FakeXurlClient`.
pub trait XurlClient {
    fn get_me(&mut self, opts: &CallOptions) -> XurlResult<ApiResponse<User>>;

    fn lookup_user(&mut self, username: &str, opts: &CallOptions) -> XurlResult<ApiResponse<User>>;

    fn read_post(&mut self, post_id: &str, opts: &CallOptions) -> XurlResult<ApiResponse<Tweet>>;

    fn search_posts(
        &mut self,
        query: &str,
        max_results: i32,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Vec<Tweet>>>;

    fn get_bookmarks(
        &mut self,
        user_id: &str,
        max_results: i32,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Vec<Tweet>>>;

    fn get_usage(&mut self, opts: &CallOptions) -> XurlResult<ApiResponse<UsageData>>;

    fn create_post(
        &mut self,
        text: &str,
        media_ids: &[String],
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Tweet>>;

    fn reply_to_post(
        &mut self,
        post_id: &str,
        text: &str,
        media_ids: &[String],
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Tweet>>;

    fn like_post(
        &mut self,
        user_id: &str,
        post_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<LikedResult>>;

    fn unlike_post(
        &mut self,
        user_id: &str,
        post_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<LikedResult>>;

    fn repost(
        &mut self,
        user_id: &str,
        post_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<RetweetedResult>>;

    fn unrepost(
        &mut self,
        user_id: &str,
        post_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<RetweetedResult>>;

    fn follow_user(
        &mut self,
        source_user_id: &str,
        target_user_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<FollowingResult>>;

    fn unfollow_user(
        &mut self,
        source_user_id: &str,
        target_user_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<FollowingResult>>;

    fn send_dm(
        &mut self,
        participant_id: &str,
        text: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<DmEvent>>;

    fn mute_user(
        &mut self,
        source_user_id: &str,
        target_user_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<MutingResult>>;

    fn unmute_user(
        &mut self,
        source_user_id: &str,
        target_user_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<MutingResult>>;

    /// Generic request entry point used by `bird raw` and by the bird
    /// `block`/`unblock` write verbs (which have no typed xurl shortcut).
    fn send_request(&mut self, options: &RequestOptions) -> XurlResult<serde_json::Value>;
}

impl XurlClient for ApiClient {
    fn get_me(&mut self, opts: &CallOptions) -> XurlResult<ApiResponse<User>> {
        ApiClient::get_me(self, opts)
    }

    fn lookup_user(&mut self, username: &str, opts: &CallOptions) -> XurlResult<ApiResponse<User>> {
        ApiClient::lookup_user(self, username, opts)
    }

    fn read_post(&mut self, post_id: &str, opts: &CallOptions) -> XurlResult<ApiResponse<Tweet>> {
        ApiClient::read_post(self, post_id, opts)
    }

    fn search_posts(
        &mut self,
        query: &str,
        max_results: i32,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Vec<Tweet>>> {
        ApiClient::search_posts(self, query, max_results, opts)
    }

    fn get_bookmarks(
        &mut self,
        user_id: &str,
        max_results: i32,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Vec<Tweet>>> {
        ApiClient::get_bookmarks(self, user_id, max_results, opts)
    }

    fn get_usage(&mut self, opts: &CallOptions) -> XurlResult<ApiResponse<UsageData>> {
        ApiClient::get_usage(self, opts)
    }

    fn create_post(
        &mut self,
        text: &str,
        media_ids: &[String],
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Tweet>> {
        ApiClient::create_post(self, text, media_ids, opts)
    }

    fn reply_to_post(
        &mut self,
        post_id: &str,
        text: &str,
        media_ids: &[String],
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Tweet>> {
        ApiClient::reply_to_post(self, post_id, text, media_ids, opts)
    }

    fn like_post(
        &mut self,
        user_id: &str,
        post_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<LikedResult>> {
        ApiClient::like_post(self, user_id, post_id, opts)
    }

    fn unlike_post(
        &mut self,
        user_id: &str,
        post_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<LikedResult>> {
        ApiClient::unlike_post(self, user_id, post_id, opts)
    }

    fn repost(
        &mut self,
        user_id: &str,
        post_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<RetweetedResult>> {
        ApiClient::repost(self, user_id, post_id, opts)
    }

    fn unrepost(
        &mut self,
        user_id: &str,
        post_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<RetweetedResult>> {
        ApiClient::unrepost(self, user_id, post_id, opts)
    }

    fn follow_user(
        &mut self,
        source_user_id: &str,
        target_user_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<FollowingResult>> {
        ApiClient::follow_user(self, source_user_id, target_user_id, opts)
    }

    fn unfollow_user(
        &mut self,
        source_user_id: &str,
        target_user_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<FollowingResult>> {
        ApiClient::unfollow_user(self, source_user_id, target_user_id, opts)
    }

    fn send_dm(
        &mut self,
        participant_id: &str,
        text: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<DmEvent>> {
        ApiClient::send_dm(self, participant_id, text, opts)
    }

    fn mute_user(
        &mut self,
        source_user_id: &str,
        target_user_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<MutingResult>> {
        ApiClient::mute_user(self, source_user_id, target_user_id, opts)
    }

    fn unmute_user(
        &mut self,
        source_user_id: &str,
        target_user_id: &str,
        opts: &CallOptions,
    ) -> XurlResult<ApiResponse<MutingResult>> {
        ApiClient::unmute_user(self, source_user_id, target_user_id, opts)
    }

    fn send_request(&mut self, options: &RequestOptions) -> XurlResult<serde_json::Value> {
        ApiClient::send_request(self, options)
    }
}

/// `xurl::api::ApiClient` is the production `XurlClient` and must remain
/// `Send + Sync` so `BirdClient` can hold it in a `std::sync::Mutex` without
/// being un-shareable across threads. Mirrors the analogous check on the
/// existing `rusqlite::Connection` field — see `src/db/client/mod.rs`.
#[allow(dead_code)]
const fn _assert_api_client_send_sync() {
    const fn check<T: Send + Sync>() {}
    check::<ApiClient>();
}

/// `XurlClient` impl used when `xurl::api::ApiClient::from_env()` fails at
/// startup. Returns the original construction error wrapped in
/// `XurlError::Internal` on every call, mirroring the subprocess
/// `XurlTransport::from_error` shape. PR1 ships this field unused — handler
/// migration in PR2 surfaces the error at the first call site that needs the
/// embedded client.
pub struct ConstructionStub {
    error: String,
}

impl ConstructionStub {
    pub fn new(error: String) -> Self {
        Self { error }
    }

    fn err<T>(&self, op: &str) -> XurlResult<T> {
        Err(xurl::error::XurlError::Internal(format!(
            "embedded xurl: {op} unavailable: {}",
            self.error
        )))
    }
}

impl XurlClient for ConstructionStub {
    fn get_me(&mut self, _opts: &CallOptions) -> XurlResult<ApiResponse<User>> {
        self.err("get_me")
    }

    fn lookup_user(
        &mut self,
        _username: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<User>> {
        self.err("lookup_user")
    }

    fn read_post(&mut self, _post_id: &str, _opts: &CallOptions) -> XurlResult<ApiResponse<Tweet>> {
        self.err("read_post")
    }

    fn search_posts(
        &mut self,
        _query: &str,
        _max_results: i32,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Vec<Tweet>>> {
        self.err("search_posts")
    }

    fn get_bookmarks(
        &mut self,
        _user_id: &str,
        _max_results: i32,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Vec<Tweet>>> {
        self.err("get_bookmarks")
    }

    fn get_usage(&mut self, _opts: &CallOptions) -> XurlResult<ApiResponse<UsageData>> {
        self.err("get_usage")
    }

    fn create_post(
        &mut self,
        _text: &str,
        _media_ids: &[String],
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Tweet>> {
        self.err("create_post")
    }

    fn reply_to_post(
        &mut self,
        _post_id: &str,
        _text: &str,
        _media_ids: &[String],
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Tweet>> {
        self.err("reply_to_post")
    }

    fn like_post(
        &mut self,
        _user_id: &str,
        _post_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<LikedResult>> {
        self.err("like_post")
    }

    fn unlike_post(
        &mut self,
        _user_id: &str,
        _post_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<LikedResult>> {
        self.err("unlike_post")
    }

    fn repost(
        &mut self,
        _user_id: &str,
        _post_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<RetweetedResult>> {
        self.err("repost")
    }

    fn unrepost(
        &mut self,
        _user_id: &str,
        _post_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<RetweetedResult>> {
        self.err("unrepost")
    }

    fn follow_user(
        &mut self,
        _source_user_id: &str,
        _target_user_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<FollowingResult>> {
        self.err("follow_user")
    }

    fn unfollow_user(
        &mut self,
        _source_user_id: &str,
        _target_user_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<FollowingResult>> {
        self.err("unfollow_user")
    }

    fn send_dm(
        &mut self,
        _participant_id: &str,
        _text: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<DmEvent>> {
        self.err("send_dm")
    }

    fn mute_user(
        &mut self,
        _source_user_id: &str,
        _target_user_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<MutingResult>> {
        self.err("mute_user")
    }

    fn unmute_user(
        &mut self,
        _source_user_id: &str,
        _target_user_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<MutingResult>> {
        self.err("unmute_user")
    }

    fn send_request(&mut self, _options: &RequestOptions) -> XurlResult<serde_json::Value> {
        self.err("send_request")
    }
}
