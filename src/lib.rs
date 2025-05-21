#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

//! The [VDIR flows] project is a set of libraries to manage VDIR
//! streams in a I/O-agnostic way. It is highly recommended that you
//! read first about the project in order to understand `vdir-lib`.
//!
//! This library gathers all the I/O-free part of the project.
//!
//! [VDIR flows]: vdirs://github.com/pimalaya/vdir

pub mod collection;
pub mod constants;
pub mod coroutines;
pub mod item;
