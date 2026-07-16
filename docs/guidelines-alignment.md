# Guidelines alignment

Log of the realignment of io-vdir with the Pimalaya documentation and naming guidelines (the org-wide GUIDELINES.md), done in July 2026.

## Landed

- **Public naming**: the data handles carry the crate scope like the coroutines already did. `Collection` became `VdirCollection`, `Item` became `VdirItem`, `ItemKind` became `VdirItemKind`. The marker-file names (displayname, description, color) and item extensions (vcf, ics, tmp) were internal implementation details, so they became crate-private constants rather than prefixed public items.

- **Module layout**: the private `types` submodules with their doc-inlined re-exports were retired. The shared handles now live in their parent module files (collection.rs and item.rs, siblings of the collection/ and item/ folders), which also hold the `pub mod` declarations of the per-operation coroutines. Public paths are unchanged: still `collection::VdirCollection` and `item::VdirItem`.

- **lib.rs header**: the README include was removed. The header is now a standalone architecture document describing the coroutine core, the layers, the source layout and the encoding features, as the guidelines require (the README and the header are two different documents).

- **Inline docs**: every public item is documented, including the coroutine error variants, the yield fields and the coroutine output fields, so `RUSTFLAGS="-D missing_docs"` passes with and without features. The resume-loop-top state traces were removed (the guidelines flag them as noise); the terminal output traces were kept.

- **README**: rewritten to carry no code and no identifier references, with the library section order (Features, Specification coverage, Usage, Examples, AI disclosure, License, Social, Contributing, Sponsoring). Usage and Examples are redirects to docs.rs and the examples folder.

- **Repository files**: added this docs/ folder; reduced CONTRIBUTING.md to the repository-specific feature matrix (io-vdir has no TLS three-layer model); gave SECURITY.md a supported-versions table; removed the git-cliff configuration in favour of a hand-maintained changelog; added the no-std category and the example path to Cargo.toml.

## Notes

The sibling filesystem crates io-maildir and io-m2dir still use the older patterns (README-as-rustdoc, private `types` submodules); they need the same realignment when touched.
