//! Time-stamped MIDI / UMP event lists for one process block (JUCE `MidiBuffer`).

use crate::message::MidiMessage;
use crate::ump::Ump;

/// One MIDI message at a sample offset within the current block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MidiEvent {
    /// Sample offset into the process block (`0..num_samples`).
    pub sample_offset: u32,
    pub message: MidiMessage,
}

/// Ordered MIDI events for one audio block. Sorted by [`MidiEvent::sample_offset`].
///
/// Format wrappers fill this from host event lists; synth/FX read it in
/// `process`. No allocation after [`MidiBuffer::with_capacity`] / reuse via
/// [`MidiBuffer::clear`].
#[derive(Clone, Debug, Default)]
pub struct MidiBuffer {
    events: Vec<MidiEvent>,
}

impl MidiBuffer {
    #[must_use]
    pub const fn new() -> Self {
        Self { events: Vec::new() }
    }

    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            events: Vec::with_capacity(cap),
        }
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn reserve(&mut self, additional: usize) {
        self.events.reserve(additional);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Push an event. Keeps order by sample offset (stable insert).
    pub fn push(&mut self, sample_offset: u32, message: MidiMessage) {
        let ev = MidiEvent {
            sample_offset,
            message,
        };
        match self
            .events
            .binary_search_by_key(&sample_offset, |e| e.sample_offset)
        {
            Ok(mut i) => {
                // After equal offsets — preserve FIFO for same sample.
                while i < self.events.len() && self.events[i].sample_offset == sample_offset {
                    i += 1;
                }
                self.events.insert(i, ev);
            }
            Err(i) => self.events.insert(i, ev),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = MidiEvent> + '_ {
        self.events.iter().copied()
    }

    /// Events with `sample_offset` in `start..end` (half-open).
    pub fn iter_range(&self, start: u32, end: u32) -> impl Iterator<Item = MidiEvent> + '_ {
        self.events
            .iter()
            .copied()
            .filter(move |e| e.sample_offset >= start && e.sample_offset < end)
    }

    /// Copy events in `start..end` with offsets rebased to `0` (chunk process).
    #[must_use]
    pub fn slice_rebased(&self, start: u32, end: u32) -> Self {
        let mut out = Self::with_capacity(self.events.len());
        out.copy_range_rebased(self, start, end);
        out
    }

    /// Like [`slice_rebased`](Self::slice_rebased) into an existing buffer (no new `Vec`).
    pub fn copy_range_rebased(&mut self, src: &Self, start: u32, end: u32) {
        self.clear();
        for ev in src.iter_range(start, end) {
            // Source is already sorted; push in order without binary insert.
            self.events.push(MidiEvent {
                sample_offset: ev.sample_offset.saturating_sub(start),
                message: ev.message,
            });
        }
    }

    /// Append events, adding `base` to each sample offset (merge chunk outs).
    pub fn extend_rebased(&mut self, other: &Self, base: u32) {
        for ev in other.iter() {
            self.push(ev.sample_offset.saturating_add(base), ev.message);
        }
    }

    #[must_use]
    pub fn as_slice(&self) -> &[MidiEvent] {
        &self.events
    }
}

/// One Universal MIDI Packet at a sample offset within the current block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UmpEvent {
    pub sample_offset: u32,
    pub packet: Ump,
}

/// Ordered UMP events for one audio block. Same insert / rebase rules as [`MidiBuffer`].
///
/// CLAP fills this from `CLAP_EVENT_MIDI2` (native) and `CLAP_EVENT_MIDI`
/// (wrapped as type-0x2 UMP). VST3/LV2 lift 7-bit MIDI the same way.
/// Packets that have no MIDI 1 image (per-note pitch bend, `SysEx8`, Flex)
/// stay here only.
#[derive(Clone, Debug, Default)]
pub struct UmpBuffer {
    events: Vec<UmpEvent>,
}

impl UmpBuffer {
    #[must_use]
    pub const fn new() -> Self {
        Self { events: Vec::new() }
    }

    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            events: Vec::with_capacity(cap),
        }
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn reserve(&mut self, additional: usize) {
        self.events.reserve(additional);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn push(&mut self, sample_offset: u32, packet: Ump) {
        let ev = UmpEvent {
            sample_offset,
            packet,
        };
        match self
            .events
            .binary_search_by_key(&sample_offset, |e| e.sample_offset)
        {
            Ok(mut i) => {
                while i < self.events.len() && self.events[i].sample_offset == sample_offset {
                    i += 1;
                }
                self.events.insert(i, ev);
            }
            Err(i) => self.events.insert(i, ev),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = UmpEvent> + '_ {
        self.events.iter().copied()
    }

    pub fn iter_range(&self, start: u32, end: u32) -> impl Iterator<Item = UmpEvent> + '_ {
        self.events
            .iter()
            .copied()
            .filter(move |e| e.sample_offset >= start && e.sample_offset < end)
    }

    #[must_use]
    pub fn slice_rebased(&self, start: u32, end: u32) -> Self {
        let mut out = Self::with_capacity(self.events.len());
        out.copy_range_rebased(self, start, end);
        out
    }

    pub fn copy_range_rebased(&mut self, src: &Self, start: u32, end: u32) {
        self.clear();
        for ev in src.iter_range(start, end) {
            self.events.push(UmpEvent {
                sample_offset: ev.sample_offset.saturating_sub(start),
                packet: ev.packet,
            });
        }
    }

    #[must_use]
    pub fn as_slice(&self) -> &[UmpEvent] {
        &self.events
    }
}

/// Lift 7-bit MIDI into type-0x2 UMP (VST3/LV2 → process `ump`).
pub fn append_midi_as_ump(ump: &mut UmpBuffer, midi: &MidiBuffer) {
    for ev in midi.iter() {
        ump.push(ev.sample_offset, Ump::from_midi1(ev.message));
    }
}

/// Down-convert UMP that has a MIDI 1 image (VST3/LV2 out). Per-note PB / `SysEx8` / Flex stay behind.
pub fn append_ump_as_midi(midi: &mut MidiBuffer, ump: &UmpBuffer) {
    for ev in ump.iter() {
        if let Some(msg) = ev.packet.to_midi1() {
            midi.push(ev.sample_offset, msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::MidiMessage;

    #[test]
    fn push_sorts_by_offset() {
        let mut buf = MidiBuffer::new();
        buf.push(10, MidiMessage::note_on(0, 60, 100));
        buf.push(0, MidiMessage::note_on(0, 48, 80));
        buf.push(10, MidiMessage::note_off(0, 60, 0));
        let offs: Vec<_> = buf.iter().map(|e| e.sample_offset).collect();
        assert_eq!(offs, vec![0, 10, 10]);
        assert!(buf.as_slice()[1].message.is_note_on());
        assert!(buf.as_slice()[2].message.is_note_off());
    }

    #[test]
    fn iter_range() {
        let mut buf = MidiBuffer::new();
        buf.push(0, MidiMessage::note_on(0, 60, 100));
        buf.push(5, MidiMessage::control_change(0, 1, 64));
        buf.push(10, MidiMessage::note_off(0, 60, 0));
        assert_eq!(buf.iter_range(0, 5).count(), 1);
        assert_eq!(buf.iter_range(5, 11).count(), 2);
    }

    #[test]
    fn copy_range_rebased_preserves_order() {
        let mut buf = MidiBuffer::new();
        buf.push(4, MidiMessage::note_on(0, 60, 100));
        buf.push(12, MidiMessage::note_off(0, 60, 0));
        let mut dest = MidiBuffer::with_capacity(4);
        dest.copy_range_rebased(&buf, 4, 13);
        assert_eq!(dest.len(), 2);
        assert_eq!(dest.as_slice()[0].sample_offset, 0);
        assert_eq!(dest.as_slice()[1].sample_offset, 8);
    }

    #[test]
    fn ump_buffer_keeps_per_note_pb() {
        let mut ump = UmpBuffer::new();
        let pb = Ump::midi2_per_note_pitch_bend(0, 0, 60, 0x8000_0000);
        ump.push(4, pb);
        ump.push(0, Ump::from_midi1(MidiMessage::note_on(0, 60, 100)));
        assert_eq!(ump.len(), 2);
        assert_eq!(ump.as_slice()[0].sample_offset, 0);
        assert!(ump.as_slice()[1].packet.is_per_note_pitch_bend());
        let mut midi = MidiBuffer::new();
        append_ump_as_midi(&mut midi, &ump);
        assert_eq!(midi.len(), 1);
        assert!(midi.as_slice()[0].message.is_note_on());
    }
}
