use std::path::PathBuf;

use io_fs::{
    coroutines::{
        create_file::{CreateFile, CreateFileError, CreateFileResult},
        rename::{Rename, RenameError, RenameResult},
    },
    io::FsIo,
};
use thiserror::Error;

use crate::{constants::TMP, item::Item};

#[derive(Clone, Debug, Error)]
pub enum UpdateItemError {
    #[error("Create temporary Vdir item file error")]
    CreateTempFile(#[from] CreateFileError),
    #[error("Save Vdir item file error")]
    SaveFile(#[from] RenameError),
}

#[derive(Clone, Debug)]
pub enum UpdateItemResult {
    Ok,
    Err(UpdateItemError),
    Io(FsIo),
}

#[derive(Debug)]
enum State {
    CreateTempItem(CreateFile),
    MoveItem(Rename),
}

#[derive(Debug)]
pub struct UpdateItem {
    path: PathBuf,
    path_tmp: PathBuf,
    state: State,
}

impl UpdateItem {
    pub fn new(item: Item) -> Self {
        let path_tmp = item.path.with_extension(TMP);
        let fs = CreateFile::new(&path_tmp, item.to_string().into_bytes());
        let state = State::CreateTempItem(fs);

        Self {
            path: item.path,
            path_tmp,
            state,
        }
    }

    pub fn resume(&mut self, mut input: Option<FsIo>) -> UpdateItemResult {
        loop {
            match &mut self.state {
                State::CreateTempItem(fs) => {
                    match fs.resume(input.take()) {
                        CreateFileResult::Ok => (),
                        CreateFileResult::Err(err) => break UpdateItemResult::Err(err.into()),
                        CreateFileResult::Io(io) => break UpdateItemResult::Io(io),
                    };
                    let fs = Rename::new(Some((&self.path_tmp, &self.path)));
                    self.state = State::MoveItem(fs);
                }
                State::MoveItem(fs) => {
                    match fs.resume(input.take()) {
                        RenameResult::Ok => (),
                        RenameResult::Err(err) => break UpdateItemResult::Err(err.into()),
                        RenameResult::Io(io) => break UpdateItemResult::Io(io),
                    };

                    break UpdateItemResult::Ok;
                }
            }
        }
    }
}
