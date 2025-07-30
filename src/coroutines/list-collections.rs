//! I/O-free coroutine to list Vdir collections.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use io_fs::{
    coroutines::{read_dir::ReadDir, read_files::ReadFiles},
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
pub enum ListCollectionsError {
    /// An error occured during the directory listing.
    #[error("List Vdir collections error")]
    ListDirsError(#[source] FsError),

    /// An error occured during the metadata file listing.
    #[error("Read Vdir collections' metadata error")]
    ListFilesError(#[source] FsError),
}

/// Output emitted when the coroutine terminates its progression.
#[derive(Clone, Debug)]
pub enum ListCollectionsResult {
    /// The coroutine successfully terminated its progression.
    Ok(HashSet<Collection>),

    /// The coroutine encountered an error.
    Err(ListCollectionsError),

    /// An I/O needs to be processed in order to make the coroutine
    /// progress further.
    Io(FsIo),
}

#[derive(Debug)]
enum State {
    ListCollections(ReadDir),
    ReadMetadataFiles(HashSet<PathBuf>, ReadFiles),
}

/// I/O-free coroutine to list Vdir collections.
#[derive(Debug)]
pub struct ListCollections {
    state: State,
}

impl ListCollections {
    /// Creates a new coroutine from the given root path.
    pub fn new(root: impl AsRef<Path>) -> Self {
        let fs = ReadDir::new(root.as_ref());
        let state = State::ListCollections(fs);

        Self { state }
    }

    /// Makes the coroutine progress.
    pub fn resume(&mut self, mut arg: Option<FsIo>) -> ListCollectionsResult {
        loop {
            match &mut self.state {
                State::ListCollections(fs) => {
                    let mut collection_paths = match fs.resume(arg.take()) {
                        FsResult::Ok(paths) => paths,
                        FsResult::Io(io) => break ListCollectionsResult::Io(io),
                        FsResult::Err(err) => {
                            let err = ListCollectionsError::ListDirsError(err);
                            break ListCollectionsResult::Err(err);
                        }
                    };

                    collection_paths.retain(|path| path.is_dir());

                    let mut metadata_paths = HashSet::new();

                    for dir in &collection_paths {
                        let name_path = dir.join(DISPLAYNAME);

                        if name_path.is_file() {
                            metadata_paths.insert(name_path);
                        }

                        let desc_path = dir.join(DESCRIPTION);

                        if desc_path.is_file() {
                            metadata_paths.insert(desc_path);
                        }

                        let color_path = dir.join(COLOR);

                        if color_path.is_file() {
                            metadata_paths.insert(color_path);
                        }
                    }

                    let flow = ReadFiles::new(metadata_paths);
                    self.state = State::ReadMetadataFiles(collection_paths, flow);
                }
                State::ReadMetadataFiles(collection_paths, fs) => {
                    let mut metadata = match fs.resume(arg.take()) {
                        FsResult::Ok(meta) => meta,
                        FsResult::Io(io) => break ListCollectionsResult::Io(io),
                        FsResult::Err(err) => {
                            let err = ListCollectionsError::ListFilesError(err);
                            break ListCollectionsResult::Err(err);
                        }
                    };

                    let mut collections = HashSet::new();

                    for path in collection_paths.clone() {
                        let display_name = path.join(DISPLAYNAME);
                        let description = path.join(DESCRIPTION);
                        let color = path.join(COLOR);

                        let mut collection = Collection {
                            path,
                            display_name: None,
                            description: None,
                            color: None,
                        };

                        if let Some(name) = &metadata.remove(&display_name) {
                            let name = String::from_utf8_lossy(name);

                            if name.trim().is_empty() {
                                collection.display_name = None
                            } else {
                                collection.display_name = Some(name.to_string());
                            }
                        }

                        if let Some(desc) = &metadata.remove(&description) {
                            let desc = String::from_utf8_lossy(desc);

                            if desc.trim().is_empty() {
                                collection.description = None
                            } else {
                                collection.description = Some(desc.to_string());
                            }
                        }

                        if let Some(color) = &metadata.remove(&color) {
                            let color = String::from_utf8_lossy(color);

                            if color.trim().is_empty() {
                                collection.color = None
                            } else {
                                collection.color = Some(color.to_string());
                            }
                        }

                        collections.insert(collection);
                    }

                    break ListCollectionsResult::Ok(collections);
                }
            }
        }
    }
}
