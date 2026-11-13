---
cairn: delta
change: checked-names
---

## ADDED Requirements

Capability: names.

### Requirement: An item id stays in its collection

An item id SHALL be refused, before any request is yielded, when it is empty or holds a NUL or a path separator: `/` on every platform, `\` and the drive or stream colon on Windows only, `\` staying a valid character elsewhere. Locate refuses it, and so do get, copy, move and delete, which locate first. Store refuses a supplied id the same way; an id it mints is a UUID and always passes.

The item extension is always appended to an id, so what passes can only name an item file of the collection. A leading dot is therefore valid (`..` names the file ...vcf), and so is an id resembling a metadata or temporary file name (`displayname` names the item file displayname.vcf).

#### Scenario: An id pointing into another collection

- GIVEN the collections contacts and work under one root, work holding the item bob
- WHEN the item `../work/bob` is deleted from contacts
- THEN the delete fails with an invalid id error, no request was yielded and bob is still there

#### Scenario: A stored id pointing outside the collection

- GIVEN a store into contacts with the id `../escaped`
- WHEN it is resumed for the first time
- THEN it completes with an invalid id error and nothing was written

### Requirement: The check refuses no id a listing returns

The id of an item returned by a listing SHALL pass the check, a dot-prefixed file included. A listing SHALL NOT hide an item to make that hold: a reader comparing two listings would take the hidden item for a deleted one.

#### Scenario: A dot-prefixed item

- GIVEN a collection holding the file .alice.vcf
- WHEN it is listed and the returned id `.alice` is located
- THEN the lookup probes .alice.vcf and returns it

### Requirement: A collection's new name is one visible path component

A collection rename SHALL refuse, before any request is yielded, a new name that fails the item id check or starts with a dot. A renamed collection therefore stays in its parent directory, under a name the collection listing does not skip.

#### Scenario: A new name holding a separator

- GIVEN the collections contacts and work under one root
- WHEN contacts is renamed to `work/nested`
- THEN the rename fails with an invalid name error and contacts is still there

#### Scenario: A hidden new name

- GIVEN the collection contacts
- WHEN it is renamed to `.contacts`
- THEN the rename fails with an invalid name error

Capability: replies.

### Requirement: A reply that does not answer the request is an error

A coroutine resumed with a reply that does not answer its pending request SHALL complete with an unexpected argument error carrying that reply. It SHALL NOT panic. A random reply shorter than the length requested is such a reply.

#### Scenario: A short random reply

- GIVEN a store minting an id, waiting on 16 random bytes
- WHEN the driver feeds back 15
- THEN the store completes with an unexpected argument error
