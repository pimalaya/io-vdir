//! I/O-free coroutine moving a Vdir item across collections.
//!
//! Locates the source item via [`VdirItemLocate`], then renames it into the
//! target collection keeping the same id and extension.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::fs;
//!
//! use io_vdir::{coroutine::*, item::r#move::*};
//!
//! let opts = VdirItemMoveOptions::default();
//! let mut coroutine = VdirItemMove::new("/tmp/vdir/contacts", "/tmp/vdir/work", "alice", opts);
//! let mut arg = None;
//!
//! loop {
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
//!         VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs)) => {
//!             for (from, to) in pairs {
//!                 fs::rename(from.as_str(), to.as_str()).unwrap();
//!             }
//!             arg = Some(VdirReply::Rename);
//!         }
//!         VdirCoroutineState::Complete(Ok(())) => break,
//!         VdirCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         state => panic!("unexpected state {state:?}"),
//!     }
//! }
//! ```

use core::fmt;

use alloc::string::{String, ToString};

use log::trace;
use thiserror::Error;

use crate::{coroutine::*, item::locate::*, path::VdirPath, vdir_try};

/// Failure causes during a [`VdirItemMove`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirItemMoveError {
    #[error("Vdir item move failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),

    #[error(transparent)]
    Locate(#[from] VdirItemLocateError),
}

/// Options for [`VdirItemMove::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirItemMoveOptions {}

/// Moves a Vdir item from a source into a target collection.
#[derive(Debug)]
pub struct VdirItemMove {
    state: State,
    #[allow(dead_code)]
    opts: VdirItemMoveOptions,
}

impl VdirItemMove {
    /// Creates a new coroutine that will move item `id` from `source`
    /// into `target`. The item keeps the same id and extension in the
    /// target collection.
    pub fn new(
        source: impl Into<VdirPath>,
        target: impl Into<VdirPath>,
        id: impl ToString,
        opts: VdirItemMoveOptions,
    ) -> Self {
        let id = id.to_string();
        let inner = VdirItemLocate::new(source, &id, VdirItemLocateOptions::default());
        Self {
            opts,
            state: State::Locate {
                target: target.into(),
                id,
                inner,
            },
        }
    }
}

impl VdirCoroutine for VdirItemMove {
    type Yield = VdirYield;
    type Return = Result<(), VdirItemMoveError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        trace!("item move: {}", self.state);

        match (&mut self.state, arg) {
            (State::Locate { target, id, inner }, arg) => {
                let out = vdir_try!(inner, arg);
                let ext = out.kind.extension();
                let target_path = target.join(&format!("{id}.{ext}"));
                let pairs = vec![(out.path, target_path)];
                self.state = State::AwaitRename;
                VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs))
            }
            (State::AwaitRename, Some(VdirReply::Rename)) => VdirCoroutineState::Complete(Ok(())),
            (_, arg) => {
                let err = VdirItemMoveError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Locate {
        target: VdirPath,
        id: String,
        inner: VdirItemLocate,
    },
    AwaitRename,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locate { .. } => f.write_str("locate source item"),
            Self::AwaitRename => f.write_str("await rename reply"),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use super::*;

    #[test]
    fn moves_located_source_to_target() {
        let mut cor =
            VdirItemMove::new("root/a", "root/b", "alice", VdirItemMoveOptions::default());

        match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileExists(_)) => {}
            state => panic!("expected WantsFileExists, got {state:?}"),
        }

        let ics = VdirPath::from("root/a/alice.ics");
        let mut exists = BTreeMap::new();
        exists.insert(VdirPath::from("root/a/alice.vcf"), false);
        exists.insert(ics.clone(), true);

        let pairs = match cor.resume(Some(VdirReply::FileExists(exists))) {
            VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs)) => pairs,
            state => panic!("expected WantsRename, got {state:?}"),
        };
        assert_eq!(pairs, vec![(ics, VdirPath::from("root/b/alice.ics"))]);

        match cor.resume(Some(VdirReply::Rename)) {
            VdirCoroutineState::Complete(Ok(())) => {}
            state => panic!("expected Complete(Ok), got {state:?}"),
        }
    }

    #[test]
    fn locate_error_is_forwarded() {
        let mut cor =
            VdirItemMove::new("root/a", "root/b", "alice", VdirItemMoveOptions::default());
        let _ = cor.resume(None);

        let err = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirItemMoveError::Locate(_)));
    }
}
