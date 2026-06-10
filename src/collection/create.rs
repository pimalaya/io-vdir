//! I/O-free coroutine creating a Vdir collection.
//!
//! Creates the collection directory first; then, when the collection
//! carries metadata (`display_name`, `description` or `color`), writes
//! the corresponding marker files.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::fs;
//!
//! use io_vdir::{
//!     collection::{create::VdirCollectionCreate, types::Collection},
//!     coroutine::*,
//! };
//!
//! let mut coroutine = VdirCollectionCreate::new(Collection::from_path("/tmp/vdir/contacts"));
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
//!         VdirCoroutineState::Complete(Ok(())) => break,
//!         VdirCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         state => panic!("unexpected state {state:?}"),
//!     }
//! }
//! ```

use core::{fmt, mem};

use alloc::collections::{BTreeMap, BTreeSet};

use log::trace;
use thiserror::Error;

use crate::{
    collection::types::{COLOR, Collection, DESCRIPTION, DISPLAYNAME},
    coroutine::*,
};

/// Failure causes during a [`VdirCollectionCreate`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirCollectionCreateError {
    #[error("Vdir collection create failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),
}

/// Creates a Vdir collection directory and its metadata marker files.
#[derive(Debug)]
pub struct VdirCollectionCreate {
    state: State,
}

impl VdirCollectionCreate {
    /// Creates a new coroutine that will create `collection`.
    pub fn new(collection: Collection) -> Self {
        Self {
            state: State::Start(collection),
        }
    }
}

impl VdirCoroutine for VdirCollectionCreate {
    type Yield = VdirYield;
    type Return = Result<(), VdirCollectionCreateError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        trace!("collection create: {}", self.state);

        match (&mut self.state, arg) {
            (State::Start(collection), None) => {
                let collection = mem::take(collection);
                let dirs = BTreeSet::from_iter([collection.path.clone()]);
                self.state = State::AwaitDirCreate(collection);
                VdirCoroutineState::Yielded(VdirYield::WantsDirCreate(dirs))
            }
            (State::AwaitDirCreate(collection), Some(VdirReply::DirCreate)) => {
                let collection = mem::take(collection);
                let mut files = BTreeMap::new();

                if let Some(name) = collection.display_name.filter(|s| !s.is_empty()) {
                    files.insert(collection.path.join(DISPLAYNAME), name.into_bytes());
                }

                if let Some(desc) = collection.description.filter(|s| !s.is_empty()) {
                    files.insert(collection.path.join(DESCRIPTION), desc.into_bytes());
                }

                if let Some(color) = collection.color.filter(|s| !s.is_empty()) {
                    files.insert(collection.path.join(COLOR), color.into_bytes());
                }

                if files.is_empty() {
                    return VdirCoroutineState::Complete(Ok(()));
                }

                self.state = State::AwaitFileCreate;
                VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files))
            }
            (State::AwaitFileCreate, Some(VdirReply::FileCreate)) => {
                VdirCoroutineState::Complete(Ok(()))
            }
            (_, arg) => {
                let err = VdirCollectionCreateError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Start(Collection),
    AwaitDirCreate(Collection),
    AwaitFileCreate,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(_) => f.write_str("start"),
            Self::AwaitDirCreate(_) => f.write_str("await dir create reply"),
            Self::AwaitFileCreate => f.write_str("await file create reply"),
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
        let collection = Collection::from_path("root/contacts");
        let mut cor = VdirCollectionCreate::new(collection);

        let dirs = expect_wants_dir_create(&mut cor);
        assert!(dirs.contains(&VdirPath::from("root/contacts")));

        expect_complete_ok(&mut cor, Some(VdirReply::DirCreate));
    }

    #[test]
    fn metadata_collection_writes_marker_files() {
        let collection = Collection {
            path: VdirPath::from("root/contacts"),
            display_name: Some("Contacts".into()),
            description: None,
            color: Some("#3366ff".into()),
        };
        let mut cor = VdirCollectionCreate::new(collection);

        let _ = expect_wants_dir_create(&mut cor);

        let files = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files)) => files,
            state => panic!("expected WantsFileCreate, got {state:?}"),
        };
        assert!(files.contains_key(&VdirPath::from("root/contacts/displayname")));
        assert!(files.contains_key(&VdirPath::from("root/contacts/color")));
        assert!(!files.contains_key(&VdirPath::from("root/contacts/description")));

        expect_complete_ok(&mut cor, Some(VdirReply::FileCreate));
    }

    #[test]
    fn empty_metadata_fields_are_skipped() {
        let collection = Collection {
            path: VdirPath::from("root/contacts"),
            display_name: Some(String::new()),
            description: None,
            color: None,
        };
        let mut cor = VdirCollectionCreate::new(collection);

        let _ = expect_wants_dir_create(&mut cor);
        expect_complete_ok(&mut cor, Some(VdirReply::DirCreate));
    }

    #[test]
    fn unexpected_reply_returns_error() {
        let mut cor = VdirCollectionCreate::new(Collection::from_path("root/contacts"));
        let _ = expect_wants_dir_create(&mut cor);

        let err = expect_complete_err(&mut cor, Some(VdirReply::FileCreate));
        assert!(matches!(err, VdirCollectionCreateError::UnexpectedArg(_)));
    }

    // --- utils

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
