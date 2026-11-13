---
cairn: tasks
change: checked-names
---

- [x] Reproduce both: an escaping id and name yielding a request, a 15-byte random reply panicking
- [x] Add `path::is_stem` and `path::is_name` with their unit tests
- [x] Refuse an invalid id in `VdirItemLocate` and `VdirItemStore` with `InvalidId`
- [x] Refuse an invalid new name in `VdirCollectionRename` with `InvalidName`
- [x] Keep dot-prefixed item ids valid, and cover that a listed one is located
- [x] Complete the store with `UnexpectedArg` on a short random reply
- [x] Cover the std client end to end in tests/integration.rs: nothing written, removed or renamed outside
- [x] Fold the delta into the names and replies specs, write the log entry, add the CHANGELOG entries
- [x] Run `nix develop --command cargo fmt`, the test suite and clippy
