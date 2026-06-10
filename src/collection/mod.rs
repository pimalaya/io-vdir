//! Vdir collections: I/O-free coroutines managing collection
//! directories and their metadata files (create, delete, list,
//! rename, update), plus the [`Collection`](types::Collection) handle
//! under [`types`].
//!
//! Collection metadata follows the [vdir specification].
//!
//! [vdir specification]: https://vdirsyncer.pimutils.org/en/stable/vdir.html

pub mod create;
pub mod delete;
pub mod list;
pub mod rename;
pub mod types;
pub mod update;
