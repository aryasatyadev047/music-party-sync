use std::hint::black_box;
use std::time::Instant;

use streaming::{AudioPacket, PacketFlags, PacketQueue};

const PACKET_COUNT: u64 = 100_000;

fn make_packet(sequence_number: u64) -> AudioPacket {
    AudioPacket::new(
        sequence_number,
        sequence_number,
        vec![0_u8; 128],
        PacketFlags::new(),
        "bench-device",
        "bench-session",
        4000,
    )
    .expect("benchmark packet is within the configured packet limit")
}

fn main() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime should start");
    runtime.block_on(async {
        let encode_start = Instant::now();
        let packets: Vec<_> = (0..PACKET_COUNT).map(make_packet).collect();
        black_box(&packets);
        println!(
            "streaming_packet_construction packets={} elapsed_ms={:.3}",
            PACKET_COUNT,
            encode_start.elapsed().as_secs_f64() * 1000.0
        );

        let queue = PacketQueue::new(PACKET_COUNT as usize);
        let enqueue_start = Instant::now();
        for packet in packets {
            queue.enqueue(packet).expect("queue has enough benchmark capacity");
        }
        println!(
            "streaming_enqueue packets={} elapsed_ms={:.3}",
            PACKET_COUNT,
            enqueue_start.elapsed().as_secs_f64() * 1000.0
        );

        let dequeue_start = Instant::now();
        for _ in 0..PACKET_COUNT {
            black_box(queue.dequeue().await.expect("packet should be available"));
        }
        println!(
            "streaming_dequeue packets={} elapsed_ms={:.3}",
            PACKET_COUNT,
            dequeue_start.elapsed().as_secs_f64() * 1000.0
        );
    });
}
