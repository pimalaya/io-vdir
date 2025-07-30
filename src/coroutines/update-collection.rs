//! I/O-free coroutine to update a Vdir collection.

use std::{collections::HashMap, path::PathBuf};

use io_fs::{
    coroutines::{create_files::CreateFiles, rename::Rename},
    error::{FsError, FsResult},
    io::FsIo,
};
use thiserror::Error;

use crate::{
    collection::Collection,
    constants::{COLOR, DESCRIPTION, DISPLAYNAME, TMP},
};

/// Errors that can occur during the coroutine progression.
#[derive(Clone, Debug, Error)]
pub enum UpdateCollectionError {
    /// An error occured during the creation of new metadata files.
    #[error("Create new Vdir collection metadata")]
    CreateNewMetadata(#[source] FsError),

    /// An error occured during the switch between old and new
    /// metadata files.
    #[error("Save Vdir collection metadata")]
    SaveMetadata(#[source] FsError),
}

/// Output emitted when the coroutine terminates its progression.
#[derive(Clone, Debug)]
pub enum UpdateCollectionResult {
    /// The coroutine successfully terminated its progression.
    Ok,

    /// The coroutine encountered an error.
    Err(UpdateCollectionError),

    /// An I/O needs to be processed in order to make the coroutine
    /// progress further.
    Io(FsIo),
}

#[derive(Debug)]
enum State {
    CreateMetadataTempFiles(CreateFiles, Vec<(PathBuf, PathBuf)>),
    MoveMetadataFiles(Rename),
}

/// I/O-free coroutine to update a Vdir collection.
#[derive(Debug)]
pub struct UpdateCollection {
    state: State,
}

impl UpdateCollection {
    /// Creates a new coroutine from the given collection.
    pub fn new(mut collection: Collection) -> Self {
        let mut contents = HashMap::new();
        let mut rename_paths = Vec::new();

        if let Some(name) = collection.display_name.take() {
            let path = collection.path.join(DISPLAYNAME);
            let tmp_path = path.with_extension(TMP);
            contents.insert(tmp_path.clone(), name.into_bytes());
            rename_paths.push((tmp_path, path));
        }

        if let Some(desc) = collection.description.take() {
            let path = collection.path.join(DESCRIPTION);
            let tmp_path = path.with_extension(TMP);
            contents.insert(tmp_path.clone(), desc.into_bytes());
            rename_paths.push((tmp_path, path));
        }

        if let Some(color) = collection.color.take() {
            let path = collection.path.join(COLOR);
            let tmp_path = path.with_extension(TMP);
            contents.insert(tmp_path.clone(), color.into_bytes());
            rename_paths.push((tmp_path, path));
        }

        let fs = CreateFiles::new(contents);
        let state = State::CreateMetadataTempFiles(fs, rename_paths);

        Self { state }
    }

    /// Makes the coroutine progress.
    pub fn resume(&mut self, mut arg: Option<FsIo>) -> UpdateCollectionResult {
        loop {
            match &mut self.state {
                State::CreateMetadataTempFiles(fs, rename_paths) => {
                    match fs.resume(arg.take()) {
                        FsResult::Ok(()) => (),
                        FsResult::Io(io) => break UpdateCollectionResult::Io(io),
                        FsResult::Err(err) => {
                            let err = UpdateCollectionError::CreateNewMetadata(err);
                            break UpdateCollectionResult::Err(err);
                        }
                    };

                    let fs = Rename::new(rename_paths.drain(..));
                    self.state = State::MoveMetadataFiles(fs);
                }
                State::MoveMetadataFiles(fs) => {
                    match fs.resume(arg.take()) {
                        FsResult::Ok(()) => (),
                        FsResult::Io(io) => break UpdateCollectionResult::Io(io),
                        FsResult::Err(err) => {
                            let err = UpdateCollectionError::SaveMetadata(err);
                            break UpdateCollectionResult::Err(err);
                        }
                    };

                    break UpdateCollectionResult::Ok;
                }
            }
        }
    }
}
