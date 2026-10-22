//! `XurlClient` test double: canned responses per method, call recording, and
//! a loud failure on empty-queue calls so undercounts surface as failures
//! rather than silently passing.

#![cfg(test)]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use xdk::api::{RequestOptions, RequestTarget};

use super::XurlClient;

/// One queued outcome for a single mock-client call.
type Outcome = xdk::Result<Value>;

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

    /// Push a raw JSON value onto the queue for `method`.
    pub fn push_value(&self, method: &'static str, value: Value) {
        self.push_outcome(method, Ok(value));
    }

    /// Push an error outcome onto the queue for `method`.
    pub fn push_err(&self, method: &'static str, err: xdk::Error) {
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
            None => Err(xdk::Error::Internal(format!(
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

impl XurlClient for MockXurlClient {
    fn send_request(&mut self, options: &RequestOptions) -> xdk::Result<Value> {
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

    const fn _assert_mock_send_sync() {
        const fn check<T: Send + Sync>() {}
        check::<MockXurlClient>();
    }

    #[test]
    fn empty_queue_returns_loud_error() {
        let mut fake = MockXurlClient::new();
        let err = fake
            .send_request(&RequestOptions::default())
            .expect_err("empty queue must surface as an error");
        match err {
            xdk::Error::Internal(msg) => assert!(
                msg.contains("send_request") && msg.contains("undercount"),
                "error message should name the method and undercount: got {msg:?}",
            ),
            other => panic!("expected Internal undercount error, got {other:?}"),
        }
    }

    #[test]
    fn values_pop_in_fifo_order() {
        let mut fake = MockXurlClient::new();
        fake.push_value("send_request", serde_json::json!({ "data": { "id": "1" } }));
        fake.push_value("send_request", serde_json::json!({ "data": { "id": "2" } }));

        let first = fake
            .send_request(&RequestOptions::default())
            .expect("first");
        let second = fake
            .send_request(&RequestOptions::default())
            .expect("second");

        assert_eq!(first["data"]["id"], "1");
        assert_eq!(second["data"]["id"], "2");
    }

    #[test]
    fn err_response_surfaces_unchanged() {
        let mut fake = MockXurlClient::new();
        fake.push_err("send_request", xdk::Error::api(429, "rate limited"));

        match fake.send_request(&RequestOptions::default()) {
            Err(xdk::Error::Api { status, body, .. }) => {
                assert_eq!(status, 429);
                assert_eq!(body, "rate limited");
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }

    #[test]
    fn calls_record_request_arguments() {
        let mut fake = MockXurlClient::new();
        fake.push_value("send_request", serde_json::json!({}));

        let options = RequestOptions {
            method: "POST".to_string(),
            target: RequestTarget::RawUrl("https://api.x.com/2/tweets".to_string()),
            data: r#"{"text":"hi"}"#.to_string(),
            ..RequestOptions::default()
        };
        fake.send_request(&options).expect("send_request");

        let calls = fake.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].method, "send_request");
        assert_eq!(calls[0].args["method"], "POST");
        assert_eq!(calls[0].args["target"]["type"], "raw_url");
        assert_eq!(calls[0].args["data"], r#"{"text":"hi"}"#);
    }
}
