//! Standard, blocking Vdir client driving any coroutine against
//! [`std::fs`].
//!
//! Holds a single filesystem root and exposes one method per common
//! coroutine. Every method runs its coroutine to completion through
//! [`VdirClient::run`] by servicing each [`VdirYield`] request via
//! [`std::fs`].

use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
    vec::Vec,
};

use std::{fs, io};

use getrandom::fill;
use log::trace;
use thiserror::Error;

use crate::{
    collection::{Collection, create::*, delete::*, list::*, rename::*, update::*},
    coroutine::*,
    item::{Item, ItemKind, copy::*, delete::*, get::*, list::*, locate::*, r#move::*, store::*},
    path::VdirPath,
};

/// Errors returned by the [`VdirClient`] helpers.
#[derive(Debug, Error)]
pub enum VdirClientError {
    #[error(transparent)]
    VdirCollectionCreate(#[from] VdirCollectionCreateError),
    #[error(transparent)]
    VdirCollectionDelete(#[from] VdirCollectionDeleteError),
    #[error(transparent)]
    VdirCollectionList(#[from] VdirCollectionListError),
    #[error(transparent)]
    VdirCollectionRename(#[from] VdirCollectionRenameError),
    #[error(transparent)]
    VdirCollectionUpdate(#[from] VdirCollectionUpdateError),

    #[error(transparent)]
    VdirItemLocate(#[from] VdirItemLocateError),
    #[error(transparent)]
    VdirItemGet(#[from] VdirItemGetError),
    #[error(transparent)]
    VdirItemList(#[from] VdirItemListError),
    #[error(transparent)]
    VdirItemStore(#[from] VdirItemStoreError),
    #[error(transparent)]
    VdirItemCopy(#[from] VdirItemCopyError),
    #[error(transparent)]
    VdirItemMove(#[from] VdirItemMoveError),
    #[error(transparent)]
    VdirItemDelete(#[from] VdirItemDeleteError),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error("Failed to gather randomness for new item id: {0}")]
    Random(getrandom::Error),
}

/// Std-blocking Vdir client wrapping a filesystem root.
#[derive(Debug)]
pub struct VdirClient {
    root: VdirPath,
}

impl VdirClient {
    /// Builds a client rooted at `root`. No filesystem check is
    /// performed at construction time.
    pub fn new(root: impl Into<VdirPath>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the filesystem root this client operates on.
    pub fn root(&self) -> &VdirPath {
        &self.root
    }

    /// Drives any standard-shape coroutine (`Yield = VdirYield`,
    /// `Return = Result<Output, Error>`) against the local filesystem
    /// until it terminates.
    pub fn run<C, T, E>(&self, mut coroutine: C) -> Result<T, VdirClientError>
    where
        C: VdirCoroutine<Yield = VdirYield, Return = Result<T, E>>,
        VdirClientError: From<E>,
    {
        let mut arg: Option<VdirReply> = None;

        loop {
            match coroutine.resume(arg.take()) {
                VdirCoroutineState::Complete(Ok(out)) => return Ok(out),
                VdirCoroutineState::Complete(Err(err)) => return Err(err.into()),
                VdirCoroutineState::Yielded(VdirYield::WantsRandom { len }) => {
                    let mut bytes = vec![0u8; len];
                    fill(&mut bytes).map_err(VdirClientError::Random)?;
                    arg = Some(VdirReply::Random(bytes));
                }
                VdirCoroutineState::Yielded(VdirYield::WantsFileExists(paths)) => {
                    arg = Some(VdirReply::FileExists(file_exists(paths)));
                }
                VdirCoroutineState::Yielded(VdirYield::WantsDirExists(paths)) => {
                    arg = Some(VdirReply::DirExists(dir_exists(paths)));
                }
                VdirCoroutineState::Yielded(VdirYield::WantsDirRead(paths)) => {
                    arg = Some(VdirReply::DirRead(read_dirs(paths)?));
                }
                VdirCoroutineState::Yielded(VdirYield::WantsFileRead(paths)) => {
                    arg = Some(VdirReply::FileRead(read_files(paths)?));
                }
                VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files)) => {
                    write_files(files)?;
                    arg = Some(VdirReply::FileCreate);
                }
                VdirCoroutineState::Yielded(VdirYield::WantsDirCreate(paths)) => {
                    create_dirs(paths)?;
                    arg = Some(VdirReply::DirCreate);
                }
                VdirCoroutineState::Yielded(VdirYield::WantsDirRemove(paths)) => {
                    remove_dirs(paths)?;
                    arg = Some(VdirReply::DirRemove);
                }
                VdirCoroutineState::Yielded(VdirYield::WantsFileRemove(paths)) => {
                    remove_files(paths)?;
                    arg = Some(VdirReply::FileRemove);
                }
                VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs)) => {
                    rename_paths(pairs)?;
                    arg = Some(VdirReply::Rename);
                }
                VdirCoroutineState::Yielded(VdirYield::WantsCopy(pairs)) => {
                    copy_paths(pairs)?;
                    arg = Some(VdirReply::Copy);
                }
            }
        }
    }

    // ---- Collection lifecycle ----------------------------------------

    /// Runs [`VdirCollectionCreate`]: creates the collection directory and
    /// writes its metadata files when present.
    pub fn create_collection(&self, collection: Collection) -> Result<(), VdirClientError> {
        self.run(VdirCollectionCreate::new(
            collection,
            VdirCollectionCreateOptions::default(),
        ))
    }

    /// Runs [`VdirCollectionDelete`]: recursively removes the collection
    /// rooted at `path`.
    pub fn delete_collection(&self, path: impl Into<VdirPath>) -> Result<(), VdirClientError> {
        self.run(VdirCollectionDelete::new(
            path,
            VdirCollectionDeleteOptions::default(),
        ))
    }

    /// Runs [`VdirCollectionList`]: enumerates every collection directly
    /// under [`self.root`](Self::root).
    pub fn list_collections(&self) -> Result<BTreeSet<Collection>, VdirClientError> {
        self.run(VdirCollectionList::new(
            self.root.clone(),
            VdirCollectionListOptions::default(),
        ))
    }

    /// Runs [`VdirCollectionRename`]: renames the collection at `path` to
    /// `name` (keeping the same parent directory).
    pub fn rename_collection(
        &self,
        path: impl Into<VdirPath>,
        name: impl ToString,
    ) -> Result<(), VdirClientError> {
        self.run(VdirCollectionRename::new(
            path,
            name,
            VdirCollectionRenameOptions::default(),
        ))
    }

    /// Runs [`VdirCollectionUpdate`]: atomically rewrites the metadata of
    /// `collection`.
    pub fn update_collection(&self, collection: Collection) -> Result<(), VdirClientError> {
        self.run(VdirCollectionUpdate::new(
            collection,
            VdirCollectionUpdateOptions::default(),
        ))
    }

    // ---- Items -------------------------------------------------------

    /// Runs [`VdirItemLocate`]: finds the on-disk path of item `id` inside
    /// `collection`.
    pub fn locate_item(
        &self,
        collection: impl Into<VdirPath>,
        id: impl ToString,
    ) -> Result<(VdirPath, ItemKind), VdirClientError> {
        let VdirItemLocateOutput { path, kind } = self.run(VdirItemLocate::new(
            collection,
            id,
            VdirItemLocateOptions::default(),
        ))?;
        Ok((path, kind))
    }

    /// Runs [`VdirItemGet`]: locates item `id` in `collection` and reads
    /// its contents from disk.
    pub fn get_item(
        &self,
        collection: impl Into<VdirPath>,
        id: impl ToString,
    ) -> Result<Item, VdirClientError> {
        self.run(VdirItemGet::new(
            collection,
            id,
            VdirItemGetOptions::default(),
        ))
    }

    /// Runs [`VdirItemList`]: scans `collection` and returns every
    /// `.vcf`/`.ics` entry, contents included.
    pub fn list_items(
        &self,
        collection: impl Into<VdirPath>,
    ) -> Result<BTreeSet<Item>, VdirClientError> {
        self.run(VdirItemList::new(
            collection,
            VdirItemListOptions::default(),
        ))
    }

    /// Runs [`VdirItemStore`]: writes `contents` as a new (or updated) item
    /// under `collection`. Returns the (possibly generated) id and
    /// final on-disk path.
    ///
    /// When `id` is `None`, a fresh UUIDv4 is generated from the system
    /// entropy source.
    pub fn store_item(
        &self,
        collection: impl Into<VdirPath>,
        id: Option<String>,
        kind: ItemKind,
        contents: Vec<u8>,
    ) -> Result<(String, VdirPath), VdirClientError> {
        let VdirItemStoreOutput { id, path } = self.run(VdirItemStore::new(
            collection,
            id,
            kind,
            contents,
            VdirItemStoreOptions::default(),
        ))?;
        Ok((id, path))
    }

    /// Runs [`VdirItemCopy`]: copies item `id` from `source` into `target`.
    pub fn copy_item(
        &self,
        source: impl Into<VdirPath>,
        target: impl Into<VdirPath>,
        id: impl ToString,
    ) -> Result<(), VdirClientError> {
        self.run(VdirItemCopy::new(
            source,
            target,
            id,
            VdirItemCopyOptions::default(),
        ))
    }

    /// Runs [`VdirItemMove`]: moves item `id` from `source` into `target`.
    pub fn move_item(
        &self,
        source: impl Into<VdirPath>,
        target: impl Into<VdirPath>,
        id: impl ToString,
    ) -> Result<(), VdirClientError> {
        self.run(VdirItemMove::new(
            source,
            target,
            id,
            VdirItemMoveOptions::default(),
        ))
    }

    /// Runs [`VdirItemDelete`]: removes item `id` from `collection`.
    pub fn delete_item(
        &self,
        collection: impl Into<VdirPath>,
        id: impl ToString,
    ) -> Result<(), VdirClientError> {
        self.run(VdirItemDelete::new(
            collection,
            id,
            VdirItemDeleteOptions::default(),
        ))
    }
}

// ---- Path normalization -----------------------------------------

fn normalize_path(path: std::path::PathBuf) -> VdirPath {
    let s = path.to_string_lossy().into_owned();
    #[cfg(windows)]
    let s = s.replace('\\', "/");
    VdirPath::new(s)
}

// ---- Filesystem helpers -----------------------------------------

fn create_dirs(paths: BTreeSet<VdirPath>) -> Result<(), io::Error> {
    for path in paths {
        trace!("create_dir_all {path}");
        fs::create_dir_all(path.as_str())?;
    }
    Ok(())
}

fn remove_dirs(paths: BTreeSet<VdirPath>) -> Result<(), io::Error> {
    for path in paths {
        trace!("remove_dir_all {path}");
        fs::remove_dir_all(path.as_str())?;
    }
    Ok(())
}

fn remove_files(paths: BTreeSet<VdirPath>) -> Result<(), io::Error> {
    for path in paths {
        trace!("remove_file {path}");
        fs::remove_file(path.as_str())?;
    }
    Ok(())
}

fn write_files(files: BTreeMap<VdirPath, Vec<u8>>) -> Result<(), io::Error> {
    for (path, contents) in files {
        trace!("write {path} ({} bytes)", contents.len());

        if let Some(parent) = std::path::Path::new(path.as_str()).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path.as_str(), &contents)?;
    }
    Ok(())
}

fn read_dirs(
    paths: BTreeSet<VdirPath>,
) -> Result<BTreeMap<VdirPath, BTreeSet<VdirPath>>, io::Error> {
    let mut entries = BTreeMap::new();

    for path in paths {
        trace!("read_dir {path}");

        let mut names = BTreeSet::new();
        match fs::read_dir(path.as_str()) {
            Ok(iter) => {
                for entry in iter {
                    let entry = entry?;
                    names.insert(normalize_path(entry.path()));
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }

        entries.insert(path, names);
    }

    Ok(entries)
}

fn read_files(paths: BTreeSet<VdirPath>) -> Result<BTreeMap<VdirPath, Vec<u8>>, io::Error> {
    let mut contents = BTreeMap::new();

    for path in paths {
        trace!("read_file {path}");
        let bytes = fs::read(path.as_str())?;
        contents.insert(path, bytes);
    }

    Ok(contents)
}

fn rename_paths(pairs: Vec<(VdirPath, VdirPath)>) -> Result<(), io::Error> {
    for (from, to) in pairs {
        trace!("rename {from} -> {to}");
        fs::rename(from.as_str(), to.as_str())?;
    }
    Ok(())
}

fn copy_paths(pairs: Vec<(VdirPath, VdirPath)>) -> Result<(), io::Error> {
    for (from, to) in pairs {
        trace!("copy {from} -> {to}");
        fs::copy(from.as_str(), to.as_str())?;
    }
    Ok(())
}

fn file_exists(paths: BTreeSet<VdirPath>) -> BTreeMap<VdirPath, bool> {
    let mut out = BTreeMap::new();
    for path in paths {
        let exists = fs::metadata(path.as_str())
            .map(|m| m.is_file())
            .unwrap_or(false);
        trace!("file_exists {path}: {exists}");
        out.insert(path, exists);
    }
    out
}

fn dir_exists(paths: BTreeSet<VdirPath>) -> BTreeMap<VdirPath, bool> {
    let mut out = BTreeMap::new();
    for path in paths {
        let exists = fs::metadata(path.as_str())
            .map(|m| m.is_dir())
            .unwrap_or(false);
        trace!("dir_exists {path}: {exists}");
        out.insert(path, exists);
    }
    out
}
