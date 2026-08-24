---
cairn: log
change: coroutine-state-naming
landed: 2026-08-23
---

# Coroutine states are named after the action, not the wait

The fourteen private `State` enums of the crate lost their `Await` prefix. A state now names the action in flight: `CreateDir`, `CreateFile`, `Rename`, `Remove`, `RemoveFiles`, `Read`, `ReadDir`, `ReadFiles`, `ReadChildren`, `ReadMetadata`, `ReadRandom`, `Probe`, `ProbeDirs`, `ProbeMetadata`, beside the `Start` and `Locate` entry points that were already right. The `Display` phrases follow, "write item into tmp", "rename into place", "probe extensions", "read metadata", where they used to read "await file create reply".

This aligns io-vdir with io-imap, the reference for coroutines across the Pimalaya libraries, and with io-maildir, swept the same day. The rule is naming-013 in the organisation guidelines rather than folklore.

No capability moved: the enums are private, no public item changed, and no behaviour changed.

As in io-maildir, none of the `Display` impls is called anywhere in this crate. io-imap uses its own through a `trace!` at the top of each resume loop, which is what the reference model and the logging guidelines ask for. Wiring the trace, or dropping the impls, is a decision for its own change.
