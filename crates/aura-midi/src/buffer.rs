//! Time-stamped MIDI event list for one process block (JUCE `MidiBuffer`).

use crate::message::MidiMessage;

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

    #[must_use]
    pub fn as_slice(&self) -> &[MidiEvent] {
        &self.events
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
}
