use std::hint::black_box;
use std::time::{Duration, Instant};

use buffer::{BufferConfig, BufferManager, SyncConfig, Synchronizer};
use streaming::{AudioPacket, PacketFlags};

const PACKET_COUNT: u64 = 100_000;

fn make_packet(sequence_number: u64) -> AudioPacket {
    let mut packet = AudioPacket::new(
        sequence_number,
        sequence_number,
        vec![0_u8; 128],
        PacketFlags::new(),
        "bench-device",
        "bench-session",
        4000,
    )
    .expect("benchmark packet is within the configured packet limit");
    packet.timestamp_ms = sequence_number;
    packet
}

fn buffer_config() -> BufferConfig {
    let mut config = BufferConfig::default();
    config.max_buffer_size = PACKET_COUNT as usize;
    config.initial_buffer_size = PACKET_COUNT as usize;
    config.duplicate_cache_size = PACKET_COUNT as usize;
    config.target_delay = Duration::ZERO;
    config.min_target_delay = Duration::ZERO;
    config.adaptive_enabled = false;
    config.max_packet_age = Duration::from_secs(3600);
    config
}

fn sync_config() -> SyncConfig {
    let mut config = SyncConfig::default();
    config.max_drift_ppm = 1.0e15;
    config.drift_window_samples = 64;
    config
}

fn main() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime should start");
    runtime.block_on(async {
        let manager = BufferManager::new(buffer_config()).expect("benchmark config is valid");

        let push_start = Instant::now();
        for sequence_number in 0..PACKET_COUNT {
            manager
                .push_packet(make_packet(sequence_number))
                .await
                .expect("benchmark buffer has enough capacity");
        }
        println!(
            "buffer_push packets={} elapsed_ms={:.3}",
            PACKET_COUNT,
            push_start.elapsed().as_secs_f64() * 1000.0
        );

        let pop_start = Instant::now();
        for _ in 0..PACKET_COUNT {
            black_box(manager.pop_packet().await.expect("pop succeeds"));
        }
        println!(
            "buffer_pop packets={} elapsed_ms={:.3}",
            PACKET_COUNT,
            pop_start.elapsed().as_secs_f64() * 1000.0
        );

        let synchronizer = Synchronizer::new(sync_config()).expect("sync config is valid");
        synchronizer.start().await.expect("synchronizer should start");
        let sync_start = Instant::now();
        for sample in 0..PACKET_COUNT {
            let local = Duration::from_millis(sample);
            let host = Duration::from_millis(sample + 25);
            synchronizer
                .synchronize(host, local)
                .await
                .expect("bounded benchmark offset should synchronize");
        }
        println!(
            "sync_pass packets={} elapsed_ms={:.3}",
            PACKET_COUNT,
            sync_start.elapsed().as_secs_f64() * 1000.0
        );
    });
}
