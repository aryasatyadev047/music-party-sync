# EchoSync — Buffer Layer

Sits between the Streaming Engine and the (future) Synchronization
Engine. Absorbs network jitter, reorders packets, detects duplicates
and loss, and delivers packets to the consumer in playback order. No
synchronization, networking, or playback code lives here — those are
separate layers.

## Files

- `src/core/mod.rs` — mounts `core::buffer` (and, later, the other
  `core::*` layers).
- `src/core/buffer/mod.rs` — module root, re-exports the public API.
- `src/core/buffer/config.rs` — `BufferConfig` (buffer sizes, target
  delay bounds, max packet age, duplicate cache size, missing-packet
  tolerance, adaptive step).
- `src/core/buffer/error.rs` — `BufferError`, the single error type
  every public fn returns.
- `src/core/buffer/packet_window.rs` — `PacketWindow`, storage-free
  sequence-number bookkeeping: admits/rejects packets by sequence
  number, detects duplicates and loss, tracks a delivery watermark.
- `src/core/buffer/jitter_buffer.rs` — `JitterBuffer`, the
  single-threaded core: owns a `BTreeMap<sequence_number, packet>` for
  storage/reordering, applies target-delay release timing, staleness
  eviction, and RFC 3550-style adaptive delay.
- `src/core/buffer/buffer_manager.rs` — `BufferManager`, the async,
  thread-safe public entry point: wraps `JitterBuffer` in a
  `tokio::sync::Mutex` so multiple producer/consumer tasks can share
  one buffer.

## How it's wired to its neighbors

```text
 Streaming Engine       BufferManager::push_packet()      JitterBuffer + PacketWindow      BufferManager::pop_packet()      Synchronization Engine
 dequeue() ──────────────────────────────────────────▶  (reorder, dedupe, target delay)  ──────────────────────────────▶  (future, out of scope)
```

The Buffer Layer only touches the Streaming Engine through its
existing, unmodified public API (`streaming::AudioPacket`) — this
crate is a separate Cargo package (`buffer`) with a path dependency on
`streaming`, so nothing in the Streaming Engine's source was changed
to build this.

## Dependency

```toml
[dependencies]
streaming = { path = "../streaming" }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
tracing = "0.1"
```

## Notes for the Synchronization Engine integrator

- `pop_packet()` returns `Ok(None)` — not an error — when the buffer is
  empty or the next packet hasn't sat for `target_delay` yet. Poll it
  on an interval (e.g. driven by the target delay) rather than treating
  `None` as a failure.
- `push_packet()` returns `Err(BufferError::DuplicatePacket)` for a
  packet still sitting in the buffer, and also for a packet that was
  already delivered and is still in the duplicate cache
  (`config.duplicate_cache_size` entries deep); once it ages out of
  the cache, a repeat arrival instead returns
  `Err(BufferError::AlreadyProcessed)`.
- `target_delay` adjusts automatically when
  `config.adaptive_enabled` is set, based on an RFC 3550-style jitter
  estimate; call `set_target_delay` to override manually, or
  `current_delay` to read the active value.
- `statistics()` returns a full snapshot (buffer size, average delay,
  loss/duplicate/late/dropped counts, max occupancy, average jitter,
  delivered/buffered totals) suitable for exposing to a dashboard or
  the Synchronization Engine's own adaptive logic.

## Verification status

This crate was written to compile and pass `cargo build` / `cargo
test` against the same toolchain as the Streaming Engine (Rust 1.75,
`tokio 1.52`, `tracing 0.1`), and every method was manually traced
against its test suite. **`cargo build` / `cargo test` could not
actually be executed in the environment this was generated in** — no
Rust toolchain and no network access to fetch one — so please run
both locally before treating this as verified:

```sh
cd echosync
cargo build --workspace
cargo test --workspace
```

If anything fails to compile, it's most likely a small trait-bound or
borrow-checker detail; the architecture and logic should not need to
change to fix it.
