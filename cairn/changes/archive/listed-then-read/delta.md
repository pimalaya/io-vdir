---
cairn: delta
change: listed-then-read
---

## ADDED Requirements

Capability: listing.

### Requirement: A listing names items and reads none

An item listing SHALL return, for each item of the collection, its id, its kind, its path and the stat of its file, and SHALL NOT ask for the bytes of any item. Reading SHALL be a separate step over the entries the caller hands in, so the caller sizes each batch.

#### Scenario: A large collection

- GIVEN a collection of 5,000 items
- WHEN it is listed
- THEN one directory read and one stat request were yielded, and no read

### Requirement: Only a regular file with a non-empty id is an item

A listing SHALL return a directory entry only when its name is `<id>.vcf` or `<id>.ics` with a non-empty id and the stat sees a regular file behind it, links followed. A directory named like an item, a dangling link, a metadata file and a leftover temporary file are not items. A file named exactly .vcf or .ics is not one either: the dot opens its name, it does not close a stem, so it has no id.

#### Scenario: A directory named like an item

- GIVEN a collection holding the item alice.vcf, a directory folder.ics and a dangling link dangling.vcf
- WHEN it is listed
- THEN only alice is returned and no error is raised

### Requirement: A batch read reports what it could not read

The batch read coroutine SHALL return the items it was given bytes for beside the entries it was not, and SHALL NOT fail or substitute empty bytes for them. Reading one item by id SHALL fail naming the path instead.

#### Scenario: An entry missing from the reply

- GIVEN a batch read of two entries
- WHEN the driver's reply holds the bytes of one
- THEN the read completes with that item and the other entry is reported as unreadable

### Requirement: A client listing fails rather than shrinks

`VdirClient::list_items` and `VdirClient::read_items` SHALL fail, naming the path and the error, when a listed entry cannot be read. A caller that compares listings takes a missing item for a deleted one, so a listing that aborts is safe and one that silently shrinks is not. An entry whose file no longer exists when it is read was deleted since the listing: it is gone, and SHALL be left out without failing.

`VdirClient::list_readable_items` and `VdirClient::read_readable_items` SHALL instead return the items they could read together with every entry they could not and the error its read failed with, the vanished ones included, so the caller holds both. Every other run of the std client SHALL fail on the first file it cannot read, naming its path.

#### Scenario: An item without read permission

- GIVEN a collection holding alice and bob, alice's file not readable
- WHEN it is listed with `list_items`
- THEN the listing fails naming alice's path and the permission error
- AND `list_readable_items` returns bob beside the entry of alice and that error

#### Scenario: An item removed after the listing

- GIVEN the entries of a listing, one of whose files was removed since
- WHEN they are read with `read_items`
- THEN the other items are returned and no error is raised

Capability: writes.

### Requirement: A store can be conditional

A store SHALL replace an item already stored under its id unless told otherwise. Told to store only if absent, it SHALL fail when the final name holds a file. Told to store only if unchanged, it SHALL fail when the stat of the final name differs from the one the caller hands in, a missing file included. The stat of an item is the one a listing or a read by id returned, taken before the bytes were read.

The condition SHALL be checked after the bytes are staged, right before the rename, and a refused store SHALL remove its temporary file. The check is not atomic: the format has no lock, so a writer slipping in between the stat and the rename is not seen.

#### Scenario: Another writer got in first

- GIVEN an item read with its stat, then rewritten by another program
- WHEN the first reader stores its edit under the condition of that stat
- THEN the store fails as changed, the other program's bytes are still there and no temporary file is left

#### Scenario: A create-only store over an existing item

- GIVEN a collection holding the item alice
- WHEN alice is stored again only if absent
- THEN the store fails as existing and the item is untouched

Capability: replies.

### Requirement: What the driver could not serve is left out of the reply

A driver that cannot read a file SHALL either fail its run or leave the file out of the read reply, and SHALL NOT answer with empty bytes. It SHALL leave out of a stat reply a path that is not a regular file. The coroutine decides what a missing answer means: a listing skips the entry, a batch read reports it, a read by id fails. The std client leaves a file out only in its two tolerant read methods.

## MODIFIED Requirements

Capability: writes.

### Requirement: A write is staged and renamed

Every file entering a vdir SHALL be written to a temporary sibling carrying the `.tmp` extension and renamed onto its final name: an item written from a caller's bytes, an item copied from another collection, and a metadata file written by a collection create or update alike. A reader listing the collection SHALL never observe a name it recognises as an item or a metadata file until every byte behind that name is there. The vdir format asks for exactly this, since it has no other way to keep a reader from seeing half a file.

The temporary name of an item SHALL be drawn from random bytes the coroutine requests, `<id>.<ext>.<random>.tmp`, so two writers of one id never share a temporary file. A writer that dies leaves its own behind, which no later write reuses or removes.

#### Scenario: A stored item

- GIVEN an item stored into a collection
- WHEN the store completes
- THEN the bytes were written to `<id>.<ext>.<random>.tmp` and renamed onto `<id>.<ext>`

#### Scenario: Two stores of one id at once

- GIVEN two stores of the id alice in flight in one collection
- WHEN both stage their bytes
- THEN each wrote a temporary file of its own, and each rename publishes one whole item

#### Scenario: A copied item

- GIVEN an item copied into another collection
- WHEN the copy completes
- THEN the bytes were copied to `<id>.<ext>.<random>.tmp` in the target and renamed onto `<id>.<ext>`, leaving no `.tmp` file behind

#### Scenario: A copy onto an item that already exists

- GIVEN a target collection already holding an item under the same id
- WHEN a copy replaces it and dies partway through
- THEN the item under that name is still the one that was there, whole

#### Scenario: A created collection carrying metadata

- GIVEN a collection created with a display name
- WHEN the create completes
- THEN the display name was written to its `.tmp` sibling and renamed onto the metadata file name

Capability: names.

### Requirement: The check refuses no id a listing returns

The id of an item returned by a listing SHALL pass the check, a dot-prefixed file included, and a lookup of that id SHALL probe the listed path. A listing SHALL NOT hide an item to make that hold: a reader comparing two listings would take the hidden item for a deleted one.

#### Scenario: A dot-prefixed item

- GIVEN a collection holding the file .alice.vcf
- WHEN it is listed and the returned id `.alice` is located
- THEN the lookup probes .alice.vcf and returns it

Capability: replies.

### Requirement: A reply that does not answer the request is an error

A coroutine resumed with a reply that does not answer its pending request SHALL complete with an unexpected argument error carrying that reply. It SHALL NOT panic. A random reply shorter than the length requested is such a reply.

#### Scenario: A short random reply

- GIVEN a store minting an id, waiting on 24 random bytes
- WHEN the driver feeds back 23
- THEN the store completes with an unexpected argument error
