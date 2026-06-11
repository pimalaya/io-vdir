//! I/O-free coroutine fetching a Vdir item by its ID.
//!
//! Locates the item file via [`VdirItemLocate`], then reads its contents.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::fs;
//!
//! use io_vdir::{coroutine::*, item::get::*};
//!
//! let opts = VdirItemGetOptions::default();
//! let mut coroutine = VdirItemGet::new("/tmp/vdir/contacts", "alice", opts);
//! let mut arg = None;
//!
//! let item = loop {
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
//!         VdirCoroutineState::Yielded(VdirYield::WantsFileRead(paths)) => {
//!             let map = paths
//!                 .into_iter()
//!                 .map(|p| {
//!                     let bytes = fs::read(p.as_str()).unwrap_or_default();
//!                     (p, bytes)
//!                 })
//!                 .collect();
//!             arg = Some(VdirReply::FileRead(map));
//!         }
//!         VdirCoroutineState::Complete(Ok(item)) => break item,
//!         VdirCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         state => panic!("unexpected state {state:?}"),
//!     }
//! };
//!
//! println!("{} bytes", item.contents.len());
//! ```

use core::{fmt, mem};

use alloc::{collections::BTreeSet, string::ToString};

use log::trace;
use thiserror::Error;

use crate::{
    coroutine::*,
    item::{
        locate::*,
        types::{Item, ItemKind},
    },
    path::VdirPath,
    vdir_try,
};

/// Failure causes during a [`VdirItemGet`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirItemGetError {
    #[error("Vdir item get failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),

    #[error(transparent)]
    Locate(#[from] VdirItemLocateError),
}

/// Options for [`VdirItemGet::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirItemGetOptions {}

/// Locates a Vdir item by ID and reads its contents.
#[derive(Debug)]
pub struct VdirItemGet {
    state: State,
    #[allow(dead_code)]
    opts: VdirItemGetOptions,
}

impl VdirItemGet {
    /// Creates a new coroutine that will retrieve item `id` from
    /// `collection`.
    pub fn new(
        collection: impl Into<VdirPath>,
        id: impl ToString,
        opts: VdirItemGetOptions,
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

impl VdirCoroutine for VdirItemGet {
    type Yield = VdirYield;
    type Return = Result<Item, VdirItemGetError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        trace!("item get: {}", self.state);

        match (&mut self.state, arg) {
            (State::Locate(c), arg) => {
                let out = vdir_try!(c, arg);
                let paths = BTreeSet::from_iter([out.path.clone()]);
                self.state = State::AwaitRead {
                    path: out.path,
                    kind: out.kind,
                };
                VdirCoroutineState::Yielded(VdirYield::WantsFileRead(paths))
            }
            (State::AwaitRead { path, kind }, Some(VdirReply::FileRead(mut map))) => {
                let path = mem::take(path);
                let kind = *kind;
                let contents = map.remove(&path).unwrap_or_default();
                VdirCoroutineState::Complete(Ok(Item {
                    path,
                    kind,
                    contents,
                }))
            }
            (_, arg) => {
                let err = VdirItemGetError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Locate(VdirItemLocate),
    AwaitRead { path: VdirPath, kind: ItemKind },
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locate(_) => f.write_str("locate item"),
            Self::AwaitRead { .. } => f.write_str("await read reply"),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use super::*;

    #[test]
    fn locates_then_reads_contents() {
        let mut cor = VdirItemGet::new("root/contacts", "alice", VdirItemGetOptions::default());

        match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileExists(_)) => {}
            state => panic!("expected WantsFileExists, got {state:?}"),
        }

        let vcf = VdirPath::from("root/contacts/alice.vcf");
        let mut exists = BTreeMap::new();
        exists.insert(vcf.clone(), true);
        exists.insert(VdirPath::from("root/contacts/alice.ics"), false);

        let paths = match cor.resume(Some(VdirReply::FileExists(exists))) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileRead(paths)) => paths,
            state => panic!("expected WantsFileRead, got {state:?}"),
        };
        assert!(paths.contains(&vcf));

        let mut contents = BTreeMap::new();
        contents.insert(vcf.clone(), b"BEGIN:VCARD".to_vec());
        let item = match cor.resume(Some(VdirReply::FileRead(contents))) {
            VdirCoroutineState::Complete(Ok(item)) => item,
            state => panic!("expected Complete(Ok), got {state:?}"),
        };
        assert_eq!(item.path, vcf);
        assert_eq!(item.kind, ItemKind::Vcard);
        assert_eq!(item.contents, b"BEGIN:VCARD");
    }

    #[test]
    fn locate_error_is_forwarded() {
        let mut cor = VdirItemGet::new("root/contacts", "alice", VdirItemGetOptions::default());
        let _ = cor.resume(None);

        let err = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirItemGetError::Locate(_)));
    }
}
