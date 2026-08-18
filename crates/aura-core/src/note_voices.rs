//! CLAP voice table keyed by `note_id` — expressions in, `NOTE_END` out.
//!
//! Plugin owns oscillators / envelopes. This tracks which host notes are
//! live and emits [`NoteEventKind::End`] when the plugin marks a voice silent
//! (or when a new note steals the slot).

use crate::note_events::{NoteBuffer, NoteEvent, NoteEventKind, NoteExpression};

/// One occupied slot in [`NoteVoiceTable`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteVoice {
    pub note_id: i32,
    pub key: i16,
    pub channel: i16,
    pub port_index: i16,
    pub velocity: f32,
    /// CLAP volume expression (1.0 = 0 dB).
    pub volume: f32,
    pub pan: f32,
    /// Tuning expression in semitones.
    pub tuning: f32,
    pub vibrato: f32,
    pub expression: f32,
    pub brightness: f32,
    pub pressure: f32,
    gated: bool,
    occupied: bool,
}

impl NoteVoice {
    const fn empty() -> Self {
        Self {
            note_id: -1,
            key: 0,
            channel: 0,
            port_index: 0,
            velocity: 0.0,
            volume: 1.0,
            pan: 0.0,
            tuning: 0.0,
            vibrato: 0.0,
            expression: 0.0,
            brightness: 0.0,
            pressure: 1.0,
            gated: false,
            occupied: false,
        }
    }

    #[must_use]
    pub const fn is_occupied(self) -> bool {
        self.occupied
    }

    #[must_use]
    pub const fn is_gated(self) -> bool {
        self.occupied && self.gated
    }
}

/// Fixed-size voice pool. No heap in `apply` / `flush_ends` after [`Self::new`].
pub struct NoteVoiceTable {
    voices: Vec<NoteVoice>,
    pending_ends: Vec<NoteEvent>,
}

impl NoteVoiceTable {
    #[must_use]
    pub fn new(max_voices: usize) -> Self {
        let n = max_voices.clamp(1, 128);
        Self {
            voices: vec![NoteVoice::empty(); n],
            pending_ends: Vec::with_capacity(n),
        }
    }

    #[must_use]
    pub fn max_voices(&self) -> usize {
        self.voices.len()
    }

    #[must_use]
    pub fn occupied_count(&self) -> usize {
        self.voices.iter().filter(|v| v.occupied).count()
    }

    #[must_use]
    pub fn voices(&self) -> &[NoteVoice] {
        &self.voices
    }

    pub fn voice_mut(&mut self, index: usize) -> Option<&mut NoteVoice> {
        self.voices.get_mut(index)
    }

    /// Route inbound CLAP notes. `On` allocates / retriggers; `Off` releases the
    /// gate; `Choke` queues `NOTE_END` immediately; expressions update matches.
    pub fn apply(&mut self, notes: &NoteBuffer) {
        for ev in notes.iter() {
            match ev.kind {
                NoteEventKind::On { velocity } => {
                    let _ = self.alloc(ev, velocity);
                }
                NoteEventKind::Off { .. } => {
                    for v in &mut self.voices {
                        if v.occupied && ev.matches_voice(v.note_id, v.key) {
                            v.gated = false;
                        }
                    }
                }
                NoteEventKind::Choke => {
                    for i in 0..self.voices.len() {
                        let v = self.voices[i];
                        if v.occupied && ev.matches_voice(v.note_id, v.key) {
                            self.queue_end(i, ev.sample_offset);
                            self.voices[i] = NoteVoice::empty();
                        }
                    }
                }
                NoteEventKind::End => {
                    // Host-originated end: drop without echoing.
                    for v in &mut self.voices {
                        if v.occupied && ev.matches_voice(v.note_id, v.key) {
                            *v = NoteVoice::empty();
                        }
                    }
                }
                NoteEventKind::Expression { id, value } => {
                    #[allow(clippy::cast_possible_truncation)]
                    let f = value as f32;
                    for v in &mut self.voices {
                        if v.occupied && ev.matches_voice(v.note_id, v.key) {
                            match id {
                                NoteExpression::Volume => v.volume = f,
                                NoteExpression::Pan => v.pan = f,
                                NoteExpression::Tuning => v.tuning = f,
                                NoteExpression::Vibrato => v.vibrato = f,
                                NoteExpression::Expression => v.expression = f,
                                NoteExpression::Brightness => v.brightness = f,
                                NoteExpression::Pressure => v.pressure = f,
                                NoteExpression::Other(_) => {}
                            }
                        }
                    }
                }
                NoteEventKind::ParamMod { .. } | NoteEventKind::ParamValue { .. } => {}
            }
        }
    }

    /// Envelope finished (or steal-equivalent). Next [`Self::flush_ends`] emits `NOTE_END`.
    pub fn mark_silent(&mut self, index: usize, sample_offset: u32) {
        if index >= self.voices.len() || !self.voices[index].occupied {
            return;
        }
        self.queue_end(index, sample_offset);
        self.voices[index] = NoteVoice::empty();
    }

    pub fn mark_silent_id(&mut self, note_id: i32, sample_offset: u32) {
        for i in 0..self.voices.len() {
            let v = self.voices[i];
            if v.occupied && (note_id < 0 || v.note_id == note_id) {
                self.mark_silent(i, sample_offset);
            }
        }
    }

    pub fn mark_all_silent(&mut self, sample_offset: u32) {
        for i in 0..self.voices.len() {
            if self.voices[i].occupied {
                self.mark_silent(i, sample_offset);
            }
        }
    }

    /// Push queued `NOTE_END` events (steal + [`Self::mark_silent`]).
    pub fn flush_ends(&mut self, out: &mut NoteBuffer) {
        for ev in self.pending_ends.drain(..) {
            out.push(ev);
        }
    }

    fn alloc(&mut self, ev: NoteEvent, velocity: f64) -> Option<usize> {
        if ev.note_id >= 0
            && let Some(i) = self
                .voices
                .iter()
                .position(|v| v.occupied && v.note_id == ev.note_id)
        {
            self.reset_voice(i, ev, velocity);
            return Some(i);
        }
        if let Some(i) = self.voices.iter().position(|v| !v.occupied) {
            self.reset_voice(i, ev, velocity);
            return Some(i);
        }
        let steal = self
            .voices
            .iter()
            .position(|v| v.occupied && !v.gated)
            .or_else(|| self.voices.iter().position(|v| v.occupied))?;
        self.queue_end(steal, ev.sample_offset);
        self.reset_voice(steal, ev, velocity);
        Some(steal)
    }

    fn reset_voice(&mut self, i: usize, ev: NoteEvent, velocity: f64) {
        #[allow(clippy::cast_possible_truncation)]
        let vel = velocity as f32;
        let mut v = NoteVoice::empty();
        v.occupied = true;
        v.gated = true;
        v.note_id = ev.note_id;
        v.key = ev.key;
        v.channel = ev.channel;
        v.port_index = ev.port_index;
        v.velocity = vel;
        self.voices[i] = v;
    }

    fn queue_end(&mut self, i: usize, sample_offset: u32) {
        let v = self.voices[i];
        if !v.occupied {
            return;
        }
        let mut end = NoteEvent::end(sample_offset, v.note_id, v.key);
        end.port_index = v.port_index;
        end.channel = v.channel;
        self.pending_ends.push(end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_off_end_and_steal() {
        let mut table = NoteVoiceTable::new(1);
        let mut notes = NoteBuffer::new();
        notes.push(NoteEvent::on(0, 3, 60, 0.8));
        table.apply(&notes);
        assert_eq!(table.occupied_count(), 1);
        assert!((table.voices()[0].velocity - 0.8).abs() < 1e-6);
        assert!(table.voices()[0].is_gated());

        notes.clear();
        notes.push(NoteEvent::off(4, 3, 60, 0.0));
        table.apply(&notes);
        assert!(table.voices()[0].is_occupied());
        assert!(!table.voices()[0].is_gated());

        let mut out = NoteBuffer::new();
        table.mark_silent(0, 20);
        table.flush_ends(&mut out);
        assert_eq!(out.len(), 1);
        assert!(matches!(out.as_slice()[0].kind, NoteEventKind::End));
        assert_eq!(out.as_slice()[0].note_id, 3);
        assert_eq!(table.occupied_count(), 0);

        notes.clear();
        notes.push(NoteEvent::on(0, 1, 64, 1.0));
        table.apply(&notes);
        notes.clear();
        notes.push(NoteEvent::on(8, 2, 67, 1.0));
        table.apply(&notes);
        out.clear();
        table.flush_ends(&mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out.as_slice()[0].note_id, 1);
        assert_eq!(table.voices()[0].note_id, 2);
    }

    #[test]
    fn expression_matches_note_id() {
        let mut table = NoteVoiceTable::new(2);
        let mut notes = NoteBuffer::new();
        notes.push(NoteEvent::on(0, 1, 60, 1.0));
        notes.push(NoteEvent::on(0, 2, 64, 1.0));
        notes.push(NoteEvent {
            sample_offset: 1,
            note_id: 2,
            port_index: 0,
            channel: 0,
            key: 64,
            kind: NoteEventKind::Expression {
                id: NoteExpression::Tuning,
                value: 7.0,
            },
        });
        table.apply(&notes);
        assert!((table.voices()[0].tuning).abs() < 1e-6);
        assert!((table.voices()[1].tuning - 7.0).abs() < 1e-6);
    }
}
