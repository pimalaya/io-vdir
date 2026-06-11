//! I/O-free coroutine listing every item inside a Vdir collection.
//!
//! Entries with a `.vcf` or `.ics` extension are considered items;
//! anything else (metadata files, dotfiles, leftover temporaries) is
//! skipped.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::{collections::{BTreeMap, BTreeSet}, fs};
//!
//! use io_vdir::{coroutine::*, item::list::*, path::VdirPath};
//!
//! let opts = VdirItemListOptions::default();
//! let mut coroutine = VdirItemList::new("/tmp/vdir/contacts", opts);
//! let mut arg = None;
//!
//! let items = loop {
//!     match coroutine.resume(arg.take()) {
//!         VdirCoroutineState::Yielded(VdirYield::WantsDirRead(paths)) => {
//!             let mut out = BTreeMap::new();
//!             for path in paths {
//!                 let mut names = BTreeSet::new();
//!                 if let Ok(rd) = fs::read_dir(path.as_str()) {
//!                     for entry in rd.flatten() {
//!                         names.insert(VdirPath::new(entry.path().to_string_lossy()));
//!                     }
//!                 }
//!                 out.insert(path, names);
//!             }
//!             arg = Some(VdirReply::DirRead(out));
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
//!         VdirCoroutineState::Complete(Ok(items)) => break items,
//!         VdirCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         state => panic!("unexpected state {state:?}"),
//!     }
//! };
//!
//! println!("found {} items", items.len());
//! ```

use core::{fmt, mem};

use alloc::collections::{BTreeMap, BTreeSet};

use log::trace;
use thiserror::Error;

use crate::{
    coroutine::*,
    item::{Item, ItemKind},
    path::VdirPath,
};

/// Failure causes during a [`VdirItemList`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirItemListError {
    #[error("Vdir item list failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),
}

/// Options for [`VdirItemList::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirItemListOptions {}

/// Lists every `.vcf` / `.ics` item inside a Vdir collection.
#[derive(Debug)]
pub struct VdirItemList {
    state: State,
    #[allow(dead_code)]
    opts: VdirItemListOptions,
}

impl VdirItemList {
    /// Creates a new coroutine that will list every item inside
    /// `collection`.
    pub fn new(collection: impl Into<VdirPath>, opts: VdirItemListOptions) -> Self {
        Self {
            opts,
            state: State::Start(collection.into()),
        }
    }
}

impl VdirCoroutine for VdirItemList {
    type Yield = VdirYield;
    type Return = Result<BTreeSet<Item>, VdirItemListError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        trace!("item list: {}", self.state);

        match (&mut self.state, arg) {
            (State::Start(collection), None) => {
                let paths = BTreeSet::from_iter([mem::take(collection)]);
                self.state = State::AwaitDirRead;
                VdirCoroutineState::Yielded(VdirYield::WantsDirRead(paths))
            }
            (State::AwaitDirRead, Some(VdirReply::DirRead(entries))) => {
                let mut kinds: BTreeMap<VdirPath, ItemKind> = BTreeMap::new();

                for paths in entries.into_values() {
                    for path in paths {
                        let Some(name) = path.file_name() else {
                            continue;
                        };

                        let Some((_, ext)) = name.rsplit_once('.') else {
                            continue;
                        };

                        let Some(kind) = ItemKind::from_extension(ext) else {
                            continue;
                        };

                        kinds.insert(path, kind);
                    }
                }

                if kinds.is_empty() {
                    return VdirCoroutineState::Complete(Ok(BTreeSet::new()));
                }

                let probes: BTreeSet<VdirPath> = kinds.keys().cloned().collect();
                self.state = State::AwaitFileRead { kinds };
                VdirCoroutineState::Yielded(VdirYield::WantsFileRead(probes))
            }
            (State::AwaitFileRead { kinds }, Some(VdirReply::FileRead(mut contents))) => {
                let mut items = BTreeSet::new();

                for (path, kind) in mem::take(kinds) {
                    let bytes = contents.remove(&path).unwrap_or_default();
                    items.insert(Item {
                        path,
                        kind,
                        contents: bytes,
                    });
                }

                trace!("listed {} items", items.len());
                VdirCoroutineState::Complete(Ok(items))
            }
            (_, arg) => {
                let err = VdirItemListError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Start(VdirPath),
    AwaitDirRead,
    AwaitFileRead { kinds: BTreeMap<VdirPath, ItemKind> },
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(_) => f.write_str("start"),
            Self::AwaitDirRead => f.write_str("await dir read reply"),
            Self::AwaitFileRead { .. } => f.write_str("await file read reply"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_vcf_and_ics_entries() {
        let mut cor = VdirItemList::new("root/contacts", VdirItemListOptions::default());

        let paths = expect_wants_dir_read(&mut cor);
        assert!(paths.contains(&VdirPath::from("root/contacts")));

        let vcf = VdirPath::from("root/contacts/alice.vcf");
        let mut entries = BTreeMap::new();
        entries.insert(
            VdirPath::from("root/contacts"),
            BTreeSet::from_iter([
                vcf.clone(),
                VdirPath::from("root/contacts/displayname"),
                VdirPath::from("root/contacts/alice.vcf.tmp"),
            ]),
        );

        let probes = match cor.resume(Some(VdirReply::DirRead(entries))) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileRead(probes)) => probes,
            state => panic!("expected WantsFileRead, got {state:?}"),
        };
        assert_eq!(probes.len(), 1);
        assert!(probes.contains(&vcf));

        let mut contents = BTreeMap::new();
        contents.insert(vcf.clone(), b"BEGIN:VCARD".to_vec());
        let items = match cor.resume(Some(VdirReply::FileRead(contents))) {
            VdirCoroutineState::Complete(Ok(items)) => items,
            state => panic!("expected Complete(Ok), got {state:?}"),
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items.iter().next().unwrap().kind, ItemKind::Vcard);
    }

    #[test]
    fn empty_collection_yields_no_items() {
        let mut cor = VdirItemList::new("root/contacts", VdirItemListOptions::default());
        let _ = expect_wants_dir_read(&mut cor);

        let mut entries = BTreeMap::new();
        entries.insert(VdirPath::from("root/contacts"), BTreeSet::new());
        let items = match cor.resume(Some(VdirReply::DirRead(entries))) {
            VdirCoroutineState::Complete(Ok(items)) => items,
            state => panic!("expected Complete(Ok), got {state:?}"),
        };
        assert!(items.is_empty());
    }

    #[test]
    fn unexpected_reply_returns_error() {
        let mut cor = VdirItemList::new("root/contacts", VdirItemListOptions::default());
        let _ = expect_wants_dir_read(&mut cor);

        let err = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirItemListError::UnexpectedArg(_)));
    }

    // --- utils

    fn expect_wants_dir_read(cor: &mut VdirItemList) -> BTreeSet<VdirPath> {
        match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsDirRead(paths)) => paths,
            state => panic!("expected WantsDirRead, got {state:?}"),
        }
    }
}
