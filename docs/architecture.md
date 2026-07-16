# Architecture

The crate is an I/O-free implementation of the [Vdir storage format](https://vdirsyncer.pimutils.org/en/stable/vdir.html): a directory tree where each collection is a directory and each item is a vCard or iCalendar file inside it. The logic never touches the filesystem itself; it computes the operations to perform and lets a caller run them.

## Coroutine core

Every operation is a state machine implementing the `coroutine::VdirCoroutine` trait. Its `resume` method takes an optional `VdirReply` and returns a `VdirCoroutineState`: either `Yielded`, carrying a `VdirYield` request the caller must service (create a directory, read a file, rename a path, draw random bytes), or `Complete`, carrying the terminal `Result`. The caller loops, servicing each yield and feeding the matching reply back, until the coroutine completes.

`VdirYield` and `VdirReply` are a single shared pair covering every filesystem primitive the crate emits, so one driver loop can run any coroutine. The `vdir_try!` macro chains one coroutine inside another: the item copy, move, get and delete coroutines all embed the locate coroutine this way, forwarding its yields and short-circuiting on its errors.

The core is no_std and allocation-only: paths and buffers use `alloc` collections, batched requests use `BTreeSet` and `BTreeMap` for deterministic ordering, and no runtime is pulled in. This lets the same coroutines drive a blocking client, an async caller or an in-memory test harness without change.

## Standard client

The `client` feature adds `client::VdirClient`, a blocking client that owns a filesystem root and services every yield through `std::fs`. `VdirClient::run` is the generic driver loop; the per-operation methods are thin wrappers that construct a coroutine and hand it to `run`. New item ids are drawn from the system entropy source and formatted as UUIDv4 strings.

## On-disk layout

Paths are a forward-slash `path::VdirPath` newtype used on every platform; `std::fs` accepts forward-slash paths on both Unix and Windows, so no boundary conversion is needed. Collections carry optional metadata in marker files (display name, description, color). Items are written atomically: the bytes go to a temporary sibling file first, then a rename swaps them onto the final name, so a reader never observes a half-written file. The same store coroutine handles both create and update.

## Encoding

Items are opaque bytes at the coroutine level. The `parser` feature decodes them into [calcard](https://docs.rs/calcard) vCard or iCalendar values on demand, and the `serde` feature derives (de)serialization on the public handles. Both are optional so the core stays dependency-light.
