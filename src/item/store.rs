//! I/O-free coroutine writing a Vdir item under a collection.
//!
//! When `id` is `None`, the coroutine requests 16 random bytes via
//! [`VdirYield::WantsRandom`] and formats them as a UUIDv4 string. The
//! item is written to `<collection>/<id>.<ext>.tmp` first, then
//! atomically renamed onto `<collection>/<id>.<ext>`; any existing
//! file at the final path is replaced. Use the same coroutine for
//! create and update.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::fs;
//!
//! use io_vdir::{coroutine::*, item::{store::VdirItemStore, types::ItemKind}};
//!
//! let bytes = b"BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Alice\r\nEND:VCARD\r\n".to_vec();
//! let mut coroutine = VdirItemStore::new("/tmp/vdir/contacts", None, ItemKind::Vcard, bytes);
//! let mut arg = None;
//!
//! let out = loop {
//!     match coroutine.resume(arg.take()) {
//!         VdirCoroutineState::Yielded(VdirYield::WantsRandom { len }) => {
//!             let bytes = vec![0u8; len]; // fill via the OS RNG in real code
//!             arg = Some(VdirReply::Random(bytes));
//!         }
//!         VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files)) => {
//!             for (path, bytes) in files {
//!                 fs::write(path.as_str(), bytes).unwrap();
//!             }
//!             arg = Some(VdirReply::FileCreate);
//!         }
//!         VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs)) => {
//!             for (from, to) in pairs {
//!                 fs::rename(from.as_str(), to.as_str()).unwrap();
//!             }
//!             arg = Some(VdirReply::Rename);
//!         }
//!         VdirCoroutineState::Complete(Ok(out)) => break out,
//!         VdirCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         state => panic!("unexpected state {state:?}"),
//!     }
//! };
//!
//! println!("stored {} at {}", out.id, out.path);
//! ```

use core::{fmt, mem};

use alloc::{collections::BTreeMap, string::String, vec::Vec};

use log::trace;
use thiserror::Error;

use crate::{
    coroutine::*,
    item::types::{ItemKind, TMP},
    path::VdirPath,
};

/// Number of random bytes a fresh UUIDv4 id is built from.
const UUID_LEN: usize = 16;

/// Failure causes during a [`VdirItemStore`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirItemStoreError {
    #[error("Vdir item store failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),
}

/// Successful output of [`VdirItemStore`].
#[derive(Clone, Debug)]
pub struct VdirItemStoreOutput {
    pub id: String,
    pub path: VdirPath,
}

/// Writes a Vdir item under a collection, minting a UUIDv4 id when
/// none is supplied.
#[derive(Debug)]
pub struct VdirItemStore {
    state: State,
}

impl VdirItemStore {
    /// Creates a new coroutine that will write `contents` as an item
    /// of `kind` under `collection`.
    ///
    /// When `id` is `Some`, the coroutine reuses it and skips the
    /// [`VdirYield::WantsRandom`] step.
    pub fn new(
        collection: impl Into<VdirPath>,
        id: Option<String>,
        kind: ItemKind,
        contents: Vec<u8>,
    ) -> Self {
        Self {
            state: State::Start {
                collection: collection.into(),
                id,
                kind,
                contents,
            },
        }
    }
}

impl VdirCoroutine for VdirItemStore {
    type Yield = VdirYield;
    type Return = Result<VdirItemStoreOutput, VdirItemStoreError>;

    fn resume(&mut self, arg: Option<VdirReply>) -> VdirCoroutineState<Self::Yield, Self::Return> {
        trace!("item store: {}", self.state);

        match (&mut self.state, arg) {
            (
                State::Start {
                    collection,
                    id,
                    kind,
                    contents,
                },
                None,
            ) => {
                let collection = mem::take(collection);
                let kind = *kind;
                let contents = mem::take(contents);

                match id.take() {
                    Some(id) => {
                        let (tmp_path, final_path) = build_paths(&collection, &id, kind);
                        let files = BTreeMap::from_iter([(tmp_path.clone(), contents)]);
                        self.state = State::AwaitFileCreate {
                            id,
                            tmp_path,
                            final_path,
                        };
                        VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files))
                    }
                    None => {
                        self.state = State::AwaitRandom {
                            collection,
                            kind,
                            contents,
                        };
                        VdirCoroutineState::Yielded(VdirYield::WantsRandom { len: UUID_LEN })
                    }
                }
            }
            (
                State::AwaitRandom {
                    collection,
                    kind,
                    contents,
                },
                Some(VdirReply::Random(bytes)),
            ) => {
                let collection = mem::take(collection);
                let kind = *kind;
                let contents = mem::take(contents);

                let id = uuid_v4(&bytes);
                let (tmp_path, final_path) = build_paths(&collection, &id, kind);
                let files = BTreeMap::from_iter([(tmp_path.clone(), contents)]);
                self.state = State::AwaitFileCreate {
                    id,
                    tmp_path,
                    final_path,
                };
                VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files))
            }
            (
                State::AwaitFileCreate {
                    id,
                    tmp_path,
                    final_path,
                },
                Some(VdirReply::FileCreate),
            ) => {
                let id = mem::take(id);
                let tmp_path = mem::take(tmp_path);
                let final_path = mem::take(final_path);

                let pairs = vec![(tmp_path, final_path.clone())];
                self.state = State::AwaitRename { id, final_path };
                VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs))
            }
            (State::AwaitRename { id, final_path }, Some(VdirReply::Rename)) => {
                let out = VdirItemStoreOutput {
                    id: mem::take(id),
                    path: mem::take(final_path),
                };
                VdirCoroutineState::Complete(Ok(out))
            }
            (_, arg) => {
                let err = VdirItemStoreError::UnexpectedArg(arg);
                VdirCoroutineState::Complete(Err(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    Start {
        collection: VdirPath,
        id: Option<String>,
        kind: ItemKind,
        contents: Vec<u8>,
    },
    AwaitRandom {
        collection: VdirPath,
        kind: ItemKind,
        contents: Vec<u8>,
    },
    AwaitFileCreate {
        id: String,
        tmp_path: VdirPath,
        final_path: VdirPath,
    },
    AwaitRename {
        id: String,
        final_path: VdirPath,
    },
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start { .. } => f.write_str("start"),
            Self::AwaitRandom { .. } => f.write_str("await random reply"),
            Self::AwaitFileCreate { .. } => f.write_str("await file create reply"),
            Self::AwaitRename { .. } => f.write_str("await rename reply"),
        }
    }
}

/// Builds the `(tmp, final)` paths for item `id` of `kind` under
/// `collection`.
fn build_paths(collection: &VdirPath, id: &str, kind: ItemKind) -> (VdirPath, VdirPath) {
    let ext = kind.extension();
    let final_path = collection.join(&format!("{id}.{ext}"));
    let tmp_path = collection.join(&format!("{id}.{ext}.{TMP}"));
    (tmp_path, final_path)
}

/// Formats 16 raw bytes as a canonical UUIDv4 string, stamping the
/// RFC 4122 version (4) and variant (10x) bits.
fn uuid_v4(bytes: &[u8]) -> String {
    let mut bytes: [u8; UUID_LEN] = bytes[..UUID_LEN].try_into().unwrap_or([0u8; UUID_LEN]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    let mut id = String::with_capacity(36);
    let groups: [&[u8]; 5] = [
        &bytes[0..4],
        &bytes[4..6],
        &bytes[6..8],
        &bytes[8..10],
        &bytes[10..16],
    ];

    for (i, group) in groups.iter().enumerate() {
        if i > 0 {
            id.push('-');
        }
        for byte in *group {
            id.push(hex_nibble(byte >> 4));
            id.push(hex_nibble(byte & 0x0f));
        }
    }

    id
}

/// Maps a 4-bit nibble to its lowercase hexadecimal digit.
fn hex_nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + (n - 10)) as char,
        _ => '0',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuses_supplied_id() {
        let mut cor = VdirItemStore::new(
            "root/contacts",
            Some("alice".into()),
            ItemKind::Vcard,
            b"BEGIN:VCARD".to_vec(),
        );

        let files = match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files)) => files,
            state => panic!("expected WantsFileCreate, got {state:?}"),
        };
        let tmp = VdirPath::from("root/contacts/alice.vcf.tmp");
        assert!(files.contains_key(&tmp));

        let pairs = match cor.resume(Some(VdirReply::FileCreate)) {
            VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs)) => pairs,
            state => panic!("expected WantsRename, got {state:?}"),
        };
        assert_eq!(
            pairs,
            vec![(tmp, VdirPath::from("root/contacts/alice.vcf"))]
        );

        let out = match cor.resume(Some(VdirReply::Rename)) {
            VdirCoroutineState::Complete(Ok(out)) => out,
            state => panic!("expected Complete(Ok), got {state:?}"),
        };
        assert_eq!(out.id, "alice");
        assert_eq!(out.path, VdirPath::from("root/contacts/alice.vcf"));
    }

    #[test]
    fn generates_uuid_when_id_missing() {
        let mut cor = VdirItemStore::new(
            "root/contacts",
            None,
            ItemKind::Ical,
            b"BEGIN:VCAL".to_vec(),
        );

        match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsRandom { len }) => {
                assert_eq!(len, UUID_LEN)
            }
            state => panic!("expected WantsRandom, got {state:?}"),
        }

        let bytes = vec![0xabu8; UUID_LEN];
        let files = match cor.resume(Some(VdirReply::Random(bytes))) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files)) => files,
            state => panic!("expected WantsFileCreate, got {state:?}"),
        };

        let tmp = files.keys().next().unwrap();
        assert!(tmp.as_str().ends_with(".ics.tmp"));
        let id = tmp.file_name().unwrap();
        // Version nibble (4) and variant nibble (8..=b) embedded in the
        // canonical 8-4-4-4-12 layout.
        assert_eq!(id.chars().nth(14), Some('4'));
        assert!(matches!(id.chars().nth(19), Some('8'..='b')));
    }

    #[test]
    fn unexpected_reply_returns_error() {
        let mut cor = VdirItemStore::new(
            "root/contacts",
            Some("alice".into()),
            ItemKind::Vcard,
            b"x".to_vec(),
        );
        let _ = cor.resume(None);

        let err = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirItemStoreError::UnexpectedArg(_)));
    }
}
