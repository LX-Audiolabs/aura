//! Shared plugin state blob — host-agnostic, so every format wrapper
//! (CLAP `clap.state`, VST3 `IComponent::getState`, LV2 state ext)
//! speaks the same byte layout and a session saved via one format
//! restores identically in another.
//!
//! v1 blob: `u32 LE` param count, then per param `u32 LE` id +
//! `u64 LE` f64 bits. Flat by design; versioning lands when the
//! layout ever changes. `#[persist]` UI state is separate
//! (`Params::serialize_persist`) and not part of this blob.

use aura_params::Params;

/// Serialize all param values (own + nested, via
/// [`Params::collect_values`]) into the flat v1 blob.
pub fn encode_state(params: &dyn Params) -> Vec<u8> {
    let (ids, values) = params.collect_values();
    let mut blob = Vec::with_capacity(4 + ids.len() * 12);
    #[allow(clippy::cast_possible_truncation)] // param counts are small
    blob.extend_from_slice(&(ids.len() as u32).to_le_bytes());
    for (id, v) in ids.iter().zip(values.iter()) {
        blob.extend_from_slice(&id.to_le_bytes());
        blob.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    blob
}

/// Restore param values from a v1 blob. Unknown IDs are ignored by
/// [`Params::restore_values`]; a truncated/malformed blob is rejected
/// wholesale (`false`, state untouched). On success smoothers snap to
/// the restored targets so no ramp audibly "catches up".
pub fn decode_state(params: &dyn Params, blob: &[u8]) -> bool {
    let mut cursor = 0usize;
    let mut take = |n: usize| -> Option<&[u8]> {
        let s = blob.get(cursor..cursor + n)?;
        cursor += n;
        Some(s)
    };
    let Some(count) = take(4)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
    else {
        return false;
    };
    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let (Some(id), Some(bits)) = (
            take(4)
                .and_then(|b| b.try_into().ok())
                .map(u32::from_le_bytes),
            take(8)
                .and_then(|b| b.try_into().ok())
                .map(u64::from_le_bytes),
        ) else {
            return false;
        };
        values.push((id, f64::from_bits(bits)));
    }

    params.restore_values(&values);
    params.snap_smoothers();
    true
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use aura_params::{ParamInfo, ParamRange, Params};

    use super::*;

    /// Minimal hand-rolled Params: two linear 0..1 params where
    /// normalized == plain. Just enough to exercise the codec.
    struct TwoParams {
        a: AtomicU64,
        b: AtomicU64,
    }

    impl TwoParams {
        fn new(a: f64, b: f64) -> Self {
            Self {
                a: AtomicU64::new(a.to_bits()),
                b: AtomicU64::new(b.to_bits()),
            }
        }

        fn get(&self, id: u32) -> Option<f64> {
            let bits = match id {
                1 => self.a.load(Ordering::Relaxed),
                2 => self.b.load(Ordering::Relaxed),
                _ => return None,
            };
            Some(f64::from_bits(bits))
        }

        fn set(&self, id: u32, value: f64) {
            let slot = match id {
                1 => &self.a,
                2 => &self.b,
                _ => return,
            };
            slot.store(value.to_bits(), Ordering::Relaxed);
        }
    }

    impl aura_params::__private::Sealed for TwoParams {}

    #[allow(clippy::unused_self)]
    impl Params for TwoParams {
        fn param_infos(&self) -> Vec<ParamInfo> {
            let mk = |id: u32, name: &'static str| ParamInfo {
                id,
                name,
                short_name: name,
                group: "",
                range: ParamRange::Linear { min: 0.0, max: 1.0 },
                default_plain: 0.0,
                flags: aura_params::ParamFlags::empty(),
                unit: aura_params::ParamUnit::None,
                kind: aura_params::ParamValueKind::Float,
                midi_map: None,
                midi_channel: None,
            };
            vec![mk(1, "A"), mk(2, "B")]
        }

        fn count(&self) -> usize {
            2
        }

        fn get_normalized(&self, id: u32) -> Option<f64> {
            self.get(id)
        }

        fn set_normalized(&self, id: u32, value: f64) {
            self.set(id, value);
        }

        fn get_plain(&self, id: u32) -> Option<f64> {
            self.get(id)
        }

        fn set_plain(&self, id: u32, value: f64) {
            self.set(id, value);
        }

        fn format_value(&self, _id: u32, value: f64) -> Option<String> {
            Some(value.to_string())
        }

        fn parse_value(&self, _id: u32, text: &str) -> Option<f64> {
            text.parse().ok()
        }

        fn snap_smoothers(&self) {}

        fn set_sample_rate(&self, _sample_rate: f64) {}

        fn collect_values(&self) -> (Vec<u32>, Vec<f64>) {
            (
                vec![1, 2],
                vec![
                    self.get(1).expect("id 1"),
                    self.get(2).expect("id 2"),
                ],
            )
        }

        fn restore_values(&self, values: &[(u32, f64)]) {
            for (id, v) in values {
                self.set(*id, *v);
            }
        }
    }

    #[test]
    fn round_trip_restores_values() {
        let src = TwoParams::new(0.25, 0.75);
        let blob = encode_state(&src);
        let dst = TwoParams::new(0.0, 0.0);
        assert!(decode_state(&dst, &blob));
        assert_eq!(dst.get_plain(1), Some(0.25));
        assert_eq!(dst.get_plain(2), Some(0.75));
    }

    #[test]
    fn malformed_blob_rejected_without_touching_state() {
        let p = TwoParams::new(0.5, 0.5);
        assert!(!decode_state(&p, &[]));
        assert!(!decode_state(&p, &[1, 0])); // truncated count/entry
        assert_eq!(p.get_plain(1), Some(0.5));
        assert_eq!(p.get_plain(2), Some(0.5));
    }
}
