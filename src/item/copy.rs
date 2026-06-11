//! I/O-free coroutine copying a Vdir item across collections.
//!
//! Locates the source item via [`VdirItemLocate`], then copies it into the
//! target collection keeping the same id and extension.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::fs;
//!
//! use io_vdir::{coroutine::*, item::copy::*};
//!
//! let opts = VdirItemCopyOptions::default();
//! let mut coroutine = VdirItemCopy::new("/tmp/vdir/contacts", "/tmp/vdir/work", "alice", opts);
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
//!         VdirCoroutineState::Yielded(VdirYield::WantsCopy(pairs)) => {
//!             for (from, to) in pairs {
//!                 fs::copy(from.as_str(), to.as_str()).unwrap();
//!             }
//!             arg = Some(VdirReply::Copy);
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

/// Failure causes during a [`VdirItemCopy`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirItemCopyError {
    #[error("Vdir item copy failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),

    #[error(transparent)]
    Locate(#[from] VdirItemLocateError),
}

/// Options for [`VdirItemCopy::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirItemCopyOptions {}

/// Copies a Vdir item from a source into a target collection.
#[derive(Debug)]
pub struct VdirItemCopy {
    state: State,
    #[allow(dead_code)]
    opts: VdirItemCopyOptions,
}

impl VdirItemCopy {
    /// Creates a new coroutine that will copy item `id` from `source`
    /// into `target`. The item keeps the same id and extension in the
    /// target collection.
    pub fn new(
        source: impl Into<VdirPath>,
        target: impl Into<VdirPath>,
        id: impl ToString,
        opts: VdirItemCopyOptions,
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

impl VdirCoroutine for VdirItemCopy {
    type Yield = VdirYield;
    type Return = Result<(), VdirItemCopyError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        trace!("item copy: {}", self.state);

        match (&mut self.state, arg) {
            (State::Locate { target, id, inner }, arg) => {
                let out = vdir_try!(inner, arg);
                let ext = out.kind.extension();
                let target_path = target.join(&format!("{id}.{ext}"));
                let pairs = vec![(out.path, target_path)];
                self.state = State::AwaitCopy;
                VdirCoroutineState::Yielded(VdirYield::WantsCopy(pairs))
            }
            (State::AwaitCopy, Some(VdirReply::Copy)) => VdirCoroutineState::Complete(Ok(())),
            (_, arg) => {
                let err = VdirItemCopyError::UnexpectedArg(arg);
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
    AwaitCopy,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locate { .. } => f.write_str("locate source item"),
            Self::AwaitCopy => f.write_str("await copy reply"),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use super::*;

    #[test]
    fn copies_located_source_to_target() {
        let mut cor =
            VdirItemCopy::new("root/a", "root/b", "alice", VdirItemCopyOptions::default());

        match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileExists(_)) => {}
            state => panic!("expected WantsFileExists, got {state:?}"),
        }

        let vcf = VdirPath::from("root/a/alice.vcf");
        let mut exists = BTreeMap::new();
        exists.insert(vcf.clone(), true);
        exists.insert(VdirPath::from("root/a/alice.ics"), false);

        let pairs = match cor.resume(Some(VdirReply::FileExists(exists))) {
            VdirCoroutineState::Yielded(VdirYield::WantsCopy(pairs)) => pairs,
            state => panic!("expected WantsCopy, got {state:?}"),
        };
        assert_eq!(pairs, vec![(vcf, VdirPath::from("root/b/alice.vcf"))]);

        match cor.resume(Some(VdirReply::Copy)) {
            VdirCoroutineState::Complete(Ok(())) => {}
            state => panic!("expected Complete(Ok), got {state:?}"),
        }
    }

    #[test]
    fn locate_error_is_forwarded() {
        let mut cor =
            VdirItemCopy::new("root/a", "root/b", "alice", VdirItemCopyOptions::default());
        let _ = cor.resume(None);

        let err = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirItemCopyError::Locate(_)));
    }
}
