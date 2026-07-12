# generated/

Code generated from `../schema/` by csilgen. **Do not hand-edit** — changes are
overwritten on regeneration, and generator issues are fixed upstream, not here.

Regenerate:

```sh
./tools.sh gen           # rust-server + all client languages
./tools.sh gen-server    # rust-server only
```

Layout: one subdirectory per target (`rust-server/`, `rust-client/`, `typescript-client/`,
…). The Rust server binding needs `chrono = "0.4"` in the consuming crate; it is otherwise
self-contained (owns its own canonical-CBOR codec, no serde).

## Known upstream issues

The current `rust-*` output does not yet pass `cargo fmt --check` or
`cargo clippy -- -D warnings`. Both are filed at
`../../csilgen/docs/csilgen-requests/rust-generator-clean-build.md` and will be fixed in
the generator. Until then, the generated `codec.gen.rs` is not clean-build compliant; wire
these crates into CI's fmt/clippy gates only after that request lands.
