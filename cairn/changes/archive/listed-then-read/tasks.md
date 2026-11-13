---
cairn: tasks
change: listed-then-read
---

- [x] Reproduce: a listing asking for bytes, a listing failing on a directory named like an item, two stores of one id staging on one name, a bare .vcf listed as an item
- [x] Add `VdirFileMeta`, `WantsFileMeta` and `FileMeta`, and answer them in the std client
- [x] Turn `VdirItemList` into a names listing returning `VdirItemEntry`, skipping empty stems and non-files
- [x] Add `VdirItemRead`, reporting unreadable entries
- [x] Stat before read in `VdirItemGet`, carry the stat in `VdirItem::meta`, fail on a missing reply
- [x] Mint the temporary name of store and copy from `WantsRandom`
- [x] Add `VdirItemStoreMode` with `IfAbsent` and `IfUnchanged`, removing the temporary file on refusal
- [x] Keep `VdirClient::list_items` and its contract: fail on an unreadable entry, leave out a vanished one
- [x] Add `list_item_entries`, `read_items`, and the tolerant `read_readable_items` and `list_readable_items` returning the causes
- [x] Unit tests per coroutine, end to end tests on the std client
- [x] Fold the delta into the specs, write the log entry, add the CHANGELOG entries
- [x] Run `nix develop --command cargo fmt`, the test suite and clippy
