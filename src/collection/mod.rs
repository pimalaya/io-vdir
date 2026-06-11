//! Vdir collections: I/O-free coroutines managing collection
//! directories and their metadata files (create, delete, list,
//! rename, update), plus the [`Collection`] handle.
//!
//! Collection metadata follows the [vdir specification].
//!
//! [vdir specification]: https://vdirsyncer.pimutils.org/en/stable/vdir.html

pub mod create;
pub mod delete;
pub mod list;
pub mod rename;
mod types;
pub mod update;

#[doc(inline)]
pub use types::*;
