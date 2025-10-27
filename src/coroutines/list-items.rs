//! I/O-free coroutine to list items in a Vdir collection.

use std::{collections::HashSet, path::Path};

use calcard::{icalendar::ICalendar, vcard::VCard};
use io_fs::{
    coroutines::{read_dir::ReadDir, read_files::ReadFiles},
    error::{FsError, FsResult},
    io::FsIo,
};
use thiserror::Error;

use crate::{
    constants::{ICS, VCF},
    item::Item,
    item::ItemKind,
};

/// Errors that can occur during the coroutine progression.
#[derive(Clone, Debug, Error)]
pub enum ListItemsError {
    /// An error occured during the directory listing.
    #[error("List Vdir items error")]
    ListDirsError(#[source] FsError),

    /// An error occured during the metadata files listing.
    #[error("Read Vdir items' metadata error")]
    ListFilesError(#[source] FsError),
}

/// Output emitted when the coroutine terminates its progression.
#[derive(Clone, Debug)]
pub enum ListItemsResult {
    /// The coroutine successfully terminated its progression.
    Ok(HashSet<Item>),

    /// The coroutine encountered an error.
    Err(ListItemsError),

    /// An I/O needs to be processed in order to make the coroutine
    /// progress further.
    Io(FsIo),
}

#[derive(Debug)]
enum State {
    ListItems(ReadDir),
    ReadItems(ReadFiles),
}

/// I/O-free coroutine to list items in a Vdir collection.
#[derive(Debug)]
pub struct ListItems {
    state: State,
}

impl ListItems {
    /// Creates a new coroutine from the given addressbook path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        let fs = ReadDir::new(path.as_ref());
        let state = State::ListItems(fs);

        Self { state }
    }

    /// Makes the coroutine progress.
    pub fn resume(&mut self, mut arg: Option<FsIo>) -> ListItemsResult {
        loop {
            match &mut self.state {
                State::ListItems(fs) => {
                    let mut item_paths = match fs.resume(arg.take()) {
                        FsResult::Ok(paths) => paths,
                        FsResult::Io(io) => break ListItemsResult::Io(io),
                        FsResult::Err(err) => {
                            let err = ListItemsError::ListDirsError(err);
                            break ListItemsResult::Err(err);
                        }
                    };

                    item_paths.retain(|path| {
                        if !path.is_file() {
                            return false;
                        }

                        let Some(ext) = path.extension() else {
                            return false;
                        };

                        if ext != VCF && ext != ICS {
                            return false;
                        }

                        return true;
                    });

                    let fs = ReadFiles::new(item_paths);
                    self.state = State::ReadItems(fs);
                }
                State::ReadItems(fs) => {
                    let contents = match fs.resume(arg.take()) {
                        FsResult::Ok(contents) => contents,
                        FsResult::Io(io) => break ListItemsResult::Io(io),
                        FsResult::Err(err) => {
                            let err = ListItemsError::ListFilesError(err);
                            break ListItemsResult::Err(err);
                        }
                    };

                    let mut items = HashSet::new();

                    for (path, contents) in contents {
                        let Some(ext) = path.extension() else {
                            continue;
                        };

                        let Ok(contents) = String::from_utf8(contents) else {
                            continue;
                        };

                        if ext == ICS {
                            let Ok(ical) = ICalendar::parse(contents) else {
                                continue;
                            };

                            items.insert(Item {
                                path,
                                kind: ItemKind::Ical(ical),
                            });

                            continue;
                        }

                        if ext == VCF {
                            let Ok(vcard) = VCard::parse(contents) else {
                                continue;
                            };

                            items.insert(Item {
                                path,
                                kind: ItemKind::Vcard(vcard),
                            });
                        }
                    }

                    break ListItemsResult::Ok(items);
                }
            }
        }
    }
}
