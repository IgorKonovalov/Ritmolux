//! The browser's thumbnail cache (ADR-0230): where one preset's still lives on
//! disk, what makes an entry stale, and the headless mode that renders one.
//!
//! **The picture is rendered on this machine, by this executable.** The shipped
//! binary carries no thumbnails and no image codec — ADR-0011 keeps `image` a
//! dev-dependency — so a cache entry is a header and raw RGBA8 rows rather than
//! a PNG, and the only thing that can write one is a run of the player in the
//! `--thumb` mode below. That mode is spawned by the pass through
//! `current_exe()`, which is why its flag is in [`crate::cli::INTERNAL_FLAGS`]
//! and `--help` does not print it.
//!
//! **The key is the preset's name and the stamp is its file's**, which is the
//! identity ADR-0228 already uses for marks paired with the one fact that says
//! an entry is out of date. A preset edited in an `RLX_PRESET_DIR` library
//! changes its file's modification time or its length, so the entry stops
//! matching and the picture is re-rendered.
//!
//! **Nothing here can stop the app starting.** A cache directory that cannot be
//! created disables the feature and is reported once; user state that can refuse
//! to launch the app is worse than user state that is lost (NFR section 10).
//!
//! Every line the mode prints goes to **standard error**, the rule
//! `--list-presets` follows: standard output belongs to the frame pipe while a
//! sink is open (ADR-0176), and each of these lines is a diagnostic about what
//! the run did rather than a document anything parses.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, UNIX_EPOCH};

use rlx_core::preset::Preset;
use rlx_core::render::Tier;
use standalone::{APP_DIR_NAME, PresetDir, preset_data_root, resolve_preset_dir, shot};

/// The cache directory's name, under the per-user app directory that holds
/// `config.toml`, `marks.toml` and the diagnostics log.
pub(crate) const CACHE_DIR: &str = "thumbnails";

/// A cached still's size in pixels. 160x90 is the browser's tile and the size
/// ADR-0230 priced both the storage and the render at.
pub(crate) const THUMB_W: u32 = 160;
pub(crate) const THUMB_H: u32 = 90;

/// The clip a thumbnail is driven by: the one synthesized kind that rises and
/// falls, so a preset bound to a band has something to react to.
pub(crate) const THUMB_SIGNAL: &str = "dynamic:110";

/// The analysis hop the still is taken at — the hop the documentation gallery's
/// cards sit on, and the loudest of the synthesized clip's phrase.
///
/// It is a judgement rather than a measurement, and backlog 0254 records the
/// other side of it: an accumulating world has not developed by hop 300. The
/// default clip yields 375 hops, so this one exists in it.
pub(crate) const THUMB_HOP: u32 = 300;

/// A cache file's first four bytes, so a file that is not one is rejected on
/// sight rather than decoded into noise.
const MAGIC: [u8; 4] = *b"RLXT";

/// The layout version. An entry written by another version is discarded and
/// re-rendered: there is nothing here worth migrating, and reading a header one
/// field wider than expected would hand the browser someone else's pixels.
const FORMAT: u32 = 1;

/// Bytes before the name: magic, version, width, height, source length, source
/// mtime, name length.
const HEADER_LEN: usize = 4 + 4 + 4 + 4 + 8 + 8 + 4;

/// What makes a cache entry stale: the source file's modification time and its
/// length, together.
///
/// Both, because either alone is cheap to fool — an editor that rewrites a file
/// to the same length moves only the time, and a filesystem with a coarse clock
/// can leave two edits within one tick sharing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stamp {
    /// Modification time in nanoseconds since the Unix epoch.
    pub(crate) mtime_nanos: u64,
    /// The file's length in bytes.
    pub(crate) len: u64,
}

impl Stamp {
    /// The stamp a preset with **no file on disk** carries — the embedded set,
    /// which a launch holds when no directory yielded anything.
    ///
    /// A distinguished value rather than an `Option`, so the cache entry's
    /// layout is one shape: an embedded preset's picture is never stale, because
    /// nothing about it can change without a new build.
    pub(crate) const EMBEDDED: Stamp = Stamp {
        mtime_nanos: 0,
        len: 0,
    };

    /// The stamp of the file at `path`, or `None` when it cannot be read.
    ///
    /// A time before the epoch, or past what nanoseconds in a `u64` can carry,
    /// saturates rather than failing: neither is a real preset file, and a
    /// saturated stamp is still a stable one.
    pub(crate) fn of(path: &Path) -> Option<Stamp> {
        let meta = std::fs::metadata(path).ok()?;
        let nanos = meta
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .map(|since| since.as_nanos().min(u128::from(u64::MAX)) as u64)
            .unwrap_or(0);
        Some(Stamp {
            mtime_nanos: nanos,
            len: meta.len(),
        })
    }

    /// The stamp for a preset whose source is `source`: the file's when there is
    /// one and it can be read, [`Stamp::EMBEDDED`] otherwise.
    pub(crate) fn for_source(source: Option<&Path>) -> Stamp {
        source.and_then(Stamp::of).unwrap_or(Stamp::EMBEDDED)
    }
}

/// One cached still: which preset it is of, what its source looked like when it
/// was rendered, and the pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    /// The preset's name, as everywhere else (ADR-0228). Stored inside the file
    /// as well as encoded in its path, so a hash collision between two names is
    /// a miss rather than the wrong picture.
    pub(crate) name: String,
    pub(crate) stamp: Stamp,
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// `width * height * 4` bytes, RGBA8, no row padding — a
    /// [`CaptureImage`](rlx_core::render::CaptureImage)'s own layout.
    pub(crate) rgba: Vec<u8>,
}

impl Entry {
    /// The file's bytes: the fixed header, the name, then the rows.
    ///
    /// Little-endian throughout. The cache is per-machine and never shipped, so
    /// this is a choice of one order rather than a portability claim.
    pub(crate) fn encode(&self) -> Vec<u8> {
        let name = self.name.as_bytes();
        let mut out = Vec::with_capacity(HEADER_LEN + name.len() + self.rgba.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&FORMAT.to_le_bytes());
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&self.stamp.len.to_le_bytes());
        out.extend_from_slice(&self.stamp.mtime_nanos.to_le_bytes());
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&self.rgba);
        out
    }

    /// Decode `bytes`, or `None` when they are not an entry this build can read.
    ///
    /// Strict on every field, including the pixel count: a truncated file is the
    /// shape a half-written entry takes, and the browser must read it as absent
    /// rather than as a picture with a torn bottom edge.
    pub(crate) fn decode(bytes: &[u8]) -> Option<Entry> {
        if bytes.len() < HEADER_LEN || bytes[..4] != MAGIC {
            return None;
        }
        // Every offset below is inside the header the length check just
        // guaranteed, so the slices cannot fail — but they are fixed-width
        // reads of a file on disk, and an arithmetic slip here would panic in
        // the browser rather than miss.
        let u32_at = |at: usize| -> Option<u32> {
            Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
        };
        let u64_at = |at: usize| -> Option<u64> {
            Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
        };
        if u32_at(4)? != FORMAT {
            return None;
        }
        let width = u32_at(8)?;
        let height = u32_at(12)?;
        let len = u64_at(16)?;
        let mtime_nanos = u64_at(24)?;
        let name_len = u32_at(32)? as usize;

        let name_end = HEADER_LEN.checked_add(name_len)?;
        let name = String::from_utf8(bytes.get(HEADER_LEN..name_end)?.to_vec()).ok()?;
        let pixels = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        let rgba = bytes.get(name_end..name_end.checked_add(pixels)?)?;
        // Exactly the pixels, with nothing after them: trailing bytes mean this
        // file is not what its header says it is.
        if name_end + pixels != bytes.len() {
            return None;
        }
        Some(Entry {
            name,
            stamp: Stamp { mtime_nanos, len },
            width,
            height,
            rgba: rgba.to_vec(),
        })
    }
}

/// FNV-1a over the preset's name, so two names that reduce to the same slug
/// still land in different files.
fn name_hash(name: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The file one preset's entry is written to, inside the cache directory.
///
/// A slug for a human reading the directory, and a hash of the **whole** name
/// for correctness: preset names carry spaces and any punctuation an author
/// types, so the slug alone is neither unique nor always a legal filename.
pub(crate) fn entry_file_name(name: &str) -> String {
    let slug: String = name
        .chars()
        .take(40)
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    format!("{slug}-{:016x}.rlxthumb", name_hash(name))
}

/// The cache directory under the per-user app directory, or `None` when no OS
/// data root resolves — the same degradation `config.toml` and `marks.toml`
/// already take.
pub(crate) fn cache_dir() -> Option<PathBuf> {
    preset_data_root().map(|root| root.join(APP_DIR_NAME).join(CACHE_DIR))
}

/// The cache directory, created if absent, or the reason it is unavailable.
pub(crate) fn ensure_cache_dir() -> Result<PathBuf, String> {
    let dir = cache_dir().ok_or_else(|| "no per-user data directory resolved".to_owned())?;
    std::fs::create_dir_all(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    Ok(dir)
}

/// The one sentence a run writes when the cache is unavailable, so the pass and
/// the child cannot describe the same condition two ways.
pub(crate) fn unavailable_note(reason: &str) -> String {
    format!("thumbnails off: cannot use the cache directory ({reason})")
}

/// Where `name`'s entry lives inside `dir`.
pub(crate) fn entry_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(entry_file_name(name))
}

/// `name`'s cached still, or `None` when there is none this build can read.
///
/// The name inside the file has to match the one asked for: the path encodes a
/// hash, and a collision must read as a miss rather than as another preset's
/// picture.
pub(crate) fn read_entry(dir: &Path, name: &str) -> Option<Entry> {
    let bytes = std::fs::read(entry_path(dir, name)).ok()?;
    Entry::decode(&bytes).filter(|entry| entry.name == name)
}

/// The browser's reader: `name`'s cached still, whatever source it was taken
/// from, or `None` when there is none.
///
/// **Not held to the stamp.** A picture of a preset's previous version is what
/// the pane shows while the current one is rendered, because a slightly stale
/// picture says more than a placeholder. Held to the size, because a still of
/// another size drawn into the pane's rectangle would be a distorted one.
pub(crate) fn cached_still(name: &str) -> Option<Entry> {
    let dir = cache_dir()?;
    read_entry(&dir, name).filter(|entry| entry.width == THUMB_W && entry.height == THUMB_H)
}

/// Write `entry` into `dir`, **atomically**: the bytes go to a temporary file
/// beside the destination and are renamed onto it.
///
/// A rename within one directory either happens or does not, which is what
/// stops a reader from ever seeing a half-written entry — the picture is either
/// the old one or the new one. A run killed mid-write leaves the temporary file
/// behind, and it is not a name [`read_entry`] ever looks for.
pub(crate) fn write_entry(dir: &Path, entry: &Entry) -> Result<(), String> {
    let path = entry_path(dir, &entry.name);
    let temp = path.with_extension("rlxthumb-part");
    std::fs::write(&temp, entry.encode()).map_err(|err| format!("{}: {err}", temp.display()))?;
    std::fs::rename(&temp, &path).map_err(|err| {
        let _ = std::fs::remove_file(&temp);
        format!("{}: {err}", path.display())
    })
}

/// Whether `dir` already holds a picture of `name` taken from a source matching
/// `stamp`, at the size this build renders.
///
/// The size is part of it: a cache written before [`THUMB_W`] moved holds a
/// picture of the right preset at the wrong size, and drawing it would be the
/// browser quietly changing shape.
pub(crate) fn is_current(dir: &Path, name: &str, stamp: Stamp) -> bool {
    read_entry(dir, name).is_some_and(|entry| {
        entry.stamp == stamp && entry.width == THUMB_W && entry.height == THUMB_H
    })
}

/// The preset set this launch holds, resolved **without seeding and without
/// printing** — `crate::preset_dir::startup_preset_names`'s rule, because the
/// child has to see the roster the parent is showing: a directory that yields at
/// least one preset replaces the embedded set, and the answer is one list or the
/// other and never their union.
fn library() -> Vec<Preset> {
    let dir = match resolve_preset_dir() {
        PresetDir::Override(dir) | PresetDir::Default(dir) => dir,
        PresetDir::Unresolved => PathBuf::new(),
    };
    let from_dir = rlx_core::preset::load_dir(&dir).presets;
    if from_dir.is_empty() {
        rlx_core::preset::default_presets()
    } else {
        from_dir
    }
}

// ---------------------------------------------------------------------------
// The pass: the parent's side (ADR-0230)
// ---------------------------------------------------------------------------

/// How often the pass's worker looks at a running child and at the stop flag.
/// Also the bound on how long stopping the pass can hold up shutdown, before
/// the kill itself.
const CHILD_POLL: Duration = Duration::from_millis(50);

/// How many [`CHILD_POLL`]s one child may run before it is killed and counted
/// as failed: 60 s, an order of magnitude past the 5.65 s ADR-0230 measured
/// for one still, so a slow GPU is not cut off and a hung child cannot hold
/// the pass forever.
const CHILD_POLLS: u32 = 1200;

/// Failed renders in a row after which the pass stops for this launch. One
/// failure is a preset; several in a row are the machine — no second GPU
/// context, or a scanner refusing the spawns — and retrying into that is the
/// loop the pass must not become.
const GIVE_UP_AFTER: u32 = 3;

/// How far below normal a child runs on a Unix, as `nice`'s increment.
#[cfg(unix)]
const NICE: &str = "10";

/// `BELOW_NORMAL_PRIORITY_CLASS | CREATE_NO_WINDOW`.
#[cfg(windows)]
const CREATION_FLAGS: u32 = 0x0000_4000 | 0x0800_0000;

/// What the pass reports to the frame loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PassEvent {
    /// One line for `diagnostics.log`.
    Note(String),
    /// A picture of this preset was written; a pane showing it reads it again.
    Landed(String),
}

/// The background pass: walks the library on a worker thread and renders each
/// preset whose picture is missing or stale, one child process at a time.
///
/// **Nothing here blocks the frame loop.** The worker owns the child, the
/// library load and every file read; the frame loop only drains a channel with
/// [`drain`](Self::drain). **Nothing here blocks shutdown beyond one poll**:
/// dropping the pass raises the stop flag, the worker kills a running child at
/// its next poll, and the join waits for that. A child killed mid-write leaves
/// a temporary file no reader looks for, and the next pass discards it.
pub(crate) struct Pass {
    stop: Arc<AtomicBool>,
    events: Receiver<PassEvent>,
    worker: Option<JoinHandle<()>>,
}

impl Pass {
    /// Start the pass. A worker thread that cannot be spawned is reported as
    /// one note on the first drain and the pass is simply absent.
    pub(crate) fn start() -> Pass {
        Pass::spawn(walk)
    }

    /// A pass that renders exactly `names` with `exe` standing in for the
    /// player — the render loop without the library or the cache behind it.
    #[cfg(test)]
    fn start_with(exe: PathBuf, names: Vec<String>) -> Pass {
        Pass::spawn(move |stop, tx| render_all(&exe, &names, stop, tx))
    }

    fn spawn(work: impl FnOnce(&AtomicBool, &Sender<PassEvent>) + Send + 'static) -> Pass {
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, events) = mpsc::channel();
        let flag = Arc::clone(&stop);
        let worker = std::thread::Builder::new()
            .name("rlx-thumbnails".to_owned())
            .spawn(move || work(&flag, &tx));
        let worker = match worker {
            Ok(handle) => Some(handle),
            Err(err) => {
                let (tx, rx) = mpsc::channel();
                let _ = tx.send(PassEvent::Note(format!(
                    "thumbnails off: cannot start the pass ({err})"
                )));
                return Pass {
                    stop,
                    events: rx,
                    worker: None,
                };
            }
        };
        Pass {
            stop,
            events,
            worker,
        }
    }

    /// Hand every event the worker has sent since the last call to `each`.
    /// Never waits.
    pub(crate) fn drain(&mut self, mut each: impl FnMut(PassEvent)) {
        while let Ok(event) = self.events.try_recv() {
            each(event);
        }
    }

    /// Stop the pass: kill a running child and wait for the worker to leave.
    /// Idempotent.
    pub(crate) fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for Pass {
    fn drop(&mut self) {
        self.stop();
    }
}

/// How one child ended.
enum Render {
    Wrote,
    Failed(String),
    /// The pass was stopped while it ran; the child was killed.
    Stopped,
    /// The process could not be started at all.
    Unstartable(String),
}

/// The worker: resolve the cache and the library, then render what is missing
/// in roster order, reporting through `tx`.
fn walk(stop: &AtomicBool, tx: &Sender<PassEvent>) {
    let note = |line: String| {
        let _ = tx.send(PassEvent::Note(line));
    };
    let dir = match ensure_cache_dir() {
        Ok(dir) => dir,
        Err(reason) => return note(unavailable_note(&reason)),
    };
    discard_partials(&dir);
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(err) => {
            return note(format!(
                "thumbnails off: cannot locate this executable ({err})"
            ));
        }
    };

    let presets = library();
    let total = presets.len();
    let missing: Vec<String> = presets
        .iter()
        .filter(|preset| {
            !is_current(
                &dir,
                &preset.name,
                Stamp::for_source(preset.source.as_deref()),
            )
        })
        .map(|preset| preset.name.clone())
        .collect();
    drop(presets);
    if missing.is_empty() {
        return;
    }
    note(format!(
        "thumbnail pass: start, {} of {total} presets to render",
        missing.len()
    ));
    render_all(&exe, &missing, stop, tx);
}

/// Render each of `missing` in turn, one child at a time, until the list is
/// done, the pass is stopped, or it gives up.
fn render_all(exe: &Path, missing: &[String], stop: &AtomicBool, tx: &Sender<PassEvent>) {
    let note = |line: String| {
        let _ = tx.send(PassEvent::Note(line));
    };
    let (mut rendered, mut failed, mut streak) = (0usize, 0usize, 0u32);
    let mut plain_priority = false;
    for name in missing {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match render_child(exe, name, stop, &mut plain_priority, &note) {
            Render::Wrote => {
                rendered += 1;
                streak = 0;
                let _ = tx.send(PassEvent::Landed(name.clone()));
            }
            Render::Failed(reason) => {
                failed += 1;
                streak += 1;
                note(format!("thumbnail failed: {name}: {reason}"));
                if streak >= GIVE_UP_AFTER {
                    return note(format!(
                        "thumbnail pass: gave up after {streak} failures in a row; \
                         {rendered} rendered this launch, the rest wait for the next"
                    ));
                }
            }
            Render::Stopped => {
                return note(format!(
                    "thumbnail pass: stopped, {rendered} rendered, {} left for the next launch",
                    missing.len() - rendered - failed
                ));
            }
            Render::Unstartable(reason) => {
                return note(format!("thumbnails off: cannot start a render ({reason})"));
            }
        }
    }
    note(format!(
        "thumbnail pass: done, {rendered} rendered, {failed} failed"
    ));
}

/// Remove the temporary files a killed child left behind, so a half-written
/// entry is discarded rather than kept. [`read_entry`] never opens one either
/// way; this is housekeeping, not correctness.
fn discard_partials(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "rlxthumb-part") {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// The command that renders `name` in a child, at low OS priority.
///
/// On a Unix the priority is `nice`'s, because the standard library has no call
/// for it; `plain` spawns the executable directly, for a system with no `nice`.
fn child_command(exe: &Path, name: &str, plain: bool) -> Command {
    #[cfg(unix)]
    let mut command = if plain {
        Command::new(exe)
    } else {
        let mut command = Command::new("nice");
        command.arg("-n").arg(NICE).arg(exe);
        command
    };
    #[cfg(not(unix))]
    let mut command = {
        let _ = plain;
        Command::new(exe)
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATION_FLAGS);
    }
    command
        .arg("--thumb")
        .arg(name)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    command
}

/// Render `name` in one child and wait for it, polling so the stop flag is
/// honoured while it runs.
fn render_child(
    exe: &Path,
    name: &str,
    stop: &AtomicBool,
    plain: &mut bool,
    note: &impl Fn(String),
) -> Render {
    let mut child = match child_command(exe, name, *plain).spawn() {
        Ok(child) => child,
        // No `nice` on this system: run at normal priority, and say so once.
        Err(err) if !*plain && err.kind() == std::io::ErrorKind::NotFound => {
            *plain = true;
            note("thumbnail pass: `nice` not found, renders run at normal priority".to_owned());
            match child_command(exe, name, true).spawn() {
                Ok(child) => child,
                Err(err) => return Render::Unstartable(err.to_string()),
            }
        }
        Err(err) => return Render::Unstartable(err.to_string()),
    };
    // Drained on its own thread so a chatty child cannot fill the pipe and
    // stall; read after it exits for the failure's reason.
    let stderr = child.stderr.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = pipe.read_to_string(&mut text);
            text
        })
    });
    let reason = |stderr: Option<JoinHandle<String>>| {
        stderr
            .and_then(|reader| reader.join().ok())
            .and_then(|text| {
                text.lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .map(str::to_owned)
            })
    };

    for _ in 0..CHILD_POLLS {
        if stop.load(Ordering::Relaxed) {
            kill(&mut child);
            return Render::Stopped;
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Render::Wrote,
            Ok(Some(status)) => {
                return Render::Failed(reason(stderr).unwrap_or_else(|| status.to_string()));
            }
            Ok(None) => std::thread::sleep(CHILD_POLL),
            Err(err) => {
                kill(&mut child);
                return Render::Failed(err.to_string());
            }
        }
    }
    kill(&mut child);
    Render::Failed(format!(
        "no picture after {} s",
        CHILD_POLL.as_millis() * u128::from(CHILD_POLLS) / 1000
    ))
}

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// The `--thumb` mode's exit code, or `None` when this run is not that mode and
/// the ordinary launch path should proceed.
///
/// Read before anything else in `main`, because this mode opens no window, binds
/// no socket and starts no capture client — it is the same "answer and exit"
/// shape `--schema` and `--check` have, one level earlier because its caller is
/// the player itself rather than an operator.
pub(crate) fn child_mode() -> Option<i32> {
    match crate::cli::parse_thumb_arg() {
        Ok(None) => None,
        Ok(Some(name)) => Some(render_one(&name)),
        Err(message) => {
            eprintln!("{message}");
            Some(2)
        }
    }
}

/// Render `name`'s still into the cache and report what happened.
///
/// Exit codes follow the launch path's own split: 2 for an argument list that is
/// wrong in shape — a preset name no library holds — and 1 for a recognized
/// request whose effect failed.
fn render_one(name: &str) -> i32 {
    let dir = match ensure_cache_dir() {
        Ok(dir) => dir,
        Err(reason) => {
            eprintln!("{}", unavailable_note(&reason));
            return 1;
        }
    };

    let presets = library();
    let Some(preset) = presets.iter().find(|preset| preset.name == name) else {
        eprintln!("--thumb `{name}`: no preset by that name in this library");
        return 2;
    };
    let stamp = Stamp::for_source(preset.source.as_deref());

    // The no-op arm, and it **says why**: the pass re-invokes this once per
    // preset on every launch, so "nothing happened" has to be distinguishable
    // from "the render failed silently" by whoever is reading the output.
    if is_current(&dir, name, stamp) {
        eprintln!(
            "up to date: {} ({name} has not changed since it was rendered)",
            entry_path(&dir, name).display()
        );
        return 0;
    }

    let (pcm, format) = match shot::args::synth_signal(THUMB_SIGNAL) {
        Ok(clip) => clip,
        Err(message) => {
            eprintln!("--thumb `{name}`: {message}");
            return 1;
        }
    };
    // The floor tier, pinned. At 160x90 the rich tier's raised budgets are not
    // visible and the pass is running beside a show that wants the GPU.
    let mut renderer = match shot::renderer(THUMB_W, THUMB_H, presets, Tier::Floor) {
        Ok(renderer) => renderer,
        Err(message) => {
            eprintln!("--thumb `{name}`: {message}");
            return 1;
        }
    };
    let frames = match renderer.capture_audio(name, &pcm, format, &[THUMB_HOP]) {
        Ok(frames) => frames,
        Err(err) => {
            eprintln!("--thumb `{name}`: capture: {err}");
            return 1;
        }
    };
    let Some(image) = frames.into_iter().next() else {
        eprintln!("--thumb `{name}`: hop {THUMB_HOP} produced no frame");
        return 1;
    };

    let entry = Entry {
        name: name.to_owned(),
        stamp,
        width: image.width,
        height: image.height,
        rgba: image.rgba,
    };
    if let Err(message) = write_entry(&dir, &entry) {
        eprintln!("{}", unavailable_note(&message));
        return 1;
    }
    eprintln!(
        "wrote {} ({}x{}, preset {name}, hop {THUMB_HOP}, signal {THUMB_SIGNAL})",
        entry_path(&dir, name).display(),
        entry.width,
        entry.height
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch cache directory under the OS temp dir, emptied first so a
    /// previous run's leftovers cannot decide the outcome.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rlx-thumbs-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the scratch cache");
        dir
    }

    fn sample(name: &str, stamp: Stamp) -> Entry {
        Entry {
            name: name.to_owned(),
            stamp,
            width: 2,
            height: 2,
            rgba: (0..16).map(|b| b as u8).collect(),
        }
    }

    /// **The walking skeleton's claim**: an entry written now is the same entry
    /// after the file. The bytes are the only thing between the two.
    #[test]
    fn an_entry_survives_the_file() {
        let dir = scratch("round-trip");
        let stamp = Stamp {
            mtime_nanos: 1_700_000_000_123_456_789,
            len: 4096,
        };
        let entry = sample("Echo Plate", stamp);
        write_entry(&dir, &entry).expect("write the entry");

        let back = read_entry(&dir, "Echo Plate").expect("the entry reads back");
        assert_eq!(back, entry, "the file is a faithful copy of the entry");

        // And one name's entry does not answer for another's.
        assert_eq!(read_entry(&dir, "Lace Grid"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A truncated or foreign file reads as absent.** That is the shape a
    /// half-written entry takes, and the browser must show its placeholder
    /// rather than a picture with a torn edge.
    #[test]
    fn a_file_that_is_not_an_entry_reads_as_no_entry() {
        let dir = scratch("malformed");
        let entry = sample("Seahorse", Stamp::EMBEDDED);
        let bytes = entry.encode();

        for cut in [0, HEADER_LEN, bytes.len() - 1] {
            std::fs::write(entry_path(&dir, "Seahorse"), &bytes[..cut]).expect("write a stub");
            assert_eq!(
                read_entry(&dir, "Seahorse"),
                None,
                "{cut} bytes of an entry read as an entry"
            );
        }

        // Trailing bytes are the other half: the header would parse and the
        // pixels would be someone else's.
        let mut extra = bytes.clone();
        extra.push(0);
        assert_eq!(Entry::decode(&extra), None);

        // A file of the right length that is not one of ours at all.
        std::fs::write(entry_path(&dir, "Seahorse"), vec![0u8; bytes.len()])
            .expect("write a foreign file");
        assert_eq!(read_entry(&dir, "Seahorse"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **The stamp is what makes an entry stale**, and the size is what makes it
    /// unusable — the two conditions `is_current` answers for.
    #[test]
    fn an_entry_is_current_only_for_the_stamp_and_the_size_it_was_written_at() {
        let dir = scratch("stale");
        let stamp = Stamp {
            mtime_nanos: 42,
            len: 900,
        };
        let mut entry = sample("Gyre", stamp);
        entry.width = THUMB_W;
        entry.height = THUMB_H;
        entry.rgba = vec![7; (THUMB_W * THUMB_H * 4) as usize];
        write_entry(&dir, &entry).expect("write the entry");

        assert!(is_current(&dir, "Gyre", stamp));
        // An edit that moved only the time, and one that moved only the length.
        assert!(!is_current(
            &dir,
            "Gyre",
            Stamp {
                mtime_nanos: 43,
                ..stamp
            }
        ));
        assert!(!is_current(&dir, "Gyre", Stamp { len: 901, ..stamp }));
        // A preset with no entry at all is not current either.
        assert!(!is_current(&dir, "Nothing Here", stamp));

        // The same picture at a size other than the one this build renders.
        let small = sample("Gyre", stamp);
        write_entry(&dir, &small).expect("write the small entry");
        assert!(
            !is_current(&dir, "Gyre", stamp),
            "an entry at the wrong size must not count as current"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A name is not a filename.** Preset names carry spaces and punctuation,
    /// so the path is a slug plus a hash of the whole name — and two names that
    /// slug alike still get two files.
    #[test]
    fn the_file_name_is_legal_and_unique_per_name() {
        let awkward = "A/B: 50% \"wide\"  *";
        let file = entry_file_name(awkward);
        assert!(
            file.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.'),
            "`{file}` is not a filename every filesystem here accepts"
        );
        assert_ne!(
            entry_file_name("Echo Plate"),
            entry_file_name("Echo/Plate"),
            "two names that slug alike must not share a file"
        );
        assert_eq!(
            entry_file_name("Echo Plate"),
            entry_file_name("Echo Plate"),
            "the same name must resolve to the same file on every launch"
        );
        // A name far longer than the slug budget still yields one path.
        let long = "x".repeat(400);
        assert!(entry_file_name(&long).len() < 80);
    }

    /// An executable shell script standing in for the player: it is run as
    /// `<script> --thumb <name>`, so `$2` is the preset.
    #[cfg(unix)]
    fn stub(dir: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("stub.sh");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write the stub");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("make the stub executable");
        path
    }

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    /// Let the worker run out on its own — no stop — and take what it said.
    fn finish(mut pass: Pass) -> Vec<PassEvent> {
        if let Some(worker) = pass.worker.take() {
            worker.join().expect("the worker does not panic");
        }
        let mut events = Vec::new();
        pass.drain(|event| events.push(event));
        events
    }

    fn notes(events: &[PassEvent]) -> Vec<&str> {
        events
            .iter()
            .filter_map(|event| match event {
                PassEvent::Note(line) => Some(line.as_str()),
                PassEvent::Landed(_) => None,
            })
            .collect()
    }

    /// **At most one child runs at a time, and the pass stops when the list is
    /// covered.** Each stub takes a lock directory for its whole run and fails
    /// if another holds it, so an overlap would surface as a failure; every
    /// preset lands, in order, and the worker leaves on its own.
    #[cfg(unix)]
    #[test]
    fn the_pass_runs_one_child_at_a_time_and_ends_when_covered() {
        let dir = scratch("one-at-a-time");
        let lock = dir.join("lock");
        let exe = stub(
            &dir,
            &format!(
                "mkdir '{lock}' || exit 7\nsleep 0.1\nrmdir '{lock}'\nexit 0",
                lock = lock.display()
            ),
        );
        let events = finish(Pass::start_with(exe, names(&["A", "B", "C", "D"])));

        let landed: Vec<&str> = events
            .iter()
            .filter_map(|event| match event {
                PassEvent::Landed(name) => Some(name.as_str()),
                PassEvent::Note(_) => None,
            })
            .collect();
        assert_eq!(landed, ["A", "B", "C", "D"], "notes: {:?}", notes(&events));
        assert_eq!(
            notes(&events).last().copied(),
            Some("thumbnail pass: done, 4 rendered, 0 failed")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **The pass gives up rather than looping.** A child that fails is named
    /// once, with the reason it printed, and is not retried; after
    /// [`GIVE_UP_AFTER`] failures in a row the pass stops and says so, and the
    /// presets after that are not attempted this launch.
    #[cfg(unix)]
    #[test]
    fn failing_children_are_named_once_and_the_pass_gives_up() {
        let dir = scratch("give-up");
        let exe = stub(&dir, "echo \"no adapter for $2\" >&2\nexit 3");
        let events = finish(Pass::start_with(exe, names(&["A", "B", "C", "D", "E"])));
        let notes = notes(&events);

        for name in ["A", "B", "C"] {
            let lines: Vec<&&str> = notes
                .iter()
                .filter(|line| line.starts_with(&format!("thumbnail failed: {name}:")))
                .collect();
            assert_eq!(lines.len(), 1, "{name} is named once: {notes:?}");
            assert!(
                lines[0].ends_with(&format!("no adapter for {name}")),
                "the child's own reason is carried: {}",
                lines[0]
            );
        }
        assert!(
            !notes
                .iter()
                .any(|line| line.contains(": D:") || line.contains(": E:")),
            "the pass kept going after giving up: {notes:?}"
        );
        assert!(
            notes
                .last()
                .is_some_and(|line| line.starts_with("thumbnail pass: gave up after 3")),
            "{notes:?}"
        );
        assert!(!events.iter().any(|e| matches!(e, PassEvent::Landed(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **Stopping the pass kills the running child and does not wait it out.**
    /// The stub would run for a minute; the stop returns within a few polls,
    /// and the child's process is gone.
    #[cfg(unix)]
    #[test]
    fn stopping_the_pass_kills_the_running_child() {
        let dir = scratch("stop");
        let pid = dir.join("pid");
        let exe = stub(
            &dir,
            &format!("echo $$ > '{}'\nexec sleep 60", pid.display()),
        );
        let mut pass = Pass::start_with(exe, names(&["A", "B"]));
        // Wait for the child to be running, by its own pid file.
        for _ in 0..200 {
            if std::fs::read_to_string(&pid).is_ok_and(|text| !text.trim().is_empty()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let child = std::fs::read_to_string(&pid).expect("the child started");
        let child = child.trim();

        let (done_tx, done_rx) = mpsc::channel();
        std::thread::spawn(move || {
            pass.stop();
            let mut events = Vec::new();
            pass.drain(|event| events.push(event));
            let _ = done_tx.send(events);
        });
        let events = done_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("stopping the pass waited out a child that runs for a minute");
        assert!(
            notes(&events)
                .last()
                .is_some_and(|line| line.starts_with("thumbnail pass: stopped")),
            "{:?}",
            notes(&events)
        );
        assert!(
            !Path::new(&format!("/proc/{child}")).exists() || !cfg!(target_os = "linux"),
            "child {child} outlived the pass"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A half-written entry is discarded**: the temporary file a killed child
    /// leaves is removed by the next pass, and an entry beside it is kept.
    #[test]
    fn the_pass_discards_what_a_killed_child_left() {
        let dir = scratch("partials");
        let entry = sample("Gyre", Stamp::EMBEDDED);
        write_entry(&dir, &entry).expect("write the entry");
        let partial = entry_path(&dir, "Lace Grid").with_extension("rlxthumb-part");
        std::fs::write(&partial, b"RLXT half").expect("write a partial");

        discard_partials(&dir);
        assert!(!partial.exists(), "the half-written file survived");
        assert_eq!(
            read_entry(&dir, "Gyre"),
            Some(entry),
            "a whole entry went with it"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A preset with no file on disk stamps as [`Stamp::EMBEDDED`], and one with
    /// a file stamps as that file — the two cases `for_source` exists to keep
    /// apart.
    #[test]
    fn a_preset_with_no_file_stamps_as_embedded() {
        assert_eq!(Stamp::for_source(None), Stamp::EMBEDDED);
        assert_eq!(
            Stamp::for_source(Some(Path::new("no/such/preset.toml"))),
            Stamp::EMBEDDED,
            "an unreadable path is the embedded case, not a failure"
        );

        let dir = scratch("stamp");
        let file = dir.join("one.toml");
        let text = "name = \"One\"\n";
        std::fs::write(&file, text).expect("write a preset file");

        let stamp = Stamp::for_source(Some(&file));
        assert_eq!(stamp.len, text.len() as u64);
        assert_ne!(
            stamp,
            Stamp::EMBEDDED,
            "a file that exists must not stamp as the embedded set"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
