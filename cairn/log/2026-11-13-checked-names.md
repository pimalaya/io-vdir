---
cairn: log
change: checked-names
landed: 2026-11-13
---

# Ids and names are checked before they reach a path

Two fixes from the family audit of 2026-11-13 (VDIR-01, VDIR-04).

- `VdirItemLocate` and `VdirItemStore` refuse an id that is empty or holds a NUL or a path separator, with a new `InvalidId` variant on each error enum and before any request. Get, copy, move and delete locate first and inherit it. Before, the id `../work/bob` made a store write outside its collection (the std client creating the parents) and a delete remove a file of the sibling collection.
- `VdirCollectionRename` refuses such a new name, and one starting with a dot, with the new `InvalidName`. Its destination is computed in the first step where the constructor used to compute it, since `new` cannot fail.
- `VdirItemStore` completes with `UnexpectedArg` on a random reply shorter than 16 bytes, where it panicked on the slice. `uuid_v4` takes the 16-byte array, so its all-zero fallback is gone.

The checks are two functions in path.rs: `is_stem` for an item id (`/` and NUL everywhere, `\` and `:` on Windows only, as io-maildir's lookup does) and `is_name` for a collection name, a stem with no leading dot. The three new variants are breaking additions to public enums.

A first draft refused a leading dot on item ids too, as io-maildir does, and was corrected before landing. The extension is always appended to an id, so a dot cannot leave the collection (`..` names the file ...vcf), while the listing returns dot-prefixed items: the lookup must accept every id the listing hands out, and hiding them from the listing instead is the dangerous direction, since a sync reads a vanished item as a deletion. The collection name keeps the refusal, matching the collection listing, which skips dot-prefixed directories.

Not applied from the audit: refusing metadata and temporary names. The extension is appended to an id, so `displayname` is the item displayname.vcf and no id yields a `.tmp` name; refusing it would hide legitimate items.

Open points, the maintainer's:

- Collection paths are still raw. `VdirClient` resolves nothing under its root, every helper but `list_collections` takes a path the caller built, and the consumers join a typed collection id onto the root themselves. Should the client gain a store type resolving checked names under its root, as io-maildir has?
- A file named exactly .vcf or .ics is listed with its whole name as id (`VdirItem::id` falls back to it on an empty stem), which the lookup accepts but cannot find. Read in the code, not changed here.

On testing: each unit test was run against the old code first, where the coroutine yielded the request (`WantsFileCreate` on root/contacts/.vcf.tmp for the empty id) or panicked (`range end index 16 out of range for slice of length 15`). The listed dot-prefixed id test failed against the first draft with `InvalidId(".alice")`. The end to end test drives the std client and asserts nothing was written, removed or renamed outside.

Capabilities moved: names (new), replies (new).
