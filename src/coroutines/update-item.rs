//! I/O-free coroutine to update a Vdir item.

use std::path::PathBuf;

use io_fs::{
    coroutines::{create_file::CreateFile, rename::Rename},
    error::{FsError, FsResult},
    io::FsIo,
};
use thiserror::Error;

use crate::{constants::TMP, item::Item};

/// Errors that can occur during the coroutine progression.
#[derive(Clone, Debug, Error)]
pub enum UpdateItemError {
    /// An error occured during the creation of new item file.
    #[error("Create temporary Vdir item file error")]
    CreateTempFile(#[source] FsError),

    /// An error occured during the switch between old and new item
    /// files.
    #[error("Save Vdir item file error")]
    SaveFile(#[source] FsError),
}

/// Output emitted when the coroutine terminates its progression.
#[derive(Clone, Debug)]
pub enum UpdateItemResult {
    /// The coroutine successfully terminated its progression.
    Ok,

    /// The coroutine encountered an error.
    Err(UpdateItemError),

    /// An I/O needs to be processed in order to make the coroutine
    /// progress further.
    Io(FsIo),
}

#[derive(Debug)]
enum State {
    CreateTempItem(CreateFile),
    MoveItem(Rename),
}

/// I/O-free coroutine to update a Vdir item.
#[derive(Debug)]
pub struct UpdateItem {
    path: PathBuf,
    path_tmp: PathBuf,
    state: State,
}

impl UpdateItem {
    /// Creates a new coroutine from the given item.
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

    /// Makes the coroutine progress.
    pub fn resume(&mut self, mut arg: Option<FsIo>) -> UpdateItemResult {
        loop {
            match &mut self.state {
                State::CreateTempItem(fs) => {
                    match fs.resume(arg.take()) {
                        FsResult::Ok(()) => (),
                        FsResult::Io(io) => break UpdateItemResult::Io(io),
                        FsResult::Err(err) => {
                            let err = UpdateItemError::CreateTempFile(err);
                            break UpdateItemResult::Err(err);
                        }
                    };
                    let fs = Rename::new(Some((&self.path_tmp, &self.path)));
                    self.state = State::MoveItem(fs);
                }
                State::MoveItem(fs) => {
                    match fs.resume(arg.take()) {
                        FsResult::Ok(()) => (),
                        FsResult::Io(io) => break UpdateItemResult::Io(io),
                        FsResult::Err(err) => {
                            let err = UpdateItemError::SaveFile(err);
                            break UpdateItemResult::Err(err);
                        }
                    };

                    break UpdateItemResult::Ok;
                }
            }
        }
    }
}
