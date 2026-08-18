//! CLAP-shaped note / expression / per-note-mod events for `process`.
//!
//! Additive to 7-bit [`crate::MidiBuffer`]. Wrappers keep filling MIDI;
//! plugins that need `note_id` or expressions read this list.

use crate::chunked_process::TimedParamEvent;

/// Wildcard used by CLAP when a field is unspecified (`-1`).
pub const NOTE_UNSPECIFIED: i32 = -1;

/// Note-scope fields shared by CLAP note / expression / per-note-mod events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoteTarget {
    pub note_id: i32,
    pub port_index: i16,
    pub channel: i16,
    pub key: i16,
}

impl NoteTarget {
    pub const UNSPECIFIED: Self = Self {
        note_id: NOTE_UNSPECIFIED,
        port_index: -1,
        channel: -1,
        key: -1,
    };
}

/// CLAP note-expression id (`clap_note_expression`). Unknown values stay raw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoteExpression {
    Volume,
    Pan,
    Tuning,
    Vibrato,
    Expression,
    Brightness,
    Pressure,
    /// Spec id we do not name yet — still delivered.
    Other(i32),
}

impl NoteExpression {
    #[must_use]
    pub fn from_clap(id: i32) -> Self {
        match id {
            0 => Self::Volume,
            1 => Self::Pan,
            2 => Self::Tuning,
            3 => Self::Vibrato,
            4 => Self::Expression,
            5 => Self::Brightness,
            6 => Self::Pressure,
            other => Self::Other(other),
        }
    }

    #[must_use]
    pub fn to_clap(self) -> i32 {
        match self {
            Self::Volume => 0,
            Self::Pan => 1,
            Self::Tuning => 2,
            Self::Vibrato => 3,
            Self::Expression => 4,
            Self::Brightness => 5,
            Self::Pressure => 6,
            Self::Other(id) => id,
        }
    }
}

/// What happened to a (possibly scoped) note.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoteEventKind {
    On {
        velocity: f64,
    },
    Off {
        velocity: f64,
    },
    Choke,
    Expression {
        id: NoteExpression,
        value: f64,
    },
    /// Per-note `PARAM_MOD` (`note_id >= 0`). Does not touch [`crate::Params`].
    ParamMod {
        param_id: u32,
        amount: f64,
    },
    /// Per-note `PARAM_VALUE` (`note_id >= 0`). Absolute plain; not the knob.
    ParamValue {
        param_id: u32,
        plain: f64,
    },
}

/// One timed note-scoped event inside a process block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteEvent {
    pub sample_offset: u32,
    /// Host note id, or [`NOTE_UNSPECIFIED`].
    pub note_id: i32,
    pub port_index: i16,
    pub channel: i16,
    pub key: i16,
    pub kind: NoteEventKind,
}

impl NoteEvent {
    #[must_use]
    pub fn on(sample_offset: u32, note_id: i32, key: i16, velocity: f64) -> Self {
        Self {
            sample_offset,
            note_id,
            port_index: 0,
            channel: 0,
            key,
            kind: NoteEventKind::On { velocity },
        }
    }

    /// True when this event applies to the sounding voice (`-1` = wildcard).
    #[must_use]
    pub fn matches_voice(self, note_id: i32, key: i16) -> bool {
        let id_ok = self.note_id < 0 || note_id < 0 || self.note_id == note_id;
        let key_ok = self.key < 0 || key < 0 || self.key == key;
        id_ok && key_ok
    }
}

/// Ordered note events for one audio block (same insert rules as MIDI).
#[derive(Clone, Debug, Default)]
pub struct NoteBuffer {
    events: Vec<NoteEvent>,
}

impl NoteBuffer {
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

    pub fn push(&mut self, ev: NoteEvent) {
        let off = ev.sample_offset;
        match self.events.binary_search_by_key(&off, |e| e.sample_offset) {
            Ok(mut i) => {
                while i < self.events.len() && self.events[i].sample_offset == off {
                    i += 1;
                }
                self.events.insert(i, ev);
            }
            Err(i) => self.events.insert(i, ev),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = NoteEvent> + '_ {
        self.events.iter().copied()
    }

    pub fn iter_range(&self, start: u32, end: u32) -> impl Iterator<Item = NoteEvent> + '_ {
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

    /// Like [`slice_rebased`](Self::slice_rebased) into an existing buffer (no new `Vec`).
    pub fn copy_range_rebased(&mut self, src: &Self, start: u32, end: u32) {
        self.clear();
        for mut ev in src.iter_range(start, end) {
            ev.sample_offset = ev.sample_offset.saturating_sub(start);
            self.events.push(ev);
        }
    }

    #[must_use]
    pub fn as_slice(&self) -> &[NoteEvent] {
        &self.events
    }
}

/// Mono `PARAM_MOD` (`note_id < 0`) → timed params. Per-note → [`NoteBuffer`].
pub fn route_param_mod(
    timed: &mut Vec<TimedParamEvent>,
    notes: &mut NoteBuffer,
    sample_offset: u32,
    param_id: u32,
    amount: f64,
    target: NoteTarget,
) {
    if target.note_id >= 0 {
        notes.push(NoteEvent {
            sample_offset,
            note_id: target.note_id,
            port_index: target.port_index,
            channel: target.channel,
            key: target.key,
            kind: NoteEventKind::ParamMod { param_id, amount },
        });
    } else {
        timed.push(TimedParamEvent::Mod {
            sample_offset,
            id: param_id,
            amount,
        });
    }
}

/// Mono `PARAM_VALUE` (`note_id < 0`) → timed params. Per-note → [`NoteBuffer`].
pub fn route_param_value(
    timed: &mut Vec<TimedParamEvent>,
    notes: &mut NoteBuffer,
    sample_offset: u32,
    param_id: u32,
    plain: f64,
    target: NoteTarget,
) {
    if target.note_id >= 0 {
        notes.push(NoteEvent {
            sample_offset,
            note_id: target.note_id,
            port_index: target.port_index,
            channel: target.channel,
            key: target.key,
            kind: NoteEventKind::ParamValue { param_id, plain },
        });
    } else {
        timed.push(TimedParamEvent::Value {
            sample_offset,
            id: param_id,
            plain,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poly_mod_does_not_enter_timed_params() {
        let mut timed = Vec::new();
        let mut notes = NoteBuffer::new();
        route_param_mod(
            &mut timed,
            &mut notes,
            8,
            1,
            0.25,
            NoteTarget {
                note_id: 7,
                port_index: 0,
                channel: 0,
                key: 60,
            },
        );
        route_param_mod(&mut timed, &mut notes, 0, 1, 0.1, NoteTarget::UNSPECIFIED);
        assert_eq!(timed.len(), 1);
        assert!(matches!(
            timed[0],
            TimedParamEvent::Mod {
                sample_offset: 0,
                id: 1,
                amount: a
            } if (a - 0.1).abs() < 1e-12
        ));
        assert_eq!(notes.len(), 1);
        let ev = notes.as_slice()[0];
        assert_eq!(ev.note_id, 7);
        assert_eq!(ev.sample_offset, 8);
        assert!(matches!(
            ev.kind,
            NoteEventKind::ParamMod {
                param_id: 1,
                amount: a
            } if (a - 0.25).abs() < 1e-12
        ));
    }

    #[test]
    fn slice_rebases_and_matches_voice() {
        let mut notes = NoteBuffer::new();
        notes.push(NoteEvent::on(10, 1, 60, 0.8));
        notes.push(NoteEvent {
            sample_offset: 20,
            note_id: 1,
            port_index: 0,
            channel: 0,
            key: 60,
            kind: NoteEventKind::Expression {
                id: NoteExpression::Tuning,
                value: 0.5,
            },
        });
        notes.push(NoteEvent::on(40, 2, 64, 0.5));
        let chunk = notes.slice_rebased(10, 30);
        assert_eq!(chunk.len(), 2);
        assert_eq!(chunk.as_slice()[0].sample_offset, 0);
        assert_eq!(chunk.as_slice()[1].sample_offset, 10);
        assert!(chunk.as_slice()[1].matches_voice(1, 60));
        assert!(!chunk.as_slice()[1].matches_voice(2, 64));
    }

    #[test]
    fn copy_range_rebased_reuses_buffer() {
        let mut notes = NoteBuffer::new();
        notes.push(NoteEvent::on(10, 1, 60, 0.8));
        notes.push(NoteEvent::on(40, 2, 64, 0.5));
        let mut dest = NoteBuffer::with_capacity(8);
        dest.copy_range_rebased(&notes, 10, 30);
        assert_eq!(dest.len(), 1);
        assert_eq!(dest.as_slice()[0].sample_offset, 0);
        dest.copy_range_rebased(&notes, 0, 50);
        assert_eq!(dest.len(), 2);
    }
}
