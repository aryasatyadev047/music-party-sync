//! # EchoSync Streaming Engine
//!
//! This module manages audio **packet flow** between the Media Layer and
//! the Transport Layer. It intentionally has no knowledge of:
//! - How packets are actually transmitted over the network (QUIC, etc.) —
//!   that's the Transport Layer's job, represented here only as an
//!   injected channel.
//! - Clock/playback synchronization across devices.
//! - UI or device discovery.
//!
//! ## Responsibilities
//! - Accept encoded Opus packets from the Media Layer ([`StreamingEngine::enqueue`]).
//! - Buffer them in a bounded, thread-safe FIFO queue ([`queue::PacketQueue`]).
//! - Forward them to the Transport Layer with retry/backoff
//!   ([`StreamingEngine::send_packet`], driven by background workers).
//! - Accept packets arriving from the Transport Layer
//!   ([`StreamingEngine::receive_packet`]).
//! - Hand them to the playback pipeline ([`StreamingEngine::dequeue`]).
//! - Support pause/resume/stop/restart of packet delivery.
//!
//! ## Example
//! ```ignore
//! // Adjust the path below to match how this `streaming` module is
//! // mounted in your crate (e.g. `crate::streaming::...`).
//! use streaming::{StreamingEngine, StreamingConfig, AudioPacket, PacketFlags};
//! use tokio::sync::mpsc;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! // The Transport Layer owns `transport_rx` and reads outbound packets
//! // from it; here we just keep the sender.
//! let (transport_tx, mut transport_rx) = mpsc::channel(128);
//!
//! let engine = StreamingEngine::new(StreamingConfig::default(), transport_tx)?;
//! engine.start().await?;
//!
//! let packet = AudioPacket::new(1, 0, vec![0u8; 32], PacketFlags::new(), "device-a", "session-1", 4000)?;
//! engine.enqueue(packet)?;
//!
//! let delivered = transport_rx.recv().await.unwrap();
//! println!("delivered sequence {}", delivered.sequence_number);
//!
//! engine.stop().await?;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod engine;
pub mod error;
pub mod packet;
pub mod queue;

// Re-export the primary public types at the module root so downstream
// crates (Media Layer glue code, Transport Layer, playback pipeline) can
// write `streaming::StreamingEngine` instead of reaching into
// `streaming::engine::StreamingEngine`.
pub use config::StreamingConfig;
pub use engine::{EngineState, EngineStatus, StreamingEngine};
pub use error::StreamingError;
pub use packet::{AudioPacket, PacketFlags};
pub use queue::PacketQueue;
