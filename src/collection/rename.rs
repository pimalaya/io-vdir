//! I/O-free coroutine renaming a Vdir collection directory.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::fs;
//!
//! use io_vdir::{collection::rename::VdirCollectionRename, coroutine::*};
//!
//! let mut coroutine = VdirCollectionRename::new("/tmp/vdir/contacts", "people");
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

use log::trace;
use thiserror::Error;

use crate::{coroutine::*, path::VdirPath};

/// Failure causes during a [`VdirCollectionRename`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirCollectionRenameError {
    #[error("Vdir collection rename failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),
}

/// Renames the collection at `path` to `name`, keeping the same parent
/// directory.
#[derive(Debug)]
pub struct VdirCollectionRename {
    state: State,
}

impl VdirCollectionRename {
    /// Creates a new coroutine that will rename the collection at
    /// `path` to `name` (keeping the same parent directory).
    pub fn new(path: impl Into<VdirPath>, name: impl ToString) -> Self {
        let from = path.into();
        let to = from.with_file_name(&name.to_string());

        Self {
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
        trace!("collection rename: {}", self.state);

        match (&mut self.state, arg) {
            (State::Start { pairs }, None) => {
                let pairs = mem::take(pairs);
                self.state = State::AwaitRename;
                VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs))
            }
            (State::AwaitRename, Some(VdirReply::Rename)) => VdirCoroutineState::Complete(Ok(())),
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
    AwaitRename,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start { .. } => f.write_str("start"),
            Self::AwaitRename => f.write_str("await rename reply"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_within_same_parent() {
        let mut cor = VdirCollectionRename::new("root/contacts", "people");

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
        let mut cor = VdirCollectionRename::new("root/contacts", "people");
        let _ = cor.resume(None);

        let err = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirCollectionRenameError::UnexpectedArg(_)));
    }
}
