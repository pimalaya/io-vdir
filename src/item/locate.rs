//! I/O-free coroutine locating a Vdir item file by its ID.
//!
//! Probes `<collection>/<id>.vcf` and `<collection>/<id>.ics` in a
//! single batched file-exists request.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::fs;
//!
//! use io_vdir::{coroutine::*, item::locate::*};
//!
//! let opts = VdirItemLocateOptions::default();
//! let mut coroutine = VdirItemLocate::new("/tmp/vdir/contacts", "alice", opts);
//! let mut arg = None;
//!
//! let out = loop {
//!     match coroutine.resume(arg.take()) {
//!         VdirCoroutineState::Yielded(VdirYield::WantsFileExists(paths)) => {
//!             let map = paths
//!                 .into_iter()
//!                 .map(|p| {
//!                     let ok = fs::metadata(p.as_str()).map(|m| m.is_file()).unwrap_or(false);
//!                     (p, ok)
//!                 })
//!                 .collect();
//!             arg = Some(VdirReply::FileExists(map));
//!         }
//!         VdirCoroutineState::Complete(Ok(out)) => break out,
//!         VdirCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         state => panic!("unexpected state {state:?}"),
//!     }
//! };
//!
//! println!("located {} ({:?})", out.path, out.kind);
//! ```

use core::{fmt, mem};

use alloc::{
    collections::BTreeSet,
    string::{String, ToString},
};

use log::trace;
use thiserror::Error;

use crate::{
    coroutine::*,
    item::types::{ICS, ItemKind, VCF},
    path::VdirPath,
};

/// Failure causes during a [`VdirItemLocate`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirItemLocateError {
    #[error("Vdir item locate failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),

    #[error("Vdir item locate failed: item {0} not found")]
    NotFound(String),
}

/// Successful output of [`VdirItemLocate`].
#[derive(Clone, Debug)]
pub struct VdirItemLocateOutput {
    pub path: VdirPath,
    pub kind: ItemKind,
}

/// Options for [`VdirItemLocate::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirItemLocateOptions {}

/// Locates a Vdir item file by its ID.
#[derive(Clone, Debug)]
pub struct VdirItemLocate {
    state: State,
    #[allow(dead_code)]
    opts: VdirItemLocateOptions,
}

impl VdirItemLocate {
    /// Creates a new coroutine that will search for item `id` inside
    /// `collection`.
    pub fn new(
        collection: impl Into<VdirPath>,
        id: impl ToString,
        opts: VdirItemLocateOptions,
    ) -> Self {
        Self {
            opts,
            state: State::Start {
                collection: collection.into(),
                id: id.to_string(),
            },
        }
    }
}

impl VdirCoroutine for VdirItemLocate {
    type Yield = VdirYield;
    type Return = Result<VdirItemLocateOutput, VdirItemLocateError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        trace!("item locate: {}", self.state);

        match (&mut self.state, arg) {
            (State::Start { collection, id }, None) => {
                let id = mem::take(id);
                let vcf_path = collection.join(&format!("{id}.{VCF}"));
                let ics_path = collection.join(&format!("{id}.{ICS}"));

                let probes = BTreeSet::from_iter([vcf_path.clone(), ics_path.clone()]);
                self.state = State::AwaitProbe {
                    id,
                    vcf_path,
                    ics_path,
                };
                VdirCoroutineState::Yielded(VdirYield::WantsFileExists(probes))
            }
            (
                State::AwaitProbe {
                    id,
                    vcf_path,
                    ics_path,
                },
                Some(VdirReply::FileExists(probes)),
            ) => {
                if probes.get(vcf_path).copied().unwrap_or(false) {
                    let out = VdirItemLocateOutput {
                        path: mem::take(vcf_path),
                        kind: ItemKind::Vcard,
                    };
                    return VdirCoroutineState::Complete(Ok(out));
                }

                if probes.get(ics_path).copied().unwrap_or(false) {
                    let out = VdirItemLocateOutput {
                        path: mem::take(ics_path),
                        kind: ItemKind::Ical,
                    };
                    return VdirCoroutineState::Complete(Ok(out));
                }

                let err = VdirItemLocateError::NotFound(mem::take(id));
                VdirCoroutineState::Complete(Err(err))
            }
            (_, arg) => {
                let err = VdirItemLocateError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Clone, Debug)]
enum State {
    Start {
        collection: VdirPath,
        id: String,
    },
    AwaitProbe {
        id: String,
        vcf_path: VdirPath,
        ics_path: VdirPath,
    },
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start { .. } => f.write_str("start"),
            Self::AwaitProbe { .. } => f.write_str("await probe reply"),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use super::*;

    #[test]
    fn found_as_vcard_returns_ok() {
        let mut cor =
            VdirItemLocate::new("root/contacts", "alice", VdirItemLocateOptions::default());

        let probes = expect_wants_file_exists(&mut cor);
        let vcf = VdirPath::from("root/contacts/alice.vcf");
        let ics = VdirPath::from("root/contacts/alice.ics");
        assert!(probes.contains(&vcf));
        assert!(probes.contains(&ics));

        let mut map = BTreeMap::new();
        map.insert(vcf.clone(), true);
        map.insert(ics, false);
        let out = expect_complete_ok(&mut cor, Some(VdirReply::FileExists(map)));
        assert_eq!(out.path, vcf);
        assert_eq!(out.kind, ItemKind::Vcard);
    }

    #[test]
    fn not_found_returns_error() {
        let mut cor =
            VdirItemLocate::new("root/contacts", "alice", VdirItemLocateOptions::default());
        let _ = expect_wants_file_exists(&mut cor);

        let mut map = BTreeMap::new();
        map.insert(VdirPath::from("root/contacts/alice.vcf"), false);
        map.insert(VdirPath::from("root/contacts/alice.ics"), false);
        let err = expect_complete_err(&mut cor, Some(VdirReply::FileExists(map)));
        assert!(matches!(err, VdirItemLocateError::NotFound(_)));
    }

    #[test]
    fn unexpected_reply_returns_error() {
        let mut cor =
            VdirItemLocate::new("root/contacts", "alice", VdirItemLocateOptions::default());
        let _ = expect_wants_file_exists(&mut cor);

        let err = expect_complete_err(&mut cor, Some(VdirReply::DirCreate));
        assert!(matches!(err, VdirItemLocateError::UnexpectedArg(_)));
    }

    // --- utils

    fn expect_wants_file_exists(cor: &mut VdirItemLocate) -> BTreeSet<VdirPath> {
        match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileExists(paths)) => paths,
            state => panic!("expected WantsFileExists, got {state:?}"),
        }
    }

    fn expect_complete_ok(
        cor: &mut VdirItemLocate,
        arg: Option<VdirReply>,
    ) -> VdirItemLocateOutput {
        match cor.resume(arg) {
            VdirCoroutineState::Complete(Ok(out)) => out,
            state => panic!("expected Complete(Ok), got {state:?}"),
        }
    }

    fn expect_complete_err(
        cor: &mut VdirItemLocate,
        arg: Option<VdirReply>,
    ) -> VdirItemLocateError {
        match cor.resume(arg) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        }
    }
}
