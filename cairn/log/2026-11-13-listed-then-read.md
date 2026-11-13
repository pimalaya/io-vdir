---
cairn: log
change: listed-then-read
landed: 2026-11-13
---

# A listing names, a read loads, a store can be conditional

Two fixes from the family audit of 2026-11-13 (VDIR-02, VDIR-03), and the open point the checked-names entry left about bare extensions.

- New vocabulary: `VdirFileMeta` (size, mtime, inode, the fields io-mbox uses), `VdirYield::WantsFileMeta`, `VdirReply::FileMeta`. A path that is not a regular file is left out of a stat reply. A driver that cannot read a file fails its run or leaves the file out of the read reply, and the coroutine decides what a missing answer means.
- `VdirItemList` returns `VdirItemEntry` values (id, kind, path, stat) from one directory read and one stat request, and reads nothing. The stat is also what filters a directory or a dangling link named like an item, so the listing and the lookup, which probes regular files, agree by construction.
- `VdirItemRead`, new, reads the entries it is handed in one request: the caller sizes the batch. Entries missing from the reply come back in `unreadable`.
- `VdirItemGet` stats, then reads, and fails with `Unreadable` on a missing reply where it returned empty bytes. `VdirItem` gained `meta`, and lost its `From` tuple conversion, which had no stat to give.
- `VdirItemStore` and `VdirItemCopy` stage on `<id>.<ext>.<16 hex digits>.tmp`, from 8 random bytes requested with the id's 16 when one is minted.
- `VdirItemStoreOptions::mode`: `Replace` by default, `IfAbsent`, `IfUnchanged(VdirFileMeta)`, refused with `Exists` or `Changed` after removing the temporary file.
- A name with an empty stem is not an item: .vcf has no id, as `std::path::Path::extension` reads it. It used to be listed under the id `.vcf`, which is the id of the file .vcf.vcf.
- The std client answers the stat request and gained `list_item_entries`, `read_items`, `read_readable_items` and `list_readable_items`. A file it cannot read fails the run as before, now with `VdirClientError::FileRead` naming the path where it was a bare `Io`.

The client's listing fails rather than shrinks. A first draft made `list_items` skip unreadable items with a warning, and was corrected before landing: the consumers of this crate synchronise, and an item missing from a listing reads there as a deletion that then propagates. A listing that aborts is safe, one that silently shrinks is not. So `list_items` and the batch `read_items` fail when a listed entry cannot be read, with one exception: an entry whose file no longer exists when it is read (`NotFound`) was deleted since the listing and is left out. The tolerant path is explicit and returns what it skipped: `read_readable_items` and `list_readable_items` hand back the items beside every unreadable entry and the `io::Error` of its read, vanished ones included. The coroutine stays tolerant, `VdirItemRead` reporting `unreadable`; the client holds the errors on its side (`run_reading`), since no reply carries one.

Callers of `VdirClient` change no signature: `list_items`, `get_item`, `store_item`, `locate_item`, `copy_item`, `move_item` and `delete_item` are as they were. What moves under them: an unreadable file is reported as `FileRead` where it was `Io`, `list_items` leaves out an item removed between its listing and its read where it failed, and a struct literal or an exhaustive match on `VdirItem`, `VdirYield`, `VdirReply` or the store, get and client error enums needs the new members. A conditional store goes through `VdirClient::run` with a `VdirItemStore` built with the wanted mode.

Decisions taken here, open to the maintainer:

- The conditional check is a stat right before the rename, not an atomic operation. The format has no lock, and an atomic create-only would need a hard link or a no-replace rename in the vocabulary, which not every filesystem offers.
- The stat an `IfUnchanged` store compares is the one taken at listing time or before a get's read, so a file rewritten in between fails the store instead of slipping through.
- A file that vanished is told from one that cannot be read by `io::ErrorKind::NotFound` alone.
- The bare-extension file is hidden rather than given an id. vdirsyncer, whose ids are whole file names, would list it.

Not done: `VdirItemMove` and `VdirItemCopy` still replace an item of the same id in the target, as the writes spec records; collection metadata is still staged on `<name>.tmp`, one name for every writer; a writer that dies leaves its randomly named temporary file behind and nothing sweeps it.

On testing: the four reproductions ran against the old code first. The listing asked for `root/contacts/alice.vcf` in a `WantsFileRead`; it took `root/contacts/.vcf` for an item; two stores of alice both staged on `root/contacts/alice.vcf.tmp`; and `list_items` over a collection holding a directory folder.ics and a dangling link failed with `Io(NotFound)`. The store modes, `VdirItemRead` and the stat are new surface, with no old behaviour to fail against. The two tests of the failing listing (a file without read permission, a directory in the place of a listed file) were run against a scratch copy with the client's strict branch disabled, where both got `Ok` with the remaining item.

Capabilities moved: listing (new), writes, replies, names.
