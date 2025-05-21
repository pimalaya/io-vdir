use std::{collections::HashMap, path::PathBuf};

use io_fs::{
    coroutines::{
        create_files::{CreateFiles, CreateFilesError, CreateFilesResult},
        rename::{Rename, RenameError, RenameResult},
    },
    io::FsIo,
};
use thiserror::Error;

use crate::{
    collection::Collection,
    constants::{COLOR, DESCRIPTION, DISPLAYNAME, TMP},
};

#[derive(Clone, Debug, Error)]
pub enum UpdateCollectionError {
    #[error("Create new Vdir collection metadata")]
    CreateNewMetadata(#[from] CreateFilesError),
    #[error("Save Vdir collection metadata")]
    SaveMetadata(#[from] RenameError),
}

#[derive(Clone, Debug)]
pub enum UpdateCollectionResult {
    Ok,
    Err(UpdateCollectionError),
    Io(FsIo),
}

#[derive(Debug)]
enum State {
    CreateMetadataTempFiles(CreateFiles, Vec<(PathBuf, PathBuf)>),
    MoveMetadataFiles(Rename),
}

#[derive(Debug)]
pub struct UpdateCollection {
    state: State,
}

impl UpdateCollection {
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

    pub fn resume(&mut self, mut input: Option<FsIo>) -> UpdateCollectionResult {
        loop {
            match &mut self.state {
                State::CreateMetadataTempFiles(fs, rename_paths) => {
                    match fs.resume(input.take()) {
                        CreateFilesResult::Ok => (),
                        CreateFilesResult::Err(err) => {
                            break UpdateCollectionResult::Err(err.into())
                        }
                        CreateFilesResult::Io(io) => break UpdateCollectionResult::Io(io),
                    };

                    let fs = Rename::new(rename_paths.drain(..));
                    self.state = State::MoveMetadataFiles(fs);
                }
                State::MoveMetadataFiles(fs) => {
                    match fs.resume(input.take()) {
                        RenameResult::Ok => (),
                        RenameResult::Err(err) => break UpdateCollectionResult::Err(err.into()),
                        RenameResult::Io(io) => break UpdateCollectionResult::Io(io),
                    };

                    break UpdateCollectionResult::Ok;
                }
            }
        }
    }
}
