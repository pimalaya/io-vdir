---
cairn: log
change: staged-writes
landed: 2026-08-23
---

# Every file entering a vdir is staged

`VdirItemCopy` copies into the target's `.tmp` sibling and renames, where it used to yield the final item name as the copy destination and complete on the copy reply. `VdirCollectionCreate` stages each metadata marker the same way, as `VdirCollectionUpdate` already did. Both gained one rename step and nothing else.

`build_paths` moved from item/store.rs to item.rs, unchanged, since copy is its second caller and item.rs already owns the extensions and the `TMP` constant it reads.

The copy case is worse than the io-maildir bug it mirrors, and worth stating plainly: a vdir copy keeps the source id, so the destination name may already hold an item. Writing into that name directly destroyed the item that was there the moment the copy started. Staging makes it a replacement that either happened or did not.

The writes capability moved: *A write is staged and renamed* now covers every file entering a vdir, copies and collection creation included.

On testing, as in io-maildir: the crash is not reproduced, since a test cannot kill itself inside `fs::copy`, and a test faking it would assert nothing. Both unit tests assert the property that closes the window, that the coroutine never names a final path as a write destination, which is a pure statement about the yields. The end to end test adds the filesystem half, that no `.tmp` file survives the copy.

The new states in copy.rs are named for the action in flight, `Copy` and `Rename`, per naming-013 in the organisation guidelines. The rest of the crate still carries `Await` prefixes, swept separately.
