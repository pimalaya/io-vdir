# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- `VdirCollectionUpdate` writes the collection it is handed rather than only its non-empty fields: a present value is written as before, and an absent or empty one has its metadata file removed. The coroutine takes a whole `VdirCollection`, a desired state rather than a patch, so this is what its signature already promised; before it, clearing a description was a silent no-op that reported success and left the file on disk.

  **Breaking**: a caller passing a partially-filled collection used to keep the fields it left unset and now clears them. Read the collection, apply your change to it, and pass the result.

- The std client's file removal is idempotent: removing a path that does not exist is a success, since a coroutine asking for a file to be gone is satisfied by its absence.

## [0.1.0] - 2026-07-16

### Changed

- Refactored the crate to a no_std core plus an opt-in std client, dropping the io-fs dependency and adopting the `Wants<Action>` coroutine convention shared with io-maildir and io-m2dir.

  Coroutines emit hoisted filesystem requests (WantsDirCreate, WantsDirRemove, WantsDirExists, WantsDirRead, WantsFileExists, WantsFileRead, WantsFileCreate, WantsRename, WantsCopy, WantsFileRemove, WantsRandom). Paths flow through a new `VdirPath(String)` newtype and collections / items use `BTreeMap` / `BTreeSet`.

- Replaced the legacy free-function coroutines with a `VdirClient` std-blocking client gated behind the `client` feature.

- Aligned the coroutine layer around a shared `VdirCoroutine` trait, a `VdirCoroutineState` (`Yielded` / `Complete`), a single `VdirYield` request enum, a `VdirReply` reply enum and a `vdir_try!` macro under a new `coroutine` module. Every coroutine implements `VdirCoroutine` and is driven by `VdirClient::run`.

- Prefixed every public type with the crate scope, following the `<Domain><Target><Verb><Ext>` naming convention. Coroutines and their companions gained it (`ItemCopy` is now `VdirItemCopy`, `CollectionCreateError` is now `VdirCollectionCreateError`), and so did the data handles: `Collection` is now `VdirCollection`, `Item` is now `VdirItem` and `ItemKind` is now `VdirItemKind`.

- Gave every coroutine a dedicated `Options` struct threaded through `new(.., opts)`. The structs (`VdirItemStoreOptions`, `VdirCollectionListOptions`, etc.) are empty placeholders today, reserved for future per-coroutine options without breaking the constructor signature.

- Moved the shared collection and item handles out of the private `types` submodules into their parent modules, reached at `collection::VdirCollection` and `item::VdirItem`; the marker-file and extension constants became crate-private.

- Realigned every documentation and metadata file with the Pimalaya guidelines: the README dropped all code and now redirects to docs.rs, the lib.rs header replaced the README include as the crate architecture document, and the CONTRIBUTING guide keeps only the repository-specific feature matrix.

### Removed

- Removed the `parser` feature, the calcard dependency and the `VdirItem::as_vcard` / `as_ical` helpers. io-vdir no longer decodes item bytes; consumers run the vCard or iCalendar parser of their choice (for example vcard-rs and ical-rs) on `VdirItem::contents`.

- Removed the dependency on io-fs and the standalone `constants` module; per-domain constants now live next to the types that consume them.

- Removed the git-cliff configuration; the changelog is now maintained by hand.

## [0.0.2] - 2025-10-27

### Added

- Add missing more projects

### Removed

- Remove wrong list items filter

## [0.0.1] - 2025-07-31

### Added

- Init coroutines
- Add audit with cargo deny, set dual license
- Clean readme, contributing and security

### Changed

- Make use of thiserror, simplify imports
- Improve inline documentation
- Release v0.0.1

[0.1.0]: https://github.com/pimalaya/io-vdir/compare/v0.0.3..v0.1.0
[0.0.2]: https://github.com/pimalaya/io-vdir/compare/v0.0.1..v0.0.2
[0.0.1]: https://github.com/pimalaya/io-vdir/compare/root..v0.0.1
