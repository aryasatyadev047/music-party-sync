# EchoSync — Playback Scheduler

Decides exactly *when* each audio packet should be released for
playback. Sits between the Buffer Layer / Clock Synchronization Engine
and the (future) Audio Output module. No decoding, network, or actual
audio-playback code lives here — this crate only computes deadlines and
hands packets back out in order.

## Files

- `src/config.rs` — `SchedulerConfig` (playback latency, late/early
  tolerance windows, scheduling interval, queue capacity, timeline
  resolution).
- `src/error.rs` — `SchedulerError`, the single error type every public
  fn returns.
- `src/statistics.rs` — `SchedulerStatistics` / `StatisticsTracker`
  (scheduled/played/dropped/late/early counts, average scheduling delay,
  average playback offset, queue occupancy).
- `src/timeline.rs` — `PlaybackTimeline`, maps a packet's capture-time
  `timestamp_ms` onto a monotonic local elapsed-time deadline. No
  `SystemTime` involved, so it stays deterministic under
  `tokio::time::pause` in tests.
- `src/playback_queue.rs` — `PlaybackQueue` (+ `ScheduledPacket`), the
  single-threaded, earliest-deadline-first ordering core: a
  `BinaryHeap` keyed on deadline, with `sequence_number`/`packet_id`
  tie-breaking for deterministic playback order.
- `src/scheduler.rs` — `PlaybackScheduler`, the async, thread-safe
  public entry point: wraps `PlaybackQueue` in a `tokio::sync::Mutex`
  (the same pattern `BufferManager` uses around `JitterBuffer`) and
  reads `Synchronizer::current_offset()` to fold the Clock
  Synchronization Engine's estimate into each packet's deadline.

## How it's wired to its neighbors

```text
 Buffer Layer          PlaybackScheduler::schedule_packet()      PlaybackQueue (EDF order)      PlaybackScheduler::next_packet()      (future) Audio Output
 pop_packet() ───────────────────────────────────────────────▶  (deadline via PlaybackTimeline  ──────────────────────────────────▶  release-ready packets only
                                                                   + Synchronizer::current_offset())
```

This crate only touches the Buffer Layer and Clock Synchronization
Engine through their existing, unmodified public APIs
(`buffer::BufferManager`, `buffer::Synchronizer`) and the Streaming
Engine only for the shared `streaming::AudioPacket` type — it's a
separate Cargo package (`scheduler`) with path dependencies on `buffer`
and `streaming`, so nothing in either crate's source was changed to
build this.

## Dependency

```toml
[dependencies]
buffer = { path = "../buffer" }
streaming = { path = "../streaming" }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
tracing = "0.1"
```

## Notes for the Audio Output integrator

- `schedule_packet()` computes a deadline from the packet's
  `timestamp_ms`, `SchedulerConfig::playback_latency`, and the current
  clock offset, then enqueues it in deadline order. It returns
  `Err(SchedulerError::PacketExpired)` — and does *not* queue the
  packet — if the deadline is already more than
  `max_late_threshold` in the past.
- `next_packet()` returns `Ok(None)` — not an error — when the queue is
  empty or the earliest-deadline packet isn't due yet. Poll it on an
  interval (e.g. driven by `SchedulerConfig::scheduling_interval`)
  rather than treating `None` as a failure, the same pattern used by
  `BufferManager::pop_packet`.
- `cancel_packet(packet_id)` removes a specific packet before release
  (e.g. if a fresher retransmission superseded it); it's safe to call
  even if the packet was already released or never queued — it just
  returns `Ok(false)`.
- `statistics()` returns a full snapshot (scheduled/played/dropped/late/
  early counts, average scheduling delay, average playback offset, live
  queue occupancy) suitable for exposing to a dashboard.

## Verification status

This crate was built and verified in-environment against Ubuntu's
packaged Rust 1.75 toolchain (`rustc`/`cargo` 1.75.0, `tokio 1.52`,
`tracing 0.1`) — the same versions already pinned by `buffer` and
`streaming`. `cargo build --workspace` and `cargo test --workspace` both
pass cleanly: 98 tests in `buffer`, 27 in `streaming`, and 51 new tests
in `scheduler` (176 total), 0 failures.
