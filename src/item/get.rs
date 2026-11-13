//! I/O-free coroutine fetching a Vdir item by its ID.
//!
//! Locates the item file via [`VdirItemLocate`], stats it, then reads
//! its contents. The stat comes first, so an item rewritten in between
//! carries a stale stat and a conditional store of it is refused.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::{collections::BTreeMap, fs};
//!
//! use io_vdir::{coroutine::*, item::get::*};
//!
//! let opts = VdirItemGetOptions::default();
//! let mut coroutine = VdirItemGet::new("/tmp/vdir/contacts", "alice", opts);
//! let mut arg = None;
//!
//! let item = loop {
//!     match coroutine.resume(arg.take()) {
//!         VdirCoroutineState::Yielded(VdirYield::WantsFileExists(paths)) => {
//!             let map = paths
//!                 .into_iter()
//!                 .map(|p| {
//!                     let ok = fs::metadata(p.as_str()).map(|m| m.is_file()).unwrap_or(false);
//!                     (p, ok)
//!                 })
//!                 .collect();
//!             arg = Some(VdirReply::FileExists(map));
//!         }
//!         VdirCoroutineState::Yielded(VdirYield::WantsFileMeta(paths)) => {
//!             let mut map = BTreeMap::new();
//!             for path in paths {
//!                 match fs::metadata(path.as_str()) {
//!                     Ok(meta) if meta.is_file() => {
//!                         // fill the mtime and the inode in real code
//!                         let size = meta.len();
//!                         map.insert(path, VdirFileMeta { size, ..Default::default() });
//!                     }
//!                     _ => {}
//!                 }
//!             }
//!             arg = Some(VdirReply::FileMeta(map));
//!         }
//!         VdirCoroutineState::Yielded(VdirYield::WantsFileRead(paths)) => {
//!             let mut map = BTreeMap::new();
//!             for path in paths {
//!                 if let Ok(bytes) = fs::read(path.as_str()) {
//!                     map.insert(path, bytes);
//!                 }
//!             }
//!             arg = Some(VdirReply::FileRead(map));
//!         }
//!         VdirCoroutineState::Complete(Ok(item)) => break item,
//!         VdirCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         state => panic!("unexpected state {state:?}"),
//!     }
//! };
//!
//! println!("{} bytes", item.contents.len());
//! ```

use core::{fmt, mem};

use alloc::{collections::BTreeSet, string::ToString};

use thiserror::Error;

use crate::{
    coroutine::*,
    item::{VdirItem, VdirItemKind, locate::*},
    path::VdirPath,
    vdir_try,
};

/// Failure causes during a [`VdirItemGet`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirItemGetError {
    /// The driver fed back a reply that does not match the pending
    /// request.
    #[error("Vdir item get failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),

    /// The inner locate coroutine failed.
    #[error(transparent)]
    Locate(#[from] VdirItemLocateError),

    /// The driver could not stat or read the located item file.
    #[error("Vdir item get failed: cannot read item file {0}")]
    Unreadable(VdirPath),
}

/// Options for [`VdirItemGet::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirItemGetOptions {}

/// Locates a Vdir item by ID and reads its contents.
#[derive(Debug)]
pub struct VdirItemGet {
    state: State,
    #[allow(dead_code)]
    opts: VdirItemGetOptions,
}

impl VdirItemGet {
    /// Creates a new coroutine that will retrieve item `id` from
    /// `collection`.
    pub fn new(
        collection: impl Into<VdirPath>,
        id: impl ToString,
        opts: VdirItemGetOptions,
    ) -> Self {
        Self {
            opts,
            state: State::Locate(VdirItemLocate::new(
                collection,
                id,
                VdirItemLocateOptions::default(),
            )),
        }
    }
}

impl VdirCoroutine for VdirItemGet {
    type Yield = VdirYield;
    type Return = Result<VdirItem, VdirItemGetError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        match (&mut self.state, arg) {
            (State::Locate(c), arg) => {
                let out = vdir_try!(c, arg);
                let paths = BTreeSet::from_iter([out.path.clone()]);
                self.state = State::Stat {
                    path: out.path,
                    kind: out.kind,
                };
                VdirCoroutineState::Yielded(VdirYield::WantsFileMeta(paths))
            }
            (State::Stat { path, kind }, Some(VdirReply::FileMeta(metas))) => {
                let path = mem::take(path);
                let kind = *kind;

                let Some(meta) = metas.get(&path).copied() else {
                    let err = VdirItemGetError::Unreadable(path);
                    return VdirCoroutineState::Complete(Err(err));
                };

                let paths = BTreeSet::from_iter([path.clone()]);
                self.state = State::Read { path, kind, meta };
                VdirCoroutineState::Yielded(VdirYield::WantsFileRead(paths))
            }
            (State::Read { path, kind, meta }, Some(VdirReply::FileRead(mut map))) => {
                let path = mem::take(path);

                let Some(contents) = map.remove(&path) else {
                    let err = VdirItemGetError::Unreadable(path);
                    return VdirCoroutineState::Complete(Err(err));
                };

                VdirCoroutineState::Complete(Ok(VdirItem {
                    path,
                    kind: *kind,
                    contents,
                    meta: *meta,
                }))
            }
            (_, arg) => {
                let err = VdirItemGetError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Locate(VdirItemLocate),
    Stat {
        path: VdirPath,
        kind: VdirItemKind,
    },
    Read {
        path: VdirPath,
        kind: VdirItemKind,
        meta: VdirFileMeta,
    },
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locate(_) => f.write_str("locate item"),
            Self::Stat { .. } => f.write_str("stat item"),
            Self::Read { .. } => f.write_str("read item"),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use super::*;

    #[test]
    fn locates_then_reads_contents() {
        let mut cor = VdirItemGet::new("root/contacts", "alice", VdirItemGetOptions::default());

        match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileExists(_)) => {}
            state => panic!("expected WantsFileExists, got {state:?}"),
        }

        let vcf = VdirPath::from("root/contacts/alice.vcf");
        let mut exists = BTreeMap::new();
        exists.insert(vcf.clone(), true);
        exists.insert(VdirPath::from("root/contacts/alice.ics"), false);

        let paths = match cor.resume(Some(VdirReply::FileExists(exists))) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileMeta(paths)) => paths,
            state => panic!("expected WantsFileMeta, got {state:?}"),
        };
        assert!(paths.contains(&vcf));

        let meta = VdirFileMeta {
            size: 11,
            ..Default::default()
        };
        let metas = BTreeMap::from_iter([(vcf.clone(), meta)]);
        let paths = match cor.resume(Some(VdirReply::FileMeta(metas))) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileRead(paths)) => paths,
            state => panic!("expected WantsFileRead, got {state:?}"),
        };
        assert!(paths.contains(&vcf));

        let mut contents = BTreeMap::new();
        contents.insert(vcf.clone(), b"BEGIN:VCARD".to_vec());
        let item = match cor.resume(Some(VdirReply::FileRead(contents))) {
            VdirCoroutineState::Complete(Ok(item)) => item,
            state => panic!("expected Complete(Ok), got {state:?}"),
        };
        assert_eq!(item.path, vcf);
        assert_eq!(item.kind, VdirItemKind::Vcard);
        assert_eq!(item.contents, b"BEGIN:VCARD");
        assert_eq!(item.meta, meta);
    }

    #[test]
    fn unread_file_returns_error() {
        let mut cor = VdirItemGet::new("root/contacts", "alice", VdirItemGetOptions::default());
        let _ = cor.resume(None);

        let vcf = VdirPath::from("root/contacts/alice.vcf");
        let exists = BTreeMap::from_iter([(vcf.clone(), true)]);
        let _ = cor.resume(Some(VdirReply::FileExists(exists)));

        let metas = BTreeMap::from_iter([(vcf.clone(), VdirFileMeta::default())]);
        let _ = cor.resume(Some(VdirReply::FileMeta(metas)));

        let err = match cor.resume(Some(VdirReply::FileRead(BTreeMap::new()))) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirItemGetError::Unreadable(path) if path == vcf));
    }

    #[test]
    fn locate_error_is_forwarded() {
        let mut cor = VdirItemGet::new("root/contacts", "alice", VdirItemGetOptions::default());
        let _ = cor.resume(None);

        let err = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirItemGetError::Locate(_)));
    }
}
