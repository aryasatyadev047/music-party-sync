//! The wire-level packet type moved through the Streaming Engine.
//!
//! `AudioPacket` carries an already Opus-encoded payload produced by the
//! Media Layer, plus the metadata the Transport Layer and playback
//! pipeline need for ordering, deduplication, and diagnostics. This
//! module has no knowledge of *how* packets are transported or decoded —
//! it only defines the shape of the data as it flows through the engine.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::StreamingError;

/// Bit flags describing special characteristics of an [`AudioPacket`].
///
/// Stored as a plain `u8` bitfield (no external `bitflags` dependency) to
/// keep the Streaming Engine's dependency footprint minimal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PacketFlags(pub u8);

impl PacketFlags {
    /// No flags set.
    pub const NONE: u8 = 0;
    /// Marks a packet that can be decoded independently (useful for
    /// stream resynchronization after a discontinuity).
    pub const KEYFRAME: u8 = 1 << 0;
    /// Marks a packet that is being resent after a prior delivery
    /// failure or a NACK from a receiving device.
    pub const RETRANSMIT: u8 = 1 << 1;
    /// Marks the final packet of a stream/session.
    pub const END_OF_STREAM: u8 = 1 << 2;
    /// Marks a packet that encodes silence (useful for cheap silence
    /// detection without touching the Opus payload).
    pub const SILENCE: u8 = 1 << 3;

    /// Creates an empty flag set.
    pub fn new() -> Self {
        Self(Self::NONE)
    }

    /// Returns `true` if every bit in `flag` is set.
    pub fn contains(self, flag: u8) -> bool {
        self.0 & flag == flag
    }

    /// Sets the given bit(s).
    pub fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }

    /// Clears the given bit(s).
    pub fn clear(&mut self, flag: u8) {
        self.0 &= !flag;
    }
}

/// A single unit of audio flowing through the Streaming Engine.
///
/// The `opus_data` payload is treated as an opaque, already-encoded blob;
/// the Streaming Engine never inspects or mutates it, it only moves it
/// between the Media Layer, the Transport Layer, and the playback
/// pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioPacket {
    /// Globally-unique identifier for this specific packet instance.
    /// Distinct from `sequence_number`: a retransmitted packet keeps the
    /// same `sequence_number` but may be assigned a new `packet_id`.
    pub packet_id: u64,

    /// Monotonically increasing sequence number within a session, used by
    /// the playback pipeline (outside this module's scope) to detect
    /// reordering, loss, and duplication.
    pub sequence_number: u64,

    /// Capture/generation timestamp, in milliseconds since the Unix
    /// epoch.
    pub timestamp_ms: u64,

    /// The Opus-encoded audio payload produced by the Media Layer.
    pub opus_data: Vec<u8>,

    /// Size, in bytes, of `opus_data`. Kept as an explicit field (rather
    /// than always recomputed) so it can travel alongside the packet
    /// metadata even before the payload is deserialized on the wire.
    pub packet_size: usize,

    /// Bit flags describing special characteristics of this packet.
    pub flags: PacketFlags,

    /// Identifier of the device/session participant that produced this
    /// packet.
    pub sender_id: String,

    /// Identifier of the synchronization session this packet belongs to.
    pub session_id: String,
}

impl AudioPacket {
    /// Creates a new `AudioPacket`, stamping it with the current wall
    /// clock time and deriving `packet_size` from `opus_data`.
    ///
    /// Returns [`StreamingError::PacketTooLarge`] if `opus_data` exceeds
    /// `max_packet_size_bytes`.
    pub fn new(
        packet_id: u64,
        sequence_number: u64,
        opus_data: Vec<u8>,
        flags: PacketFlags,
        sender_id: impl Into<String>,
        session_id: impl Into<String>,
        max_packet_size_bytes: usize,
    ) -> Result<Self, StreamingError> {
        let packet_size = opus_data.len();

        if packet_size > max_packet_size_bytes {
            return Err(StreamingError::PacketTooLarge {
                max: max_packet_size_bytes,
                actual: packet_size,
            });
        }

        let timestamp_ms = current_timestamp_ms();

        Ok(Self {
            packet_id,
            sequence_number,
            timestamp_ms,
            opus_data,
            packet_size,
            flags,
            sender_id: sender_id.into(),
            session_id: session_id.into(),
        })
    }
}

/// Returns the current wall-clock time in milliseconds since the Unix
/// epoch. Falls back to `0` in the practically-impossible case that the
/// system clock is set before the epoch.
fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_packet_computes_size_and_timestamp() {
        let data = vec![1_u8, 2, 3, 4, 5];
        let packet = AudioPacket::new(
            1,
            0,
            data.clone(),
            PacketFlags::new(),
            "device-a",
            "session-1",
            4000,
        )
        .expect("packet within size limit should construct");

        assert_eq!(packet.packet_size, data.len());
        assert_eq!(packet.opus_data, data);
        assert!(packet.timestamp_ms > 0);
    }

    #[test]
    fn new_packet_rejects_oversized_payload() {
        let data = vec![0_u8; 100];
        let result = AudioPacket::new(1, 0, data, PacketFlags::new(), "device-a", "session-1", 50);

        assert_eq!(
            result,
            Err(StreamingError::PacketTooLarge { max: 50, actual: 100 })
        );
    }

    #[test]
    fn flags_set_contains_and_clear_round_trip() {
        let mut flags = PacketFlags::new();
        assert!(!flags.contains(PacketFlags::KEYFRAME));

        flags.set(PacketFlags::KEYFRAME);
        assert!(flags.contains(PacketFlags::KEYFRAME));

        flags.set(PacketFlags::END_OF_STREAM);
        assert!(flags.contains(PacketFlags::KEYFRAME));
        assert!(flags.contains(PacketFlags::END_OF_STREAM));

        flags.clear(PacketFlags::KEYFRAME);
        assert!(!flags.contains(PacketFlags::KEYFRAME));
        assert!(flags.contains(PacketFlags::END_OF_STREAM));
    }
}
