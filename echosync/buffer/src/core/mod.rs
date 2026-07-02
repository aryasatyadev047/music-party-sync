//! Module root for EchoSync's `core` layers.
//!
//! [`buffer`] and [`sync`] (the Clock Synchronization Engine) are
//! implemented here. Discovery, Transport (QUIC), and Security are
//! separate workstreams and are intentionally not present in this crate;
//! when they land, this file is the place they get mounted
//! (`pub mod discovery;`, `pub mod transport;`, `pub mod security;`),
//! alongside a `contracts` module of shared traits that those layers
//! should depend on rather than reaching into each other's concrete
//! types.

pub mod buffer;
pub mod sync;
