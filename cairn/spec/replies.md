---
cairn: spec
capability: replies
status: current
---

# Replies fed back by the driver

What a coroutine does with the reply it is resumed with.

### Requirement: A reply that does not answer the request is an error

A coroutine resumed with a reply that does not answer its pending request SHALL complete with an unexpected argument error carrying that reply. It SHALL NOT panic. A random reply shorter than the length requested is such a reply.

#### Scenario: A short random reply

- GIVEN a store minting an id, waiting on 24 random bytes
- WHEN the driver feeds back 23
- THEN the store completes with an unexpected argument error

### Requirement: What the driver could not serve is left out of the reply

A driver that cannot read a file SHALL either fail its run or leave the file out of the read reply, and SHALL NOT answer with empty bytes. It SHALL leave out of a stat reply a path that is not a regular file. The coroutine decides what a missing answer means: a listing skips the entry, a batch read reports it, a read by id fails. The std client leaves a file out only in its two tolerant read methods.
