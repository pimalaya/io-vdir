//! I/O-free coroutine to delete a Vdir item.

use std::path::Path;

use io_fs::{
    coroutines::remove_file::RemoveFile,
    error::{FsError, FsResult},
    io::FsIo,
};
use thiserror::Error;

/// Errors that can occur during the coroutine progression.
#[derive(Clone, Debug, Error)]
pub enum DeleteItemError {
    /// An error occured during the file deletion.
    #[error("Delete Vdir item error")]
    RemoveFileError(#[from] FsError),
}

/// Output emitted when the coroutine terminates its progression.
#[derive(Clone, Debug)]
pub enum DeleteItemResult {
    /// The coroutine successfully terminated its progression.
    Ok,

    /// The coroutine encountered an error.
    Err(DeleteItemError),

    /// An I/O needs to be processed in order to make the coroutine
    /// progress further.
    Io(FsIo),
}

/// I/O-free coroutine to delete a Vdir item.
#[derive(Debug)]
pub struct DeleteItem(RemoveFile);

impl DeleteItem {
    /// Creates a new coroutine from the given collection's path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self(RemoveFile::new(path.as_ref()))
    }

    /// Makes the coroutine progress.
    pub fn resume(&mut self, arg: Option<FsIo>) -> DeleteItemResult {
        match self.0.resume(arg) {
            FsResult::Ok(()) => DeleteItemResult::Ok,
            FsResult::Err(err) => DeleteItemResult::Err(err.into()),
            FsResult::Io(io) => DeleteItemResult::Io(io),
        }
    }
}
