//! Vdir items: the [`VdirItem`] handle, its [`VdirItemKind`] and the
//! [`VdirItemEntry`] a listing names it by, plus the I/O-free
//! coroutines for the item lifecycle (store, get, list, read, locate,
//! copy, move, delete).
//!
//! An item is a single vCard ([RFC 6350]) or iCalendar ([RFC 5545])
//! file, as laid out by the [vdir specification]. The coroutine
//! submodules own one operation each; this module owns the shared
//! handle and the file extensions they key off.
//!
//! [RFC 6350]: https://www.rfc-editor.org/rfc/rfc6350
//! [RFC 5545]: https://www.rfc-editor.org/rfc/rfc5545
//! [vdir specification]: https://vdirsyncer.pimutils.org/en/stable/vdir.html

use core::hash::{Hash, Hasher};

use alloc::{string::String, vec::Vec};

use crate::{
    coroutine::VdirFileMeta,
    path::{VdirPath, is_stem},
};

pub mod copy;
pub mod delete;
pub mod get;
pub mod list;
pub mod locate;
pub mod r#move;
pub mod read;
pub mod store;

/// File extension of vCard items.
pub(crate) const VCF: &str = "vcf";

/// File extension of iCalendar items.
pub(crate) const ICS: &str = "ics";

/// Temporary file extension used while atomically replacing an item
/// or a metadata file.
pub(crate) const TMP: &str = "tmp";

/// Number of random bytes an item's temporary file name is built from.
pub(crate) const TMP_LEN: usize = 8;

/// Kind of a Vdir item.
///
/// Either an iCalendar component (`.ics`) or a vCard (`.vcf`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum VdirItemKind {
    /// iCalendar variant: event, task, alarm, or any valid iCalendar
    /// component.
    Ical,
    /// vCard variant: a contact.
    Vcard,
}

impl VdirItemKind {
    /// Returns the file extension associated with the item kind.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Ical => ICS,
            Self::Vcard => VCF,
        }
    }

    /// Parses an extension string into a [`VdirItemKind`].
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            ICS => Some(Self::Ical),
            VCF => Some(Self::Vcard),
            _ => None,
        }
    }
}

/// A Vdir collection's item.
///
/// Carries the on-disk path, the parsed kind (from the file
/// extension) and the raw file bytes. The bytes stay raw: io-vdir
/// never decodes them, leaving the choice of vCard or iCalendar
/// parser to the caller.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VdirItem {
    /// On-disk path of the item file.
    pub path: VdirPath,
    /// Kind of the item, derived from the file extension.
    pub kind: VdirItemKind,
    /// Raw file bytes.
    pub contents: Vec<u8>,
    /// Stat of the item file, taken before its bytes were read.
    pub meta: VdirFileMeta,
}

impl VdirItem {
    /// Returns the item id: the file stem (final path component
    /// without the extension), or the whole file name when no
    /// extension is present.
    pub fn id(&self) -> Option<&str> {
        let name = self.path.file_name()?;
        Some(match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem,
            _ => name,
        })
    }

    /// Returns the item bytes.
    pub fn contents(&self) -> &[u8] {
        &self.contents
    }
}

impl Hash for VdirItem {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

impl AsRef<VdirPath> for VdirItem {
    fn as_ref(&self) -> &VdirPath {
        &self.path
    }
}

/// A Vdir item as a listing names it, without its bytes.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VdirItemEntry {
    /// Id of the item: its file stem.
    pub id: String,
    /// Kind of the item, derived from the file extension.
    pub kind: VdirItemKind,
    /// On-disk path of the item file.
    pub path: VdirPath,
    /// Stat of the item file when it was listed.
    pub meta: VdirFileMeta,
}

/// Builds the `(tmp, final)` paths for item `id` of `kind` under
/// `collection`. Bytes land on the first, named after `seed`, and are
/// renamed onto the second, so the item is only ever enumerated
/// whole. The caller checks `id` with [`is_stem`] first.
pub(crate) fn build_paths(
    collection: &VdirPath,
    id: &str,
    kind: VdirItemKind,
    seed: [u8; TMP_LEN],
) -> (VdirPath, VdirPath) {
    debug_assert!(is_stem(id));

    let ext = kind.extension();
    let final_path = collection.join(&format!("{id}.{ext}"));

    let mut tmp_name = format!("{id}.{ext}.");
    push_hex(&mut tmp_name, &seed);
    tmp_name.push('.');
    tmp_name.push_str(TMP);

    (collection.join(&tmp_name), final_path)
}

/// Appends `bytes` to `out` as lowercase hexadecimal digits.
pub(crate) fn push_hex(out: &mut String, bytes: &[u8]) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";

    for byte in bytes {
        out.push(DIGITS[usize::from(byte >> 4)] as char);
        out.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
}
