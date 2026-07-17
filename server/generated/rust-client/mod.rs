//! Generated Rust code from CSIL specification

//! ## Additional dependencies for the consuming crate
//!
//! chrono = "0.4"
//!
pub mod types;
pub use types::*;

#[path = "codec.gen.rs"]
pub mod codec;
pub use codec::*;

pub mod client;
pub use client::*;

pub mod client_async;
pub use client_async::*;
