//! SIGTERM and SIGINT while an X API call is in flight.
//!
//! bird drives each xdk call to completion on a current-thread runtime, so no
//! task is awake between calls to receive a signal. The handler works on two
//! flags instead. With no call in flight the signal takes its default action,
//! exactly as if bird had installed nothing. During a call the signal is
//! recorded, the call is dropped within one poll interval, and it fails with
//! [`Cancelled`]; the error envelope reports `cancelled` and exits with 128 plus
//! the signal number. A signal that lands as a call completes keeps that call's
//! result and cancels the next call instead.

use std::future::Future;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

/// How often an in-flight call checks for a recorded signal. Bounds the time
/// from SIGTERM to the call being dropped.
const POLL: Duration = Duration::from_millis(50);

/// A call dropped because the process received `signal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled {
    pub signal: i32,
}

impl Cancelled {
    /// The shell convention for a process stopped by a signal.
    pub fn exit_code(self) -> u8 {
        u8::try_from(128 + self.signal).unwrap_or(u8::MAX)
    }

    fn signal_name(self) -> String {
        match self.signal {
            libc::SIGTERM => "SIGTERM".to_string(),
            libc::SIGINT => "SIGINT".to_string(),
            other => format!("signal {other}"),
        }
    }

    /// The `Cancelled` this error was built from, if any, found anywhere in
    /// its source chain.
    pub fn find(err: &(dyn std::error::Error + 'static)) -> Option<Self> {
        let mut current = Some(err);
        while let Some(e) = current {
            if let Some(cancelled) = e.downcast_ref::<Self>() {
                return Some(*cancelled);
            }
            current = e.source();
        }
        None
    }
}

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "request cancelled by {}", self.signal_name())
    }
}

impl std::error::Error for Cancelled {}

/// The seam returns `xdk::Error`; the cancellation rides along as its source
/// so [`Cancelled::find`] recovers it from the boxed error a command returns.
impl From<Cancelled> for xdk::Error {
    fn from(cancelled: Cancelled) -> Self {
        xdk::Error::io(cancelled.to_string()).with_source(cancelled)
    }
}

/// The two flags the signal handler and the in-flight call share.
#[derive(Debug, Clone)]
pub struct Stop {
    received: Arc<AtomicUsize>,
    idle: Arc<AtomicBool>,
}

impl Default for Stop {
    fn default() -> Self {
        Self {
            received: Arc::new(AtomicUsize::new(0)),
            idle: Arc::new(AtomicBool::new(true)),
        }
    }
}

impl Stop {
    /// The process's flags. They only ever change once [`install`] has run,
    /// which the binary does and tests never do.
    pub fn process() -> &'static Stop {
        static PROCESS: OnceLock<Stop> = OnceLock::new();
        PROCESS.get_or_init(Stop::default)
    }

    /// The signal recorded during a call, if one arrived.
    pub fn received(&self) -> Option<Cancelled> {
        match self.received.load(Ordering::SeqCst) {
            0 => None,
            signal => i32::try_from(signal)
                .ok()
                .map(|signal| Cancelled { signal }),
        }
    }

    /// Records `signal` as the handler does, for tests that drive the
    /// cancellation path without raising a real signal.
    #[cfg(test)]
    pub(crate) fn record(&self, signal: i32) {
        self.received
            .store(usize::try_from(signal).unwrap_or(0), Ordering::SeqCst);
    }

    /// Runs `future` to completion on `runtime`, or drops it once a signal is
    /// recorded. A signal recorded before the call starts cancels it unpolled.
    pub fn block_on<F: Future>(
        &self,
        runtime: &tokio::runtime::Runtime,
        future: F,
    ) -> Result<F::Output, Cancelled> {
        if let Some(cancelled) = self.received() {
            return Err(cancelled);
        }
        self.idle.store(false, Ordering::SeqCst);
        let outcome = runtime.block_on(async {
            tokio::select! {
                output = future => Ok(output),
                cancelled = self.signalled() => Err(cancelled),
            }
        });
        self.idle.store(true, Ordering::SeqCst);
        outcome
    }

    async fn signalled(&self) -> Cancelled {
        loop {
            if let Some(cancelled) = self.received() {
                return cancelled;
            }
            tokio::time::sleep(POLL).await;
        }
    }

    #[cfg(test)]
    pub(crate) fn is_idle(&self) -> bool {
        self.idle.load(Ordering::SeqCst)
    }
}

/// Installs the SIGTERM and SIGINT handler on the process's [`Stop`].
/// Changing a signal's disposition affects the whole process, so only the
/// binary calls this. A signal whose handler fails to register keeps its
/// default action.
#[cfg(unix)]
pub fn install() {
    let stop = Stop::process();
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        let received = Arc::clone(&stop.received);
        let idle = Arc::clone(&stop.idle);
        let recorded = usize::try_from(signal).unwrap_or(0);
        // Recording before reading `idle` pairs with `Stop::block_on` storing
        // `idle` before its last read of `received`, so a signal at the end
        // of a call is either seen by the call or takes its default action.
        let action = move || {
            received.store(recorded, Ordering::SeqCst);
            if idle.load(Ordering::SeqCst) {
                let _ = signal_hook::low_level::emulate_default_handler(signal);
            }
        };
        // SAFETY: the action only stores and loads atomics and re-raises the
        // signal with its default disposition, all async-signal-safe.
        let _ = unsafe { signal_hook::low_level::register(signal, action) };
    }
}

/// Windows has no SIGTERM; Ctrl+C keeps its default action.
#[cfg(not(unix))]
pub fn install() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
    }

    #[test]
    fn a_signal_during_the_call_drops_it() {
        let stop = Stop::default();
        let signaller = stop.clone();
        let rt = runtime();
        let started = Instant::now();
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            signaller.record(libc::SIGTERM);
        });

        let outcome = stop.block_on(&rt, std::future::pending::<()>());
        handle.join().expect("signaller thread");

        assert_eq!(
            outcome,
            Err(Cancelled {
                signal: libc::SIGTERM
            })
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(stop.is_idle());
    }

    #[test]
    fn a_recorded_signal_cancels_the_next_call_unpolled() {
        let stop = Stop::default();
        stop.record(libc::SIGINT);
        let rt = runtime();

        let outcome = stop.block_on(&rt, async { panic!("polled after cancellation") });

        assert_eq!(
            outcome,
            Err(Cancelled {
                signal: libc::SIGINT
            })
        );
    }

    #[test]
    fn the_call_is_marked_in_flight_while_it_runs() {
        let stop = Stop::default();
        let observer = stop.clone();
        let rt = runtime();

        let in_flight = stop.block_on(&rt, async move { !observer.is_idle() });

        assert_eq!(in_flight, Ok(true));
        assert!(stop.is_idle());
    }

    #[test]
    fn exit_code_is_128_plus_the_signal() {
        let term = Cancelled {
            signal: libc::SIGTERM,
        };
        let int = Cancelled {
            signal: libc::SIGINT,
        };
        assert_eq!(term.exit_code(), 143);
        assert_eq!(int.exit_code(), 130);
        assert_eq!(term.to_string(), "request cancelled by SIGTERM");
    }

    #[test]
    fn find_recovers_the_cancellation_through_an_xdk_error() {
        let err: Box<dyn std::error::Error + Send + Sync> = Box::new(xdk::Error::from(Cancelled {
            signal: libc::SIGTERM,
        }));
        assert_eq!(
            Cancelled::find(err.as_ref()),
            Some(Cancelled {
                signal: libc::SIGTERM
            })
        );
        let plain: Box<dyn std::error::Error + Send + Sync> = Box::new(xdk::Error::io("disk"));
        assert_eq!(Cancelled::find(plain.as_ref()), None);
    }
}
