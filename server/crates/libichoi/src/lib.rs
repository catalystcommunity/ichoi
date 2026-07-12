//! libichoi — pure, I/O-free domain logic plus the generated CSIL service surface.
//!
//! This crate holds nothing that touches the filesystem, a database, the network, or an
//! async runtime, so it stays WASM-viable and trivially testable. The server crate
//! (`ichoi`) depends on this; never the reverse (house convention).

pub mod account;
pub mod codec;
pub mod csil;
pub mod error;
pub mod m3u;
pub mod share;

pub use error::DomainError;
