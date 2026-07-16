# Contributing guide

Thank you for investing your time in contributing to I/O Vdir.

Whether you are a human or an AI agent, read these in order before touching the code:

1. the [Pimalaya README](https://github.com/pimalaya) for what the project is and how its repositories stack;
2. the [Pimalaya CONTRIBUTING](https://github.com/pimalaya/.github/blob/master/CONTRIBUTING.md) guide, which chains to the shared architecture and guidelines;
3. the inline header documentation, starting with src/lib.rs: it is the architecture document of this crate;
4. the docs/ folder for the development history and living plans.

Everything below documents only what differs from the Pimalaya standards.

## Feature matrix

I/O Vdir ships no network or TLS layer, so the shared three-layer build model does not apply. It never parses item bytes either, leaving vCard and iCalendar decoding to the consumer. On top of the I/O-free coroutines it exposes two independent features: client (the blocking client over the local filesystem) and serde. Check that the coroutine core stays free of the gated code:

```sh
cargo build --no-default-features                    # coroutines only, no std leak
cargo build --no-default-features --features client  # blocking filesystem client
cargo build --no-default-features --features serde   # (de)serialization of the handles
cargo build                                          # every feature (default)
```
