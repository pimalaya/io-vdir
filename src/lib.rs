#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! # io-vdir
//!
//! I/O-free Vdir coroutines: every filesystem operation on a [vdir]
//! tree is a resumable state machine that emits requests (create a
//! directory, read a file, rename a path, draw random bytes) instead
//! of performing the I/O itself. The caller services each request and
//! resumes the coroutine, so the same logic drives blocking, async or
//! in-memory harnesses without change.
//!
//! ## Layers
//!
//! The core [`coroutine`] layer is `no_std` and pulls no runtime: it
//! defines the [`coroutine::VdirCoroutine`] trait, the shared
//! [`coroutine::VdirYield`] request and [`coroutine::VdirReply`]
//! response enums, and the `vdir_try!` macro that chains one
//! coroutine into another. The optional `client` feature adds
//! [`client::VdirClient`], a blocking client that runs any coroutine
//! against the local filesystem through `std::fs`.
//!
//! ## Layout
//!
//! The source tree mirrors the two Vdir concepts. [`collection`] owns
//! the [`collection::VdirCollection`] handle and one coroutine per
//! directory operation (create, delete, list, rename, update).
//! [`item`] owns the [`item::VdirItem`] handle, its
//! [`item::VdirItemKind`] and one coroutine per item operation
//! (store, get, list, locate, copy, move, delete). [`path`] holds the
//! forward-slash [`path::VdirPath`] newtype shared across both.
//!
//! ## Encoding
//!
//! Items are opaque bytes at the coroutine level; the optional
//! `parser` feature decodes them into [calcard] vCard or iCalendar
//! values, and the `serde` feature derives (de)serialization on the
//! public handles.
//!
//! [vdir]: https://vdirsyncer.pimutils.org/en/stable/vdir.html
//! [calcard]: https://docs.rs/calcard

#[macro_use]
extern crate alloc;
#[cfg(feature = "client")]
extern crate std;

#[cfg(feature = "client")]
pub mod client;
pub mod collection;
pub mod coroutine;
pub mod item;
pub mod path;
