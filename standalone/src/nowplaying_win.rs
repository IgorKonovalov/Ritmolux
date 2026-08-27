//! Windows now-playing metadata via SMTC (Plan 0097 Phase 2, ADR-0110).
//!
//! `GlobalSystemMediaTransportControlsSessionManager` is the same feed that
//! drives the OS media flyout, so whatever app is publishing now-playing —
//! foobar2000 included — reports through it. The shell's only job is to turn
//! that into the one string the core's banner takes; the core never learns SMTC
//! exists.
//!
//! **This is not the audio callback and must never become it.** The WinRT event
//! handlers below allocate freely — they format a string and take an
//! uncontended lock — because they run on a WinRT thread-pool thread, not on
//! the capture loop. What they must *not* do is call into the renderer: the
//! string is left in a slot the render thread picks up on its next frame.
//!
//! # Why a dedicated thread
//!
//! WinRT needs an initialized apartment, and the two obvious ones are both
//! taken: the capture thread is real-time and off limits, and the winit event
//! loop is an STA (winit calls `OleInitialize` for drag-and-drop), which an
//! `MTA` init would fight. So this owns a thread whose entire job is to hold a
//! multithreaded apartment and keep the manager — and therefore its event
//! registrations — alive. It parks after setup and costs nothing per frame.
//!
//! # Failure is silence
//!
//! No session, no player, a title-less stream, a denied permission and any
//! WinRT error all mean the same thing: no banner (NFR §10). Every fallible
//! step below discards its error and returns, and nothing here logs per frame.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use lmv_core::render::now_playing::SEPARATOR;
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager, MediaPropertiesChangedEventArgs,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::core::Ref;

/// The seam between the WinRT callback threads and the render thread: a slot
/// holding the most recent string nobody has drawn yet.
///
/// A `Mutex` rather than a channel because only the newest track matters — if
/// three changes land between two frames, the render thread should see the
/// third and not replay the first two.
#[derive(Default)]
struct Slot {
    pending: Mutex<Option<String>>,
}

impl Slot {
    /// Publish `text` for the render thread. A poisoned lock is dropped on the
    /// floor rather than propagated: a missing banner is the correct
    /// degradation, and this is a UI nicety, not state anything depends on.
    fn publish(&self, text: String) {
        if let Ok(mut slot) = self.pending.lock() {
            *slot = Some(text);
        }
    }

    fn take(&self) -> Option<String> {
        self.pending.lock().ok()?.take()
    }
}

/// The live metadata source. Dropping it releases the SMTC subscriptions.
pub struct NowPlayingSource {
    slot: Arc<Slot>,
    stop: Arc<AtomicBool>,
    /// Kept only so [`Drop`] can unpark the worker. `None` if the spawn failed,
    /// which is one more way this degrades to no banner rather than to an error.
    worker: Option<std::thread::Thread>,
}

impl NowPlayingSource {
    /// Start watching SMTC on a dedicated thread. Returns a handle immediately —
    /// the WinRT setup (`RequestAsync`, which blocks) happens on that thread, so
    /// a slow or absent media session cannot stall the window opening.
    ///
    /// Never fails: if the thread cannot spawn or WinRT refuses, the source
    /// simply never reports anything.
    pub fn start() -> Self {
        let slot = Arc::new(Slot::default());
        let stop = Arc::new(AtomicBool::new(false));

        let worker_slot = Arc::clone(&slot);
        let worker_stop = Arc::clone(&stop);
        let worker = std::thread::Builder::new()
            .name("lmv-nowplaying".to_owned())
            .spawn(move || watch(&worker_slot, &worker_stop))
            .ok()
            .map(|handle| handle.thread().clone());

        Self { slot, stop, worker }
    }

    /// The track that changed since the last call, if any. Called once per
    /// frame from the render thread; an uncontended lock and no allocation in
    /// the common (nothing-changed) case.
    pub fn take_change(&self) -> Option<String> {
        self.slot.take()
    }
}

impl Drop for NowPlayingSource {
    fn drop(&mut self) {
        // Set the flag *then* unpark, so the worker sees it on the wake it is
        // about to get. Deliberately not joined: the worker may be blocked
        // inside `RequestAsync`, and a shell that hangs on exit waiting for a
        // metadata query is a worse bug than a thread the process reaps.
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = &self.worker {
            worker.unpark();
        }
    }
}

/// What the worker thread holds for the life of the process: the slot to
/// publish into, and the session it is currently subscribed to.
struct Watcher {
    slot: Arc<Slot>,
    stop: Arc<AtomicBool>,
    /// The session whose `MediaPropertiesChanged` we hold a registration on, so
    /// a session change can unsubscribe before subscribing to the next one.
    /// Without this, switching players would leave a handler on a dead session
    /// reporting a track nobody is playing.
    bound: Mutex<Option<(Session, i64)>>,
}

/// Set up the SMTC subscription and then park. Every step degrades to silence.
fn watch(slot: &Arc<Slot>, stop: &Arc<AtomicBool>) {
    // A multithreaded apartment of this thread's own — see the module docs for
    // why neither of the app's existing threads can host this. The `HRESULT` is
    // ignored: `S_FALSE` (already initialized) is success for our purposes and
    // a hard failure surfaces as the `RequestAsync` error below.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };

    let Ok(manager) = Manager::RequestAsync().and_then(|op| op.join()) else {
        return; // no SMTC on this system, or it refused — no banner
    };

    let watcher = Arc::new(Watcher {
        slot: Arc::clone(slot),
        stop: Arc::clone(stop),
        bound: Mutex::new(None),
    });

    // The active player changed (or the user switched apps). Re-bind to
    // whatever is current and report what it is playing.
    let on_session = Arc::clone(&watcher);
    let handler = TypedEventHandler::new(
        move |sender: Ref<'_, Manager>, _: Ref<'_, CurrentSessionChangedEventArgs>| {
            if let Some(manager) = sender.as_ref() {
                rebind(manager, &on_session);
            }
            Ok(())
        },
    );
    if manager.CurrentSessionChanged(&handler).is_err() {
        return; // cannot follow track changes — better no banner than a stale one
    }

    // The session that already exists at startup fires no event, so read it once.
    rebind(&manager, &watcher);

    // Park forever, holding `manager` — and with it every registration above —
    // alive. `park` rather than a sleep loop: this thread has no periodic work,
    // which is the point of using events at all.
    while !watcher.stop.load(Ordering::Relaxed) {
        std::thread::park();
    }
}

/// Point the property subscription at the manager's current session and publish
/// what it is playing.
fn rebind(manager: &Manager, watcher: &Arc<Watcher>) {
    // Drop the previous registration first: a handler left on a superseded
    // session would keep reporting a track that has stopped playing.
    if let Ok(mut bound) = watcher.bound.lock()
        && let Some((session, token)) = bound.take()
    {
        let _ = session.RemoveMediaPropertiesChanged(token);
    }

    let Ok(session) = manager.GetCurrentSession() else {
        // Every player closed. Clear the banner rather than leave the last
        // track announced.
        watcher.slot.publish(String::new());
        return;
    };

    let on_properties = Arc::clone(watcher);
    let handler = TypedEventHandler::new(
        move |sender: Ref<'_, Session>, _: Ref<'_, MediaPropertiesChangedEventArgs>| {
            if let Some(session) = sender.as_ref() {
                publish_current(session, &on_properties.slot);
            }
            Ok(())
        },
    );
    if let Ok(token) = session.MediaPropertiesChanged(&handler)
        && let Ok(mut bound) = watcher.bound.lock()
    {
        *bound = Some((session.clone(), token));
    }

    publish_current(&session, &watcher.slot);
}

/// Read the session's artist and title and hand the joined string over.
fn publish_current(session: &Session, slot: &Slot) {
    let Ok(properties) = session
        .TryGetMediaPropertiesAsync()
        .and_then(|op| op.join())
    else {
        return; // a transient read failure is not evidence the track changed
    };

    let artist = properties
        .Artist()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let title = properties
        .Title()
        .map(|s| s.to_string())
        .unwrap_or_default();
    slot.publish(join(artist.trim(), title.trim()));
}

/// Join the two fields the way the plugin's `titleformat` will, so both sources
/// hand the core the same shape and its one split rule serves both.
///
/// A title-less stream (internet radio often reports neither) publishes an
/// empty string, which clears the banner rather than announcing a blank.
fn join(artist: &str, title: &str) -> String {
    match (artist.is_empty(), title.is_empty()) {
        (_, true) => String::new(),
        (true, false) => title.to_owned(),
        (false, false) => format!("{artist}{SEPARATOR}{title}"),
    }
}

#[cfg(test)]
mod tests {
    use super::join;

    #[test]
    fn both_fields_join_with_the_separator_the_core_splits_on() {
        assert_eq!(
            join("Boards of Canada", "Roygbiv"),
            "Boards of Canada - Roygbiv"
        );
    }

    #[test]
    fn a_missing_artist_leaves_the_title_alone() {
        // Rather than " - Roygbiv", which the core would draw as a lone title
        // line with a leading separator.
        assert_eq!(join("", "Roygbiv"), "Roygbiv");
    }

    #[test]
    fn a_missing_title_announces_nothing() {
        // Internet radio routinely reports an artist and no title; an empty
        // string clears the banner rather than announcing a blank line.
        assert_eq!(join("Some Station", ""), "");
        assert_eq!(join("", ""), "");
    }
}
