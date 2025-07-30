//! Module dedicated to the Vdir collection's item.

use std::{
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use calcard::{icalendar::ICalendar, vcard::VCard};
use uuid::Uuid;

use crate::{
    collection::Collection,
    constants::{ICS, VCF},
};

/// The Vdir collection's item.
///
/// An item can be either a vCard (.vcf) or a iCalendar file (.ics).
///
/// See [`crate::collection::Collection`] and [`ItemKind`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    /// The file path of the collection's item.
    pub path: PathBuf,

    /// The collection's item kind.
    pub kind: ItemKind,
}

impl Item {
    /// Creates a new collection's item for the given collection and
    /// the given kind.
    ///
    /// This does not create the filesystem file, it just creates an
    /// empty collection's item with an auto-generated UUID.
    pub fn new(collection: &Collection, kind: ItemKind) -> Item {
        let path = collection
            .path
            .join(Uuid::new_v4().to_string())
            .with_extension(kind.extension());

        Self { path, kind }
    }
}

impl Hash for Item {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

impl AsRef<Path> for Item {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl ToString for Item {
    fn to_string(&self) -> String {
        match &self.kind {
            ItemKind::Ical(ical) => ical.to_string(),
            ItemKind::Vcard(vcard) => vcard.to_string(),
        }
    }
}

/// The Vdir collection's item's kind.
///
/// Represents either an iCalendar file (.ics) or a vCard (.vcf).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ItemKind {
    /// The iCalendar item variant.
    ///
    /// Represents an event, a task, an alarm, or any valid iCalendar
    /// component.
    Ical(ICalendar),

    /// The vCard item variant.
    ///
    /// Represents a contact.
    Vcard(VCard),
}

impl ItemKind {
    /// Returns the file extension associated to the item's kind.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Ical(_) => ICS,
            Self::Vcard(_) => VCF,
        }
    }
}
