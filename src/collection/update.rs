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
//!     collection::{types::Collection, update::VdirCollectionUpdate},
//!     coroutine::*,
//! };
//!
//! let mut collection = Collection::from_path("/tmp/vdir/contacts");
//! collection.display_name = Some("Contacts".into());
//! let mut coroutine = VdirCollectionUpdate::new(collection);
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

use alloc::{collections::BTreeMap, vec::Vec};

use log::trace;
use thiserror::Error;

use crate::{
    collection::types::{COLOR, Collection, DESCRIPTION, DISPLAYNAME},
    coroutine::*,
    item::types::TMP,
    path::VdirPath,
};

/// Failure causes during a [`VdirCollectionUpdate`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirCollectionUpdateError {
    #[error("Vdir collection update failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),
}

/// Rewrites Vdir collection metadata atomically via temp files.
#[derive(Debug)]
pub struct VdirCollectionUpdate {
    state: State,
}

impl VdirCollectionUpdate {
    /// Creates a new coroutine that will write the metadata of
    /// `collection` to disk.
    pub fn new(collection: Collection) -> Self {
        Self {
            state: State::Start(collection),
        }
    }
}

impl VdirCoroutine for VdirCollectionUpdate {
    type Yield = VdirYield;
    type Return = Result<(), VdirCollectionUpdateError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        trace!("collection update: {}", self.state);

        match (&mut self.state, arg) {
            (State::Start(collection), None) => {
                let collection = mem::take(collection);
                let mut files = BTreeMap::new();
                let mut renames = Vec::new();

                if let Some(name) = collection.display_name.filter(|s| !s.is_empty()) {
                    let final_path = collection.path.join(DISPLAYNAME);
                    let tmp_path = final_path.with_file_name(&format!("{DISPLAYNAME}.{TMP}"));
                    files.insert(tmp_path.clone(), name.into_bytes());
                    renames.push((tmp_path, final_path));
                }

                if let Some(desc) = collection.description.filter(|s| !s.is_empty()) {
                    let final_path = collection.path.join(DESCRIPTION);
                    let tmp_path = final_path.with_file_name(&format!("{DESCRIPTION}.{TMP}"));
                    files.insert(tmp_path.clone(), desc.into_bytes());
                    renames.push((tmp_path, final_path));
                }

                if let Some(color) = collection.color.filter(|s| !s.is_empty()) {
                    let final_path = collection.path.join(COLOR);
                    let tmp_path = final_path.with_file_name(&format!("{COLOR}.{TMP}"));
                    files.insert(tmp_path.clone(), color.into_bytes());
                    renames.push((tmp_path, final_path));
                }

                if files.is_empty() {
                    return VdirCoroutineState::Complete(Ok(()));
                }

                self.state = State::AwaitFileCreate { renames };
                VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files))
            }
            (State::AwaitFileCreate { renames }, Some(VdirReply::FileCreate)) => {
                let renames = mem::take(renames);
                self.state = State::AwaitRename;
                VdirCoroutineState::Yielded(VdirYield::WantsRename(renames))
            }
            (State::AwaitRename, Some(VdirReply::Rename)) => VdirCoroutineState::Complete(Ok(())),
            (_, arg) => {
                let err = VdirCollectionUpdateError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Start(Collection),
    AwaitFileCreate { renames: Vec<(VdirPath, VdirPath)> },
    AwaitRename,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(_) => f.write_str("start"),
            Self::AwaitFileCreate { .. } => f.write_str("await file create reply"),
            Self::AwaitRename => f.write_str("await rename reply"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_temp_files_then_renames() {
        let collection = Collection {
            path: VdirPath::from("root/contacts"),
            display_name: Some("Contacts".into()),
            description: None,
            color: None,
        };
        let mut cor = VdirCollectionUpdate::new(collection);

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

        match cor.resume(Some(VdirReply::Rename)) {
            VdirCoroutineState::Complete(Ok(())) => {}
            state => panic!("expected Complete(Ok), got {state:?}"),
        }
    }

    #[test]
    fn no_metadata_completes_immediately() {
        let mut cor = VdirCollectionUpdate::new(Collection::from_path("root/contacts"));

        match cor.resume(None) {
            VdirCoroutineState::Complete(Ok(())) => {}
            state => panic!("expected Complete(Ok), got {state:?}"),
        }
    }

    #[test]
    fn unexpected_reply_returns_error() {
        let collection = Collection {
            path: VdirPath::from("root/contacts"),
            display_name: Some("Contacts".into()),
            description: None,
            color: None,
        };
        let mut cor = VdirCollectionUpdate::new(collection);
        let _ = cor.resume(None);

        let err = match cor.resume(Some(VdirReply::DirExists(BTreeMap::new()))) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirCollectionUpdateError::UnexpectedArg(_)));
    }
}
