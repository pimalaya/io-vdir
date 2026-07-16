//! I/O-free coroutine deleting a Vdir collection and all its contents.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::fs;
//!
//! use io_vdir::{collection::delete::*, coroutine::*};
//!
//! let opts = VdirCollectionDeleteOptions::default();
//! let mut coroutine = VdirCollectionDelete::new("/tmp/vdir/contacts", opts);
//! let mut arg = None;
//!
//! loop {
//!     match coroutine.resume(arg.take()) {
//!         VdirCoroutineState::Yielded(VdirYield::WantsDirRemove(paths)) => {
//!             for path in paths {
//!                 fs::remove_dir_all(path.as_str()).unwrap();
//!             }
//!             arg = Some(VdirReply::DirRemove);
//!         }
//!         VdirCoroutineState::Complete(Ok(())) => break,
//!         VdirCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         state => panic!("unexpected state {state:?}"),
//!     }
//! }
//! ```

use core::{fmt, mem};

use alloc::collections::BTreeSet;

use thiserror::Error;

use crate::{coroutine::*, path::VdirPath};

/// Failure causes during a [`VdirCollectionDelete`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirCollectionDeleteError {
    /// The driver fed back a reply that does not match the pending
    /// request.
    #[error("Vdir collection delete failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),
}

/// Options for [`VdirCollectionDelete::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirCollectionDeleteOptions {}

/// Recursively removes the collection directory rooted at `path`.
#[derive(Debug)]
pub struct VdirCollectionDelete {
    state: State,
    #[allow(dead_code)]
    opts: VdirCollectionDeleteOptions,
}

impl VdirCollectionDelete {
    /// Creates a new coroutine that will recursively remove the
    /// collection at `path`.
    pub fn new(path: impl Into<VdirPath>, opts: VdirCollectionDeleteOptions) -> Self {
        let paths = BTreeSet::from_iter([path.into()]);
        Self {
            opts,
            state: State::Start { paths },
        }
    }
}

impl VdirCoroutine for VdirCollectionDelete {
    type Yield = VdirYield;
    type Return = Result<(), VdirCollectionDeleteError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        match (&mut self.state, arg) {
            (State::Start { paths }, None) => {
                let paths = mem::take(paths);
                self.state = State::AwaitRemove;
                VdirCoroutineState::Yielded(VdirYield::WantsDirRemove(paths))
            }
            (State::AwaitRemove, Some(VdirReply::DirRemove)) => {
                VdirCoroutineState::Complete(Ok(()))
            }
            (_, arg) => {
                let err = VdirCollectionDeleteError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Start { paths: BTreeSet<VdirPath> },
    AwaitRemove,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start { .. } => f.write_str("start"),
            Self::AwaitRemove => f.write_str("await remove reply"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_collection_directory() {
        let mut cor =
            VdirCollectionDelete::new("root/contacts", VdirCollectionDeleteOptions::default());

        let paths = match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsDirRemove(paths)) => paths,
            state => panic!("expected WantsDirRemove, got {state:?}"),
        };
        assert!(paths.contains(&VdirPath::from("root/contacts")));

        match cor.resume(Some(VdirReply::DirRemove)) {
            VdirCoroutineState::Complete(Ok(())) => {}
            state => panic!("expected Complete(Ok), got {state:?}"),
        }
    }

    #[test]
    fn unexpected_reply_returns_error() {
        let mut cor =
            VdirCollectionDelete::new("root/contacts", VdirCollectionDeleteOptions::default());
        let _ = cor.resume(None);

        let err = match cor.resume(Some(VdirReply::FileRemove)) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirCollectionDeleteError::UnexpectedArg(_)));
    }
}
