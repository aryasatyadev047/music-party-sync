//! # EchoSync Buffer Layer crate
//!
//! Thin crate root that mounts the `core` module tree. The Buffer Layer
//! itself lives at [`core::buffer`]; this crate exists so the Buffer
//! Layer can be built, tested, and depended upon independently of the
//! Streaming Engine, Transport, Discovery, Synchronization, Security, and
//! UI layers that make up the rest of EchoSync.

pub mod core;

pub use core::buffer::{
    BufferConfig, BufferError, BufferManager, BufferStatistics, JitterBuffer,
    JitterBufferStatistics, PacketWindow, WindowStatistics,
};

pub use core::sync::{
    ClockManager, DriftEstimator, SyncConfig, SyncError, SyncStatistics, Synchronizer,
};
