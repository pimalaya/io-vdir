//! I/O-free coroutine rewriting Vdir collection metadata atomically.
//!
//! Writes each non-empty metadata field (`display_name`,
//! `description`, `color`) to a temporary file, then renames it onto
//! the canonical name.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::fs;
//!
//! use io_vdir::{
//!     collection::{VdirCollection, update::*},
//!     coroutine::*,
//! };
//!
//! let mut collection = VdirCollection::from_path("/tmp/vdir/contacts");
//! collection.display_name = Some("Contacts".into());
//! let opts = VdirCollectionUpdateOptions::default();
//! let mut coroutine = VdirCollectionUpdate::new(collection, opts);
//! let mut arg = None;
//!
//! loop {
//!     match coroutine.resume(arg.take()) {
//!         VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files)) => {
//!             for (path, bytes) in files {
//!                 fs::write(path.as_str(), bytes).unwrap();
//!             }
//!             arg = Some(VdirReply::FileCreate);
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

use alloc::{
    collections::{BTreeMap, BTreeSet},
    format,
    string::String,
    vec::Vec,
};

use thiserror::Error;

use crate::{
    collection::{COLOR, DESCRIPTION, DISPLAYNAME, VdirCollection},
    coroutine::*,
    item::TMP,
    path::VdirPath,
};

/// Failure causes during a [`VdirCollectionUpdate`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirCollectionUpdateError {
    /// The driver fed back a reply that does not match the pending
    /// request.
    #[error("Vdir collection update failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),
}

/// Options for [`VdirCollectionUpdate::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirCollectionUpdateOptions {}

/// Rewrites Vdir collection metadata atomically via temp files.
#[derive(Debug)]
pub struct VdirCollectionUpdate {
    state: State,
    #[allow(dead_code)]
    opts: VdirCollectionUpdateOptions,
}

impl VdirCollectionUpdate {
    /// Creates a new coroutine that will write the metadata of
    /// `collection` to disk.
    pub fn new(collection: VdirCollection, opts: VdirCollectionUpdateOptions) -> Self {
        Self {
            opts,
            state: State::Start(collection),
        }
    }
}

impl VdirCoroutine for VdirCollectionUpdate {
    type Yield = VdirYield;
    type Return = Result<(), VdirCollectionUpdateError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        match (&mut self.state, arg) {
            (State::Start(collection), None) => {
                let collection = mem::take(collection);
                let mut files = BTreeMap::new();
                let mut renames = Vec::new();
                let mut removals = BTreeSet::new();

                // NOTE: the collection is a desired state, not a patch, so a
                // field it does not carry is one the collection should not
                // have: its file goes. Writing only what is set would leave a
                // cleared description on disk and report success.
                let mut field = |name: &str, value: Option<String>| {
                    let final_path = collection.path.join(name);
                    match value.filter(|value| !value.is_empty()) {
                        Some(value) => {
                            let tmp_path = final_path.with_file_name(&format!("{name}.{TMP}"));
                            files.insert(tmp_path.clone(), value.into_bytes());
                            renames.push((tmp_path, final_path));
                        }
                        None => {
                            removals.insert(final_path);
                        }
                    }
                };

                field(DISPLAYNAME, collection.display_name.clone());
                field(DESCRIPTION, collection.description.clone());
                field(COLOR, collection.color.clone());

                if files.is_empty() {
                    self.state = State::AwaitFileRemove;
                    return VdirCoroutineState::Yielded(VdirYield::WantsFileRemove(removals));
                }

                self.state = State::AwaitFileCreate { renames, removals };
                VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files))
            }
            (State::AwaitFileCreate { renames, removals }, Some(VdirReply::FileCreate)) => {
                let renames = mem::take(renames);
                let removals = mem::take(removals);
                self.state = State::AwaitRename { removals };
                VdirCoroutineState::Yielded(VdirYield::WantsRename(renames))
            }
            (State::AwaitRename { removals }, Some(VdirReply::Rename)) => {
                let removals = mem::take(removals);
                if removals.is_empty() {
                    return VdirCoroutineState::Complete(Ok(()));
                }

                self.state = State::AwaitFileRemove;
                VdirCoroutineState::Yielded(VdirYield::WantsFileRemove(removals))
            }
            (State::AwaitFileRemove, Some(VdirReply::FileRemove)) => {
                VdirCoroutineState::Complete(Ok(()))
            }
            (_, arg) => {
                let err = VdirCollectionUpdateError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Start(VdirCollection),
    AwaitFileCreate {
        renames: Vec<(VdirPath, VdirPath)>,
        removals: BTreeSet<VdirPath>,
    },
    AwaitRename {
        removals: BTreeSet<VdirPath>,
    },
    AwaitFileRemove,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(_) => f.write_str("start"),
            Self::AwaitFileCreate { .. } => f.write_str("await file create reply"),
            Self::AwaitRename { .. } => f.write_str("await rename reply"),
            Self::AwaitFileRemove => f.write_str("await file remove reply"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_temp_files_then_renames() {
        let collection = VdirCollection {
            path: VdirPath::from("root/contacts"),
            display_name: Some("Contacts".into()),
            description: None,
            color: None,
        };
        let mut cor = VdirCollectionUpdate::new(collection, VdirCollectionUpdateOptions::default());

        let files = match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files)) => files,
            state => panic!("expected WantsFileCreate, got {state:?}"),
        };
        let tmp = VdirPath::from("root/contacts/displayname.tmp");
        assert!(files.contains_key(&tmp));

        let pairs = match cor.resume(Some(VdirReply::FileCreate)) {
            VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs)) => pairs,
            state => panic!("expected WantsRename, got {state:?}"),
        };
        assert_eq!(
            pairs,
            vec![(tmp, VdirPath::from("root/contacts/displayname"))]
        );

        // NOTE: the two fields the collection does not carry are cleared,
        // so their files are removed after the rename.
        let removals = match cor.resume(Some(VdirReply::Rename)) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileRemove(removals)) => removals,
            state => panic!("expected WantsFileRemove, got {state:?}"),
        };
        assert!(removals.contains(&VdirPath::from("root/contacts/description")));
        assert!(removals.contains(&VdirPath::from("root/contacts/color")));
        assert!(!removals.contains(&VdirPath::from("root/contacts/displayname")));

        match cor.resume(Some(VdirReply::FileRemove)) {
            VdirCoroutineState::Complete(Ok(())) => {}
            state => panic!("expected Complete(Ok), got {state:?}"),
        }
    }

    #[test]
    fn no_metadata_removes_every_file() {
        let mut cor = VdirCollectionUpdate::new(
            VdirCollection::from_path("root/contacts"),
            VdirCollectionUpdateOptions::default(),
        );

        // NOTE: a collection carrying no metadata at all asks for nothing to
        // be written and for all three files to go.
        let removals = match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileRemove(removals)) => removals,
            state => panic!("expected WantsFileRemove, got {state:?}"),
        };
        assert_eq!(removals.len(), 3);

        match cor.resume(Some(VdirReply::FileRemove)) {
            VdirCoroutineState::Complete(Ok(())) => {}
            state => panic!("expected Complete(Ok), got {state:?}"),
        }
    }

    #[test]
    fn unexpected_reply_returns_error() {
        let collection = VdirCollection {
            path: VdirPath::from("root/contacts"),
            display_name: Some("Contacts".into()),
            description: None,
            color: None,
        };
        let mut cor = VdirCollectionUpdate::new(collection, VdirCollectionUpdateOptions::default());
        let _ = cor.resume(None);

        let err = match cor.resume(Some(VdirReply::DirExists(BTreeMap::new()))) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirCollectionUpdateError::UnexpectedArg(_)));
    }
}
