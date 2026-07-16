//! Vdir collections: the [`VdirCollection`] handle plus the I/O-free
//! coroutines managing collection directories and their metadata
//! files (create, delete, list, rename, update).
//!
//! A collection is a directory holding items; it may also carry the
//! optional metadata markers (display name, description, color)
//! defined by the [vdir specification]. The coroutine submodules own
//! one operation each; this module owns the shared handle and the
//! marker file names they read and write.
//!
//! [vdir specification]: https://vdirsyncer.pimutils.org/en/stable/vdir.html#metadata

use core::hash::{Hash, Hasher};

use alloc::string::String;

use crate::path::VdirPath;

pub mod create;
pub mod delete;
pub mod list;
pub mod rename;
pub mod update;

/// File name of the optional UTF-8 display name marker.
pub(crate) const DISPLAYNAME: &str = "displayname";

/// File name of the optional UTF-8 description marker.
pub(crate) const DESCRIPTION: &str = "description";

/// File name of the optional `#RRGGBB` color marker.
pub(crate) const COLOR: &str = "color";

/// A Vdir collection.
///
/// Represents a directory that contains only items (vCard or
/// iCalendar files). A collection may also carry the optional
/// [metadata] files defined by the vdir specification.
///
/// See [`crate::item::VdirItem`].
///
/// [metadata]: https://vdirsyncer.pimutils.org/en/stable/vdir.html#metadata
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VdirCollection {
    /// On-disk directory of the collection.
    pub path: VdirPath,
    /// Display name of the collection, when the `displayname` file
    /// exists and is non-empty.
    pub display_name: Option<String>,
    /// Description of the collection, when the `description` file
    /// exists and is non-empty.
    pub description: Option<String>,
    /// ASCII `#RRGGBB` hex color of the collection, when the `color`
    /// file exists and is non-empty.
    pub color: Option<String>,
}

impl VdirCollection {
    /// Wraps `path` as a bare collection with no metadata. Performs
    /// no filesystem check.
    pub fn from_path(path: impl Into<VdirPath>) -> Self {
        Self {
            path: path.into(),
            display_name: None,
            description: None,
            color: None,
        }
    }

    /// Returns the collection id: the final path component, or an
    /// empty string when the path has no file name.
    pub fn id(&self) -> &str {
        self.path.file_name().unwrap_or("")
    }
}

impl Hash for VdirCollection {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

impl AsRef<VdirPath> for VdirCollection {
    fn as_ref(&self) -> &VdirPath {
        &self.path
    }
}

impl From<VdirPath> for VdirCollection {
    fn from(path: VdirPath) -> Self {
        Self::from_path(path)
    }
}
