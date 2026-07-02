//! The Streaming Engine: the middleman that moves [`AudioPacket`]s between
//! the Media Layer, the Transport Layer, and the playback pipeline.
//!
//! ## Data flow
//!
//! ```text
//!  Media Layer                 Streaming Engine                 Transport Layer
//! ------------      enqueue()      ┌───────────────┐   send_packet()
//!  Opus encoder  ───────────────▶  │ outbound_queue │ ───worker(s)───▶  transport_tx
//!                                  └───────────────┘                 (wire, out of scope)
//!
//!  Playback pipeline           Streaming Engine                 Transport Layer
//! ------------      dequeue()      ┌───────────────┐  receive_packet()
//!  Decoder/mixer ◀───────────────  │ inbound_queue  │  ◀──────────────  (wire, out of scope)
//!                                  └───────────────┘
//! ```
//!
//! The engine never touches the network itself: the Transport Layer is
//! represented purely by an injected `tokio::sync::mpsc::Sender<AudioPacket>`
//! (packets going out) and the `receive_packet` method (packets coming in).
//! This keeps the Streaming Engine fully testable and decoupled from any
//! particular QUIC/transport implementation.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::{mpsc, watch, Mutex as AsyncMutex};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::config::StreamingConfig;
use crate::error::StreamingError;
use crate::packet::AudioPacket;
use crate::queue::PacketQueue;

/// Lifecycle state of a [`StreamingEngine`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    /// No worker tasks are running; queued packets are preserved but not
    /// forwarded.
    Stopped,
    /// Worker tasks are actively draining the outbound queue and
    /// forwarding packets to the Transport Layer.
    Running,
    /// Worker tasks are alive but idle: packets may still be enqueued and
    /// received, but nothing is forwarded until `resume()` is called.
    Paused,
}

/// A point-in-time snapshot of the engine's health and throughput,
/// returned by [`StreamingEngine::status`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineStatus {
    /// Current lifecycle state.
    pub state: EngineState,
    /// Number of packets currently buffered awaiting delivery to the
    /// Transport Layer.
    pub outbound_queue_len: usize,
    /// Number of packets currently buffered awaiting delivery to the
    /// playback pipeline.
    pub inbound_queue_len: usize,
    /// Total packets successfully handed to the Transport Layer since the
    /// engine was constructed.
    pub packets_sent: u64,
    /// Total packets successfully accepted from the Transport Layer since
    /// the engine was constructed.
    pub packets_received: u64,
    /// Total packets dropped due to queue overflow.
    pub packets_dropped: u64,
    /// Total packets that failed to send after exhausting all retries.
    pub packets_send_failed: u64,
}

/// Atomic throughput counters shared between the engine and its worker
/// tasks.
#[derive(Debug, Default)]
struct EngineMetrics {
    packets_sent: AtomicU64,
    packets_received: AtomicU64,
    packets_dropped: AtomicU64,
    packets_send_failed: AtomicU64,
}

/// The Streaming Engine: manages audio packet flow between the Media
/// Layer and the Transport Layer.
///
/// Cheap to clone: cloning a `StreamingEngine` clones its internal
/// `Arc`/channel handles, so all clones share the same queues, workers,
/// and state. This makes it convenient to hand a handle to multiple
/// producer/consumer tasks.
#[derive(Clone)]
pub struct StreamingEngine {
    config: StreamingConfig,

    /// Packets accepted from the Media Layer, waiting to be forwarded to
    /// the Transport Layer.
    outbound_queue: PacketQueue,

    /// Packets accepted from the Transport Layer, waiting to be pulled by
    /// the playback pipeline.
    inbound_queue: PacketQueue,

    /// The Transport Layer's intake channel. The actual transport
    /// implementation (out of scope here) owns the corresponding
    /// `Receiver`.
    transport_tx: mpsc::Sender<AudioPacket>,

    /// Broadcasts lifecycle state changes to worker tasks.
    state_tx: watch::Sender<EngineState>,
    state_rx: watch::Receiver<EngineState>,

    /// Handles of currently running worker tasks, retained so `stop()`
    /// can await their graceful shutdown.
    worker_handles: Arc<AsyncMutex<Vec<JoinHandle<()>>>>,

    metrics: Arc<EngineMetrics>,
}

impl StreamingEngine {
    /// Creates a new `StreamingEngine` in the [`EngineState::Stopped`]
    /// state.
    ///
    /// `transport_tx` is the Transport Layer's packet intake channel:
    /// the engine forwards outbound packets onto it, and the (out of
    /// scope) transport implementation owns the matching `Receiver`.
    pub fn new(
        config: StreamingConfig,
        transport_tx: mpsc::Sender<AudioPacket>,
    ) -> Result<Self, StreamingError> {
        let outbound_queue = PacketQueue::new(config.outbound_queue_capacity);
        let inbound_queue = PacketQueue::new(config.inbound_queue_capacity);
        let (state_tx, state_rx) = watch::channel(EngineState::Stopped);

        Ok(Self {
            config,
            outbound_queue,
            inbound_queue,
            transport_tx,
            state_tx,
            state_rx,
            worker_handles: Arc::new(AsyncMutex::new(Vec::new())),
            metrics: Arc::new(EngineMetrics::default()),
        })
    }

    /// Starts the engine: spawns `config.worker_count` background tasks
    /// that drain the outbound queue and forward packets to the Transport
    /// Layer.
    ///
    /// Safe to call again after [`StreamingEngine::stop`] to restart the
    /// engine; previously buffered, undelivered packets are preserved
    /// across the restart.
    pub async fn start(&self) -> Result<(), StreamingError> {
        if *self.state_rx.borrow() == EngineState::Running {
            return Err(StreamingError::EngineAlreadyRunning);
        }

        self.state_tx
            .send(EngineState::Running)
            .map_err(|_| StreamingError::InvalidState("state channel closed".into()))?;

        let mut handles = self.worker_handles.lock().await;
        for worker_id in 0..self.config.worker_count.max(1) {
            let outbound_queue = self.outbound_queue.clone();
            let transport_tx = self.transport_tx.clone();
            let state_rx = self.state_rx.clone();
            let config = self.config.clone();
            let metrics = Arc::clone(&self.metrics);

            handles.push(tokio::spawn(run_outbound_worker(
                worker_id,
                outbound_queue,
                transport_tx,
                state_rx,
                config,
                metrics,
            )));
        }

        info!(workers = handles.len(), "Engine Started");
        Ok(())
    }

    /// Stops the engine: signals every worker task to exit and waits for
    /// them to finish. Buffered packets in both queues are preserved so a
    /// subsequent [`StreamingEngine::start`] call can resume delivery.
    pub async fn stop(&self) -> Result<(), StreamingError> {
        if *self.state_rx.borrow() == EngineState::Stopped {
            return Err(StreamingError::InvalidState(
                "engine is already stopped".into(),
            ));
        }

        self.state_tx
            .send(EngineState::Stopped)
            .map_err(|_| StreamingError::InvalidState("state channel closed".into()))?;

        let mut handles = self.worker_handles.lock().await;
        for handle in handles.drain(..) {
            if let Err(join_err) = handle.await {
                error!(error = %join_err, "Engine Errors: worker task panicked during shutdown");
            }
        }

        info!("Engine Stopped");
        Ok(())
    }

    /// Pauses delivery: worker tasks remain alive but idle, and packets
    /// may continue to be enqueued/received while paused. Requires the
    /// engine to currently be [`EngineState::Running`].
    pub fn pause(&self) -> Result<(), StreamingError> {
        if *self.state_rx.borrow() != EngineState::Running {
            return Err(StreamingError::InvalidState(
                "cannot pause unless the engine is running".into(),
            ));
        }

        self.state_tx
            .send(EngineState::Paused)
            .map_err(|_| StreamingError::InvalidState("state channel closed".into()))?;

        info!("Engine Paused");
        Ok(())
    }

    /// Resumes delivery after a [`StreamingEngine::pause`]. Requires the
    /// engine to currently be [`EngineState::Paused`].
    pub fn resume(&self) -> Result<(), StreamingError> {
        if *self.state_rx.borrow() != EngineState::Paused {
            return Err(StreamingError::InvalidState(
                "cannot resume unless the engine is paused".into(),
            ));
        }

        self.state_tx
            .send(EngineState::Running)
            .map_err(|_| StreamingError::InvalidState("state channel closed".into()))?;

        info!("Engine Resumed");
        Ok(())
    }

    /// Accepts an already Opus-encoded packet from the Media Layer and
    /// buffers it in the outbound queue for delivery to the Transport
    /// Layer.
    ///
    /// Requires the engine to not be [`EngineState::Stopped`] (packets may
    /// still be enqueued while paused; they will be forwarded once
    /// [`StreamingEngine::resume`] is called).
    pub fn enqueue(&self, packet: AudioPacket) -> Result<(), StreamingError> {
        if *self.state_rx.borrow() == EngineState::Stopped {
            return Err(StreamingError::EngineNotRunning);
        }

        match self.outbound_queue.enqueue(packet) {
            Ok(()) => {
                debug!(
                    queue_len = self.outbound_queue.len(),
                    "Packet Queued"
                );
                Ok(())
            }
            Err(err @ StreamingError::QueueOverflow { .. }) => {
                self.metrics.packets_dropped.fetch_add(1, Ordering::Relaxed);
                warn!(error = %err, "Queue Overflow");
                Err(err)
            }
            Err(err) => Err(err),
        }
    }

    /// Pulls the next packet ready for playback from the inbound queue,
    /// waiting asynchronously if none are currently available.
    pub async fn dequeue(&self) -> Result<AudioPacket, StreamingError> {
        self.inbound_queue.dequeue().await
    }

    /// Directly attempts to hand `packet` to the Transport Layer,
    /// retrying up to `config.max_retry_count` times with a backoff delay
    /// between attempts.
    ///
    /// This is the low-level send primitive used internally by the
    /// outbound worker tasks; it is also exposed publicly for callers
    /// that need to bypass the outbound queue (e.g. urgent, unbuffered
    /// control packets).
    pub async fn send_packet(&self, packet: AudioPacket) -> Result<(), StreamingError> {
        deliver_to_transport(&packet, &self.transport_tx, &self.config, &self.metrics).await
    }

    /// Accepts a packet delivered by the Transport Layer and buffers it
    /// in the inbound queue for the playback pipeline to consume via
    /// [`StreamingEngine::dequeue`].
    ///
    /// Requires the engine to not be [`EngineState::Stopped`].
    pub fn receive_packet(&self, packet: AudioPacket) -> Result<(), StreamingError> {
        if *self.state_rx.borrow() == EngineState::Stopped {
            return Err(StreamingError::EngineNotRunning);
        }

        if packet.packet_size > self.config.max_packet_size_bytes {
            return Err(StreamingError::PacketTooLarge {
                max: self.config.max_packet_size_bytes,
                actual: packet.packet_size,
            });
        }

        match self.inbound_queue.enqueue(packet) {
            Ok(()) => {
                self.metrics
                    .packets_received
                    .fetch_add(1, Ordering::Relaxed);
                debug!(queue_len = self.inbound_queue.len(), "Packet Received");
                Ok(())
            }
            Err(err @ StreamingError::QueueOverflow { .. }) => {
                self.metrics.packets_dropped.fetch_add(1, Ordering::Relaxed);
                warn!(error = %err, "Queue Overflow");
                Err(err)
            }
            Err(err) => Err(err),
        }
    }

    /// Empties both the outbound and inbound queues without processing
    /// their contents. Safe to call in any lifecycle state.
    pub async fn clear(&self) -> Result<(), StreamingError> {
        self.outbound_queue.clear().await;
        self.inbound_queue.clear().await;
        info!("Engine queues cleared");
        Ok(())
    }

    /// Returns a snapshot of the engine's current lifecycle state, queue
    /// depths, and throughput counters.
    pub fn status(&self) -> Result<EngineStatus, StreamingError> {
        Ok(EngineStatus {
            state: *self.state_rx.borrow(),
            outbound_queue_len: self.outbound_queue.len(),
            inbound_queue_len: self.inbound_queue.len(),
            packets_sent: self.metrics.packets_sent.load(Ordering::Relaxed),
            packets_received: self.metrics.packets_received.load(Ordering::Relaxed),
            packets_dropped: self.metrics.packets_dropped.load(Ordering::Relaxed),
            packets_send_failed: self.metrics.packets_send_failed.load(Ordering::Relaxed),
        })
    }
}

/// Background task body: repeatedly drains `outbound_queue` and forwards
/// packets to the Transport Layer while the engine is `Running`, pausing
/// (without busy-waiting) while `Paused`, and exiting cleanly once the
/// engine transitions to `Stopped` or the queue is closed.
async fn run_outbound_worker(
    worker_id: usize,
    outbound_queue: PacketQueue,
    transport_tx: mpsc::Sender<AudioPacket>,
    mut state_rx: watch::Receiver<EngineState>,
    config: StreamingConfig,
    metrics: Arc<EngineMetrics>,
) {
    loop {
        // Block here (without polling) until the engine is Running or
        // has been Stopped.
        loop {
            let current = *state_rx.borrow();
            match current {
                EngineState::Running => break,
                EngineState::Stopped => {
                    debug!(worker_id, "outbound worker exiting: engine stopped");
                    return;
                }
                EngineState::Paused => {
                    if state_rx.changed().await.is_err() {
                        debug!(worker_id, "outbound worker exiting: state channel closed");
                        return;
                    }
                }
            }
        }

        // Race the next dequeue against a state change so a `pause()` or
        // `stop()` call is noticed promptly even while idle waiting for a
        // packet.
        tokio::select! {
            biased;
            changed = state_rx.changed() => {
                if changed.is_err() {
                    debug!(worker_id, "outbound worker exiting: state channel closed");
                    return;
                }
                continue;
            }
            dequeued = outbound_queue.dequeue() => {
                match dequeued {
                    Ok(packet) => {
                        if let Err(err) =
                            deliver_to_transport(&packet, &transport_tx, &config, &metrics).await
                        {
                            error!(worker_id, error = %err, "Engine Errors");
                        }
                    }
                    Err(_) => {
                        debug!(worker_id, "outbound worker exiting: queue closed");
                        return;
                    }
                }
            }
        }
    }
}

/// Hands a single packet to the Transport Layer's intake channel,
/// retrying with a fixed backoff on timeout, and recording metrics/logs
/// for the outcome.
async fn deliver_to_transport(
    packet: &AudioPacket,
    transport_tx: &mpsc::Sender<AudioPacket>,
    config: &StreamingConfig,
    metrics: &EngineMetrics,
) -> Result<(), StreamingError> {
    let mut attempts_remaining = config.max_retry_count + 1;

    loop {
        attempts_remaining -= 1;

        match timeout(config.packet_timeout, transport_tx.send(packet.clone())).await {
            Ok(Ok(())) => {
                metrics.packets_sent.fetch_add(1, Ordering::Relaxed);
                debug!(
                    packet_id = packet.packet_id,
                    sequence_number = packet.sequence_number,
                    "Packet Sent"
                );
                return Ok(());
            }
            Ok(Err(_send_error)) => {
                // The Transport Layer's receiving half has been dropped;
                // retrying will never succeed.
                metrics
                    .packets_send_failed
                    .fetch_add(1, Ordering::Relaxed);
                error!("Engine Errors: transport layer sink unavailable");
                return Err(StreamingError::TransportUnavailable);
            }
            Err(_elapsed) => {
                if attempts_remaining == 0 {
                    metrics
                        .packets_send_failed
                        .fetch_add(1, Ordering::Relaxed);
                    error!(
                        packet_id = packet.packet_id,
                        "Engine Errors: send failed after exhausting retries"
                    );
                    return Err(StreamingError::SendFailed(format!(
                        "timed out after {} attempt(s)",
                        config.max_retry_count + 1
                    )));
                }

                warn!(
                    packet_id = packet.packet_id,
                    attempts_remaining, "packet send timed out, retrying"
                );
                tokio::time::sleep(config.retry_backoff).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::PacketFlags;
    use std::time::Duration;

    fn make_packet(sequence_number: u64) -> AudioPacket {
        AudioPacket::new(
            sequence_number,
            sequence_number,
            vec![0_u8; 4],
            PacketFlags::new(),
            "sender",
            "session",
            4000,
        )
        .expect("test packet should construct")
    }

    /// Builds an engine wired to a transport channel large enough that
    /// tests don't need to actively drain it unless they want to.
    fn make_engine() -> (StreamingEngine, mpsc::Receiver<AudioPacket>) {
        let (transport_tx, transport_rx) = mpsc::channel(64);
        let engine = StreamingEngine::new(StreamingConfig::for_tests(), transport_tx)
            .expect("engine should construct");
        (engine, transport_rx)
    }

    #[tokio::test]
    async fn start_moves_engine_to_running_and_status_reflects_it() {
        let (engine, _transport_rx) = make_engine();

        engine.start().await.expect("start should succeed");
        let status = engine.status().expect("status should succeed");

        assert_eq!(status.state, EngineState::Running);
    }

    #[tokio::test]
    async fn starting_an_already_running_engine_errors() {
        let (engine, _transport_rx) = make_engine();
        engine.start().await.unwrap();

        let result = engine.start().await;
        assert_eq!(result, Err(StreamingError::EngineAlreadyRunning));
    }

    #[tokio::test]
    async fn stop_moves_engine_to_stopped() {
        let (engine, _transport_rx) = make_engine();
        engine.start().await.unwrap();

        engine.stop().await.expect("stop should succeed");
        let status = engine.status().unwrap();

        assert_eq!(status.state, EngineState::Stopped);
    }

    #[tokio::test]
    async fn stopping_a_stopped_engine_errors() {
        let (engine, _transport_rx) = make_engine();

        let result = engine.stop().await;
        assert!(matches!(result, Err(StreamingError::InvalidState(_))));
    }

    #[tokio::test]
    async fn pause_then_resume_round_trip() {
        let (engine, _transport_rx) = make_engine();
        engine.start().await.unwrap();

        engine.pause().expect("pause should succeed");
        assert_eq!(engine.status().unwrap().state, EngineState::Paused);

        engine.resume().expect("resume should succeed");
        assert_eq!(engine.status().unwrap().state, EngineState::Running);

        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn pause_without_running_errors() {
        let (engine, _transport_rx) = make_engine();

        let result = engine.pause();
        assert!(matches!(result, Err(StreamingError::InvalidState(_))));
    }

    #[tokio::test]
    async fn resume_without_pause_errors() {
        let (engine, _transport_rx) = make_engine();
        engine.start().await.unwrap();

        let result = engine.resume();
        assert!(matches!(result, Err(StreamingError::InvalidState(_))));

        engine.stop().await.unwrap();
    }

    /// While paused, enqueued packets must remain buffered and undelivered
    /// until `resume()` is called.
    #[tokio::test]
    async fn paused_engine_buffers_without_delivering() {
        let (engine, mut transport_rx) = make_engine();
        engine.start().await.unwrap();
        engine.pause().unwrap();

        engine.enqueue(make_packet(0)).unwrap();

        // Give the worker a chance to (incorrectly) deliver the packet if
        // the pause logic were broken.
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(transport_rx.try_recv().is_err());

        engine.resume().unwrap();

        let delivered = tokio::time::timeout(Duration::from_millis(200), transport_rx.recv())
            .await
            .expect("packet should arrive after resume")
            .expect("channel should not be closed");
        assert_eq!(delivered.sequence_number, 0);

        engine.stop().await.unwrap();
    }

    /// Packets enqueued before `stop()` and not yet delivered must survive
    /// a stop/start restart cycle.
    #[tokio::test]
    async fn restart_preserves_buffered_packets() {
        let (engine, mut transport_rx) = make_engine();
        engine.start().await.unwrap();
        engine.pause().unwrap();

        engine.enqueue(make_packet(7)).unwrap();

        engine.stop().await.unwrap();
        assert_eq!(engine.status().unwrap().outbound_queue_len, 1);

        engine.start().await.unwrap();
        // Engine restarts in Running state (per start()'s contract), so
        // the buffered packet should now be delivered.
        let delivered = tokio::time::timeout(Duration::from_millis(200), transport_rx.recv())
            .await
            .expect("packet should arrive after restart")
            .expect("channel should not be closed");
        assert_eq!(delivered.sequence_number, 7);

        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn enqueue_while_stopped_errors() {
        let (engine, _transport_rx) = make_engine();

        let result = engine.enqueue(make_packet(0));
        assert_eq!(result, Err(StreamingError::EngineNotRunning));
    }

    #[tokio::test]
    async fn enqueue_reports_overflow_and_increments_dropped_counter() {
        let (engine, _transport_rx) = make_engine();
        engine.start().await.unwrap();
        engine.pause().unwrap(); // prevent the worker from draining the queue

        let capacity = StreamingConfig::for_tests().outbound_queue_capacity;
        for i in 0..capacity as u64 {
            engine.enqueue(make_packet(i)).expect("enqueue within capacity should succeed");
        }

        let result = engine.enqueue(make_packet(capacity as u64));
        assert!(matches!(result, Err(StreamingError::QueueOverflow { .. })));
        assert_eq!(engine.status().unwrap().packets_dropped, 1);

        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn receive_packet_then_dequeue_round_trip() {
        let (engine, _transport_rx) = make_engine();
        engine.start().await.unwrap();

        engine.receive_packet(make_packet(42)).expect("receive should succeed");
        let packet = engine.dequeue().await.expect("dequeue should succeed");

        assert_eq!(packet.sequence_number, 42);
        assert_eq!(engine.status().unwrap().packets_received, 1);

        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn receive_packet_while_stopped_errors() {
        let (engine, _transport_rx) = make_engine();

        let result = engine.receive_packet(make_packet(0));
        assert_eq!(result, Err(StreamingError::EngineNotRunning));
    }

    #[tokio::test]
    async fn clear_empties_both_queues() {
        let (engine, _transport_rx) = make_engine();
        engine.start().await.unwrap();
        engine.pause().unwrap();

        engine.enqueue(make_packet(0)).unwrap();
        engine.receive_packet(make_packet(1)).unwrap();

        engine.clear().await.expect("clear should succeed");

        let status = engine.status().unwrap();
        assert_eq!(status.outbound_queue_len, 0);
        assert_eq!(status.inbound_queue_len, 0);

        engine.stop().await.unwrap();
    }

    /// End-to-end: packets enqueued in order must be handed to the
    /// transport channel in the same order.
    #[tokio::test]
    async fn packet_ordering_is_preserved_end_to_end() {
        let (engine, mut transport_rx) = make_engine();
        engine.start().await.unwrap();

        for i in 0..5_u64 {
            engine.enqueue(make_packet(i)).unwrap();
        }

        let mut received = Vec::new();
        for _ in 0..5 {
            let packet = tokio::time::timeout(Duration::from_millis(200), transport_rx.recv())
                .await
                .expect("packet should arrive")
                .expect("channel should not be closed");
            received.push(packet.sequence_number);
        }

        assert_eq!(received, vec![0, 1, 2, 3, 4]);

        engine.stop().await.unwrap();
    }

    /// Multiple producer tasks calling `enqueue` concurrently must not
    /// lose or duplicate packets.
    #[tokio::test]
    async fn concurrent_producer_tasks_all_packets_delivered() {
        let (transport_tx, mut transport_rx) = mpsc::channel(256);
        let mut config = StreamingConfig::for_tests();
        config.outbound_queue_capacity = 256;
        let engine = StreamingEngine::new(config, transport_tx).unwrap();
        engine.start().await.unwrap();

        let mut handles = Vec::new();
        for producer_id in 0..8_u64 {
            let engine = engine.clone();
            handles.push(tokio::spawn(async move {
                for i in 0..8_u64 {
                    let seq = producer_id * 8 + i;
                    engine.enqueue(make_packet(seq)).expect("enqueue should succeed");
                }
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }

        let mut received = Vec::new();
        for _ in 0..64 {
            let packet = tokio::time::timeout(Duration::from_millis(500), transport_rx.recv())
                .await
                .expect("packet should arrive")
                .expect("channel should not be closed");
            received.push(packet.sequence_number);
        }
        received.sort_unstable();
        assert_eq!(received, (0..64).collect::<Vec<_>>());

        engine.stop().await.unwrap();
    }

    /// Multiple consumer tasks calling `dequeue` concurrently must each
    /// receive a distinct packet with none lost or duplicated.
    #[tokio::test]
    async fn concurrent_consumer_tasks_each_packet_delivered_exactly_once() {
        let (transport_tx, _transport_rx) = mpsc::channel(64);
        let mut config = StreamingConfig::for_tests();
        config.inbound_queue_capacity = 64;
        let engine = StreamingEngine::new(config, transport_tx).unwrap();
        engine.start().await.unwrap();

        for i in 0..32_u64 {
            engine.receive_packet(make_packet(i)).unwrap();
        }

        let results = Arc::new(AsyncMutex::new(Vec::new()));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let engine = engine.clone();
            let results = Arc::clone(&results);
            handles.push(tokio::spawn(async move {
                for _ in 0..4 {
                    let packet = engine.dequeue().await.expect("dequeue should succeed");
                    results.lock().await.push(packet.sequence_number);
                }
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }

        let mut received = results.lock().await.clone();
        received.sort_unstable();
        assert_eq!(received, (0..32).collect::<Vec<_>>());

        engine.stop().await.unwrap();
    }

    /// When the transport receiver is dropped, sends must fail fast with
    /// `TransportUnavailable` rather than retrying forever.
    #[tokio::test]
    async fn send_packet_fails_when_transport_receiver_dropped() {
        let (transport_tx, transport_rx) = mpsc::channel(1);
        drop(transport_rx);

        let engine = StreamingEngine::new(StreamingConfig::for_tests(), transport_tx).unwrap();

        let result = engine.send_packet(make_packet(0)).await;
        assert_eq!(result, Err(StreamingError::TransportUnavailable));
    }
}
