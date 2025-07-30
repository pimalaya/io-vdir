//! I/O-free coroutine to create a Vdir collection.

use std::collections::HashMap;

use io_fs::{
    coroutines::{create_dir::CreateDir, create_files::CreateFiles},
    error::{FsError, FsResult},
    io::FsIo,
};
use thiserror::Error;

use crate::{
    collection::Collection,
    constants::{COLOR, DESCRIPTION, DISPLAYNAME},
};

/// Errors that can occur during the coroutine progression.
#[derive(Clone, Debug, Error)]
pub enum CreateCollectionError {
    /// An error occured during the directory creation.
    #[error("Create Vdir collection error")]
    CreateDirError(#[source] FsError),

    /// An error occured during the metadata files creation.
    #[error("Create Vdir metadata error")]
    CreateFilesError(#[source] FsError),
}

/// Output emitted when the coroutine terminates its progression.
#[derive(Clone, Debug)]
pub enum CreateCollectionResult {
    /// The coroutine successfully terminated its progression.
    Ok,

    /// The coroutine encountered an error.
    Err(CreateCollectionError),

    /// An I/O needs to be processed in order to make the coroutine
    /// progress further.
    Io(FsIo),
}

#[derive(Debug)]
enum State {
    CreateCollection(CreateDir),
    CreateMetadataFiles(CreateFiles),
}

/// I/O-free coroutine to create a Vdir collection.
#[derive(Debug)]
pub struct CreateCollection {
    collection: Collection,
    state: State,
}

impl CreateCollection {
    /// Creates a new coroutine from the given collection.
    pub fn new(collection: Collection) -> Self {
        let fs = CreateDir::new(&collection.path);
        let state = State::CreateCollection(fs);

        Self { collection, state }
    }

    /// Makes the coroutine progress.
    pub fn resume(&mut self, mut arg: Option<FsIo>) -> CreateCollectionResult {
        loop {
            match &mut self.state {
                State::CreateCollection(fs) => {
                    match fs.resume(arg.take()) {
                        FsResult::Ok(()) => (),
                        FsResult::Io(io) => break CreateCollectionResult::Io(io),
                        FsResult::Err(err) => {
                            let err = CreateCollectionError::CreateDirError(err);
                            break CreateCollectionResult::Err(err);
                        }
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
                State::CreateMetadataFiles(fs) => {
                    break match fs.resume(arg.take()) {
                        FsResult::Ok(()) => CreateCollectionResult::Ok,
                        FsResult::Io(io) => CreateCollectionResult::Io(io),
                        FsResult::Err(err) => {
                            let err = CreateCollectionError::CreateFilesError(err);
                            CreateCollectionResult::Err(err)
                        }
                    }
                }
            }
        }
    }
}
