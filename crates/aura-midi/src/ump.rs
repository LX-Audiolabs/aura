//! Universal MIDI Packet (MIDI 2.0) stubs.
//!
//! Process still speaks [`MidiMessage`] (7-bit channel voice). These types let
//! authors encode/decode UMP and let format wrappers ingest `CLAP_EVENT_MIDI2`.
//! `SysEx8` and Flex Data packets are encoded here; process still does not
//! see them until a plugin asks for a typed path.

use crate::message::{MidiMessage, MidiStatus};

/// One UMP packet: 1, 2, or 4 native-endian 32-bit words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ump {
    words: [u32; 4],
    /// Word count: 1, 2, or 4.
    len: u8,
}

impl Ump {
    /// Build from four CLAP / wire words. `len` follows message type.
    #[must_use]
    pub const fn from_words(words: [u32; 4]) -> Self {
        let mt = ((words[0] >> 28) & 0xF) as u8;
        let len = match mt {
            0x0..=0x2 => 1,
            0x3 | 0x4 => 2,
            _ => 4,
        };
        Self { words, len }
    }

    /// MIDI 1.0 channel-voice packet (message type 0x2, one word).
    #[must_use]
    pub const fn from_midi1(msg: MidiMessage) -> Self {
        let w0 = (0x2u32 << 28)
            | ((msg.status_byte() as u32) << 16)
            | ((msg.data1 as u32) << 8)
            | (msg.data2 as u32);
        Self {
            words: [w0, 0, 0, 0],
            len: 1,
        }
    }

    /// MIDI 2.0 Note On (type 0x4, status 0x9). Velocity is 16-bit.
    #[must_use]
    pub const fn midi2_note_on(group: u8, channel: u8, note: u8, velocity: u16) -> Self {
        midi2_note(0x9, group, channel, note, velocity)
    }

    /// MIDI 2.0 Note Off (type 0x4, status 0x8). Velocity is 16-bit.
    #[must_use]
    pub const fn midi2_note_off(group: u8, channel: u8, note: u8, velocity: u16) -> Self {
        midi2_note(0x8, group, channel, note, velocity)
    }

    /// Max payload bytes in one complete `SysEx8` packet (stream ID uses one slot).
    pub const SYSEX8_MAX_PAYLOAD: usize = 13;

    /// Complete `SysEx8` packet (type 0x5, status 0x0). `payload` max 13 bytes.
    #[must_use]
    pub fn sysex8(group: u8, stream_id: u8, payload: &[u8]) -> Option<Self> {
        Self::sysex8_packet(0x0, group, stream_id, payload)
    }

    /// `SysEx8` start / continue / end (`status` 0x1 / 0x2 / 0x3).
    #[must_use]
    pub fn sysex8_packet(status: u8, group: u8, stream_id: u8, payload: &[u8]) -> Option<Self> {
        if payload.len() > Self::SYSEX8_MAX_PAYLOAD {
            return None;
        }
        let nbytes = u32::try_from(payload.len()).ok()? + 1; // include stream ID
        let mut words = [0u32; 4];
        words[0] = (0x5u32 << 28)
            | ((u32::from(group) & 0xF) << 24)
            | ((u32::from(status) & 0xF) << 20)
            | (nbytes << 16)
            | (u32::from(stream_id) << 8);
        // First payload byte sits in word0 low 8 bits; rest pack big-endian across words 1–3.
        if let Some(&b0) = payload.first() {
            words[0] |= u32::from(b0);
        }
        for (i, &b) in payload.iter().skip(1).enumerate() {
            let word = 1 + i / 4;
            let shift = 24 - 8 * (i % 4);
            words[word] |= u32::from(b) << shift;
        }
        Some(Self { words, len: 4 })
    }

    /// Flex Data packet (type 0xD, 4 words). `form`: 0 complete, 1 start, 2 cont, 3 end.
    #[must_use]
    pub const fn flex_data(
        group: u8,
        form: u8,
        addr: u8,
        channel: u8,
        status_bank: u8,
        status: u8,
        data: [u32; 3],
    ) -> Self {
        let w0 = (0xDu32 << 28)
            | ((group as u32 & 0xF) << 24)
            | ((form as u32 & 0x3) << 22)
            | ((addr as u32 & 0x3) << 20)
            | ((channel as u32 & 0xF) << 16)
            | ((status_bank as u32) << 8)
            | (status as u32);
        Self {
            words: [w0, data[0], data[1], data[2]],
            len: 4,
        }
    }

    /// Flex Data "set tempo": 10 ns ticks per quarter note in word 1.
    #[must_use]
    pub const fn flex_set_tempo(group: u8, ten_ns_per_quarter: u32) -> Self {
        Self::flex_data(group, 0, 1, 0, 0x00, 0x00, [ten_ns_per_quarter, 0, 0])
    }

    /// MIDI 2.0 per-note pitch bend (status 0x6). `value` is 32-bit, center `0x8000_0000`.
    #[must_use]
    pub const fn midi2_per_note_pitch_bend(group: u8, channel: u8, note: u8, value: u32) -> Self {
        let w0 = (0x4u32 << 28)
            | ((group as u32 & 0xF) << 24)
            | (0x6u32 << 20)
            | ((channel as u32 & 0xF) << 16)
            | ((note as u32 & 0x7F) << 8);
        Self {
            words: [w0, value, 0, 0],
            len: 2,
        }
    }

    #[must_use]
    pub const fn message_type(self) -> u8 {
        ((self.words[0] >> 28) & 0xF) as u8
    }

    #[must_use]
    pub const fn group(self) -> u8 {
        ((self.words[0] >> 24) & 0xF) as u8
    }

    #[must_use]
    pub fn as_words(&self) -> &[u32] {
        &self.words[..self.len as usize]
    }

    #[must_use]
    pub const fn words(self) -> [u32; 4] {
        self.words
    }

    #[must_use]
    pub const fn word_count(self) -> u8 {
        self.len
    }

    /// Lossy down-convert to MIDI 1.0 channel voice. `None` for utility / `SysEx` / stream.
    #[must_use]
    pub const fn to_midi1(self) -> Option<MidiMessage> {
        match self.message_type() {
            0x2 => {
                let status = ((self.words[0] >> 16) & 0xFF) as u8;
                let data1 = ((self.words[0] >> 8) & 0xFF) as u8;
                let data2 = (self.words[0] & 0xFF) as u8;
                Some(MidiMessage::raw(status, data1, data2))
            }
            0x4 => midi2_cv_to_midi1(self.words[0], self.words[1]),
            _ => None,
        }
    }

    /// 16-bit velocity for MIDI 2 note on/off; 7-bit MIDI 1 scaled to 16-bit.
    #[must_use]
    pub const fn velocity_u16(self) -> Option<u16> {
        match self.message_type() {
            0x2 => {
                let status = ((self.words[0] >> 16) & 0xF0) as u8;
                if matches!(status, 0x80 | 0x90) {
                    let v7 = (self.words[0] & 0x7F) as u8;
                    Some(scale_u7_to_u16(v7))
                } else {
                    None
                }
            }
            0x4 => {
                let status = ((self.words[0] >> 20) & 0xF) as u8;
                if matches!(status, 0x8 | 0x9) {
                    Some((self.words[1] >> 16) as u16)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_note_on(self) -> bool {
        match self.to_midi1() {
            Some(m) => m.is_note_on(),
            None => false,
        }
    }

    #[must_use]
    pub const fn is_note_off(self) -> bool {
        match self.to_midi1() {
            Some(m) => m.is_note_off(),
            None => false,
        }
    }

    #[must_use]
    pub const fn is_sysex8(self) -> bool {
        self.message_type() == 0x5 && matches!((self.words[0] >> 20) & 0xF, 0x0..=0x3)
    }

    #[must_use]
    pub const fn is_flex_data(self) -> bool {
        self.message_type() == 0xD
    }

    /// `SysEx8` stream ID (second byte of word 0).
    #[must_use]
    pub const fn sysex8_stream_id(self) -> Option<u8> {
        if self.is_sysex8() {
            Some(((self.words[0] >> 8) & 0xFF) as u8)
        } else {
            None
        }
    }
}

const fn midi2_note(status_nibble: u8, group: u8, channel: u8, note: u8, velocity: u16) -> Ump {
    let w0 = (0x4u32 << 28)
        | ((group as u32 & 0xF) << 24)
        | ((status_nibble as u32 & 0xF) << 20)
        | ((channel as u32 & 0xF) << 16)
        | ((note as u32 & 0x7F) << 8);
    let w1 = (velocity as u32) << 16;
    Ump {
        words: [w0, w1, 0, 0],
        len: 2,
    }
}

const fn midi2_cv_to_midi1(w0: u32, w1: u32) -> Option<MidiMessage> {
    let status_nibble = ((w0 >> 20) & 0xF) as u8;
    let channel = ((w0 >> 16) & 0xF) as u8;
    let data1 = ((w0 >> 8) & 0x7F) as u8;
    let status = match status_nibble {
        0x8 => MidiStatus::NoteOff,
        0x9 => MidiStatus::NoteOn,
        0xA => MidiStatus::PolyAftertouch,
        0xB => MidiStatus::ControlChange,
        0xC => MidiStatus::ProgramChange,
        0xD => MidiStatus::ChannelPressure,
        0xE => MidiStatus::PitchBend,
        _ => return None,
    };
    let data2 = match status {
        // MIDI 2 note-on vel 0 is a real note, not MIDI 1 note-off.
        MidiStatus::NoteOn => {
            let v = scale_u16_to_u7((w1 >> 16) as u16);
            if v == 0 { 1 } else { v }
        }
        MidiStatus::NoteOff => scale_u16_to_u7((w1 >> 16) as u16),
        MidiStatus::ControlChange | MidiStatus::PolyAftertouch | MidiStatus::ChannelPressure => {
            scale_u32_to_u7(w1)
        }
        MidiStatus::ProgramChange => 0,
        MidiStatus::PitchBend => {
            // MIDI 2 pitch is 32-bit; MIDI 1 is 14-bit.
            let v14 = (w1 >> 18) as u16;
            return Some(MidiMessage::pitch_bend(channel, v14));
        }
        MidiStatus::System => return None,
    };
    Some(MidiMessage {
        status,
        channel,
        data1,
        data2,
    })
}

const fn scale_u7_to_u16(v: u8) -> u16 {
    if v == 0 {
        0
    } else if v >= 127 {
        0xFFFF
    } else {
        (v as u16) * 0x200
    }
}

const fn scale_u16_to_u7(v: u16) -> u8 {
    let scaled = v / 0x200;
    if scaled > 127 { 127 } else { scaled as u8 }
}

const fn scale_u32_to_u7(v: u32) -> u8 {
    (v >> 25) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi1_roundtrip() {
        let m = MidiMessage::note_on(3, 60, 100);
        let u = Ump::from_midi1(m);
        assert_eq!(u.message_type(), 0x2);
        assert_eq!(u.word_count(), 1);
        assert_eq!(u.to_midi1(), Some(m));
        assert!(u.is_note_on());
    }

    #[test]
    fn midi2_note_on_downconverts() {
        let u = Ump::midi2_note_on(0, 1, 64, 0x8000);
        assert_eq!(u.message_type(), 0x4);
        assert_eq!(u.word_count(), 2);
        assert_eq!(u.velocity_u16(), Some(0x8000));
        let m = u.to_midi1().expect("cv");
        assert!(m.is_note_on());
        assert_eq!(m.channel, 1);
        assert_eq!(m.note_number(), Some(64));
        assert_eq!(m.data2, 64);
    }

    #[test]
    fn midi2_zero_vel_downconverts_to_vel_1() {
        // MIDI 2 note-on vel 0 is a real note — bump to 1 so MIDI 1 does not
        // treat it as note-off.
        let u = Ump::midi2_note_on(0, 0, 60, 0);
        let m = u.to_midi1().unwrap();
        assert!(m.is_note_on());
        assert_eq!(m.data2, 1);
    }

    #[test]
    fn clap_words_roundtrip() {
        let u = Ump::midi2_note_off(2, 0, 48, 0x1000);
        let again = Ump::from_words(u.words());
        assert_eq!(again, u);
        assert_eq!(again.group(), 2);
    }

    #[test]
    fn sysex8_complete_roundtrip() {
        let u = Ump::sysex8(1, 0x42, &[0xF0, 0x7E, 0x00]).unwrap();
        assert!(u.is_sysex8());
        assert_eq!(u.word_count(), 4);
        assert_eq!(u.group(), 1);
        assert_eq!(u.sysex8_stream_id(), Some(0x42));
        assert_eq!(u.to_midi1(), None);
        assert!(Ump::sysex8(0, 0, &[0; 14]).is_none());
    }

    #[test]
    fn flex_tempo() {
        let u = Ump::flex_set_tempo(0, 500_000_000);
        assert!(u.is_flex_data());
        assert_eq!(u.message_type(), 0xD);
        assert_eq!(u.words()[1], 500_000_000);
        assert_eq!(u.to_midi1(), None);
    }
}
