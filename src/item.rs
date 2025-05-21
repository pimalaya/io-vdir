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
/// Represents either a vCard (.vcf) or a iCalendar file (.ics).
///
/// See [`crate::Collection`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub path: PathBuf,

    /// The item kind.
    pub kind: ItemKind,
}

impl Item {
    pub fn new(collection: &Collection, kind: ItemKind) -> Item {
        let path = collection
            .path
            .join(Uuid::new_v4().to_string())
            .with_extension(kind.extension());

        Item { path, kind }
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ItemKind {
    Ical(ICalendar),
    Vcard(VCard),
}

impl ItemKind {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Ical(_) => ICS,
            Self::Vcard(_) => VCF,
        }
    }
}
