//! [`PresetError`]: why a preset failed to load.
//!
//! Every variant is recoverable. A bad preset returns `Err` and never panics,
//! so the caller degrades to the last good preset (ADR-0002 / NFR 10).

// A continuation of one module split across several files, so it needs the
// names `preset/schema/mod.rs` has in scope.
use super::*;

/// Why a preset failed to load. Every variant is recoverable — the caller
/// keeps the previous good preset.
#[derive(Debug)]
pub enum PresetError {
    /// The TOML itself was malformed.
    Toml(toml::de::Error),
    /// `system` named a built-in that does not exist.
    UnknownSystem(String),
    /// A parameter's expression failed to compile.
    Expr {
        /// The parameter whose expression was invalid.
        param: String,
        /// The compile error.
        err: ExprError,
    },
    /// A structural-config table (`[curve]`/`[generator]`) was invalid — an
    /// unknown family, an out-of-range value, an undefined grammar symbol.
    Config(String),
    /// The preset file could not be read (message from the I/O error).
    Io(String),
}

impl fmt::Display for PresetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PresetError::Toml(e) => write!(f, "invalid preset TOML: {e}"),
            PresetError::UnknownSystem(s) => write!(f, "unknown system '{s}'"),
            PresetError::Expr { param, err } => {
                write!(f, "parameter '{param}' has an invalid expression: {err}")
            }
            PresetError::Config(msg) => write!(f, "invalid structural config: {msg}"),
            PresetError::Io(msg) => write!(f, "could not read preset file: {msg}"),
        }
    }
}

impl std::error::Error for PresetError {}
