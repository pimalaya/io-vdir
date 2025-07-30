//! I/O-free coroutine to create a Vdir item.

use io_fs::{
    coroutines::create_file::CreateFile,
    error::{FsError, FsResult},
    io::FsIo,
};
use thiserror::Error;

use crate::item::Item;

/// Errors that can occur during the coroutine progression.
#[derive(Clone, Debug, Error)]
pub enum CreateItemError {
    /// An error occured during the file creation.
    #[error("Create Vdir item error")]
    CreateFileError(#[from] FsError),
}

/// Output emitted when the coroutine terminates its progression.
#[derive(Clone, Debug)]
pub enum CreateItemResult {
    /// The coroutine successfully terminated its progression.
    Ok,

    /// The coroutine encountered an error.
    Err(CreateItemError),

    /// An I/O needs to be processed in order to make the coroutine
    /// progress further.
    Io(FsIo),
}

/// I/O-free coroutine to create a Vdir item.
#[derive(Debug)]
pub struct CreateItem(CreateFile);

impl CreateItem {
    /// Creates a new coroutine from the given item.
    pub fn new(item: Item) -> Self {
        let bytes = item.to_string().into_bytes();
        Self(CreateFile::new(item.path, bytes))
    }

    /// Makes the coroutine progress.
    pub fn resume(&mut self, arg: Option<FsIo>) -> CreateItemResult {
        match self.0.resume(arg) {
            FsResult::Ok(()) => CreateItemResult::Ok,
            FsResult::Err(err) => CreateItemResult::Err(err.into()),
            FsResult::Io(io) => CreateItemResult::Io(io),
        }
    }
}
