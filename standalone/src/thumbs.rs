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

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

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

        // The same picture at a size this build no longer renders.
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
