//! I/O-free coroutine reading the bytes of listed Vdir items.
//!
//! Takes the entries [`VdirItemList`] returned, or any part of them:
//! the caller picks the batch, so a large collection never has to sit
//! in one reply. An entry whose file the driver could not read is
//! reported back instead of failing the batch.
//!
//! [`VdirItemList`]: crate::item::list::VdirItemList
//!
//! # Example
//!
//! ```rust,no_run
//! use std::{collections::BTreeMap, fs};
//!
//! use io_vdir::{coroutine::*, item::{read::*, VdirItemEntry}};
//!
//! let entries: Vec<VdirItemEntry> = Vec::new(); // a batch of listed entries
//! let opts = VdirItemReadOptions::default();
//! let mut coroutine = VdirItemRead::new(entries, opts);
//! let mut arg = None;
//!
//! let out = loop {
//!     match coroutine.resume(arg.take()) {
//!         VdirCoroutineState::Yielded(VdirYield::WantsFileRead(paths)) => {
//!             let mut map = BTreeMap::new();
//!             for path in paths {
//!                 if let Ok(bytes) = fs::read(path.as_str()) {
//!                     map.insert(path, bytes);
//!                 }
//!             }
//!             arg = Some(VdirReply::FileRead(map));
//!         }
//!         VdirCoroutineState::Complete(Ok(out)) => break out,
//!         VdirCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         state => panic!("unexpected state {state:?}"),
//!     }
//! };
//!
//! println!("read {}, skipped {}", out.items.len(), out.unreadable.len());
//! ```

use core::{fmt, mem};

use alloc::collections::BTreeSet;

use log::trace;
use thiserror::Error;

use crate::{
    coroutine::*,
    item::{VdirItem, VdirItemEntry},
};

/// Failure causes during a [`VdirItemRead`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirItemReadError {
    /// The driver fed back a reply that does not match the pending
    /// request.
    #[error("Vdir item read failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),
}

/// Successful output of [`VdirItemRead`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirItemReadOutput {
    /// Items whose bytes were read.
    pub items: BTreeSet<VdirItem>,
    /// Entries whose file the driver could not read.
    pub unreadable: BTreeSet<VdirItemEntry>,
}

/// Options for [`VdirItemRead::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirItemReadOptions {}

/// Reads the bytes of a batch of listed Vdir items.
#[derive(Debug)]
pub struct VdirItemRead {
    state: State,
    #[allow(dead_code)]
    opts: VdirItemReadOptions,
}

impl VdirItemRead {
    /// Creates a new coroutine that will read the bytes of `entries`
    /// in one request.
    pub fn new(
        entries: impl IntoIterator<Item = VdirItemEntry>,
        opts: VdirItemReadOptions,
    ) -> Self {
        Self {
            opts,
            state: State::Start(entries.into_iter().collect()),
        }
    }
}

impl VdirCoroutine for VdirItemRead {
    type Yield = VdirYield;
    type Return = Result<VdirItemReadOutput, VdirItemReadError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        match (&mut self.state, arg) {
            (State::Start(entries), None) => {
                let entries = mem::take(entries);

                if entries.is_empty() {
                    return VdirCoroutineState::Complete(Ok(VdirItemReadOutput::default()));
                }

                let paths = entries.iter().map(|entry| entry.path.clone()).collect();
                self.state = State::Read(entries);
                VdirCoroutineState::Yielded(VdirYield::WantsFileRead(paths))
            }
            (State::Read(entries), Some(VdirReply::FileRead(mut contents))) => {
                let mut out = VdirItemReadOutput::default();

                for entry in mem::take(entries) {
                    let Some(contents) = contents.remove(&entry.path) else {
                        out.unreadable.insert(entry);
                        continue;
                    };

                    out.items.insert(VdirItem {
                        path: entry.path,
                        kind: entry.kind,
                        contents,
                        meta: entry.meta,
                    });
                }

                trace!("read {} items", out.items.len());
                trace!("skipped {} unreadable items", out.unreadable.len());
                VdirCoroutineState::Complete(Ok(out))
            }
            (_, arg) => {
                let err = VdirItemReadError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Start(BTreeSet<VdirItemEntry>),
    Read(BTreeSet<VdirItemEntry>),
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(_) => f.write_str("start"),
            Self::Read(_) => f.write_str("read items"),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::{collections::BTreeMap, vec::Vec};

    use crate::{
        item::{VdirItemEntry, VdirItemKind, read::*},
        path::VdirPath,
    };

    #[test]
    fn unreadable_entry_is_reported_not_fatal() {
        let alice = entry("alice");
        let bob = entry("bob");
        let opts = VdirItemReadOptions::default();
        let mut cor = VdirItemRead::new([alice.clone(), bob.clone()], opts);

        let paths = match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileRead(paths)) => paths,
            state => panic!("expected WantsFileRead, got {state:?}"),
        };
        assert_eq!(paths.len(), 2);

        let contents = BTreeMap::from_iter([(alice.path.clone(), b"BEGIN:VCARD".to_vec())]);
        let out = match cor.resume(Some(VdirReply::FileRead(contents))) {
            VdirCoroutineState::Complete(Ok(out)) => out,
            state => panic!("expected Complete(Ok), got {state:?}"),
        };

        let items: Vec<_> = out.items.into_iter().collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path, alice.path);
        assert_eq!(items[0].contents, b"BEGIN:VCARD");
        assert_eq!(items[0].meta, alice.meta);
        assert_eq!(out.unreadable.into_iter().collect::<Vec<_>>(), [bob]);
    }

    #[test]
    fn empty_batch_asks_nothing() {
        let mut cor = VdirItemRead::new([], VdirItemReadOptions::default());

        match cor.resume(None) {
            VdirCoroutineState::Complete(Ok(out)) => assert!(out.items.is_empty()),
            state => panic!("expected Complete(Ok), got {state:?}"),
        }
    }

    #[test]
    fn unexpected_reply_returns_error() {
        let mut cor = VdirItemRead::new([entry("alice")], VdirItemReadOptions::default());
        let _ = cor.resume(None);

        let err = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirItemReadError::UnexpectedArg(_)));
    }

    fn entry(id: &str) -> VdirItemEntry {
        VdirItemEntry {
            id: id.into(),
            kind: VdirItemKind::Vcard,
            path: VdirPath::from("root/contacts").join(&format!("{id}.vcf")),
            meta: VdirFileMeta {
                size: 11,
                ..Default::default()
            },
        }
    }
}
