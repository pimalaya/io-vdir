//! Vdir collection's item: a single vCard (`.vcf`, [RFC 6350]) or
//! iCalendar (`.ics`, [RFC 5545]) file inside a collection.
//!
//! [RFC 6350]: https://www.rfc-editor.org/rfc/rfc6350
//! [RFC 5545]: https://www.rfc-editor.org/rfc/rfc5545

use core::hash::{Hash, Hasher};

use alloc::vec::Vec;

use crate::path::VdirPath;

/// File extension of vCard items.
pub const VCF: &str = "vcf";

/// File extension of iCalendar items.
pub const ICS: &str = "ics";

/// Temporary file extension used while atomically replacing an item
/// or a metadata file.
pub const TMP: &str = "tmp";

/// Kind of a Vdir item.
///
/// Either an iCalendar component (`.ics`) or a vCard (`.vcf`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ItemKind {
    /// iCalendar variant: event, task, alarm, or any valid iCalendar
    /// component.
    Ical,

    /// vCard variant: a contact.
    Vcard,
}

impl ItemKind {
    /// Returns the file extension associated with the item kind.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Ical => ICS,
            Self::Vcard => VCF,
        }
    }

    /// Parses an extension string into an [`ItemKind`].
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
pub struct Item {
    /// On-disk path of the item file.
    pub path: VdirPath,

    /// Kind of the item, derived from the file extension.
    pub kind: ItemKind,

    /// Raw file bytes.
    pub contents: Vec<u8>,
}

impl Item {
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
    /// [`ItemKind::Ical`].
    #[cfg(feature = "parser")]
    pub fn as_ical(&self) -> Option<calcard::icalendar::ICalendar> {
        if !matches!(self.kind, ItemKind::Ical) {
            return None;
        }
        let s = core::str::from_utf8(&self.contents).ok()?;
        calcard::icalendar::ICalendar::parse(s).ok()
    }

    /// Parses the bytes as a vCard, when the item kind is
    /// [`ItemKind::Vcard`].
    #[cfg(feature = "parser")]
    pub fn as_vcard(&self) -> Option<calcard::vcard::VCard> {
        if !matches!(self.kind, ItemKind::Vcard) {
            return None;
        }
        let s = core::str::from_utf8(&self.contents).ok()?;
        calcard::vcard::VCard::parse(s).ok()
    }
}

impl Hash for Item {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

impl AsRef<VdirPath> for Item {
    fn as_ref(&self) -> &VdirPath {
        &self.path
    }
}

impl From<(VdirPath, ItemKind, Vec<u8>)> for Item {
    fn from((path, kind, contents): (VdirPath, ItemKind, Vec<u8>)) -> Self {
        Self {
            path,
            kind,
            contents,
        }
    }
}
