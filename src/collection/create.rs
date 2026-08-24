//! I/O-free coroutine creating a Vdir collection.
//!
//! Creates the collection directory first; then, when the collection
//! carries metadata (`display_name`, `description` or `color`), writes
//! the corresponding marker files.
//!
//! Each marker file lands on its `.tmp` sibling and is renamed into
//! place, as [`VdirCollectionUpdate`] writes them, so a reader
//! scanning the new collection reads a whole value or no file at all.
//!
//! [`VdirCollectionUpdate`]: crate::collection::update::VdirCollectionUpdate
//!
//! # Example
//!
//! ```rust,no_run
//! use std::fs;
//!
//! use io_vdir::{
//!     collection::{create::*, VdirCollection},
//!     coroutine::*,
//! };
//!
//! let opts = VdirCollectionCreateOptions::default();
//! let mut coroutine = VdirCollectionCreate::new(VdirCollection::from_path("/tmp/vdir/contacts"), opts);
//! let mut arg = None;
//!
//! loop {
//!     match coroutine.resume(arg.take()) {
//!         VdirCoroutineState::Yielded(VdirYield::WantsDirCreate(paths)) => {
//!             for path in paths {
//!                 fs::create_dir_all(path.as_str()).unwrap();
//!             }
//!             arg = Some(VdirReply::DirCreate);
//!         }
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

/// Failure causes during a [`VdirCollectionCreate`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirCollectionCreateError {
    /// The driver fed back a reply that does not match the pending
    /// request.
    #[error("Vdir collection create failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),
}

/// Options for [`VdirCollectionCreate::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirCollectionCreateOptions {}

/// Creates a Vdir collection directory and its metadata marker files.
#[derive(Debug)]
pub struct VdirCollectionCreate {
    state: State,
    #[allow(dead_code)]
    opts: VdirCollectionCreateOptions,
}

impl VdirCollectionCreate {
    /// Creates a new coroutine that will create `collection`.
    pub fn new(collection: VdirCollection, opts: VdirCollectionCreateOptions) -> Self {
        Self {
            opts,
            state: State::Start(collection),
        }
    }
}

impl VdirCoroutine for VdirCollectionCreate {
    type Yield = VdirYield;
    type Return = Result<(), VdirCollectionCreateError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        match (&mut self.state, arg) {
            (State::Start(collection), None) => {
                let collection = mem::take(collection);
                let dirs = BTreeSet::from_iter([collection.path.clone()]);
                self.state = State::CreateDir(collection);
                VdirCoroutineState::Yielded(VdirYield::WantsDirCreate(dirs))
            }
            (State::CreateDir(collection), Some(VdirReply::DirCreate)) => {
                let collection = mem::take(collection);
                let mut files = BTreeMap::new();
                let mut renames = Vec::new();

                let mut field = |name: &str, value: Option<String>| {
                    let Some(value) = value.filter(|value| !value.is_empty()) else {
                        return;
                    };

                    let final_path = collection.path.join(name);
                    let tmp_path = final_path.with_file_name(&format!("{name}.{TMP}"));
                    files.insert(tmp_path.clone(), value.into_bytes());
                    renames.push((tmp_path, final_path));
                };

                field(DISPLAYNAME, collection.display_name.clone());
                field(DESCRIPTION, collection.description.clone());
                field(COLOR, collection.color.clone());

                if files.is_empty() {
                    return VdirCoroutineState::Complete(Ok(()));
                }

                self.state = State::CreateFile { renames };
                VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files))
            }
            (State::CreateFile { renames }, Some(VdirReply::FileCreate)) => {
                let renames = mem::take(renames);
                self.state = State::Rename;
                VdirCoroutineState::Yielded(VdirYield::WantsRename(renames))
            }
            (State::Rename, Some(VdirReply::Rename)) => VdirCoroutineState::Complete(Ok(())),
            (_, arg) => {
                let err = VdirCollectionCreateError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Start(VdirCollection),
    CreateDir(VdirCollection),
    CreateFile { renames: Vec<(VdirPath, VdirPath)> },
    Rename,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(_) => f.write_str("start"),
            Self::CreateDir(_) => f.write_str("create collection dir"),
            Self::CreateFile { .. } => f.write_str("write metadata into tmp"),
            Self::Rename => f.write_str("rename into place"),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use crate::path::VdirPath;

    use super::*;

    #[test]
    fn bare_collection_creates_dir_only() {
        let collection = VdirCollection::from_path("root/contacts");
        let mut cor = VdirCollectionCreate::new(collection, VdirCollectionCreateOptions::default());

        let dirs = expect_wants_dir_create(&mut cor);
        assert!(dirs.contains(&VdirPath::from("root/contacts")));

        expect_complete_ok(&mut cor, Some(VdirReply::DirCreate));
    }

    #[test]
    fn metadata_collection_writes_marker_files() {
        let collection = VdirCollection {
            path: VdirPath::from("root/contacts"),
            display_name: Some("Contacts".into()),
            description: None,
            color: Some("#3366ff".into()),
        };
        let mut cor = VdirCollectionCreate::new(collection, VdirCollectionCreateOptions::default());

        let _ = expect_wants_dir_create(&mut cor);

        // The values land on their tmp siblings, never on the marker
        // names a reader looks for.
        let files = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files)) => files,
            state => panic!("expected WantsFileCreate, got {state:?}"),
        };
        assert!(files.contains_key(&VdirPath::from("root/contacts/displayname.tmp")));
        assert!(files.contains_key(&VdirPath::from("root/contacts/color.tmp")));
        assert!(!files.contains_key(&VdirPath::from("root/contacts/displayname")));
        assert!(!files.contains_key(&VdirPath::from("root/contacts/description.tmp")));

        let pairs = match cor.resume(Some(VdirReply::FileCreate)) {
            VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs)) => pairs,
            state => panic!("expected WantsRename, got {state:?}"),
        };
        assert!(pairs.contains(&(
            VdirPath::from("root/contacts/displayname.tmp"),
            VdirPath::from("root/contacts/displayname"),
        )));
        assert!(pairs.contains(&(
            VdirPath::from("root/contacts/color.tmp"),
            VdirPath::from("root/contacts/color"),
        )));

        expect_complete_ok(&mut cor, Some(VdirReply::Rename));
    }

    #[test]
    fn empty_metadata_fields_are_skipped() {
        let collection = VdirCollection {
            path: VdirPath::from("root/contacts"),
            display_name: Some(String::new()),
            description: None,
            color: None,
        };
        let mut cor = VdirCollectionCreate::new(collection, VdirCollectionCreateOptions::default());

        let _ = expect_wants_dir_create(&mut cor);
        expect_complete_ok(&mut cor, Some(VdirReply::DirCreate));
    }

    #[test]
    fn unexpected_reply_returns_error() {
        let mut cor = VdirCollectionCreate::new(
            VdirCollection::from_path("root/contacts"),
            VdirCollectionCreateOptions::default(),
        );
        let _ = expect_wants_dir_create(&mut cor);

        let err = expect_complete_err(&mut cor, Some(VdirReply::FileCreate));
        assert!(matches!(err, VdirCollectionCreateError::UnexpectedArg(_)));
    }

    fn expect_wants_dir_create(cor: &mut VdirCollectionCreate) -> BTreeSet<VdirPath> {
        match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsDirCreate(paths)) => paths,
            state => panic!("expected WantsDirCreate, got {state:?}"),
        }
    }

    fn expect_complete_ok(cor: &mut VdirCollectionCreate, arg: Option<VdirReply>) {
        match cor.resume(arg) {
            VdirCoroutineState::Complete(Ok(())) => {}
            state => panic!("expected Complete(Ok), got {state:?}"),
        }
    }

    fn expect_complete_err(
        cor: &mut VdirCollectionCreate,
        arg: Option<VdirReply>,
    ) -> VdirCollectionCreateError {
        match cor.resume(arg) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        }
    }
}
