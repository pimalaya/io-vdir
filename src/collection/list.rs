//! I/O-free coroutine listing every Vdir collection directly under a
//! root directory.
//!
//! A collection is any subdirectory of the root; existing
//! `displayname`, `description` and `color` marker files are loaded to
//! populate the corresponding optional fields.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::{collections::{BTreeMap, BTreeSet}, fs};
//!
//! use io_vdir::{collection::list::*, coroutine::*, path::VdirPath};
//!
//! let opts = VdirCollectionListOptions::default();
//! let mut coroutine = VdirCollectionList::new("/tmp/vdir", opts);
//! let mut arg = None;
//!
//! let collections = loop {
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
//!         VdirCoroutineState::Yielded(VdirYield::WantsDirExists(paths)) => {
//!             let map = paths
//!                 .into_iter()
//!                 .map(|p| {
//!                     let ok = fs::metadata(p.as_str()).map(|m| m.is_dir()).unwrap_or(false);
//!                     (p, ok)
//!                 })
//!                 .collect();
//!             arg = Some(VdirReply::DirExists(map));
//!         }
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
//!         VdirCoroutineState::Complete(Ok(collections)) => break collections,
//!         VdirCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         state => panic!("unexpected state {state:?}"),
//!     }
//! };
//!
//! println!("found {} collections", collections.len());
//! ```

use core::{fmt, mem};

use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
};

use log::trace;
use thiserror::Error;

use crate::{
    collection::types::{COLOR, Collection, DESCRIPTION, DISPLAYNAME},
    coroutine::*,
    path::VdirPath,
};

/// Failure causes during a [`VdirCollectionList`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirCollectionListError {
    #[error("Vdir collection list failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),
}

/// Options for [`VdirCollectionList::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirCollectionListOptions {}

/// Lists every Vdir collection directly under a root directory.
#[derive(Debug)]
pub struct VdirCollectionList {
    state: State,
    #[allow(dead_code)]
    opts: VdirCollectionListOptions,
}

impl VdirCollectionList {
    /// Creates a new coroutine that will list collections inside
    /// `root`.
    pub fn new(root: impl Into<VdirPath>, opts: VdirCollectionListOptions) -> Self {
        Self {
            opts,
            state: State::Start(root.into()),
        }
    }
}

impl VdirCoroutine for VdirCollectionList {
    type Yield = VdirYield;
    type Return = Result<BTreeSet<Collection>, VdirCollectionListError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        trace!("collection list: {}", self.state);

        match (&mut self.state, arg) {
            (State::Start(root), None) => {
                let paths = BTreeSet::from_iter([mem::take(root)]);
                self.state = State::AwaitChildrenRead;
                VdirCoroutineState::Yielded(VdirYield::WantsDirRead(paths))
            }
            (State::AwaitChildrenRead, Some(VdirReply::DirRead(entries))) => {
                let mut candidates = BTreeSet::new();

                for paths in entries.into_values() {
                    for path in paths {
                        let Some(name) = path.file_name() else {
                            continue;
                        };

                        if name.starts_with('.') {
                            continue;
                        }

                        candidates.insert(path);
                    }
                }

                if candidates.is_empty() {
                    return VdirCoroutineState::Complete(Ok(BTreeSet::new()));
                }

                let probes = candidates.clone();
                self.state = State::AwaitDirExists { candidates };
                VdirCoroutineState::Yielded(VdirYield::WantsDirExists(probes))
            }
            (State::AwaitDirExists { candidates }, Some(VdirReply::DirExists(probes))) => {
                let collection_paths: BTreeSet<VdirPath> = mem::take(candidates)
                    .into_iter()
                    .filter(|p| probes.get(p).copied().unwrap_or(false))
                    .collect();

                if collection_paths.is_empty() {
                    return VdirCoroutineState::Complete(Ok(BTreeSet::new()));
                }

                let mut probes = BTreeMap::new();

                for path in &collection_paths {
                    probes.insert(path.join(DISPLAYNAME), path.clone());
                    probes.insert(path.join(DESCRIPTION), path.clone());
                    probes.insert(path.join(COLOR), path.clone());
                }

                let probe_paths: BTreeSet<VdirPath> = probes.keys().cloned().collect();
                self.state = State::AwaitMetadataExists {
                    collection_paths,
                    probes,
                };
                VdirCoroutineState::Yielded(VdirYield::WantsFileExists(probe_paths))
            }
            (
                State::AwaitMetadataExists {
                    collection_paths,
                    probes,
                },
                Some(VdirReply::FileExists(exists)),
            ) => {
                let collection_paths = mem::take(collection_paths);
                let probes: BTreeMap<VdirPath, VdirPath> = mem::take(probes)
                    .into_iter()
                    .filter(|(probe, _)| exists.get(probe).copied().unwrap_or(false))
                    .collect();

                if probes.is_empty() {
                    let collections = collection_paths
                        .into_iter()
                        .map(Collection::from_path)
                        .collect();
                    return VdirCoroutineState::Complete(Ok(collections));
                }

                let probe_paths: BTreeSet<VdirPath> = probes.keys().cloned().collect();
                self.state = State::AwaitMetadataRead {
                    collection_paths,
                    probes,
                };
                VdirCoroutineState::Yielded(VdirYield::WantsFileRead(probe_paths))
            }
            (
                State::AwaitMetadataRead {
                    collection_paths,
                    probes,
                },
                Some(VdirReply::FileRead(mut contents)),
            ) => {
                let mut by_path: BTreeMap<VdirPath, Collection> = mem::take(collection_paths)
                    .into_iter()
                    .map(|path| (path.clone(), Collection::from_path(path)))
                    .collect();

                for (probe, owner) in mem::take(probes) {
                    let Some(bytes) = contents.remove(&probe) else {
                        continue;
                    };

                    let Some(collection) = by_path.get_mut(&owner) else {
                        continue;
                    };

                    let Some(name) = probe.file_name() else {
                        continue;
                    };

                    let value = String::from_utf8_lossy(&bytes).trim().to_string();
                    if value.is_empty() {
                        continue;
                    }

                    match name {
                        DISPLAYNAME => collection.display_name = Some(value),
                        DESCRIPTION => collection.description = Some(value),
                        COLOR => collection.color = Some(value),
                        _ => {}
                    }
                }

                let collections: BTreeSet<Collection> = by_path.into_values().collect();
                trace!("found {} collections", collections.len());
                VdirCoroutineState::Complete(Ok(collections))
            }
            (_, arg) => {
                let err = VdirCollectionListError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Start(VdirPath),
    AwaitChildrenRead,
    AwaitDirExists {
        candidates: BTreeSet<VdirPath>,
    },
    AwaitMetadataExists {
        collection_paths: BTreeSet<VdirPath>,
        probes: BTreeMap<VdirPath, VdirPath>,
    },
    AwaitMetadataRead {
        collection_paths: BTreeSet<VdirPath>,
        probes: BTreeMap<VdirPath, VdirPath>,
    },
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(_) => f.write_str("start"),
            Self::AwaitChildrenRead => f.write_str("await children read reply"),
            Self::AwaitDirExists { .. } => f.write_str("await dir exists reply"),
            Self::AwaitMetadataExists { .. } => f.write_str("await metadata exists reply"),
            Self::AwaitMetadataRead { .. } => f.write_str("await metadata read reply"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_root_yields_no_collections() {
        let mut cor = VdirCollectionList::new("root", VdirCollectionListOptions::default());

        let paths = expect_wants_dir_read(&mut cor);
        assert!(paths.contains(&VdirPath::from("root")));

        let mut entries = BTreeMap::new();
        entries.insert(VdirPath::from("root"), BTreeSet::new());

        let collections = expect_complete_ok(&mut cor, Some(VdirReply::DirRead(entries)));
        assert!(collections.is_empty());
    }

    #[test]
    fn bare_collection_without_metadata() {
        let mut cor = VdirCollectionList::new("root", VdirCollectionListOptions::default());
        let _ = expect_wants_dir_read(&mut cor);

        let mut entries = BTreeMap::new();
        let contacts = VdirPath::from("root/contacts");
        entries.insert(
            VdirPath::from("root"),
            BTreeSet::from_iter([contacts.clone()]),
        );

        let probes = match cor.resume(Some(VdirReply::DirRead(entries))) {
            VdirCoroutineState::Yielded(VdirYield::WantsDirExists(probes)) => probes,
            state => panic!("expected WantsDirExists, got {state:?}"),
        };
        assert!(probes.contains(&contacts));

        let mut exists = BTreeMap::new();
        exists.insert(contacts.clone(), true);

        let metadata = match cor.resume(Some(VdirReply::DirExists(exists))) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileExists(probes)) => probes,
            state => panic!("expected WantsFileExists, got {state:?}"),
        };
        assert!(metadata.contains(&contacts.join(DISPLAYNAME)));

        let absent: BTreeMap<VdirPath, bool> = metadata.into_iter().map(|p| (p, false)).collect();
        let collections = expect_complete_ok(&mut cor, Some(VdirReply::FileExists(absent)));
        assert_eq!(collections.len(), 1);
        assert_eq!(collections.iter().next().unwrap().id(), "contacts");
    }

    #[test]
    fn dotfiles_are_skipped() {
        let mut cor = VdirCollectionList::new("root", VdirCollectionListOptions::default());
        let _ = expect_wants_dir_read(&mut cor);

        let mut entries = BTreeMap::new();
        entries.insert(
            VdirPath::from("root"),
            BTreeSet::from_iter([VdirPath::from("root/.hidden")]),
        );

        let collections = expect_complete_ok(&mut cor, Some(VdirReply::DirRead(entries)));
        assert!(collections.is_empty());
    }

    #[test]
    fn unexpected_reply_returns_error() {
        let mut cor = VdirCollectionList::new("root", VdirCollectionListOptions::default());
        let _ = expect_wants_dir_read(&mut cor);

        let err = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirCollectionListError::UnexpectedArg(_)));
    }

    // --- utils

    fn expect_wants_dir_read(cor: &mut VdirCollectionList) -> BTreeSet<VdirPath> {
        match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsDirRead(paths)) => paths,
            state => panic!("expected WantsDirRead, got {state:?}"),
        }
    }

    fn expect_complete_ok(
        cor: &mut VdirCollectionList,
        arg: Option<VdirReply>,
    ) -> BTreeSet<Collection> {
        match cor.resume(arg) {
            VdirCoroutineState::Complete(Ok(collections)) => collections,
            state => panic!("expected Complete(Ok), got {state:?}"),
        }
    }
}
