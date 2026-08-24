//! I/O-free coroutine copying a Vdir item across collections.
//!
//! Locates the source item via [`VdirItemLocate`], then copies it into the
//! target collection keeping the same id and extension.
//!
//! The bytes land on the target's `.tmp` sibling and one rename
//! publishes them, as [`VdirItemStore`] writes its own. Keeping the id
//! means the target name may already hold an item, and staging is what
//! keeps that item whole until the copy is complete: it is replaced in
//! one step or not at all.
//!
//! [`VdirItemStore`]: crate::item::store::VdirItemStore
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

use alloc::string::{String, ToString};

use thiserror::Error;

use crate::{
    coroutine::*,
    item::{build_paths, locate::*},
    path::VdirPath,
    vdir_try,
};

/// Failure causes during a [`VdirItemCopy`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirItemCopyError {
    /// The driver fed back a reply that does not match the pending
    /// request.
    #[error("Vdir item copy failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),

    /// The inner locate coroutine failed.
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
        match (&mut self.state, arg) {
            (State::Locate { target, id, inner }, arg) => {
                let out = vdir_try!(inner, arg);
                let (tmp_path, final_path) = build_paths(target, id, out.kind);
                let pairs = vec![(out.path, tmp_path.clone())];
                self.state = State::Copy {
                    tmp_path,
                    final_path,
                };
                VdirCoroutineState::Yielded(VdirYield::WantsCopy(pairs))
            }
            (
                State::Copy {
                    tmp_path,
                    final_path,
                },
                Some(VdirReply::Copy),
            ) => {
                let pairs = vec![(mem::take(tmp_path), mem::take(final_path))];
                self.state = State::Rename;
                VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs))
            }
            (State::Rename, Some(VdirReply::Rename)) => VdirCoroutineState::Complete(Ok(())),
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
    Copy {
        tmp_path: VdirPath,
        final_path: VdirPath,
    },
    Rename,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locate { .. } => f.write_str("locate source item"),
            Self::Copy { .. } => f.write_str("copy into tmp"),
            Self::Rename => f.write_str("rename into place"),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use super::*;

    #[test]
    fn copies_located_source_through_tmp() {
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

        // The bytes land on the tmp sibling, never on the name the
        // target collection is enumerated by.
        let staged = VdirPath::from("root/b/alice.vcf.tmp");
        let pairs = match cor.resume(Some(VdirReply::FileExists(exists))) {
            VdirCoroutineState::Yielded(VdirYield::WantsCopy(pairs)) => pairs,
            state => panic!("expected WantsCopy, got {state:?}"),
        };
        assert_eq!(pairs, vec![(vcf, staged.clone())]);

        let pairs = match cor.resume(Some(VdirReply::Copy)) {
            VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs)) => pairs,
            state => panic!("expected WantsRename, got {state:?}"),
        };
        assert_eq!(pairs, vec![(staged, VdirPath::from("root/b/alice.vcf"))]);

        match cor.resume(Some(VdirReply::Rename)) {
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
