# EchoSync — Streaming Engine

Manages audio packet flow between the Media Layer and the Transport
Layer. No QUIC/networking, sync, discovery, or UI code lives here —
those are separate layers.

## Files

- `mod.rs` — module root, re-exports the public API.
- `config.rs` — `StreamingConfig` (queue sizes, timeout, max packet size, retry count, worker count).
- `error.rs` — `StreamingError`, the single error type every public fn returns.
- `packet.rs` — `AudioPacket` (packet id, sequence number, timestamp, Opus payload, size, flags, sender/session id) + `PacketFlags`.
- `queue.rs` — `PacketQueue`, a bounded, thread-safe, FIFO, async queue (Tokio `mpsc` + a shared `Mutex` on the receiving half so multiple consumer tasks can drain it safely).
- `engine.rs` — `StreamingEngine`, with `new`, `start`, `stop`, `pause`, `resume`, `send_packet`, `receive_packet`, `enqueue`, `dequeue`, `clear`, `status`, plus the background worker loop and 27 unit tests.

## How it's wired to its neighbors

```text
 Media Layer         StreamingEngine::enqueue()      outbound_queue      worker(s)      transport_tx (Transport Layer, out of scope)
 Transport Layer  →  StreamingEngine::receive_packet() → inbound_queue → StreamingEngine::dequeue()  →  playback pipeline
```

The engine never touches the network. The Transport Layer boundary is a
plain `tokio::sync::mpsc::Sender<AudioPacket>` injected into
`StreamingEngine::new` — whoever implements the real transport owns the
matching `Receiver` and calls `receive_packet()` on the engine when data
arrives off the wire.

## Dependency

```toml
[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
tracing = "0.1"
```

## Verified

Compiled and tested on Rust 1.75 with `tokio 1.52` and `tracing 0.1`.
`cargo build` and `cargo test` both pass cleanly with **zero warnings**;
all **27 unit tests** pass, run 5x in a row with no flakiness, covering:
queue FIFO ordering, queue overflow, concurrent producers/consumers
(both at the queue and engine level), start/stop/pause/resume, restart
with buffered-packet preservation, packet ordering end-to-end, oversized
packet rejection, and transport-sink-unavailable handling.

## Notes for the Transport Layer integrator

- `enqueue()` / `receive_packet()` return `StreamingError::EngineNotRunning`
  once the engine is `Stopped` — call `start()` first.
- Both are still accepted while `Paused` (packets buffer; delivery
  resumes on `resume()`).
- `stop()` preserves any undelivered, buffered packets in both queues so
  a following `start()` resumes delivery of exactly what was left.
- `send_packet()` retries `config.max_retry_count` times with
  `config.retry_backoff` between attempts, bounded by
  `config.packet_timeout` per attempt, before returning
  `StreamingError::SendFailed`. If the transport's `Receiver` has been
  dropped, it fails immediately with `StreamingError::TransportUnavailable`
  instead of retrying.
