#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

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
