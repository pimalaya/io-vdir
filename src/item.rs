//! Vdir items: the [`VdirItem`] handle and its [`VdirItemKind`], plus
//! the I/O-free coroutines for the item lifecycle (store, get, list,
//! locate, copy, move, delete).
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
#[cfg(feature = "parser")]
use core::str::from_utf8;

use alloc::vec::Vec;

#[cfg(feature = "parser")]
use calcard::{icalendar::ICalendar, vcard::VCard};

use crate::path::VdirPath;

pub mod copy;
pub mod delete;
pub mod get;
pub mod list;
pub mod locate;
pub mod r#move;
pub mod store;

/// File extension of vCard items.
pub(crate) const VCF: &str = "vcf";

/// File extension of iCalendar items.
pub(crate) const ICS: &str = "ics";

/// Temporary file extension used while atomically replacing an item
/// or a metadata file.
pub(crate) const TMP: &str = "tmp";

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
/// extension) and the raw file bytes. Bytes stay un-decoded at this
/// level; the optional `parser` feature exposes calcard-backed
/// helpers on top.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VdirItem {
    /// On-disk path of the item file.
    pub path: VdirPath,
    /// Kind of the item, derived from the file extension.
    pub kind: VdirItemKind,
    /// Raw file bytes.
    pub contents: Vec<u8>,
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

    /// Parses the bytes as an iCalendar, when the item kind is
    /// [`VdirItemKind::Ical`].
    #[cfg(feature = "parser")]
    pub fn as_ical(&self) -> Option<ICalendar> {
        if !matches!(self.kind, VdirItemKind::Ical) {
            return None;
        }
        let contents = from_utf8(&self.contents).ok()?;
        ICalendar::parse(contents).ok()
    }

    /// Parses the bytes as a vCard, when the item kind is
    /// [`VdirItemKind::Vcard`].
    #[cfg(feature = "parser")]
    pub fn as_vcard(&self) -> Option<VCard> {
        if !matches!(self.kind, VdirItemKind::Vcard) {
            return None;
        }
        let contents = from_utf8(&self.contents).ok()?;
        VCard::parse(contents).ok()
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

impl From<(VdirPath, VdirItemKind, Vec<u8>)> for VdirItem {
    fn from((path, kind, contents): (VdirPath, VdirItemKind, Vec<u8>)) -> Self {
        Self {
            path,
            kind,
            contents,
        }
    }
}
