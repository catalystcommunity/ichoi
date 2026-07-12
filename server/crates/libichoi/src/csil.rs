//! The generated CSIL service surface: types, service traits, and the self-contained
//! canonical-CBOR codec, generated from `schema/` by csilgen.
//!
//! **Never hand-edited.** Regenerate with `./tools.sh gen-server`. Lints are relaxed here
//! because the current generator output is not yet fmt/clippy clean — tracked upstream in
//! `csilgen/docs/csilgen-requests/rust-generator-clean-build.md`. The `#![allow(...)]`
//! cascades into the `#[path]`-included child modules.
#![allow(clippy::all, unused, dead_code)]

#[path = "../../../generated/rust-server/types.rs"]
pub mod types;

#[path = "../../../generated/rust-server/codec.gen.rs"]
pub mod codec;

#[path = "../../../generated/rust-server/services.rs"]
pub mod services;

pub use services::*;
pub use types::*;
