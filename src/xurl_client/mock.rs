//! `XurlClient` test double — canned responses per method, call recording, and
//! a loud failure on empty-queue calls so undercounts surface as failures
//! rather than silently passing. Mirrors the `MockTransport` shape (queue +
//! Mutex) at `src/transport.rs`.

#![cfg(test)]
#![allow(clippy::result_large_err)]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::Value;
use xurl::api::{
    ApiResponse, CallOptions, DmEvent, FollowingResult, LikedResult, MutingResult, RequestOptions,
    RequestTarget, RetweetedResult, Tweet, UsageData, User,
};
use xurl::error::{Result as XurlResult, XurlError};

use super::XurlClient;

/// One queued outcome for a single mock-client call.
type Outcome = XurlResult<Value>;

/// Records every call dispatched to the fake along with the method name and
/// the popped outcome, useful when a test wants to assert on call order or
/// argument capture.
#[derive(Debug, Clone)]
pub struct Call {
    pub method: &'static str,
    pub args: Value,
}

#[derive(Default)]
struct Inner {
    queues: HashMap<&'static str, VecDeque<Outcome>>,
    calls: Vec<Call>,
}

/// In-memory `XurlClient` impl whose responses are programmed by the test.
/// Tracks calls and pops canned outcomes per method.
///
/// State lives behind `Arc<Mutex<_>>` so a test can hand the mock to
/// `BirdClient` (as `Box<dyn XurlClient + Send>`) while keeping a separate
/// handle outside via [`MockXurlClient::clone_handle`] for inspecting
/// recorded calls afterward.
pub struct MockXurlClient {
    inner: Arc<Mutex<Inner>>,
}

impl MockXurlClient {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }

    /// Returns a separate handle pointing at the same shared queues + call
    /// log. Use when the primary `MockXurlClient` is moved into `BirdClient`
    /// and the test still needs to read what was dispatched.
    pub fn clone_handle(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }

    /// Push a typed success payload onto the queue for `method`. Wraps `body`
    /// in `ApiResponse { data: body, .. }` and serializes to a JSON value the
    /// trait method later deserializes on the way out.
    pub fn push_ok<T>(&self, method: &'static str, body: T)
    where
        T: Default + Serialize,
    {
        let response = ApiResponse {
            data: body,
            ..ApiResponse::<T>::default()
        };
        let value =
            serde_json::to_value(&response).expect("mock: ApiResponse must serialize cleanly");
        self.push_outcome(method, Ok(value));
    }

    /// Push a pre-built `ApiResponse<T>` onto the queue. Use when the test
    /// needs to control `meta`, `includes`, or `errors` alongside `data`.
    pub fn push_response<T>(&self, method: &'static str, response: ApiResponse<T>)
    where
        T: Default + Serialize,
    {
        let value =
            serde_json::to_value(&response).expect("mock: ApiResponse must serialize cleanly");
        self.push_outcome(method, Ok(value));
    }

    /// Push a raw JSON value onto the queue. Used for `send_request` whose
    /// trait-level return type is `serde_json::Value`, not `ApiResponse<T>`.
    pub fn push_value(&self, method: &'static str, value: Value) {
        self.push_outcome(method, Ok(value));
    }

    /// Push an error outcome onto the queue for `method`.
    pub fn push_err(&self, method: &'static str, err: XurlError) {
        self.push_outcome(method, Err(err));
    }

    fn push_outcome(&self, method: &'static str, outcome: Outcome) {
        let mut inner = self.inner.lock().expect("mock: queue mutex poisoned");
        inner.queues.entry(method).or_default().push_back(outcome);
    }

    /// Snapshot of every call dispatched, in order.
    pub fn calls(&self) -> Vec<Call> {
        let inner = self.inner.lock().expect("mock: queue mutex poisoned");
        inner.calls.clone()
    }

    fn pop(&self, method: &'static str, args: Value) -> Outcome {
        let mut inner = self.inner.lock().expect("mock: queue mutex poisoned");
        inner.calls.push(Call { method, args });
        match inner.queues.get_mut(method).and_then(VecDeque::pop_front) {
            Some(outcome) => outcome,
            None => Err(XurlError::Internal(format!(
                "MockXurlClient: no canned response queued for {method:?} (test undercount)"
            ))),
        }
    }
}

impl Default for MockXurlClient {
    fn default() -> Self {
        Self::new()
    }
}

fn deserialize<T>(method: &'static str, value: Value) -> XurlResult<ApiResponse<T>>
where
    T: Default + serde::de::DeserializeOwned,
{
    serde_json::from_value(value).map_err(|e| {
        XurlError::Internal(format!(
            "MockXurlClient: queued response for {method:?} failed to deserialize: {e}"
        ))
    })
}

impl XurlClient for MockXurlClient {
    fn get_me(&mut self, _opts: &CallOptions) -> XurlResult<ApiResponse<User>> {
        let value = self.pop("get_me", Value::Null)?;
        deserialize("get_me", value)
    }

    fn lookup_user(
        &mut self,
        username: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<User>> {
        let args = serde_json::json!({ "username": username });
        let value = self.pop("lookup_user", args)?;
        deserialize("lookup_user", value)
    }

    fn read_post(&mut self, post_id: &str, _opts: &CallOptions) -> XurlResult<ApiResponse<Tweet>> {
        let args = serde_json::json!({ "post_id": post_id });
        let value = self.pop("read_post", args)?;
        deserialize("read_post", value)
    }

    fn search_posts(
        &mut self,
        query: &str,
        max_results: i32,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Vec<Tweet>>> {
        let args = serde_json::json!({ "query": query, "max_results": max_results });
        let value = self.pop("search_posts", args)?;
        deserialize("search_posts", value)
    }

    fn get_bookmarks(
        &mut self,
        user_id: &str,
        max_results: i32,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Vec<Tweet>>> {
        let args = serde_json::json!({ "user_id": user_id, "max_results": max_results });
        let value = self.pop("get_bookmarks", args)?;
        deserialize("get_bookmarks", value)
    }

    fn get_usage(&mut self, _opts: &CallOptions) -> XurlResult<ApiResponse<UsageData>> {
        let value = self.pop("get_usage", Value::Null)?;
        deserialize("get_usage", value)
    }

    fn create_post(
        &mut self,
        text: &str,
        media_ids: &[String],
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Tweet>> {
        let args = serde_json::json!({ "text": text, "media_ids": media_ids });
        let value = self.pop("create_post", args)?;
        deserialize("create_post", value)
    }

    fn reply_to_post(
        &mut self,
        post_id: &str,
        text: &str,
        media_ids: &[String],
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<Tweet>> {
        let args = serde_json::json!({
            "post_id": post_id,
            "text": text,
            "media_ids": media_ids,
        });
        let value = self.pop("reply_to_post", args)?;
        deserialize("reply_to_post", value)
    }

    fn like_post(
        &mut self,
        user_id: &str,
        post_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<LikedResult>> {
        let args = serde_json::json!({ "user_id": user_id, "post_id": post_id });
        let value = self.pop("like_post", args)?;
        deserialize("like_post", value)
    }

    fn unlike_post(
        &mut self,
        user_id: &str,
        post_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<LikedResult>> {
        let args = serde_json::json!({ "user_id": user_id, "post_id": post_id });
        let value = self.pop("unlike_post", args)?;
        deserialize("unlike_post", value)
    }

    fn repost(
        &mut self,
        user_id: &str,
        post_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<RetweetedResult>> {
        let args = serde_json::json!({ "user_id": user_id, "post_id": post_id });
        let value = self.pop("repost", args)?;
        deserialize("repost", value)
    }

    fn unrepost(
        &mut self,
        user_id: &str,
        post_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<RetweetedResult>> {
        let args = serde_json::json!({ "user_id": user_id, "post_id": post_id });
        let value = self.pop("unrepost", args)?;
        deserialize("unrepost", value)
    }

    fn follow_user(
        &mut self,
        source_user_id: &str,
        target_user_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<FollowingResult>> {
        let args = serde_json::json!({
            "source_user_id": source_user_id,
            "target_user_id": target_user_id,
        });
        let value = self.pop("follow_user", args)?;
        deserialize("follow_user", value)
    }

    fn unfollow_user(
        &mut self,
        source_user_id: &str,
        target_user_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<FollowingResult>> {
        let args = serde_json::json!({
            "source_user_id": source_user_id,
            "target_user_id": target_user_id,
        });
        let value = self.pop("unfollow_user", args)?;
        deserialize("unfollow_user", value)
    }

    fn send_dm(
        &mut self,
        participant_id: &str,
        text: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<DmEvent>> {
        let args = serde_json::json!({ "participant_id": participant_id, "text": text });
        let value = self.pop("send_dm", args)?;
        deserialize("send_dm", value)
    }

    fn mute_user(
        &mut self,
        source_user_id: &str,
        target_user_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<MutingResult>> {
        let args = serde_json::json!({
            "source_user_id": source_user_id,
            "target_user_id": target_user_id,
        });
        let value = self.pop("mute_user", args)?;
        deserialize("mute_user", value)
    }

    fn unmute_user(
        &mut self,
        source_user_id: &str,
        target_user_id: &str,
        _opts: &CallOptions,
    ) -> XurlResult<ApiResponse<MutingResult>> {
        let args = serde_json::json!({
            "source_user_id": source_user_id,
            "target_user_id": target_user_id,
        });
        let value = self.pop("unmute_user", args)?;
        deserialize("unmute_user", value)
    }

    fn send_request(&mut self, options: &RequestOptions) -> XurlResult<Value> {
        let args = serde_json::json!({
            "method": options.method,
            "target": serialize_request_target(&options.target),
            "data": options.data,
            "headers": options.headers,
            "auth_type": options.auth_type,
            "no_auth": options.no_auth,
        });
        self.pop("send_request", args)
    }
}

/// Renders a `RequestTarget` into the JSON shape recorded in the mock's
/// `Call.args`. Tests inspect the `target` sub-object to assert that
/// `RequestTarget::Template` survived round-trip without bird pre-rendering
/// the URL (which would surface as `{ "type": "raw_url", "url": "..." }`).
fn serialize_request_target(target: &RequestTarget) -> Value {
    match target {
        RequestTarget::Template {
            path,
            path_params,
            query,
        } => {
            let path_params_value = serde_json::to_value(path_params).unwrap_or(Value::Null);
            let query_value: Vec<Value> = query
                .iter()
                .map(|(k, v)| serde_json::json!([k, v]))
                .collect();
            serde_json::json!({
                "type": "template",
                "path": path,
                "path_params": path_params_value,
                "query": query_value,
            })
        }
        RequestTarget::RawUrl(url) => serde_json::json!({
            "type": "raw_url",
            "url": url,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xurl::api::Tweet;

    const fn _assert_mock_send_sync() {
        const fn check<T: Send + Sync>() {}
        check::<MockXurlClient>();
    }

    #[test]
    fn empty_queue_returns_loud_error() {
        let mut fake = MockXurlClient::new();
        let result = fake.get_me(&CallOptions::default());
        let err = result.expect_err("empty queue must surface as an error");
        match err {
            XurlError::Internal(msg) => assert!(
                msg.contains("get_me") && msg.contains("undercount"),
                "error message should name the method and undercount: got {msg:?}",
            ),
            other => panic!("expected Internal undercount error, got {other:?}"),
        }
    }

    #[test]
    fn ok_responses_pop_in_fifo_order() {
        let mut fake = MockXurlClient::new();
        let tweet_a = Tweet {
            id: "1".to_string(),
            ..Tweet::default()
        };
        let tweet_b = Tweet {
            id: "2".to_string(),
            ..Tweet::default()
        };
        fake.push_ok("get_bookmarks", vec![tweet_a]);
        fake.push_ok("get_bookmarks", vec![tweet_b]);

        let first = fake
            .get_bookmarks("user", 10, &CallOptions::default())
            .expect("first pop");
        let second = fake
            .get_bookmarks("user", 10, &CallOptions::default())
            .expect("second pop");

        assert_eq!(first.data[0].id, "1");
        assert_eq!(second.data[0].id, "2");
    }

    #[test]
    fn err_response_surfaces_unchanged() {
        let mut fake = MockXurlClient::new();
        fake.push_err(
            "get_bookmarks",
            XurlError::Api {
                status: 429,
                body: "rate limited".to_string(),
            },
        );

        let result = fake.get_bookmarks("user", 10, &CallOptions::default());
        match result {
            Err(XurlError::Api { status, body }) => {
                assert_eq!(status, 429);
                assert_eq!(body, "rate limited");
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }

    #[test]
    fn distinct_methods_have_independent_queues() {
        let mut fake = MockXurlClient::new();
        fake.push_ok("get_me", User::default());
        fake.push_err("lookup_user", XurlError::Validation("nope".to_string()));

        fake.get_me(&CallOptions::default())
            .expect("get_me queue should be untouched");
        let lookup = fake.lookup_user("alice", &CallOptions::default());
        assert!(
            matches!(lookup, Err(XurlError::Validation(_))),
            "lookup_user queue should still hold its error: got {lookup:?}",
        );
    }

    #[test]
    fn send_request_returns_raw_value() {
        let mut fake = MockXurlClient::new();
        fake.push_value(
            "send_request",
            serde_json::json!({ "data": { "ok": true } }),
        );

        let value = fake
            .send_request(&RequestOptions::default())
            .expect("send_request");
        assert_eq!(value["data"]["ok"], true);
    }

    #[test]
    fn calls_are_recorded_in_order() {
        let mut fake = MockXurlClient::new();
        fake.push_ok("get_me", User::default());
        fake.push_ok("lookup_user", User::default());

        fake.get_me(&CallOptions::default()).unwrap();
        fake.lookup_user("alice", &CallOptions::default()).unwrap();

        let calls = fake.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].method, "get_me");
        assert_eq!(calls[1].method, "lookup_user");
        assert_eq!(calls[1].args["username"], "alice");
    }
}
