//! Module dedicated to Vdir constants.

/// The display name of the collection.
///
/// Represents the name of the file containing the display name of the
/// collection (metadata).
pub const DISPLAYNAME: &'static str = "displayname";

/// The description of the collection.
///
/// Represents the name of the file containing the description of the
/// collection (metadata).
pub const DESCRIPTION: &'static str = "description";

/// The color of the collection.
///
/// Represents the name of the file containing the color of the
/// collection (metadata).
pub const COLOR: &'static str = "color";

/// The temporary file extension, used to move iCalendars or vCards.
pub const TMP: &'static str = "tmp";

/// The VCF file extension, used by vCard files.
pub const VCF: &'static str = "vcf";

/// The ICS file extension, used by iCalendar files.
pub const ICS: &'static str = "ics";
