//! I/O-free coroutine to read a Vdir item.

use std::{mem, path::PathBuf};

use calcard::{icalendar::ICalendar, vcard::VCard};
use io_fs::{
    coroutines::read_file::ReadFile,
    error::{FsError, FsResult},
    io::FsIo,
};
use thiserror::Error;

use crate::{
    constants::{ICS, VCF},
    item::{Item, ItemKind},
};

/// Errors that can occur during the coroutine progression.
#[derive(Clone, Debug, Error)]
pub enum ReadItemError {
    /// An error occured during the file item reading.
    #[error("Read Vdir item file error")]
    ReadFile(#[from] FsError),

    /// The Vdir item does not have a file extension.
    #[error("Missing Vdir item file extension at {0}")]
    MissingExt(PathBuf),

    /// The Vdir item has an invalid file extension.
    #[error("Invalid Vdir item file extension at {0}")]
    InvalidExt(PathBuf),

    /// The Vdir item has an invalid file contents.
    #[error("Invalid Vdir item file contents at {0}")]
    InvalidContents(PathBuf),

    /// The Vdir item has an invalid vCard contents.
    #[error("Invalid vCard contents at {1} ({0})")]
    InvalidVcardContents(String, PathBuf),

    /// The Vdir item has an invalid iCalendar contents.
    #[error("Invalid iCal contents at {1} ({0})")]
    InvalidIcalContents(String, PathBuf),
}

/// Output emitted when the coroutine terminates its progression.
#[derive(Clone, Debug)]
pub enum ReadItemResult {
    /// The coroutine successfully terminated its progression.
    Ok(Item),

    /// The coroutine encountered an error.
    Err(ReadItemError),

    /// An I/O needs to be processed in order to make the coroutine
    /// progress further.
    Io(FsIo),
}

/// I/O-free coroutine to read a Vdir item.
#[derive(Debug)]
pub struct ReadItem {
    path: PathBuf,
    fs: ReadFile,
}

impl ReadItem {
    /// Creates a new coroutine from the given item path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let fs = ReadFile::new(&path);

        Self { path, fs }
    }

    /// Makes the coroutine progress.
    pub fn resume(&mut self, arg: Option<FsIo>) -> ReadItemResult {
        let p = self.path.clone();

        let Some(ext) = self.path.extension() else {
            return ReadItemResult::Err(ReadItemError::MissingExt(p));
        };

        let contents = match self.fs.resume(arg) {
            FsResult::Ok(paths) => paths,
            FsResult::Err(err) => return ReadItemResult::Err(err.into()),
            FsResult::Io(io) => return ReadItemResult::Io(io),
        };

        let Ok(contents) = String::from_utf8(contents) else {
            return ReadItemResult::Err(ReadItemError::InvalidContents(p));
        };

        if ext == VCF {
            let vcard = match VCard::parse(contents) {
                Ok(vcard) => vcard,
                Err(err) => {
                    // NOTE: err is not a regular error
                    // TODO: make better mapping
                    let err = ReadItemError::InvalidVcardContents(format!("{err:?}"), p);
                    return ReadItemResult::Err(err);
                }
            };

            let item = Item {
                path: mem::take(&mut self.path),
                kind: ItemKind::Vcard(vcard),
            };

            return ReadItemResult::Ok(item);
        }

        if ext == ICS {
            let ical = match ICalendar::parse(contents) {
                Ok(ical) => ical,
                Err(err) => {
                    // NOTE: err is not a regular error
                    // TODO: make better mapping
                    let err = ReadItemError::InvalidIcalContents(format!("{err:?}"), p);
                    return ReadItemResult::Err(err);
                }
            };

            let item = Item {
                path: mem::take(&mut self.path),
                kind: ItemKind::Ical(ical),
            };

            return ReadItemResult::Ok(item);
        }

        ReadItemResult::Err(ReadItemError::InvalidExt(p))
    }
}
