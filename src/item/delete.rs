//! I/O-free coroutine deleting a Vdir item by its ID.
//!
//! Locates the item file via [`VdirItemLocate`], then removes it.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::fs;
//!
//! use io_vdir::{coroutine::*, item::delete::*};
//!
//! let opts = VdirItemDeleteOptions::default();
//! let mut coroutine = VdirItemDelete::new("/tmp/vdir/contacts", "alice", opts);
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
//!         VdirCoroutineState::Yielded(VdirYield::WantsFileRemove(paths)) => {
//!             for path in paths {
//!                 fs::remove_file(path.as_str()).unwrap();
//!             }
//!             arg = Some(VdirReply::FileRemove);
//!         }
//!         VdirCoroutineState::Complete(Ok(())) => break,
//!         VdirCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         state => panic!("unexpected state {state:?}"),
//!     }
//! }
//! ```

use core::fmt;

use alloc::{collections::BTreeSet, string::ToString};

use thiserror::Error;

use crate::{coroutine::*, item::locate::*, path::VdirPath, vdir_try};

/// Failure causes during a [`VdirItemDelete`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirItemDeleteError {
    /// The driver fed back a reply that does not match the pending
    /// request.
    #[error("Vdir item delete failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),

    /// The inner locate coroutine failed.
    #[error(transparent)]
    Locate(#[from] VdirItemLocateError),
}

/// Options for [`VdirItemDelete::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirItemDeleteOptions {}

/// Locates a Vdir item by its ID and removes it.
#[derive(Debug)]
pub struct VdirItemDelete {
    state: State,
    #[allow(dead_code)]
    opts: VdirItemDeleteOptions,
}

impl VdirItemDelete {
    /// Creates a new coroutine that will delete item `id` from
    /// `collection`.
    pub fn new(
        collection: impl Into<VdirPath>,
        id: impl ToString,
        opts: VdirItemDeleteOptions,
    ) -> Self {
        Self {
            opts,
            state: State::Locate(VdirItemLocate::new(
                collection,
                id,
                VdirItemLocateOptions::default(),
            )),
        }
    }
}

impl VdirCoroutine for VdirItemDelete {
    type Yield = VdirYield;
    type Return = Result<(), VdirItemDeleteError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        match (&mut self.state, arg) {
            (State::Locate(c), arg) => {
                let out = vdir_try!(c, arg);
                let paths = BTreeSet::from_iter([out.path]);
                self.state = State::Remove;
                VdirCoroutineState::Yielded(VdirYield::WantsFileRemove(paths))
            }
            (State::Remove, Some(VdirReply::FileRemove)) => VdirCoroutineState::Complete(Ok(())),
            (_, arg) => {
                let err = VdirItemDeleteError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Locate(VdirItemLocate),
    Remove,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locate(_) => f.write_str("locate item"),
            Self::Remove => f.write_str("remove item"),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use super::*;

    #[test]
    fn removes_located_item() {
        let mut cor =
            VdirItemDelete::new("root/contacts", "alice", VdirItemDeleteOptions::default());

        match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileExists(_)) => {}
            state => panic!("expected WantsFileExists, got {state:?}"),
        }

        let vcf = VdirPath::from("root/contacts/alice.vcf");
        let mut exists = BTreeMap::new();
        exists.insert(vcf.clone(), true);
        exists.insert(VdirPath::from("root/contacts/alice.ics"), false);

        let paths = match cor.resume(Some(VdirReply::FileExists(exists))) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileRemove(paths)) => paths,
            state => panic!("expected WantsFileRemove, got {state:?}"),
        };
        assert!(paths.contains(&vcf));

        match cor.resume(Some(VdirReply::FileRemove)) {
            VdirCoroutineState::Complete(Ok(())) => {}
            state => panic!("expected Complete(Ok), got {state:?}"),
        }
    }

    #[test]
    fn locate_error_is_forwarded() {
        let mut cor =
            VdirItemDelete::new("root/contacts", "alice", VdirItemDeleteOptions::default());
        let _ = cor.resume(None);

        let err = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirItemDeleteError::Locate(_)));
    }
}
