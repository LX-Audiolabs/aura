//! Sample-accurate automation / modulation chunking.
//!
//! Host events carry a sample offset (`time`) within the block. For parameters
//! with [`ParamFlags::CHUNKED`](aura_params::ParamFlags::CHUNKED), the format
//! wrapper splits the audio block at those offsets so each `process` call sees
//! a constant set of param targets for its sub-range.
//!
//! Non-`CHUNKED` params apply at block start (last event wins) — no split.

use aura_params::{ParamFlags, ParamInfo, Params};

/// Timed host → plugin parameter event (value or mono modulation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimedParamEvent {
    /// Absolute base value (CLAP `PARAM_VALUE` / host automation).
    Value {
        sample_offset: u32,
        id: u32,
        plain: f64,
    },
    /// Non-destructive modulation offset (CLAP `PARAM_MOD`).
    ///
    /// Effective DSP target = clamp(base + amount). Host UI still shows base.
    Mod {
        sample_offset: u32,
        id: u32,
        amount: f64,
    },
}

impl TimedParamEvent {
    #[must_use]
    pub fn sample_offset(self) -> u32 {
        match self {
            Self::Value { sample_offset, .. } | Self::Mod { sample_offset, .. } => sample_offset,
        }
    }

    #[must_use]
    pub fn param_id(self) -> u32 {
        match self {
            Self::Value { id, .. } | Self::Mod { id, .. } => id,
        }
    }
}

/// Whether this event should force a sub-block split.
///
/// Only `CHUNKED` params split. Unknown ids do not split (apply as non-chunked).
#[must_use]
pub fn is_split_event(event: &TimedParamEvent, infos: &[ParamInfo]) -> bool {
    let id = event.param_id();
    infos
        .iter()
        .find(|i| i.id == id)
        .is_some_and(|i| i.flags.contains(ParamFlags::CHUNKED))
}

/// Sorted unique split points in `0..=frames` (always includes `0` and `frames`).
///
/// Split candidates: sample offsets of [`is_split_event`] events with
/// `0 < t < frames`.
#[must_use]
pub fn split_points(frames: u32, events: &[TimedParamEvent], infos: &[ParamInfo]) -> Vec<u32> {
    let mut pts = Vec::with_capacity(events.len() + 2);
    pts.push(0);
    pts.push(frames);
    for ev in events {
        if !is_split_event(ev, infos) {
            continue;
        }
        let t = ev.sample_offset();
        if t > 0 && t < frames {
            pts.push(t);
        }
    }
    pts.sort_unstable();
    pts.dedup();
    pts
}

/// Apply one event to params (base value or mono mod amount).
pub fn apply_event(params: &dyn Params, event: TimedParamEvent) {
    match event {
        TimedParamEvent::Value { id, plain, .. } => params.set_plain(id, plain),
        TimedParamEvent::Mod { id, amount, .. } => params.set_mod(id, amount),
    }
}

/// Apply all non-split events (non-`CHUNKED` or unknown id) in order.
///
/// Call once at block start. Later events for the same id overwrite earlier ones.
pub fn apply_non_chunked(params: &dyn Params, events: &[TimedParamEvent], infos: &[ParamInfo]) {
    for ev in events {
        if !is_split_event(ev, infos) {
            apply_event(params, *ev);
        }
    }
}

/// Apply all split events whose `sample_offset == time` (chunk boundary).
pub fn apply_at_time(
    params: &dyn Params,
    events: &[TimedParamEvent],
    time: u32,
    infos: &[ParamInfo],
) {
    for ev in events {
        if is_split_event(ev, infos) && ev.sample_offset() == time {
            apply_event(params, *ev);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aura_params::{ParamInfo, ParamRange, ParamUnit};

    fn info(id: u32, chunked: bool) -> ParamInfo {
        ParamInfo {
            id,
            name: "p",
            short_name: "p",
            group: "",
            unit: ParamUnit::None,
            range: ParamRange::Linear { min: 0.0, max: 1.0 },
            default_plain: 0.0,
            flags: if chunked {
                ParamFlags::AUTOMATABLE | ParamFlags::CHUNKED
            } else {
                ParamFlags::AUTOMATABLE
            },
            kind: aura_params::ParamValueKind::Float,
            midi_map: None,
            midi_channel: None,
        }
    }

    #[test]
    fn split_only_chunked_mid_block() {
        let infos = [info(1, true), info(2, false)];
        let events = [
            TimedParamEvent::Value {
                sample_offset: 0,
                id: 1,
                plain: 0.5,
            },
            TimedParamEvent::Value {
                sample_offset: 64,
                id: 1,
                plain: 1.0,
            },
            TimedParamEvent::Value {
                sample_offset: 32,
                id: 2,
                plain: 0.25,
            },
        ];
        assert_eq!(split_points(128, &events, &infos), vec![0, 64, 128]);
    }

    #[test]
    fn no_split_when_nothing_chunked() {
        let infos = [info(1, false)];
        let events = [TimedParamEvent::Value {
            sample_offset: 50,
            id: 1,
            plain: 1.0,
        }];
        assert_eq!(split_points(100, &events, &infos), vec![0, 100]);
    }
}
