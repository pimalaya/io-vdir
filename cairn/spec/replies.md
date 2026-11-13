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

- GIVEN a store minting an id, waiting on 16 random bytes
- WHEN the driver feeds back 15
- THEN the store completes with an unexpected argument error
