//! I/O-free coroutine writing a Vdir item under a collection.
//!
//! The coroutine requests random bytes via [`VdirYield::WantsRandom`]:
//! 8 for the name of its temporary file, and 16 more when `id` is
//! `None`, formatted as a UUIDv4 string. The item is written to
//! `<collection>/<id>.<ext>.<random>.tmp` first, then atomically
//! renamed onto `<collection>/<id>.<ext>`. Use the same coroutine for
//! create and update.
//!
//! By default any existing file at the final path is replaced;
//! [`VdirItemStoreMode`] makes the store conditional instead.
//!
//! A supplied id that is empty, or holds a NUL or a path separator,
//! is refused before any request.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::fs;
//!
//! use io_vdir::{coroutine::*, item::{store::*, VdirItemKind}};
//!
//! let bytes = b"BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Alice\r\nEND:VCARD\r\n".to_vec();
//! let opts = VdirItemStoreOptions::default();
//! let mut coroutine = VdirItemStore::new("/tmp/vdir/contacts", None, VdirItemKind::Vcard, bytes, opts);
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

use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};

use thiserror::Error;

use crate::{
    coroutine::*,
    item::{TMP_LEN, VdirItemKind, build_paths, push_hex},
    path::{VdirPath, is_stem},
};

/// Number of random bytes a fresh UUIDv4 id is built from.
const UUID_LEN: usize = 16;

/// Failure causes during a [`VdirItemStore`] step.
#[derive(Clone, Debug, Error)]
pub enum VdirItemStoreError {
    /// The driver fed back a reply that does not match the pending
    /// request.
    #[error("Vdir item store failed: unexpected arg {0:?}")]
    UnexpectedArg(Option<VdirReply>),

    /// The supplied id cannot name an item of the collection.
    #[error("Vdir item store failed: invalid item id {0:?}")]
    InvalidId(String),

    /// An item is already stored under the id.
    #[error("Vdir item store failed: item {0} already exists")]
    Exists(String),

    /// The item changed, or is gone, since the caller read it.
    #[error("Vdir item store failed: item {0} changed since it was read")]
    Changed(String),
}

/// Successful output of [`VdirItemStore`].
#[derive(Clone, Debug)]
pub struct VdirItemStoreOutput {
    /// Id of the stored item, reused from the caller or freshly
    /// minted as a UUIDv4.
    pub id: String,
    /// Final on-disk path of the stored item file.
    pub path: VdirPath,
}

/// What a [`VdirItemStore`] does about an item already stored under
/// the id.
///
/// A condition is checked right before the rename. The format has no
/// lock, so a writer slipping in between the two is not seen.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VdirItemStoreMode {
    /// Replaces it, whatever it holds.
    #[default]
    Replace,
    /// Fails with [`VdirItemStoreError::Exists`].
    IfAbsent,
    /// Replaces it while its file still has this stat, and fails with
    /// [`VdirItemStoreError::Changed`] otherwise.
    IfUnchanged(VdirFileMeta),
}

/// Options for [`VdirItemStore::new`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VdirItemStoreOptions {
    /// What to do about an item already stored under the id.
    pub mode: VdirItemStoreMode,
}

/// Writes a Vdir item under a collection, minting a UUIDv4 id when
/// none is supplied.
#[derive(Debug)]
pub struct VdirItemStore {
    state: State,
    opts: VdirItemStoreOptions,
}

impl VdirItemStore {
    /// Creates a new coroutine that will write `contents` as an item
    /// of `kind` under `collection`.
    ///
    /// When `id` is `Some`, the coroutine reuses it instead of
    /// minting one.
    pub fn new(
        collection: impl Into<VdirPath>,
        id: Option<String>,
        kind: VdirItemKind,
        contents: Vec<u8>,
        opts: VdirItemStoreOptions,
    ) -> Self {
        Self {
            opts,
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
                let id = id.take();

                let len = match &id {
                    Some(id) if !is_stem(id) => {
                        let err = VdirItemStoreError::InvalidId(id.clone());
                        return VdirCoroutineState::Complete(Err(err));
                    }
                    Some(_) => TMP_LEN,
                    None => TMP_LEN + UUID_LEN,
                };

                self.state = State::ReadRandom {
                    collection: mem::take(collection),
                    id,
                    kind: *kind,
                    contents: mem::take(contents),
                };
                VdirCoroutineState::Yielded(VdirYield::WantsRandom { len })
            }
            (
                State::ReadRandom {
                    collection,
                    id,
                    kind,
                    contents,
                },
                Some(VdirReply::Random(bytes)),
            ) => {
                let seeds = bytes.split_first_chunk().and_then(|(tmp_seed, rest)| {
                    let id = match id.take() {
                        Some(id) => id,
                        None => uuid_v4(*rest.first_chunk()?),
                    };
                    Some((id, *tmp_seed))
                });

                let Some((id, tmp_seed)) = seeds else {
                    let arg = Some(VdirReply::Random(bytes));
                    let err = VdirItemStoreError::UnexpectedArg(arg);
                    return VdirCoroutineState::Complete(Err(err));
                };

                let (tmp_path, final_path) = build_paths(collection, &id, *kind, tmp_seed);
                let files = BTreeMap::from_iter([(tmp_path.clone(), mem::take(contents))]);
                self.state = State::CreateFile {
                    id,
                    tmp_path,
                    final_path,
                };
                VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files))
            }
            (
                State::CreateFile {
                    id,
                    tmp_path,
                    final_path,
                },
                Some(VdirReply::FileCreate),
            ) => {
                let id = mem::take(id);
                let tmp_path = mem::take(tmp_path);
                let final_path = mem::take(final_path);

                if self.opts.mode != VdirItemStoreMode::Replace {
                    let paths = BTreeSet::from_iter([final_path.clone()]);
                    self.state = State::Stat {
                        id,
                        tmp_path,
                        final_path,
                    };
                    return VdirCoroutineState::Yielded(VdirYield::WantsFileMeta(paths));
                }

                let pairs = vec![(tmp_path, final_path.clone())];
                self.state = State::Rename { id, final_path };
                VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs))
            }
            (
                State::Stat {
                    id,
                    tmp_path,
                    final_path,
                },
                Some(VdirReply::FileMeta(metas)),
            ) => {
                let id = mem::take(id);
                let tmp_path = mem::take(tmp_path);
                let final_path = mem::take(final_path);
                let found = metas.get(&final_path).copied();

                let err = match self.opts.mode {
                    VdirItemStoreMode::IfAbsent if found.is_some() => {
                        Some(VdirItemStoreError::Exists(id.clone()))
                    }
                    VdirItemStoreMode::IfUnchanged(meta) if found != Some(meta) => {
                        Some(VdirItemStoreError::Changed(id.clone()))
                    }
                    _ => None,
                };

                if let Some(err) = err {
                    let paths = BTreeSet::from_iter([tmp_path]);
                    self.state = State::RemoveTmp(err);
                    return VdirCoroutineState::Yielded(VdirYield::WantsFileRemove(paths));
                }

                let pairs = vec![(tmp_path, final_path.clone())];
                self.state = State::Rename { id, final_path };
                VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs))
            }
            (State::Rename { id, final_path }, Some(VdirReply::Rename)) => {
                let out = VdirItemStoreOutput {
                    id: mem::take(id),
                    path: mem::take(final_path),
                };
                VdirCoroutineState::Complete(Ok(out))
            }
            (State::RemoveTmp(err), Some(VdirReply::FileRemove)) => {
                VdirCoroutineState::Complete(Err(err.clone()))
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
        kind: VdirItemKind,
        contents: Vec<u8>,
    },
    ReadRandom {
        collection: VdirPath,
        id: Option<String>,
        kind: VdirItemKind,
        contents: Vec<u8>,
    },
    CreateFile {
        id: String,
        tmp_path: VdirPath,
        final_path: VdirPath,
    },
    Stat {
        id: String,
        tmp_path: VdirPath,
        final_path: VdirPath,
    },
    Rename {
        id: String,
        final_path: VdirPath,
    },
    RemoveTmp(VdirItemStoreError),
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start { .. } => f.write_str("start"),
            Self::ReadRandom { .. } => f.write_str("read random"),
            Self::CreateFile { .. } => f.write_str("write item into tmp"),
            Self::Stat { .. } => f.write_str("stat stored item"),
            Self::Rename { .. } => f.write_str("rename into place"),
            Self::RemoveTmp(_) => f.write_str("remove tmp"),
        }
    }
}

/// Formats 16 raw bytes as a canonical UUIDv4 string, stamping the
/// RFC 4122 version (4) and variant (10x) bits.
fn uuid_v4(mut bytes: [u8; UUID_LEN]) -> String {
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
        push_hex(&mut id, group);
    }

    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplied_id_is_staged_then_renamed() {
        let mut cor = store(Some("alice"), VdirItemStoreMode::Replace);

        match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsRandom { len }) => {
                assert_eq!(len, TMP_LEN)
            }
            state => panic!("expected WantsRandom, got {state:?}"),
        }

        let bytes = vec![0xabu8; TMP_LEN];
        let files = match cor.resume(Some(VdirReply::Random(bytes))) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files)) => files,
            state => panic!("expected WantsFileCreate, got {state:?}"),
        };
        let tmp = VdirPath::from("root/contacts/alice.vcf.abababababababab.tmp");
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
        let mut cor = store(None, VdirItemStoreMode::Replace);

        match cor.resume(None) {
            VdirCoroutineState::Yielded(VdirYield::WantsRandom { len }) => {
                assert_eq!(len, TMP_LEN + UUID_LEN)
            }
            state => panic!("expected WantsRandom, got {state:?}"),
        }

        let bytes = vec![0xabu8; TMP_LEN + UUID_LEN];
        let files = match cor.resume(Some(VdirReply::Random(bytes))) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files)) => files,
            state => panic!("expected WantsFileCreate, got {state:?}"),
        };

        let tmp = files.keys().next().unwrap();
        assert!(tmp.as_str().ends_with(".vcf.abababababababab.tmp"));
        let id = tmp.file_name().unwrap();
        // NOTE: version nibble (4) and variant nibble (8..=b) embedded in
        // the canonical 8-4-4-4-12 layout.
        assert_eq!(id.chars().nth(14), Some('4'));
        assert!(matches!(id.chars().nth(19), Some('8'..='b')));
    }

    #[test]
    fn concurrent_stores_stage_on_distinct_names() {
        let stage = |seed: u8| {
            let mut cor = store(Some("alice"), VdirItemStoreMode::Replace);

            let mut arg = None;
            loop {
                match cor.resume(arg.take()) {
                    VdirCoroutineState::Yielded(VdirYield::WantsRandom { len }) => {
                        arg = Some(VdirReply::Random(vec![seed; len]));
                    }
                    VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files)) => {
                        break files.into_keys().next().unwrap();
                    }
                    state => panic!("expected WantsFileCreate, got {state:?}"),
                }
            }
        };

        assert_ne!(stage(1), stage(2));
    }

    #[test]
    fn if_absent_refuses_an_existing_item() {
        let (mut cor, tmp) = staged(VdirItemStoreMode::IfAbsent);
        let found = BTreeMap::from_iter([(VdirPath::from(FINAL), VdirFileMeta::default())]);

        let paths = match cor.resume(Some(VdirReply::FileMeta(found))) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileRemove(paths)) => paths,
            state => panic!("expected WantsFileRemove, got {state:?}"),
        };
        assert_eq!(paths, BTreeSet::from_iter([tmp]));

        match cor.resume(Some(VdirReply::FileRemove)) {
            VdirCoroutineState::Complete(Err(VdirItemStoreError::Exists(id))) => {
                assert_eq!(id, "alice")
            }
            state => panic!("expected Exists, got {state:?}"),
        }
    }

    #[test]
    fn if_absent_stores_a_missing_item() {
        let (mut cor, tmp) = staged(VdirItemStoreMode::IfAbsent);

        match cor.resume(Some(VdirReply::FileMeta(BTreeMap::new()))) {
            VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs)) => {
                assert_eq!(pairs, vec![(tmp, VdirPath::from(FINAL))])
            }
            state => panic!("expected WantsRename, got {state:?}"),
        }
    }

    #[test]
    fn if_unchanged_stores_over_the_stat_it_was_given() {
        let read = VdirFileMeta {
            size: 11,
            mtime_secs: 1,
            ..Default::default()
        };
        let (mut cor, tmp) = staged(VdirItemStoreMode::IfUnchanged(read));
        let found = BTreeMap::from_iter([(VdirPath::from(FINAL), read)]);

        match cor.resume(Some(VdirReply::FileMeta(found))) {
            VdirCoroutineState::Yielded(VdirYield::WantsRename(pairs)) => {
                assert_eq!(pairs, vec![(tmp, VdirPath::from(FINAL))])
            }
            state => panic!("expected WantsRename, got {state:?}"),
        }
    }

    #[test]
    fn if_unchanged_refuses_a_changed_or_missing_item() {
        let read = VdirFileMeta {
            size: 11,
            mtime_secs: 1,
            ..Default::default()
        };
        let rewritten = VdirFileMeta {
            mtime_secs: 2,
            ..read
        };

        for found in [Some(rewritten), None] {
            let (mut cor, tmp) = staged(VdirItemStoreMode::IfUnchanged(read));
            let metas = found.map(|meta| (VdirPath::from(FINAL), meta));

            let paths = match cor.resume(Some(VdirReply::FileMeta(metas.into_iter().collect()))) {
                VdirCoroutineState::Yielded(VdirYield::WantsFileRemove(paths)) => paths,
                state => panic!("expected WantsFileRemove, got {state:?}"),
            };
            assert_eq!(paths, BTreeSet::from_iter([tmp]));

            match cor.resume(Some(VdirReply::FileRemove)) {
                VdirCoroutineState::Complete(Err(VdirItemStoreError::Changed(_))) => {}
                state => panic!("expected Changed, got {state:?}"),
            }
        }
    }

    #[test]
    fn unexpected_reply_returns_error() {
        let mut cor = store(Some("alice"), VdirItemStoreMode::Replace);
        let _ = cor.resume(None);

        let err = match cor.resume(Some(VdirReply::DirCreate)) {
            VdirCoroutineState::Complete(Err(err)) => err,
            state => panic!("expected Complete(Err), got {state:?}"),
        };
        assert!(matches!(err, VdirItemStoreError::UnexpectedArg(_)));
    }

    #[test]
    fn short_random_reply_returns_error() {
        for (id, len) in [(None, TMP_LEN + UUID_LEN - 1), (Some("alice"), TMP_LEN - 1)] {
            let mut cor = store(id, VdirItemStoreMode::Replace);
            let _ = cor.resume(None);

            let bytes = vec![0xabu8; len];
            let err = match cor.resume(Some(VdirReply::Random(bytes))) {
                VdirCoroutineState::Complete(Err(err)) => err,
                state => panic!("expected Complete(Err), got {state:?}"),
            };
            assert!(matches!(err, VdirItemStoreError::UnexpectedArg(_)));
        }
    }

    #[test]
    fn invalid_id_returns_error() {
        for id in ["", "../work/alice", "a/b", "a\0b"] {
            let mut cor = store(Some(id), VdirItemStoreMode::Replace);

            match cor.resume(None) {
                VdirCoroutineState::Complete(Err(VdirItemStoreError::InvalidId(_))) => {}
                state => panic!("expected InvalidId for {id:?}, got {state:?}"),
            }
        }
    }

    const FINAL: &str = "root/contacts/alice.vcf";

    fn store(id: Option<&str>, mode: VdirItemStoreMode) -> VdirItemStore {
        VdirItemStore::new(
            "root/contacts",
            id.map(String::from),
            VdirItemKind::Vcard,
            b"BEGIN:VCARD".to_vec(),
            VdirItemStoreOptions { mode },
        )
    }

    /// Runs a store of `alice` up to its stat of the stored item.
    fn staged(mode: VdirItemStoreMode) -> (VdirItemStore, VdirPath) {
        let mut cor = store(Some("alice"), mode);
        let _ = cor.resume(None);

        let bytes = vec![0xabu8; TMP_LEN];
        let files = match cor.resume(Some(VdirReply::Random(bytes))) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileCreate(files)) => files,
            state => panic!("expected WantsFileCreate, got {state:?}"),
        };
        let tmp = files.into_keys().next().unwrap();

        match cor.resume(Some(VdirReply::FileCreate)) {
            VdirCoroutineState::Yielded(VdirYield::WantsFileMeta(paths)) => {
                assert_eq!(paths, BTreeSet::from_iter([VdirPath::from(FINAL)]))
            }
            state => panic!("expected WantsFileMeta, got {state:?}"),
        }

        (cor, tmp)
    }
}
