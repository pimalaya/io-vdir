---
cairn: log
change: cairn-adoption
landed: 2026-08-23
---

# Cairn adoption

The repository now carries a Cairn root at cairn/, with AGENTS.md activating it and CLAUDE.md pointing there. cairn/verify.sh is the vendored bash port of the nine conformance checks, optional as always.

One capability is seeded from what the crate does today: writes, covering the staging of a stored item and of a collection update, the desired-state semantics of collection metadata, the single rename behind a move, and the idempotent removal the std client performs.

The seed is deliberately silent where the crate's behaviour is under review rather than settled: it says nothing about staging a copy, and nothing about staging the metadata a collection create writes. Those two holes are the subject of the change proposed alongside it, staged-writes, which fills them rather than restating a rule the code already breaks.

docs/architecture.md and docs/guidelines-alignment.md are untouched. Whether they retire into the spec, as they did in ical-rs, is a separate decision.
