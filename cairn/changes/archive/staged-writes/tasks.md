---
cairn: tasks
change: staged-writes
---

- [x] Move `build_paths` from item/store.rs to item.rs as a `pub(crate)` helper, and call it from both
- [x] Copy stages into `<id>.<ext>.tmp` and renames onto `<id>.<ext>`, gaining a rename step
- [x] Collection create stages each metadata file through its `.tmp` sibling and renames, as update does
- [x] Reword the copy module header, which documents the copy as landing directly on the target name
- [x] Cover both in their unit tests: the staged destination, then the rename onto the final name
- [x] Cover the copy end to end in tests/integration.rs, asserting the collection holds no leftover `.tmp` file
- [x] Add the CHANGELOG entries under `[Unreleased]`
- [x] Run `nix develop --command cargo fmt` and the test suite
