use std::path::Path;

use io_fs::{
    coroutines::remove_file::{RemoveFile, RemoveFileError, RemoveFileResult},
    io::FsIo,
};
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum DeleteItemError {
    #[error("Delete Vdir item error")]
    RemoveFileError(#[from] RemoveFileError),
}

#[derive(Clone, Debug)]
pub enum DeleteItemResult {
    Ok,
    Err(DeleteItemError),
    Io(FsIo),
}

#[derive(Debug)]
pub struct DeleteItem(RemoveFile);

impl DeleteItem {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self(RemoveFile::new(path.as_ref()))
    }

    pub fn resume(&mut self, input: Option<FsIo>) -> DeleteItemResult {
        match self.0.resume(input) {
            RemoveFileResult::Ok => DeleteItemResult::Ok,
            RemoveFileResult::Err(err) => DeleteItemResult::Err(err.into()),
            RemoveFileResult::Io(io) => DeleteItemResult::Io(io),
        }
    }
}
