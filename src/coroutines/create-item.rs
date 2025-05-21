use io_fs::{
    coroutines::create_file::{CreateFile, CreateFileError, CreateFileResult},
    io::FsIo,
};
use thiserror::Error;

use crate::item::Item;

#[derive(Clone, Debug, Error)]
pub enum CreateItemError {
    #[error("Create Vdir item error")]
    CreateFileError(#[from] CreateFileError),
}

#[derive(Clone, Debug)]
pub enum CreateItemResult {
    Ok,
    Err(CreateItemError),
    Io(FsIo),
}

#[derive(Debug)]
pub struct CreateItem(CreateFile);

impl CreateItem {
    pub fn new(item: Item) -> Self {
        let bytes = item.to_string().into_bytes();
        Self(CreateFile::new(item.path, bytes))
    }

    pub fn resume(&mut self, input: Option<FsIo>) -> CreateItemResult {
        match self.0.resume(input) {
            CreateFileResult::Ok => CreateItemResult::Ok,
            CreateFileResult::Err(err) => CreateItemResult::Err(err.into()),
            CreateFileResult::Io(io) => CreateItemResult::Io(io),
        }
    }
}
