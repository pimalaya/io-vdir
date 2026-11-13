---
cairn: spec
capability: listing
status: current
---

# Listing and reading items

How the items of a collection are enumerated and how their bytes are loaded.

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
