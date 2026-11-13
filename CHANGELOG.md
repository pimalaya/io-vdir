# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added I/O-free `VdirItemRead` coroutine.

  It reads the batch of listed entries it is handed in one request and returns the items beside the entries that could not be read.

- Added `VdirItemEntry`, an item as a listing names it: id, kind, path and stat, without its bytes.

- Added `VdirFileMeta` (size, modification time, inode) with the `VdirYield::WantsFileMeta` request and its `VdirReply::FileMeta` reply.

  **Breaking** for a hand-written driver: a new request to answer, leaving out any path that is not a regular file.

- Added `VdirItemStoreMode` and the `VdirItemStoreOptions::mode` field: `Replace` (the default), `IfAbsent` and `IfUnchanged`.

  A conditional store stats the item right before its rename and fails with the new `VdirItemStoreError::Exists` or `VdirItemStoreError::Changed`, removing its temporary file. The check is not atomic.

- Added the `VdirItem::meta` field, the stat taken before the bytes were read.

  **Breaking**: a `VdirItem` struct literal needs the field.

- Added `VdirClient::list_item_entries`.

- Added `VdirClient::read_items`.

  It reads a batch of listed entries and fails when one cannot be read, leaving out only an entry whose file is gone since the listing.

- Added `VdirClient::read_readable_items`.

  It returns the items it could read beside every entry it could not, with the error each read failed with.

- Added `VdirClient::list_readable_items`, the same over a whole collection.

- Added `VdirClientError::FileRead`, naming the file a run could not read.

### Changed

- Changed `VdirItemList` to return `VdirItemEntry` values and read no item.

  **Breaking**: it stats the candidates through `WantsFileMeta` where it read them through `WantsFileRead`. `VdirClient::list_items` still returns the items, contents included.

- Changed `VdirItemStore` and `VdirItemCopy` to stage on `<id>.<ext>.<random>.tmp`.

  **Breaking** for a hand-written driver: a store always requests random bytes (8, or 24 when it mints the id) and a copy requests 8.

- Changed `VdirItemGet` to stat the item before reading it.

  **Breaking**: one more request, and a new `VdirItemGetError::Unreadable` where a file missing from the reply was returned as empty bytes.

- Changed the std client to fail a run on a file it cannot read with `VdirClientError::FileRead` where it returned `VdirClientError::Io`.

  **Breaking**: `VdirClientError` gained the `FileRead` and `VdirItemRead` variants.

- Changed `VdirClient::list_items` to leave out an item removed between its listing and its read, where it failed.

  It still fails when a listed item cannot be read for any other reason.

### Removed

- Removed the `From<(VdirPath, VdirItemKind, Vec<u8>)>` implementation of `VdirItem`, which cannot supply a stat.

### Fixed

- Fixed an item listing failing on a directory or a dangling link named like an item: only regular files are listed.

- Fixed a file named exactly .vcf or .ics being listed under an id no lookup could find: it is not an item.

- Fixed two concurrent stores or copies of one id sharing one temporary file.

- Fixed item ids reaching paths unchecked.

  `VdirItemLocate` and `VdirItemStore` refuse an id that is empty or holds a NUL or a path separator, before any request; get, copy, move and delete inherit it from the locate.

  **Breaking**: `VdirItemLocateError` and `VdirItemStoreError` gained an `InvalidId` variant.

- Fixed `VdirCollectionRename` joining its new name unchecked.

  A name that is empty, starts with a dot, or holds a NUL or a path separator is refused before any request.

  **Breaking**: `VdirCollectionRenameError` gained an `InvalidName` variant.

- Fixed `VdirItemStore` panicking on a random reply shorter than requested: it completes with `UnexpectedArg`.

## [0.2.0] - 2026-08-24

### Changed

- `VdirCollectionUpdate` writes the collection it is handed rather than only its non-empty fields: a present value is written as before, and an absent or empty one has its metadata file removed. The coroutine takes a whole `VdirCollection`, a desired state rather than a patch, so this is what its signature already promised; before it, clearing a description was a silent no-op that reported success and left the file on disk.

  **Breaking**: a caller passing a partially-filled collection used to keep the fields it left unset and now clears them. Read the collection, apply your change to it, and pass the result.

- The std client's file removal is idempotent: removing a path that does not exist is a success, since a coroutine asking for a file to be gone is satisfied by its absence.

### Fixed

- Fixed `VdirItemCopy` writing straight onto the target item name.

  The bytes went to the name the target collection is enumerated under, so a reader scanning it during the copy saw a truncated item, and a process dying mid-copy left one behind. A copy keeps the source id, so that name may already hold an item, which was destroyed rather than replaced. The copy now lands on the target's `.tmp` sibling and one rename publishes it, as the store already did, so the item under that name is either the old one or the new one.

- Fixed `VdirCollectionCreate` writing its metadata marker files straight onto their final names.

  A reader scanning a collection being created could read an empty or half-written display name and take it for the truth. Each marker file now lands on its `.tmp` sibling and is renamed into place, as `VdirCollectionUpdate` already wrote them.

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

[unreleased]: https://github.com/pimalaya/io-vdir/compare/v0.2.0..HEAD
[0.2.0]: https://github.com/pimalaya/io-vdir/compare/v0.1.0..v0.2.0
[0.1.0]: https://github.com/pimalaya/io-vdir/compare/v0.0.3..v0.1.0
[0.0.2]: https://github.com/pimalaya/io-vdir/compare/v0.0.1..v0.0.2
[0.0.1]: https://github.com/pimalaya/io-vdir/compare/root..v0.0.1
