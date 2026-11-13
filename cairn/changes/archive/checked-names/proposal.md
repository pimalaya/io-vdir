---
cairn: change
id: checked-names
status: landed
created: 2026-11-13
---

# Ids and names are checked before they reach a path

## Why

Two findings of the family audit of 2026-11-13.

VDIR-01: an item id and a collection's new name were joined onto a directory as given. `VdirItemStore` with the id `../work/bob` wrote and renamed outside its collection, the std client creating the missing parents on the way; `VdirItemDelete` with the same id removed a file of the sibling collection, and copy and move reached it too. `VdirCollectionRename` joined any name, separators included. An id could also be empty or hold a NUL. No consumer derives a file name from remote data today (new items get a minted UUID), but the ids of update, delete, copy and move are whatever the caller typed.

VDIR-04: `VdirItemStore` sliced the random reply to 16 bytes before looking at its length, so a driver answering with fewer bytes made the coroutine panic, where every other wrong reply completes with an error. The fallback behind the slice was unreachable and worse than an error: the all-zero id, the same for every item.

## What

- Two checks in path.rs. `is_stem`, for an item id: not empty, no NUL, no `/`, and on Windows no `\` or `:`. `is_name`, for a collection name: a stem with no leading dot.
- `VdirItemLocate` applies `is_stem` before its first request and completes with the new `InvalidId`, which get, copy, move and delete inherit since they locate first. `VdirItemStore` applies it to a supplied id, with its own `InvalidId`.
- `VdirCollectionRename` applies `is_name` to the new name, with the new `InvalidName`. The destination is now computed in the first step, since the constructor cannot fail.
- A random reply shorter than 16 bytes completes the store with `UnexpectedArg`, carrying the reply.

An item id keeps its leading dot. The extension is always appended, so a dot cannot leave the collection (`..` names the file ...vcf), and the listing returns dot-prefixed items: a lookup has to accept every id the listing hands out, and hiding those items from the listing instead would make a sync read them as deleted. A collection name loses it, as the collection listing already skips dot-prefixed directories.

## Out of scope

- The audit also asked to refuse metadata and temporary names. That does not hold for an id: the item extension is appended to it, so `displayname` names displayname.vcf and never the marker file, and no id can produce a name ending in `.tmp`. Refusing them would make legitimate items of other tools unreachable.
- Collection paths stay raw: create, delete, update and the item coroutines take the directory the caller built, and `VdirClient` does not resolve names under its root. Confining them needs a store type, which is the maintainer's decision.
