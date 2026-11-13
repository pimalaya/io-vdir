//! I/O-free coroutine listing the items of a Vdir collection.
//!
//! Regular files named `<id>.vcf` or `<id>.ics` are items; anything
//! else (metadata files, leftover temporaries, directories, dangling
//! links, a bare `.vcf`) is skipped. The listing stats each item and
//! reads none: [`VdirItemRead`] loads the bytes of the entries the
//! caller picks, in batches of its choice.
//!
//! [`VdirItemRead`]: crate::item::read::VdirItemRead
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
//! let entries = loop {
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
//!         VdirCoroutineState::Yielded(VdirYield::WantsFileMeta(paths)) => {
//!             let mut out = BTreeMap::new();
//!             for path in paths {
//!                 match fs::metadata(path.as_str()) {
//!                     Ok(meta) if meta.is_file() => {
//!                         // fill the mtime and the inode in real code
//!                         let size = meta.len();
//!                         out.insert(path, VdirFileMeta { size, ..Default::default() });
//!                     }
//!                     _ => {}
//!                 }
//!             }
//!             arg = Some(VdirReply::FileMeta(out));
//!         }
//!         VdirCoroutineState::Complete(Ok(entries)) => break entries,
//!         VdirCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         state => panic!("unexpected state {state:?}"),
//!     }
//! };
//!
//! println!("found {} items", entries.len());
//! ```

use core::{fmt, mem};

use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
};

use log::trace;
use thiserror::Error;

use crate::{
    coroutine::*,
    item::{VdirItemEntry, VdirItemKind},
    path::{VdirPath, is_stem},
};

/// Failure causes during a [`VdirItemList`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirItemListError {
    /// The driver fed back a reply that does not match the pending
    /// request.
    #[error("Vdir item list failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),
}

/// Options for [`VdirItemList::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirItemListOptions {}

/// Lists the `.vcf` / `.ics` items of a Vdir collection, bytes left
/// unread.
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
    type Return = Result<BTreeSet<VdirItemEntry>, VdirItemListError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        match (&mut self.state, arg) {
            (State::Start(collection), None) => {
                let paths = BTreeSet::from_iter([mem::take(collection)]);
                self.state = State::ReadDir;
                VdirCoroutineState::Yielded(VdirYield::WantsDirRead(paths))
            }
            (State::ReadDir, Some(VdirReply::DirRead(entries))) => {
                let mut names = BTreeMap::new();

                for paths in entries.into_values() {
                    for path in paths {
                        let Some(name) = path.file_name() else {
                            continue;
                        };

                        let Some((id, ext)) = name.rsplit_once('.') else {
                            continue;
                        };

                        let Some(kind) = VdirItemKind::from_extension(ext) else {
                            continue;
                        };

                        if !is_stem(id) {
                            continue;
                        }

                        let id = id.to_string();
                        names.insert(path, (id, kind));
                    }
                }

                if names.is_empty() {
                    return VdirCoroutineState::Complete(Ok(BTreeSet::new()));
                }

                let probes = names.keys().cloned().collect();
                self.state = State::Probe { names };
                VdirCoroutineState::Yielded(VdirYield::WantsFileMeta(probes))
            }
            (State::Probe { names }, Some(VdirReply::FileMeta(metas))) => {
                let mut entries = BTreeSet::new();

                for (path, (id, kind)) in mem::take(names) {
                    let Some(meta) = metas.get(&path).copied() else {
                        continue;
                    };

                    entries.insert(VdirItemEntry {
                        id,
                        kind,
                        path,
                        meta,
                    });
                }

                trace!("listed {} items", entries.len());
                VdirCoroutineState::Complete(Ok(entries))
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
    ReadDir,
    Probe {
        names: BTreeMap<VdirPath, (String, VdirItemKind)>,
    },
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(_) => f.write_str("start"),
            Self::ReadDir => f.write_str("read collection dir"),
            Self::Probe { .. } => f.write_str("probe items"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_vcf_and_ics_files_without_reading_them() {
        let mut cor = VdirItemList::new("root/contacts", VdirItemListOptions::default());

        let paths = expect_wants_dir_read(&mut cor);
        assert!(paths.contains(&VdirPath::from("root/contacts")));

        let vcf = VdirPath::from("root/contacts/alice.vcf");
        let ics = VdirPath::from("root/contacts/.bob.ics");
        let probes = expect_wants_file_meta(
            &mut cor,
            [
                vcf.clone(),
                ics.clone(),
                VdirPath::from("root/contacts/displayname"),
                VdirPath::from("root/contacts/alice.vcf.0011223344556677.tmp"),
            ],
        );
        assert_eq!(probes, BTreeSet::from_iter([vcf.clone(), ics.clone()]));

        let meta = VdirFileMeta {
            size: 11,
            ..Default::default()
        };
        let metas = BTreeMap::from_iter([(vcf.clone(), meta), (ics.clone(), meta)]);
        let entries = match cor.resume(Some(VdirReply::FileMeta(metas))) {
            VdirCoroutineState::Complete(Ok(entries)) => entries,
            state => panic!("expected Complete(Ok), got {state:?}"),
        };

        let bob = VdirItemEntry {
            id: ".bob".into(),
            kind: VdirItemKind::Ical,
            path: ics,
            meta,
        };
        let alice = VdirItemEntry {
            id: "alice".into(),
            kind: VdirItemKind::Vcard,
            path: vcf,
            meta,
        };
        assert_eq!(entries, BTreeSet::from_iter([bob, alice]));
    }

    #[test]
    fn bare_extension_is_not_an_item() {
        let mut cor = VdirItemList::new("root/contacts", VdirItemListOptions::default());
        let _ = expect_wants_dir_read(&mut cor);

        let vcf = VdirPath::from("root/contacts/alice.vcf");
        let probes = expect_wants_file_meta(
            &mut cor,
            [
                vcf.clone(),
                VdirPath::from("root/contacts/.vcf"),
                VdirPath::from("root/contacts/.ics"),
            ],
        );
        assert_eq!(probes, BTreeSet::from_iter([vcf]));
    }

    #[test]
    fn entry_that_is_not_a_file_is_skipped() {
        let mut cor = VdirItemList::new("root/contacts", VdirItemListOptions::default());
        let _ = expect_wants_dir_read(&mut cor);

        let vcf = VdirPath::from("root/contacts/alice.vcf");
        let dir = VdirPath::from("root/contacts/folder.ics");
        let _ = expect_wants_file_meta(&mut cor, [vcf.clone(), dir]);

        let metas = BTreeMap::from_iter([(vcf.clone(), VdirFileMeta::default())]);
        let entries = match cor.resume(Some(VdirReply::FileMeta(metas))) {
            VdirCoroutineState::Complete(Ok(entries)) => entries,
            state => panic!("expected Complete(Ok), got {state:?}"),
        };
        let paths: BTreeSet<VdirPath> = entries.into_iter().map(|entry| entry.path).collect();
        assert_eq!(paths, BTreeSet::from_iter([vcf]));
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

    fn expect_wants_dir_read(cor: &mut VdirItemList) -> BTreeSet<VdirPath> {
        match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsDirRead(paths)) => paths,
            state => panic!("expected WantsDirRead, got {state:?}"),
        }
    }

    fn expect_wants_file_meta(
        cor: &mut VdirItemList,
        names: impl IntoIterator<Item = VdirPath>,
    ) -> BTreeSet<VdirPath> {
        let names = BTreeSet::from_iter(names);
        let entries = BTreeMap::from_iter([(VdirPath::from("root/contacts"), names)]);

        match cor.resume(Some(VdirReply::DirRead(entries))) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileMeta(paths)) => paths,
            state => panic!("expected WantsFileMeta, got {state:?}"),
        }
    }
}
