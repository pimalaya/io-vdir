use std::path::Path;

use io_fs::{
    coroutines::remove_dir::{RemoveDir, RemoveDirError, RemoveDirResult},
    io::FsIo,
};
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum DeleteCollectionError {
    #[error("Delete Vdir collection error")]
    RemoveDirError(#[from] RemoveDirError),
}

#[derive(Clone, Debug)]
pub enum DeleteCollectionResult {
    Ok,
    Err(DeleteCollectionError),
    Io(FsIo),
}

#[derive(Debug)]
pub struct DeleteCollection(RemoveDir);

impl DeleteCollection {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self(RemoveDir::new(path.as_ref()))
    }

    pub fn resume(&mut self, input: Option<FsIo>) -> DeleteCollectionResult {
        match self.0.resume(input) {
            RemoveDirResult::Ok => DeleteCollectionResult::Ok,
            RemoveDirResult::Err(err) => DeleteCollectionResult::Err(err.into()),
            RemoveDirResult::Io(io) => DeleteCollectionResult::Io(io),
        }
    }
}
