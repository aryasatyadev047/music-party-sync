use std::hint::black_box;
use std::time::{Duration, Instant};

use buffer::{SyncConfig, Synchronizer};
use scheduler::{PlaybackScheduler, SchedulerConfig};
use streaming::{AudioPacket, PacketFlags};

const PACKET_COUNT: u64 = 100_000;

fn make_packet(packet_id: u64) -> AudioPacket {
    let mut packet = AudioPacket::new(
        packet_id,
        packet_id,
        vec![0_u8; 128],
        PacketFlags::new(),
        "bench-device",
        "bench-session",
        4000,
    )
    .expect("benchmark packet is within the configured packet limit");
    packet.timestamp_ms = packet_id;
    packet
}

fn scheduler_config() -> SchedulerConfig {
    let mut config = SchedulerConfig::default();
    config.queue_capacity = PACKET_COUNT as usize;
    config.max_early_threshold = Duration::from_secs(3600);
    config.playback_latency = Duration::from_millis(20);
    config
}

fn sync_config() -> SyncConfig {
    let mut config = SyncConfig::default();
    config.max_drift_ppm = 1.0e15;
    config
}

fn main() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime should start");
    runtime.block_on(async {
        let synchronizer = Synchronizer::new(sync_config()).expect("sync config is valid");
        synchronizer.start().await.expect("synchronizer should start");

        let scheduler =
            PlaybackScheduler::new(scheduler_config(), synchronizer).expect("scheduler config is valid");
        scheduler.start().await.expect("scheduler should start");

        let schedule_start = Instant::now();
        for packet_id in 0..PACKET_COUNT {
            scheduler
                .schedule_packet(make_packet(packet_id))
                .await
                .expect("benchmark queue has enough capacity");
        }
        println!(
            "scheduler_schedule packets={} elapsed_ms={:.3}",
            PACKET_COUNT,
            schedule_start.elapsed().as_secs_f64() * 1000.0
        );

        let stats = scheduler.statistics().await.expect("stats should be available");
        black_box(stats);
    });
}
