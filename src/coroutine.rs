//! # Generator-shape coroutine driver
//!
//! Mirrors the shape of `core::ops::Coroutine`: a `Yield` associated
//! type for intermediate progress, a `Return` associated type for
//! terminal output, and a two-variant [`VdirCoroutineState`]
//! (`Yielded` / `Complete`).
//!
//! io-vdir is filesystem-flavoured, so every coroutine in the crate
//! picks the standard [`VdirYield`] directly: it gathers every
//! filesystem primitive the crate emits, namely directory create /
//! remove / read / exists, file create / read / exists / remove, path
//! rename / copy, plus the random bytes input needed to mint new item
//! identifiers.
//!
//! [`VdirClient::run`] drives any standard-Yield coroutine to
//! completion against the local filesystem.
//!
//! [`VdirClient::run`]: crate::client::VdirClient::run

use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};

use crate::path::VdirPath;

/// State yielded by a [`VdirCoroutine::resume`] step.
///
/// Two-variant by design (matches std's `core::ops::CoroutineState`):
/// any further variation lives inside the per-coroutine `Yield` type.
#[derive(Debug)]
pub enum VdirCoroutineState<Y, R> {
    /// Intermediate yield. The driver reacts to `Y` (perform the
    /// requested filesystem op, supply random bytes) and resumes the
    /// coroutine again.
    Yielded(Y),
    /// Terminal yield. By convention `R = Result<Output, Error>`.
    Complete(R),
}

/// Standard-shape Vdir coroutine.
///
/// Implementors own their internal state machine and declare their
/// per-step `Yield` plus a terminal `Return`. The driver reacts to
/// each `Yield` variant and resumes until `Complete`.
pub trait VdirCoroutine {
    /// Intermediate value handed back on every step. Per-coroutine:
    /// each implementor picks exactly the variants it needs. In
    /// io-vdir every coroutine picks [`VdirYield`].
    type Yield;
    /// Terminal value. By convention `Result<Output, Error>`; the "ok"
    /// arm carries the operation's final output, the "error" arm
    /// carries the cause.
    type Return;

    /// Advances the coroutine one step.
    ///
    /// Pass [`None`] on the initial call. Pass `Some(arg)` carrying the
    /// value matching the previous `Yielded` variant.
    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return>;
}

/// Standard filesystem Yield. Every io-vdir coroutine picks `type Yield
/// = VdirYield`.
///
/// Each variant is paired with the matching [`VdirReply`] variant the
/// driver feeds back on the next `resume`.
#[derive(Debug)]
pub enum VdirYield {
    /// Driver must supply `len` random bytes and feed back
    /// [`VdirReply::Random`].
    WantsRandom { len: usize },

    /// Driver must check each path for existence as a regular file and
    /// feed back [`VdirReply::FileExists`].
    WantsFileExists(BTreeSet<VdirPath>),

    /// Driver must check each path for existence as a directory and
    /// feed back [`VdirReply::DirExists`].
    WantsDirExists(BTreeSet<VdirPath>),

    /// Driver must list each directory's entries and feed back
    /// [`VdirReply::DirRead`].
    WantsDirRead(BTreeSet<VdirPath>),

    /// Driver must read each file's bytes and feed back
    /// [`VdirReply::FileRead`].
    WantsFileRead(BTreeSet<VdirPath>),

    /// Driver must write each `(path, bytes)` pair and feed back
    /// [`VdirReply::FileCreate`].
    WantsFileCreate(BTreeMap<VdirPath, Vec<u8>>),

    /// Driver must create each directory (with parents) and feed back
    /// [`VdirReply::DirCreate`].
    WantsDirCreate(BTreeSet<VdirPath>),

    /// Driver must recursively remove each directory and feed back
    /// [`VdirReply::DirRemove`].
    WantsDirRemove(BTreeSet<VdirPath>),

    /// Driver must remove each file and feed back
    /// [`VdirReply::FileRemove`].
    WantsFileRemove(BTreeSet<VdirPath>),

    /// Driver must rename each `(from, to)` pair and feed back
    /// [`VdirReply::Rename`].
    WantsRename(Vec<(VdirPath, VdirPath)>),

    /// Driver must copy each `(from, to)` pair and feed back
    /// [`VdirReply::Copy`].
    WantsCopy(Vec<(VdirPath, VdirPath)>),
}

/// Reply fed back into [`VdirCoroutine::resume`] by the driver.
///
/// One variant per [`VdirYield`] request; the coroutine asserts the
/// variant it expects and ignores the rest.
#[derive(Clone, Debug)]
pub enum VdirReply {
    /// Reply to [`VdirYield::WantsRandom`].
    Random(Vec<u8>),

    /// Reply to [`VdirYield::WantsFileExists`].
    FileExists(BTreeMap<VdirPath, bool>),

    /// Reply to [`VdirYield::WantsDirExists`].
    DirExists(BTreeMap<VdirPath, bool>),

    /// Reply to [`VdirYield::WantsDirRead`].
    DirRead(BTreeMap<VdirPath, BTreeSet<VdirPath>>),

    /// Reply to [`VdirYield::WantsFileRead`].
    FileRead(BTreeMap<VdirPath, Vec<u8>>),

    /// Acknowledgement of [`VdirYield::WantsFileCreate`].
    FileCreate,

    /// Acknowledgement of [`VdirYield::WantsDirCreate`].
    DirCreate,

    /// Acknowledgement of [`VdirYield::WantsDirRemove`].
    DirRemove,

    /// Acknowledgement of [`VdirYield::WantsFileRemove`].
    FileRemove,

    /// Acknowledgement of [`VdirYield::WantsRename`].
    Rename,

    /// Acknowledgement of [`VdirYield::WantsCopy`].
    Copy,
}

/// Coroutine `?`: forwards `Yielded` (via `Into`), short-circuits on
/// `Err`, evaluates to the inner `Ok` value.
#[macro_export]
macro_rules! vdir_try {
    ($coroutine:expr, $arg:expr $(,)?) => {
        match $crate::coroutine::VdirCoroutine::resume($coroutine, $arg) {
            $crate::coroutine::VdirCoroutineState::Yielded(y) => {
                return $crate::coroutine::VdirCoroutineState::Yielded(y.into());
            }
            $crate::coroutine::VdirCoroutineState::Complete(Err(err)) => {
                return $crate::coroutine::VdirCoroutineState::Complete(Err(err.into()));
            }
            $crate::coroutine::VdirCoroutineState::Complete(Ok(value)) => value,
        }
    };
}
