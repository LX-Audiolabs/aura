//! Acoustic simulation wrappers around the [`goonj`] crate.
//!
//! Requires the `acoustics` feature flag. Provides room simulation reverb,
//! convolution reverb, binaural HRTF processing, FDN reverb, acoustic
//! analysis metrics, and ambisonics encoding.

pub mod ambisonics;
pub mod analysis;
pub mod binaural;
pub mod convolution;
pub mod coupled;
pub mod directivity;
pub mod fdn_reverb;
pub mod room;

use goonj::material::AcousticMaterial;

/// Look up a built-in [`AcousticMaterial`] by name.
///
/// Supports: `"concrete"`, `"carpet"`, `"glass"`, `"wood"`, `"curtain"`,
/// `"drywall"`, `"tile"`. Returns `None` for unknown names.
#[must_use]
fn material_by_name(name: &str) -> Option<AcousticMaterial> {
    match name {
        "concrete" => Some(AcousticMaterial::concrete()),
        "carpet" => Some(AcousticMaterial::carpet()),
        "glass" => Some(AcousticMaterial::glass()),
        "wood" => Some(AcousticMaterial::wood()),
        "curtain" => Some(AcousticMaterial::curtain()),
        "drywall" => Some(AcousticMaterial::drywall()),
        "tile" => Some(AcousticMaterial::tile()),
        _ => None,
    }
}
