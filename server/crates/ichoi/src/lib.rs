//! Ichoi server crate. The binary (`main.rs`) is a thin shell over this library so that
//! integration tests can exercise handlers as plain functions (§12).

pub mod app;
pub mod art;
pub mod audio;
pub mod auth;
pub mod cli;
pub mod config;
pub mod db;
pub mod handlers;
pub mod install;
pub mod media;
pub mod scan;
pub mod server;
pub mod satellite;
pub mod tls;
pub mod transport;
