//! I/O-free coroutine to delete a Vdir collection.

use std::path::Path;

use io_fs::{
    coroutines::remove_dir::RemoveDir,
    error::{FsError, FsResult},
    io::FsIo,
};
use thiserror::Error;

/// Errors that can occur during the coroutine progression.
#[derive(Clone, Debug, Error)]
pub enum DeleteCollectionError {
    /// An error occured during the directory deletion.
    #[error("Delete Vdir collection error")]
    RemoveDirError(#[from] FsError),
}

/// Output emitted when the coroutine terminates its progression.
#[derive(Clone, Debug)]
pub enum DeleteCollectionResult {
    /// The coroutine successfully terminated its progression.
    Ok,

    /// The coroutine encountered an error.
    Err(DeleteCollectionError),

    /// An I/O needs to be processed in order to make the coroutine
    /// progress further.
    Io(FsIo),
}

/// I/O-free coroutine to delete a Vdir collection.
#[derive(Debug)]
pub struct DeleteCollection(RemoveDir);

impl DeleteCollection {
    /// Creates a new coroutine from the given collection's path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self(RemoveDir::new(path.as_ref()))
    }

    /// Makes the coroutine progress.
    pub fn resume(&mut self, arg: Option<FsIo>) -> DeleteCollectionResult {
        match self.0.resume(arg) {
            FsResult::Ok(()) => DeleteCollectionResult::Ok,
            FsResult::Err(err) => DeleteCollectionResult::Err(err.into()),
            FsResult::Io(io) => DeleteCollectionResult::Io(io),
        }
    }
}
