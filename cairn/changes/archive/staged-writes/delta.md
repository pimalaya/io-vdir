---
cairn: delta
change: staged-writes
---

## MODIFIED Requirements

### Requirement: A write is staged and renamed

Every file entering a vdir SHALL be written to a temporary sibling carrying the `.tmp` extension and renamed onto its final name: an item written from a caller's bytes, an item copied from another collection, and a metadata file written by a collection create or update alike. A reader listing the collection SHALL never observe a name it recognises as an item or a metadata file until every byte behind that name is there. The vdir format asks for exactly this, since it has no other way to keep a reader from seeing half a file.

#### Scenario: A stored item

- GIVEN an item stored into a collection
- WHEN the store completes
- THEN the bytes were written to `<id>.<ext>.tmp` and renamed onto `<id>.<ext>`

#### Scenario: A copied item

- GIVEN an item copied into another collection
- WHEN the copy completes
- THEN the bytes were copied to `<id>.<ext>.tmp` in the target and renamed onto `<id>.<ext>`, leaving no `.tmp` file behind

#### Scenario: A copy onto an item that already exists

- GIVEN a target collection already holding an item under the same id
- WHEN a copy replaces it and dies partway through
- THEN the item under that name is still the one that was there, whole

#### Scenario: A created collection carrying metadata

- GIVEN a collection created with a display name
- WHEN the create completes
- THEN the display name was written to its `.tmp` sibling and renamed onto the metadata file name
