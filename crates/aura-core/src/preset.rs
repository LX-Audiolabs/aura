//! Factory presets and filesystem preset load (CLAP `preset-load` /
//! `preset-discovery`). Same v1 param blob as [`crate::state`].

use std::path::Path;

use aura_params::Params;

use crate::state::decode_state;

/// One bundled factory preset for the host browser.
pub struct FactoryPreset {
    /// Machine key passed back via `preset-load` (`load_key`).
    pub key: &'static str,
    /// Display name in the host preset browser.
    pub name: &'static str,
    pub state: FactoryPresetState,
}

/// How a factory preset stores its parameter values.
pub enum FactoryPresetState {
    /// `(param id, plain value)` pairs — applied with [`Params::restore_values`].
    Values(&'static [(u32, f64)]),
    /// Raw v1 blob ([`crate::encode_state`] layout).
    Blob(&'static [u8]),
}

/// Apply a factory preset onto `params`. Returns `false` if a blob is malformed.
pub fn apply_factory_preset(params: &dyn Params, preset: &FactoryPreset) -> bool {
    match preset.state {
        FactoryPresetState::Values(pairs) => {
            params.restore_values(pairs);
            params.snap_smoothers();
            true
        }
        FactoryPresetState::Blob(blob) => decode_state(params, blob),
    }
}

/// Load a v1 state file from disk. Default [`PluginLogic`] file path.
pub fn load_preset_file(params: &dyn Params, path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if decode_state(params, &bytes) {
        Ok(())
    } else {
        Err("not a valid AURA state blob".into())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use aura_params::{ParamInfo, ParamRange, Params};

    use super::*;
    use crate::state::encode_state;

    struct OneParam {
        v: AtomicU64,
    }

    impl OneParam {
        fn new(v: f64) -> Self {
            Self {
                v: AtomicU64::new(v.to_bits()),
            }
        }
        fn get(&self) -> f64 {
            f64::from_bits(self.v.load(Ordering::Relaxed))
        }
    }

    impl aura_params::__private::Sealed for OneParam {}

    impl Params for OneParam {
        fn param_infos(&self) -> Vec<ParamInfo> {
            vec![ParamInfo {
                id: 1,
                name: "P",
                short_name: "P",
                group: "",
                range: ParamRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
                default_plain: 0.0,
                flags: aura_params::ParamFlags::empty(),
                unit: aura_params::ParamUnit::None,
                kind: aura_params::ParamValueKind::Float,
                midi_map: None,
                midi_channel: None,
            }]
        }
        fn count(&self) -> usize {
            1
        }
        fn get_plain(&self, id: u32) -> Option<f64> {
            (id == 1).then(|| self.get())
        }
        fn get_normalized(&self, id: u32) -> Option<f64> {
            self.get_plain(id).map(|v| (v + 24.0) / 48.0)
        }
        fn set_plain(&self, id: u32, value: f64) {
            if id == 1 {
                self.v.store(value.to_bits(), Ordering::Relaxed);
            }
        }
        fn set_normalized(&self, id: u32, value: f64) {
            self.set_plain(id, value * 48.0 - 24.0);
        }
        fn format_value(&self, _id: u32, value: f64) -> Option<String> {
            Some(format!("{value}"))
        }
        fn parse_value(&self, _id: u32, text: &str) -> Option<f64> {
            text.parse().ok()
        }
        fn snap_smoothers(&self) {}
        fn set_sample_rate(&self, _sample_rate: f64) {}
        fn collect_values(&self) -> (Vec<u32>, Vec<f64>) {
            (vec![1], vec![self.get()])
        }
        fn restore_values(&self, values: &[(u32, f64)]) {
            for (id, v) in values {
                self.set_plain(*id, *v);
            }
        }
    }

    #[test]
    fn values_preset_applies() {
        let p = OneParam::new(0.0);
        let preset = FactoryPreset {
            key: "hot",
            name: "Hot",
            state: FactoryPresetState::Values(&[(1, 6.0)]),
        };
        assert!(apply_factory_preset(&p, &preset));
        assert!((p.get() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn blob_preset_applies() {
        let src = OneParam::new(-3.0);
        let blob = encode_state(&src);
        let leaked: &'static [u8] = Box::leak(blob.into_boxed_slice());
        let dest = OneParam::new(0.0);
        let preset = FactoryPreset {
            key: "blob",
            name: "Blob",
            state: FactoryPresetState::Blob(leaked),
        };
        assert!(apply_factory_preset(&dest, &preset));
        assert!((dest.get() + 3.0).abs() < 1e-9);
    }

    #[test]
    fn file_load_round_trip() {
        let dir = std::env::temp_dir();
        let path = dir.join("aura-preset-load-test.bin");
        let src = OneParam::new(4.5);
        std::fs::write(&path, encode_state(&src)).unwrap();
        let dest = OneParam::new(0.0);
        load_preset_file(&dest, &path).unwrap();
        assert!((dest.get() - 4.5).abs() < 1e-9);
        let _ = std::fs::remove_file(&path);
    }
}
