---
cairn: change
id: listed-then-read
status: landed
created: 2026-11-13
---

# A listing names, a read loads, a store can be conditional

## Why

Two findings of the family audit of 2026-11-13, and one point left open by the checked-names change.

VDIR-03: `VdirItemList` was the only way to enumerate a collection and it asked for the bytes of every item in one `WantsFileRead`, so a collection was held whole in one reply before the caller saw its first id. Names were not probed first: a directory called x.ics, a dangling link or one unreadable file made the std client's read fail, and the whole listing with it. A path missing from a read reply was taken for an empty item.

VDIR-02: an item was staged on `<id>.<ext>.tmp`, the same name for every writer of that id. Two concurrent stores truncated and filled one file, the first rename published it while the second writer was still writing, which the writes spec rules out, and the second rename failed on a missing source. A store also had no way to say "only if absent" or "only if unchanged": the yields carried no stat.

Open point: a file named exactly .vcf or .ics was listed with its whole name as id, which the lookup probed as .vcf.vcf and never found.

## What

- `VdirFileMeta` (size, mtime, inode, as io-mbox has it) with `VdirYield::WantsFileMeta` and `VdirReply::FileMeta`; a path that is not a regular file is left out of the reply. A file that could not be read is left out of `VdirReply::FileRead`, and the coroutine decides whether that is fatal.
- `VdirItemList` reads the directory, stats the candidates and returns `VdirItemEntry` values (id, kind, path, stat): no bytes. A name with an empty stem is not an item, and neither is anything the stat does not see as a regular file.
- `VdirItemRead`, new, reads the batch of entries it is handed in one request and returns the items beside the entries that could not be read.
- `VdirItemGet` stats the located file before reading it, and fails with `Unreadable` where it used to return empty bytes. `VdirItem` carries that stat in a new `meta` field.
- `VdirItemStore` and `VdirItemCopy` ask for 8 random bytes and stage on `<id>.<ext>.<16 hex digits>.tmp`.
- `VdirItemStoreOptions::mode`: `Replace` (the default, unchanged), `IfAbsent`, `IfUnchanged(VdirFileMeta)`. A conditional store stats the final name after staging, right before the rename, and removes its temporary file when the condition fails.
- The std client answers the new request and gains `list_item_entries`, `read_items`, `read_readable_items` and `list_readable_items`. A file it cannot read still fails the run, now as `VdirClientError::FileRead` naming the path. `list_items` keeps its signature and its contract: it lists, reads everything in one batch and fails when a listed entry cannot be read, since a consumer that synchronises reads a missing item as a deletion. The one exception is an entry whose file is gone when it is read, deleted since the listing, which is left out. The two `readable` methods are the tolerant path: they return the items beside every unreadable entry and the error its read failed with.

## Out of scope

- A conditional store is not atomic: the format has no lock, so a writer slipping in between the stat and the rename is not seen. An atomic create-only would need a link or a no-replace rename in the vocabulary, which not every filesystem offers.
- `VdirItemMove` and `VdirItemCopy` still replace an item of the same id in the target, as the writes spec records.
- Collection metadata is still staged on `<name>.tmp`, one name for every writer.
- Replies still carry no failure: the std client keeps the error of an unread file on its own side and hands it back from its tolerant methods, and another driver has to do the same.
