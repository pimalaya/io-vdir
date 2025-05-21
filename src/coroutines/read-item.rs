use std::{mem, path::PathBuf};

use calcard::{icalendar::ICalendar, vcard::VCard};
use io_fs::{
    coroutines::read_file::{ReadFile, ReadFileError, ReadFileResult},
    io::FsIo,
};
use thiserror::Error;

use crate::{
    constants::{ICS, VCF},
    item::{Item, ItemKind},
};

#[derive(Clone, Debug, Error)]
pub enum ReadItemError {
    #[error("Read Vdir item file error")]
    ReadFile(#[from] ReadFileError),
    #[error("Missing Vdir item file extension at {0}")]
    MissingExt(PathBuf),
    #[error("Invalid Vdir item file extension at {0}")]
    InvalidExt(PathBuf),
    #[error("Invalid Vdir item file contents at {0}")]
    InvalidContents(PathBuf),
    #[error("Invalid vCard contents at {1} ({0})")]
    InvalidVcardContents(String, PathBuf),
    #[error("Invalid iCal contents at {1} ({0})")]
    InvalidIcalContents(String, PathBuf),
}

#[derive(Clone, Debug)]
pub enum ReadItemResult {
    Ok(Item),
    Err(ReadItemError),
    Io(FsIo),
}

#[derive(Debug)]
pub struct ReadItem {
    path: PathBuf,
    fs: ReadFile,
}

impl ReadItem {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let fs = ReadFile::new(&path);

        Self { path, fs }
    }

    pub fn resume(&mut self, input: Option<FsIo>) -> ReadItemResult {
        let p = self.path.clone();

        let Some(ext) = self.path.extension() else {
            return ReadItemResult::Err(ReadItemError::MissingExt(p));
        };

        let contents = match self.fs.resume(input) {
            ReadFileResult::Ok(paths) => paths,
            ReadFileResult::Err(err) => return ReadItemResult::Err(err.into()),
            ReadFileResult::Io(io) => return ReadItemResult::Io(io),
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
