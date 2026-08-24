---
cairn: change
id: coroutine-state-naming
status: landed
created: 2026-08-23
---

# Coroutine states are named after the action, not the wait

## Why

Every `State` enum in the crate named its variants after what the driver was waiting for: `AwaitDirCreate`, `AwaitFileCreate`, `AwaitRename`, `AwaitProbe`, `AwaitRandom`. io-imap, the reference for coroutines across the Pimalaya libraries, names them after the action in flight instead, with `Start` for the entry point. The `Display` impls carried the same posture, reading "await rename reply" where io-imap reads "send move".

A state is a place the coroutine is at, doing something. Naming it for the driver's posture describes the caller rather than the machine, and it diverges from every sibling library. io-maildir was swept the same day; this is its twin.

## What

Rename the variants of all fourteen `State` enums to present-tense actions, and reword the `Display` phrases to the verb plus its object. Nothing else moves: the enums are private, no public item is touched, and no behaviour changes.

The convention is naming-013 in the organisation guidelines.
