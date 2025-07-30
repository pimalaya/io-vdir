//! Module dedicated to the Vdir collection.

use std::{
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use uuid::Uuid;

/// The Vdir collection.
///
/// Represents a directory that contains only files (items). A
/// collection may have [metadata], as defined in the vdirsyncer
/// standard.
///
/// See [`crate::item::Item`].
///
/// [metadata]: https://vdirsyncer.pimutils.org/en/stable/vdir.html#metadata
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Collection {
    /// The directory path of the Vdir collection.
    pub path: PathBuf,

    /// The name of the collection.
    ///
    /// Files called displayname contain a UTF-8 encoded label, that
    /// may be used to represent the vdir in UIs.
    pub display_name: Option<String>,

    /// The description of the collection.
    ///
    /// Files called description contain a UTF-8 encoded description,
    /// that may be used to represent the vdir in UIs.
    pub description: Option<String>,

    /// The color of the collection.
    ///
    /// A file called color inside the vdir indicates the vdir’s
    /// color, a property that is only relevant in UI design.
    ///
    /// Its content is an ASCII-encoded hex-RGB value of the form
    /// #RRGGBB. For example, a file content of #FF0000 indicates that
    /// the vdir has a red (user-visible) color. No short forms or
    /// informal values such as red (as known from CSS, for example)
    /// are allowed. The prefixing # must be present.
    pub color: Option<String>,
}

impl Collection {
    /// Creates a new collection for the given root directory path.
    ///
    /// This does not create the filesystem directory, it just creates
    /// an empty collection with an auto-generated UUID.
    pub fn new(root: impl AsRef<Path>) -> Collection {
        Collection {
            path: root.as_ref().join(Uuid::new_v4().to_string()),
            display_name: None,
            description: None,
            color: None,
        }
    }
}

impl Hash for Collection {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

impl AsRef<Path> for Collection {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}
