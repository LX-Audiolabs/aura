//! Channel / system MIDI message (sample-accurate payload only).

/// High nibble of status byte (channel messages use low nibble as channel).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MidiStatus {
    NoteOff = 0x80,
    NoteOn = 0x90,
    PolyAftertouch = 0xA0,
    ControlChange = 0xB0,
    ProgramChange = 0xC0,
    ChannelPressure = 0xD0,
    PitchBend = 0xE0,
    /// System messages (0xF0..); channel field is unused.
    System = 0xF0,
}

impl MidiStatus {
    #[must_use]
    pub const fn from_status_byte(status: u8) -> Self {
        match status & 0xF0 {
            0x80 => Self::NoteOff,
            0x90 => Self::NoteOn,
            0xA0 => Self::PolyAftertouch,
            0xB0 => Self::ControlChange,
            0xC0 => Self::ProgramChange,
            0xD0 => Self::ChannelPressure,
            0xE0 => Self::PitchBend,
            _ => Self::System,
        }
    }
}

/// One MIDI message. Channel is `0..=15` for channel voice messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MidiMessage {
    pub status: MidiStatus,
    /// Wire channel `0..=15`. Ignored for [`MidiStatus::System`].
    pub channel: u8,
    pub data1: u8,
    pub data2: u8,
}

impl MidiMessage {
    #[must_use]
    pub const fn raw(status_byte: u8, data1: u8, data2: u8) -> Self {
        let status = MidiStatus::from_status_byte(status_byte);
        let channel = if matches!(status, MidiStatus::System) {
            0
        } else {
            status_byte & 0x0F
        };
        Self {
            status,
            channel,
            data1,
            data2,
        }
    }

    #[must_use]
    pub const fn note_on(channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            status: MidiStatus::NoteOn,
            channel: channel & 0x0F,
            data1: note & 0x7F,
            data2: velocity & 0x7F,
        }
    }

    #[must_use]
    pub const fn note_off(channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            status: MidiStatus::NoteOff,
            channel: channel & 0x0F,
            data1: note & 0x7F,
            data2: velocity & 0x7F,
        }
    }

    #[must_use]
    pub const fn control_change(channel: u8, controller: u8, value: u8) -> Self {
        Self {
            status: MidiStatus::ControlChange,
            channel: channel & 0x0F,
            data1: controller & 0x7F,
            data2: value & 0x7F,
        }
    }

    #[must_use]
    pub const fn pitch_bend(channel: u8, value_14: u16) -> Self {
        let v = if value_14 > 0x3FFF { 0x3FFF } else { value_14 };
        Self {
            status: MidiStatus::PitchBend,
            channel: channel & 0x0F,
            data1: (v & 0x7F) as u8,
            data2: ((v >> 7) & 0x7F) as u8,
        }
    }

    #[must_use]
    pub const fn program_change(channel: u8, program: u8) -> Self {
        Self {
            status: MidiStatus::ProgramChange,
            channel: channel & 0x0F,
            data1: program & 0x7F,
            data2: 0,
        }
    }

    #[must_use]
    pub const fn channel_pressure(channel: u8, pressure: u8) -> Self {
        Self {
            status: MidiStatus::ChannelPressure,
            channel: channel & 0x0F,
            data1: pressure & 0x7F,
            data2: 0,
        }
    }

    /// Status byte with channel in the low nibble (system → 0xF0).
    #[must_use]
    pub const fn status_byte(self) -> u8 {
        match self.status {
            MidiStatus::System => 0xF0,
            other => (other as u8) | (self.channel & 0x0F),
        }
    }

    #[must_use]
    pub const fn is_note_on(self) -> bool {
        matches!(self.status, MidiStatus::NoteOn) && self.data2 > 0
    }

    #[must_use]
    pub const fn is_note_off(self) -> bool {
        matches!(self.status, MidiStatus::NoteOff)
            || (matches!(self.status, MidiStatus::NoteOn) && self.data2 == 0)
    }

    #[must_use]
    pub const fn note_number(self) -> Option<u8> {
        match self.status {
            MidiStatus::NoteOn | MidiStatus::NoteOff | MidiStatus::PolyAftertouch => {
                Some(self.data1)
            }
            _ => None,
        }
    }

    /// 14-bit pitch bend `0..=16383` (center = 8192).
    #[must_use]
    pub const fn pitch_bend_value(self) -> Option<u16> {
        if matches!(self.status, MidiStatus::PitchBend) {
            Some((self.data1 as u16) | ((self.data2 as u16) << 7))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_on_roundtrip_status() {
        let m = MidiMessage::note_on(3, 60, 100);
        assert!(m.is_note_on());
        assert_eq!(m.note_number(), Some(60));
        assert_eq!(m.status_byte(), 0x93);
        assert_eq!(MidiMessage::raw(m.status_byte(), m.data1, m.data2), m);
    }

    #[test]
    fn note_on_zero_vel_is_note_off() {
        let m = MidiMessage::note_on(0, 48, 0);
        assert!(m.is_note_off());
        assert!(!m.is_note_on());
    }

    #[test]
    fn pitch_bend_14bit() {
        let m = MidiMessage::pitch_bend(0, 8192);
        assert_eq!(m.pitch_bend_value(), Some(8192));
    }
}
