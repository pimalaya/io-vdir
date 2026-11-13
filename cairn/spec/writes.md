---
cairn: spec
capability: writes
status: current
---

# Writing into a vdir

How bytes reach a collection: what a reader may observe while a write is in flight, and what a write means when the caller only fills part of what it hands over.

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

### Requirement: Collection metadata is a desired state, not a patch

A collection update SHALL write the collection it is handed rather than only its non-empty fields. A field carrying a value SHALL be written, and a field left absent or empty SHALL have its metadata file removed, so clearing a description is a write rather than a silent no-op.

#### Scenario: A cleared description

- GIVEN a collection whose description file exists on disk
- WHEN it is updated with no description
- THEN the description file is gone

### Requirement: A move is a single rename

Moving an item into another collection SHALL be one rename from the source path to the destination name, so the item exists at exactly one of the two paths at any instant.

### Requirement: Removing what is already absent is a success

A coroutine asking for a file to be gone SHALL be satisfied by its absence. Removing a path that does not exist SHALL NOT fail.

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
