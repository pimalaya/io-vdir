---
cairn: change
id: staged-writes
status: landed
created: 2026-08-23
---

# Every file entering a vdir is staged

## Why

Two write paths skip the staging the rest of the crate performs.

`VdirItemCopy` yields its copy straight onto `<target>/<id>.<ext>`, the name a reader enumerates, and the std client services that with `fs::copy`, which creates the destination at zero bytes and then streams into it. Worse than the same defect in io-maildir, which this mirrors: a copy here keeps the source id, so the destination name may already hold an item. That item is truncated the moment the copy starts, and a process dying mid-copy leaves it destroyed rather than merely leaves a stray file behind.

`VdirCollectionCreate` writes the display name, description and colour files directly, where `VdirCollectionUpdate` already stages each one through a `.tmp` sibling. A reader scanning a collection while it is being created can read an empty or half-written display name and take it for the truth.

The crate already claims otherwise. README.md lists copy among the operations "written atomically through a temporary file and a rename", and docs/architecture.md states the property for items in general. The vdir format asks for it too: a collection is a plain directory, so a name appearing before its bytes is all a reader gets.

## What

Both paths stage through a `.tmp` sibling and rename, as store and update already do.

`build_paths` moves from item/store.rs to its sibling item.rs, since copy is its second caller and item.rs is where the extensions and the `TMP` constant already live.

The rename is unconditional in both. A copy onto a path that already exists replaces it in one step, which is what the caller asked for and what the format wants: an item is either the old bytes or the new ones, never a mixture.

Out of scope: the client's `write_files` and `copy_paths`, which stay plain. Staging is a decision the coroutines make and express in their yields, so a caller driving them with its own I/O gets the same guarantee rather than one that depends on which driver it uses.
