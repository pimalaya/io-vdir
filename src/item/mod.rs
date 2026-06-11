//! Vdir items: I/O-free coroutines for the item lifecycle (store,
//! get, list, locate, copy, move, delete), plus the [`Item`] handle
//! and [`ItemKind`].
//!
//! An item is a single vCard ([RFC 6350]) or iCalendar ([RFC 5545])
//! file, as laid out by the [vdir specification].
//!
//! [RFC 6350]: https://www.rfc-editor.org/rfc/rfc6350
//! [RFC 5545]: https://www.rfc-editor.org/rfc/rfc5545
//! [vdir specification]: https://vdirsyncer.pimutils.org/en/stable/vdir.html

pub mod copy;
pub mod delete;
pub mod get;
pub mod list;
pub mod locate;
pub mod r#move;
pub mod store;
mod types;

#[doc(inline)]
pub use types::*;
