use std::collections::HashMap;

use io_fs::{
    coroutines::{
        create_dir::{CreateDir, CreateDirError, CreateDirResult},
        create_files::{CreateFiles, CreateFilesError, CreateFilesResult},
    },
    io::FsIo,
};
use thiserror::Error;

use crate::{
    collection::Collection,
    constants::{COLOR, DESCRIPTION, DISPLAYNAME},
};

#[derive(Clone, Debug, Error)]
pub enum CreateCollectionError {
    #[error("Create Vdir collection error")]
    CreateDirError(#[from] CreateDirError),
    #[error("Create Vdir metadata error")]
    CreateFilesError(#[from] CreateFilesError),
}

#[derive(Clone, Debug)]
pub enum CreateCollectionResult {
    Ok,
    Err(CreateCollectionError),
    Io(FsIo),
}

#[derive(Debug)]
enum State {
    CreateCollection(CreateDir),
    CreateMetadataFiles(CreateFiles),
}

#[derive(Debug)]
pub struct CreateCollection {
    collection: Collection,
    state: State,
}

impl CreateCollection {
    pub fn new(collection: Collection) -> Self {
        let fs = CreateDir::new(&collection.path);
        let state = State::CreateCollection(fs);

        Self { collection, state }
    }

    pub fn resume(&mut self, mut input: Option<FsIo>) -> CreateCollectionResult {
        loop {
            match &mut self.state {
                State::CreateCollection(fs) => {
                    match fs.resume(input.take()) {
                        CreateDirResult::Ok => (),
                        CreateDirResult::Err(err) => break CreateCollectionResult::Err(err.into()),
                        CreateDirResult::Io(io) => break CreateCollectionResult::Io(io),
                    };

                    let display_name = self.collection.display_name.clone();
                    let description = self.collection.description.take();
                    let color = self.collection.color.take();

                    let mut contents = HashMap::new();

                    if let Some(name) = display_name {
                        contents.insert(self.collection.path.join(DISPLAYNAME), name.into_bytes());
                    }

                    if let Some(desc) = description {
                        contents.insert(self.collection.path.join(DESCRIPTION), desc.into_bytes());
                    }

                    if let Some(color) = color {
                        contents.insert(self.collection.path.join(COLOR), color.into_bytes());
                    }

                    if contents.is_empty() {
                        break CreateCollectionResult::Ok;
                    }

                    let fs = CreateFiles::new(contents);
                    self.state = State::CreateMetadataFiles(fs);
                }
                State::CreateMetadataFiles(fs) => match fs.resume(input.take()) {
                    CreateFilesResult::Ok => break CreateCollectionResult::Ok,
                    CreateFilesResult::Err(err) => break CreateCollectionResult::Err(err.into()),
                    CreateFilesResult::Io(io) => break CreateCollectionResult::Io(io),
                },
            }
        }
    }
}
