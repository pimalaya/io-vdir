//! I/O-free coroutine renaming a Vdir collection directory.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::fs;
//!
//! use io_vdir::{collection::rename::*, coroutine::*};
//!
//! let opts = VdirCollectionRenameOptions::default();
//! let mut coroutine = VdirCollectionRename::new("/tmp/vdir/contacts", "people", opts);
//! let mut arg = None;
//!
//! loop {
//!     match coroutine.resume(arg.take()) {
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

use core::{fmt, mem};

use alloc::{string::ToString, vec::Vec};

use thiserror::Error;

use crate::{coroutine::*, path::VdirPath};

/// Failure causes during a [`VdirCollectionRename`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirCollectionRenameError {
    /// The driver fed back a reply that does not match the pending
    /// request.
    #[error("Vdir collection rename failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),
}

/// Options for [`VdirCollectionRename::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirCollectionRenameOptions {}

/// Renames the collection at `path` to `name`, keeping the same parent
/// directory.
#[derive(Debug)]
pub struct VdirCollectionRename {
    state: State,
    #[allow(dead_code)]
    opts: VdirCollectionRenameOptions,
}

impl VdirCollectionRename {
    /// Creates a new coroutine that will rename the collection at
    /// `path` to `name` (keeping the same parent directory).
    pub fn new(
        path: impl Into<VdirPath>,
        name: impl ToString,
        opts: VdirCollectionRenameOptions,
    ) -> Self {
        let from = path.into();
        let to = from.with_file_name(&name.to_string());

        Self {
            opts,
            state: State::Start {
                pairs: vec![(from, to)],
            },
        }
    }
}

impl VdirCoroutine for VdirCollectionRename {
    type Yield = VdirYield;
    type Return = Result<(), VdirCollectionRenameError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        match (&mut self.state, arg) {
            (State::Start { pairs }, None) => {
                let pairs = mem::take(pairs);
                self.state = State::Rename;
                VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs))
            }
            (State::Rename, Some(VdirReply::Rename)) => VdirCoroutineState::Complete(Ok(())),
            (_, arg) => {
                let err = VdirCollectionRenameError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Start { pairs: Vec<(VdirPath, VdirPath)> },
    Rename,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start { .. } => f.write_str("start"),
            Self::Rename => f.write_str("rename collection"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_within_same_parent() {
        let mut cor = VdirCollectionRename::new(
            "root/contacts",
            "people",
            VdirCollectionRenameOptions::default(),
        );

        let pairs = match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs)) => pairs,
            state => panic!("expected WantsRename, got {state:?}"),
        };
        assert_eq!(
            pairs,
            vec![(
                VdirPath::from("root/contacts"),
                VdirPath::from("root/people")
            )]
        );

        match cor.resume(Some(VdirReply::Rename)) {
            VdirCoroutineState::Complete(Ok(())) => {}
            state => panic!("expected Complete(Ok), got {state:?}"),
        }
    }

    #[test]
    fn unexpected_reply_returns_error() {
        let mut cor = VdirCollectionRename::new(
            "root/contacts",
            "people",
            VdirCollectionRenameOptions::default(),
        );
        let _ = cor.resume(None);

        let err = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirCollectionRenameError::UnexpectedArg(_)));
    }
}
