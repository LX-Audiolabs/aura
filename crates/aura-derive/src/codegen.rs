//! Codegen helpers for `#[derive(Params)]`.
//!
//! Pure functions turning parsed [`crate::parse`] data into
//! `TokenStream2` fragments - range/unit/flags/smoothing literals, the
//! `ParamInfo { ... }` struct literal, field constructors, the
//! `<Struct>ParamId` companion enum, and the `#[persist]` byte codec.
//! [`crate::params::expand`] assembles these into the final derive
//! output.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::parse::{ParamField, ParamKind, PersistScalar, PersistType, PersistWrapper};

/// Emit an `f64` as a decimal literal (`60.0`, `-60.0`) rather than the
/// suffixed-integer form `quote! { #f64 }` produces for whole numbers
/// (`60f64`). rustc accepts `60f64`, but rust-analyzer's proc-macro
/// expansion rejects it with "expected Expr". The decimal form
/// round-trips cleanly and matches the hand-written `0.0` / `1.0`
/// literals the range-less default path emits.
pub(crate) fn f64_lit(v: f64) -> TokenStream2 {
    if v < 0.0 {
        let abs = proc_macro2::Literal::f64_unsuffixed(-v);
        quote! { -#abs }
    } else {
        let lit = proc_macro2::Literal::f64_unsuffixed(v);
        quote! { #lit }
    }
}

/// Like [`f64_lit`] for `i64` - emits `12` / `-12`, not `12i64`.
pub(crate) fn i64_lit(v: i64) -> TokenStream2 {
    let abs = proc_macro2::Literal::u64_unsuffixed(v.unsigned_abs());
    if v < 0 {
        quote! { -#abs }
    } else {
        quote! { #abs }
    }
}

/// Like [`f64_lit`] for `usize` - emits `2`, not `2usize`.
pub(crate) fn usize_lit(v: usize) -> TokenStream2 {
    let lit = proc_macro2::Literal::usize_unsuffixed(v);
    quote! { #lit }
}

/// Parse the two power-law taper shapes (`skewed(min, max, factor)` and
/// `sym_skewed(min, max, factor, center)`). `None` when neither prefix
/// matches; `Some(compile_error!)` on a malformed match. Split out of
/// [`parse_range_tokens`] to keep that function under the line-length lint.
#[allow(clippy::too_many_lines)]
pub(crate) fn parse_skew_range(range: &str) -> Option<TokenStream2> {
    let bad = |msg: String| quote! { compile_error!(#msg) };
    // Reject non-positive and NaN factors without a `!(x > 0.0)` (which
    // clippy flags on partially-ordered floats).
    let bad_factor = |factor: f64| factor <= 0.0 || factor.is_nan();

    if let Some(inner) = range
        .strip_prefix("skewed(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() != 3 {
            return Some(bad(format!(
                "skewed range needs three arguments `skewed(min, max, factor)`, got `skewed({inner})`"
            )));
        }
        let Ok(min) = parts[0].parse::<f64>() else {
            return Some(bad(format!(
                "skewed range min `{}` is not a number",
                parts[0]
            )));
        };
        let Ok(max) = parts[1].parse::<f64>() else {
            return Some(bad(format!(
                "skewed range max `{}` is not a number",
                parts[1]
            )));
        };
        let Ok(factor) = parts[2].parse::<f64>() else {
            return Some(bad(format!(
                "skewed range factor `{}` is not a number",
                parts[2]
            )));
        };
        if min >= max {
            return Some(bad(format!(
                "skewed range needs min < max, got `skewed({min}, {max}, {factor})`"
            )));
        }
        if bad_factor(factor) {
            return Some(bad(format!(
                "skewed range needs a strictly positive factor, got `{factor}`"
            )));
        }
        let (min, max, factor) = (f64_lit(min), f64_lit(max), f64_lit(factor));
        return Some(quote! {
            ::aura::params::ParamRange::Skewed { min: #min, max: #max, factor: #factor }
        });
    }
    if let Some(inner) = range
        .strip_prefix("sym_skewed(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() != 4 {
            return Some(bad(format!(
                "sym_skewed range needs four arguments `sym_skewed(min, max, factor, center)`, \
                 got `sym_skewed({inner})`"
            )));
        }
        let Ok(min) = parts[0].parse::<f64>() else {
            return Some(bad(format!(
                "sym_skewed range min `{}` is not a number",
                parts[0]
            )));
        };
        let Ok(max) = parts[1].parse::<f64>() else {
            return Some(bad(format!(
                "sym_skewed range max `{}` is not a number",
                parts[1]
            )));
        };
        let Ok(factor) = parts[2].parse::<f64>() else {
            return Some(bad(format!(
                "sym_skewed range factor `{}` is not a number",
                parts[2]
            )));
        };
        let Ok(center) = parts[3].parse::<f64>() else {
            return Some(bad(format!(
                "sym_skewed range center `{}` is not a number",
                parts[3]
            )));
        };
        if min >= max {
            return Some(bad(format!(
                "sym_skewed range needs min < max, got `sym_skewed({min}, {max}, {factor}, {center})`"
            )));
        }
        if bad_factor(factor) {
            return Some(bad(format!(
                "sym_skewed range needs a strictly positive factor, got `{factor}`"
            )));
        }
        if center <= min || center >= max {
            return Some(bad(format!(
                "sym_skewed center must be strictly between min and max, got center={center} \
                 for [{min}, {max}]"
            )));
        }
        let (min, max, factor, center) =
            (f64_lit(min), f64_lit(max), f64_lit(factor), f64_lit(center));
        return Some(quote! {
            ::aura::params::ParamRange::SymmetricalSkewed {
                min: #min, max: #max, factor: #factor, center: #center
            }
        });
    }
    None
}

#[allow(clippy::too_many_lines)]
pub(crate) fn parse_range_tokens(range: &str) -> TokenStream2 {
    let bad = |msg: String| quote! { compile_error!(#msg) };

    if let Some(inner) = range
        .strip_prefix("linear(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() != 2 {
            return bad(format!(
                "linear range needs two arguments `linear(min, max)`, got `linear({inner})`"
            ));
        }
        let Ok(min) = parts[0].parse::<f64>() else {
            return bad(format!("linear range min `{}` is not a number", parts[0]));
        };
        let Ok(max) = parts[1].parse::<f64>() else {
            return bad(format!("linear range max `{}` is not a number", parts[1]));
        };
        if min >= max {
            return bad(format!(
                "linear range needs min < max, got `linear({min}, {max})`"
            ));
        }
        let (min, max) = (f64_lit(min), f64_lit(max));
        return quote! { ::aura::params::ParamRange::Linear { min: #min, max: #max } };
    }
    if let Some(inner) = range.strip_prefix("log(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() != 2 {
            return bad(format!(
                "log range needs two arguments `log(min, max)`, got `log({inner})`"
            ));
        }
        let Ok(min) = parts[0].parse::<f64>() else {
            return bad(format!("log range min `{}` is not a number", parts[0]));
        };
        let Ok(max) = parts[1].parse::<f64>() else {
            return bad(format!("log range max `{}` is not a number", parts[1]));
        };
        if min <= 0.0 || max <= 0.0 {
            return bad(format!(
                "log range needs strictly positive bounds, got `log({min}, {max})`"
            ));
        }
        if min >= max {
            return bad(format!(
                "log range needs min < max, got `log({min}, {max})`"
            ));
        }
        let (min, max) = (f64_lit(min), f64_lit(max));
        return quote! { ::aura::params::ParamRange::Logarithmic { min: #min, max: #max } };
    }
    if let Some(inner) = range
        .strip_prefix("discrete(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() != 2 {
            return bad(format!(
                "discrete range needs two arguments `discrete(min, max)`, got `discrete({inner})`"
            ));
        }
        let Ok(min) = parts[0].parse::<i64>() else {
            return bad(format!(
                "discrete range min `{}` is not an integer",
                parts[0]
            ));
        };
        let Ok(max) = parts[1].parse::<i64>() else {
            return bad(format!(
                "discrete range max `{}` is not an integer",
                parts[1]
            ));
        };
        if min >= max {
            return bad(format!(
                "discrete range needs min < max, got `discrete({min}, {max})`"
            ));
        }
        let (min, max) = (i64_lit(min), i64_lit(max));
        return quote! { ::aura::params::ParamRange::Discrete { min: #min, max: #max } };
    }
    if let Some(inner) = range
        .strip_prefix("enum(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let Ok(count) = inner.trim().parse::<usize>() else {
            return bad(format!(
                "enum count `{}` is not a non-negative integer",
                inner.trim()
            ));
        };
        if count < 2 {
            return bad(format!(
                "enum range needs at least 2 variants, got `enum({count})`"
            ));
        }
        let count = usize_lit(count);
        return quote! { ::aura::params::ParamRange::Enum { count: #count } };
    }
    if let Some(tokens) = parse_skew_range(range) {
        return tokens;
    }
    if let Some(inner) = range
        .strip_prefix("reversed(")
        .and_then(|s| s.strip_suffix(')'))
    {
        // Recursively parse the wrapped shape and flip it. The inner is a
        // const expression, so `&` promotes it to the `&'static` the
        // `Reversed` variant holds.
        let inner_tokens = parse_range_tokens(inner.trim());
        return quote! { ::aura::params::ParamRange::Reversed(&#inner_tokens) };
    }
    bad(format!(
        "unknown range `{range}` - supported: linear(min, max), log(min, max), \
         skewed(min, max, factor), sym_skewed(min, max, factor, center), \
         discrete(min, max), enum(count), reversed(<range>)"
    ))
}

/// The concrete `(min, max)` a range string spans, for expansion-time
/// default validation. `None` when the bounds aren't statically known -
/// a rangeless param, or an `enum(N)` whose count comes from the
/// enum type's `variant_count()` rather than a literal.
pub(crate) fn range_bounds(range: &str) -> Option<(f64, f64)> {
    let range = range.trim();
    if let Some(inner) = range
        .strip_prefix("reversed(")
        .and_then(|s| s.strip_suffix(')'))
    {
        // Reversal flips only the mapping; the numeric bounds are the same.
        return range_bounds(inner);
    }
    let leading_pair = |inner: &str| -> Option<(f64, f64)> {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        let lo = parts.first()?.parse::<f64>().ok()?;
        let hi = parts.get(1)?.parse::<f64>().ok()?;
        Some((lo.min(hi), lo.max(hi)))
    };
    for prefix in ["linear(", "log(", "skewed(", "sym_skewed("] {
        if let Some(inner) = range.strip_prefix(prefix).and_then(|s| s.strip_suffix(')')) {
            return leading_pair(inner);
        }
    }
    if let Some(inner) = range
        .strip_prefix("discrete(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        let lo = parts.first()?.parse::<i64>().ok()?;
        let hi = parts.get(1)?.parse::<i64>().ok()?;
        #[allow(clippy::cast_precision_loss)]
        return Some((lo.min(hi) as f64, lo.max(hi) as f64));
    }
    if let Some(inner) = range
        .strip_prefix("enum(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let count = inner.trim().parse::<u32>().ok()?;
        return (count >= 1).then(|| (0.0, f64::from(count - 1)));
    }
    None
}

/// Compile-time range-containment check for a field's default. `Some`
/// message when the default (explicit, or the omitted `0.0` fallback)
/// sits outside the declared range - which panics at instantiation for
/// Int / Enum / Bool and ships mis-displayed DSP for Float. `None` for
/// shapes whose bounds aren't statically known.
pub(crate) fn default_range_error(f: &ParamField) -> Option<String> {
    let a = &f.attrs;
    let (lo, hi) = range_bounds(a.range.as_deref()?)?;
    let effective = a.default.unwrap_or(0.0);
    if !effective.is_finite() || (effective >= lo && effective <= hi) {
        return None;
    }
    let name = a.name.as_deref().unwrap_or("Unnamed");
    Some(if a.default.is_some() {
        format!("`{name}` default {effective} is outside its range [{lo}, {hi}]")
    } else {
        format!(
            "`{name}` has no `default` and its range [{lo}, {hi}] doesn't contain 0.0 - \
             add `default = <value in range>`"
        )
    })
}

/// Parse a unit string into `ParamUnit` tokens.
pub(crate) fn parse_unit_tokens(unit: &str) -> TokenStream2 {
    match unit {
        "dB" | "Db" | "db" => quote! { ::aura::params::ParamUnit::Db },
        "Hz" | "hz" => quote! { ::aura::params::ParamUnit::Hz },
        "ms" => quote! { ::aura::params::ParamUnit::Milliseconds },
        "s" => quote! { ::aura::params::ParamUnit::Seconds },
        "%" => quote! { ::aura::params::ParamUnit::Percent },
        "st" => quote! { ::aura::params::ParamUnit::Semitones },
        "pan" => quote! { ::aura::params::ParamUnit::Pan },
        "deg" | "°" => quote! { ::aura::params::ParamUnit::Degrees },
        "" | "none" => quote! { ::aura::params::ParamUnit::None },
        // Loud compile-error rather than silent fallback - typos like
        // `"hz "` (trailing space) or `"DB"` (uppercase) shouldn't map
        // to `ParamUnit::None` and surface only as "0.5" instead of
        // "0.5 Hz" in the host.
        other => {
            let msg =
                format!("unknown unit `{other}` - supported: dB, Hz, ms, s, %, st, pan, deg, none");
            quote! { compile_error!(#msg) }
        }
    }
}

/// Parse a flags string into `ParamFlags` tokens.
pub(crate) fn parse_flags_tokens(flags: &str) -> TokenStream2 {
    let mut parts = Vec::new();
    for flag in flags.split('|').map(|s| s.trim().to_lowercase()) {
        match flag.as_str() {
            "automatable" => parts.push(quote! { ::aura::params::ParamFlags::AUTOMATABLE }),
            "hidden" => parts.push(quote! { ::aura::params::ParamFlags::HIDDEN }),
            "readonly" => parts.push(quote! { ::aura::params::ParamFlags::READONLY }),
            "bypass" => parts.push(quote! { ::aura::params::ParamFlags::IS_BYPASS }),
            "modulatable" => parts.push(quote! { ::aura::params::ParamFlags::MODULATABLE }),
            // Per-note modulation implies mono modulation.
            "modulatable_per_note" => parts.push(quote! {
                ::aura::params::ParamFlags::MODULATABLE
                    | ::aura::params::ParamFlags::MODULATABLE_PER_NOTE
            }),
            // Tolerate empty segments (a trailing `|`); reject typos loudly
            // like every sibling parser, so a `read_only` -> `readonly`
            // slip can't silently ship an automatable "read-only" param.
            "" => {}
            other => {
                let msg = format!(
                    "unknown param flag `{other}` - supported: automatable, hidden, \
                     readonly, bypass, modulatable, modulatable_per_note",
                );
                return quote! { compile_error!(#msg) };
            }
        }
    }
    if parts.is_empty() {
        quote! { ::aura::params::ParamFlags::AUTOMATABLE }
    } else {
        quote! { #(#parts)|* }
    }
}

/// Parse a smoothing string into `SmoothingStyle` tokens. Same
/// loud-on-malformed contract as `parse_unit_tokens` /
/// `parse_range_tokens`: every typo emits a `compile_error!` instead
/// of silently swallowing the bad value.
pub(crate) fn parse_smooth_tokens(smooth: &str) -> TokenStream2 {
    let bad = |msg: String| quote! { compile_error!(#msg) };
    if smooth == "none" {
        return quote! { ::aura::params::SmoothingStyle::None };
    }
    if let Some(inner) = smooth
        .strip_prefix("linear(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return match inner.trim().parse::<f64>() {
            Ok(ms) => {
                let ms = f64_lit(ms);
                quote! { ::aura::params::SmoothingStyle::Linear(#ms) }
            }
            Err(_) => bad(format!(
                "smooth = \"linear({inner})\" expects a numeric milliseconds value \
                 (e.g. `smooth = \"linear(20)\"`)",
            )),
        };
    }
    if let Some(inner) = smooth
        .strip_prefix("exp(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return match inner.trim().parse::<f64>() {
            Ok(ms) => {
                let ms = f64_lit(ms);
                quote! { ::aura::params::SmoothingStyle::Exponential(#ms) }
            }
            Err(_) => bad(format!(
                "smooth = \"exp({inner})\" expects a numeric milliseconds value \
                 (e.g. `smooth = \"exp(5)\"`)",
            )),
        };
    }
    if let Some(inner) = smooth
        .strip_prefix("log(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return match inner.trim().parse::<f64>() {
            Ok(ms) => {
                let ms = f64_lit(ms);
                quote! { ::aura::params::SmoothingStyle::Logarithmic(#ms) }
            }
            Err(_) => bad(format!(
                "smooth = \"log({inner})\" expects a numeric milliseconds value \
                 (e.g. `smooth = \"log(20)\"` for multiplicative smoothing)",
            )),
        };
    }
    bad(format!(
        "unknown smoothing style `{smooth}` - supported: \"none\", \"linear(<ms>)\", \
         \"exp(<ms>)\", \"log(<ms>)\"",
    ))
}

/// Build the `ParamInfo { ... }` literal for a `#[param(...)]` field.
///
/// Shared between [`gen_field_constructor`] (which wraps it in a
/// `FloatParam`/`BoolParam`/etc. constructor at runtime) and the
/// derive's static-metadata path
/// ([`Params::param_infos_static`](aura_params::Params::param_infos_static)),
/// which lifts the same literal into a `LazyLock<Vec<ParamInfo>>` so
/// format wrappers can read parameter metadata without constructing a
/// plugin instance. Returns `None` when a compile-time validation
/// (non-integer Int/Enum default etc.) failed; the caller's
/// `compile_error!` path handles that branch.
pub(crate) fn gen_param_info_literal(f: &ParamField) -> Option<TokenStream2> {
    let a = &f.attrs;
    let id = f.id();
    let name = a.name.as_deref().unwrap_or("Unnamed");
    let short_name = a.short_name.as_deref().unwrap_or(name);
    let group = a.group.as_deref().unwrap_or("");
    let default_plain: TokenStream2 = if let Some(value) = a.default {
        f64_lit(value)
    } else {
        quote! { 0.0 }
    };

    if let Some(d) = a.default {
        // Integer round-trip exactness checks - an epsilon-based
        // comparison would silently accept fractional defaults like
        // `2.5` for an `Int` / `Enum` param. The `as i64` / `as u32`
        // truncations are the round-trip's whole point.
        #[allow(
            clippy::float_cmp,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let invalid = match f.kind {
            ParamKind::Bool => d != 0.0 && d != 1.0,
            ParamKind::Int => !d.is_finite() || (d as i64 as f64) != d,
            ParamKind::Enum => !d.is_finite() || d < 0.0 || f64::from(d as u32) != d,
            ParamKind::Float => !d.is_finite(),
        };
        if invalid {
            return None;
        }
    }

    let range = match &a.range {
        Some(r) => parse_range_tokens(r),
        None => match f.kind {
            ParamKind::Bool => quote! { ::aura::params::ParamRange::Discrete { min: 0, max: 1 } },
            ParamKind::Enum => {
                if let Some(ref enum_ty) = f.enum_type {
                    quote! { ::aura::params::ParamRange::Enum { count: <#enum_ty as ::aura::params::ParamEnum>::variant_count() } }
                } else {
                    quote! { ::aura::params::ParamRange::Enum { count: 2 } }
                }
            }
            _ => quote! { ::aura::params::ParamRange::Linear { min: 0.0, max: 1.0 } },
        },
    };

    let unit = if let Some(u) = &a.unit {
        parse_unit_tokens(u)
    } else {
        quote! { ::aura::params::ParamUnit::None }
    };

    // The explicit-flags path lets a plugin pass `flags = "hidden |
    // bypass"` to override AUTOMATABLE; OR in CHUNKED on the default
    // path (and on the explicit path unless the plugin opted out via
    // `chunk = false`) so the wrapper-side chunker treats every param
    // as a split point by default.
    let base_flags = if let Some(fl) = &a.flags {
        parse_flags_tokens(fl)
    } else {
        quote! { ::aura::params::ParamFlags::AUTOMATABLE }
    };
    let flags = if a.chunk.unwrap_or(true) {
        quote! { (#base_flags).union(::aura::params::ParamFlags::CHUNKED) }
    } else {
        base_flags
    };

    let kind = match f.kind {
        ParamKind::Float => quote! { ::aura::params::ParamValueKind::Float },
        ParamKind::Int => quote! { ::aura::params::ParamValueKind::Int },
        ParamKind::Bool => quote! { ::aura::params::ParamValueKind::Bool },
        ParamKind::Enum => quote! { ::aura::params::ParamValueKind::Enum },
    };

    let midi_map = a.midi_map.map_or_else(
        || quote! { None },
        |k| {
            let src = k.to_tokens();
            quote! { Some(#src) }
        },
    );
    let midi_channel = a.midi_channel.map_or_else(
        || quote! { None },
        |ch| {
            let lit = proc_macro2::Literal::u8_unsuffixed(ch);
            quote! { Some(#lit) }
        },
    );

    Some(quote! {
        ::aura::params::ParamInfo {
            id: #id,
            name: #name,
            short_name: #short_name,
            group: #group,
            range: #range,
            default_plain: #default_plain,
            flags: #flags,
            unit: #unit,
            kind: #kind,
            midi_map: #midi_map,
            midi_channel: #midi_channel,
        }
    })
}

/// Generate a constructor call for a field with `#[param(...)]` attributes.
///
/// `f.id()` carries the `expect`-guarded "must run after validation"
/// invariant; using it here surfaces the order-of-call contract at
/// construction time instead of silently minting `id = 0`.
pub(crate) fn gen_field_constructor(f: &ParamField) -> TokenStream2 {
    let a = &f.attrs;
    let name = a.name.as_deref().unwrap_or("Unnamed");

    // Compile-time sanity check on `default = ...`. Surfaces user
    // errors that would otherwise silently saturate at runtime (`as
    // u32` on a negative `default_plain`, `as i64` on a fractional
    // value). The variant-count range check for `EnumParam` still
    // runs at construction time because `variant_count()` isn't
    // visible to the macro at expansion time without per-call
    // const-eval plumbing.
    if let Some(d) = a.default {
        // Integer round-trip exactness checks - an epsilon-based
        // comparison would silently accept fractional defaults like
        // `2.5` for an `Int` / `Enum` param. The `as i64` / `as u32`
        // truncations are the round-trip's whole point.
        #[allow(
            clippy::float_cmp,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let err = match f.kind {
            ParamKind::Bool if d != 0.0 && d != 1.0 => {
                Some(format!("BoolParam default {name} must be 0 or 1; got {d}"))
            }
            ParamKind::Int if !d.is_finite() || (d as i64 as f64) != d => Some(format!(
                "IntParam '{name}' default must be an integer literal; got {d}"
            )),
            ParamKind::Enum if !d.is_finite() || d < 0.0 || f64::from(d as u32) != d => {
                Some(format!(
                    "EnumParam '{name}' default must be a non-negative integer (variant index); got {d}"
                ))
            }
            ParamKind::Float if !d.is_finite() => Some(format!(
                "FloatParam '{name}' default must be finite; got {d}"
            )),
            _ => None,
        };
        if let Some(msg) = err {
            return quote! { compile_error!(#msg) };
        }
    }

    // Range containment - lands in `field: <expr>` position, so no `;`.
    if let Some(msg) = default_range_error(f) {
        return quote! { compile_error!(#msg) };
    }

    let Some(info) = gen_param_info_literal(f) else {
        // Validation block above already returned a `compile_error!`
        // for every shape that `gen_param_info_literal` rejects.
        // Surface a fallback diagnostic so a future divergence
        // between the two checks fails loudly instead of silently
        // emitting bad code.
        let msg = format!("invalid `#[param]` attributes on field `{name}`");
        return quote! { compile_error!(#msg) };
    };

    match f.kind {
        ParamKind::Float => {
            let smooth = if let Some(s) = &a.smooth {
                parse_smooth_tokens(s)
            } else {
                quote! { ::aura::params::SmoothingStyle::None }
            };
            quote! { ::aura::params::FloatParam::new(#info, #smooth) }
        }
        ParamKind::Bool => quote! { ::aura::params::BoolParam::new(#info) },
        ParamKind::Int => quote! { ::aura::params::IntParam::new(#info) },
        ParamKind::Enum => quote! { ::aura::params::EnumParam::new(#info) },
    }
}

/// `gain_db` / `r#type` → `GainDb` / `Type` for `*ParamId` variant names.
pub(crate) fn snake_to_pascal(ident: &str) -> String {
    let ident = ident.strip_prefix("r#").unwrap_or(ident);
    ident
        .split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

/// Generate the `<Struct>ParamId` companion enum: one unit variant per
/// own param field (`PascalCase` of the field name), mapped to the
/// explicit `id = N` via `id()` / `from_id()`. Nested structs emit
/// their own enum through their own derive; meter slots are excluded
/// (auto-assigned IDs, not editor-facing params).
///
/// Call only after the explicit-`id` validation in `derive_params` has
/// run (the variants need `ParamField::id`).
pub(crate) fn gen_param_id_enum(struct_name: &syn::Ident, param_fields: &[ParamField]) -> TokenStream2 {
    if param_fields.is_empty() {
        return TokenStream2::new();
    }
    let enum_name = quote::format_ident!("{struct_name}ParamId");
    let variants: Vec<syn::Ident> = param_fields
        .iter()
        .map(|f| quote::format_ident!("{}", snake_to_pascal(&f.ident.to_string())))
        .collect();
    let ids: Vec<u32> = param_fields.iter().map(ParamField::id).collect();

    quote! {
        /// Parameter IDs of
        #[doc = concat!("`", stringify!(#struct_name), "`")]
        /// — one variant per `#[param(id = N, ...)]` field, generated by
        /// `#[derive(Params)]`. Stable by construction: each variant
        /// carries the explicit ID, field reordering can't renumber.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum #enum_name {
            #(#variants),*
        }

        impl #enum_name {
            /// The wire ID declared via `#[param(id = N, ...)]`.
            #[must_use]
            pub const fn id(self) -> u32 {
                match self {
                    #(Self::#variants => #ids),*
                }
            }

            /// Map a wire ID back to a variant, if it belongs to this struct.
            #[must_use]
            pub fn from_id(id: u32) -> Option<Self> {
                match id {
                    #(#ids => Some(Self::#variants),)*
                    _ => None,
                }
            }
        }

        impl From<#enum_name> for u32 {
            fn from(p: #enum_name) -> u32 {
                p.id()
            }
        }
    }
}

/// Generate the value-append statement for a `#[persist]` field
/// (writes the field's current value onto `__buf`).
pub(crate) fn gen_persist_write(ident: &syn::Ident, ty: &PersistType) -> TokenStream2 {
    let scalar = ty.inner;
    // Read access through the wrapper. Poisoned locks recover the
    // inner value - a poisoned lock must not wedge host state save.
    let read = match ty.wrapper {
        PersistWrapper::RwLock => {
            quote! { *self.#ident.read().unwrap_or_else(|e| e.into_inner()) }
        }
        PersistWrapper::Mutex => {
            quote! { *self.#ident.lock().unwrap_or_else(|e| e.into_inner()) }
        }
    };
    if scalar == PersistScalar::String {
        // Borrow through the guard (no deref - `String` isn't `Copy`).
        let read_ref = match ty.wrapper {
            PersistWrapper::RwLock => {
                quote! { self.#ident.read().unwrap_or_else(|e| e.into_inner()) }
            }
            PersistWrapper::Mutex => {
                quote! { self.#ident.lock().unwrap_or_else(|e| e.into_inner()) }
            }
        };
        return quote! { __buf.extend_from_slice(#read_ref.as_bytes()); };
    }
    if scalar == PersistScalar::Bool {
        return quote! { __buf.push(u8::from(#read)); };
    }
    let ty_tokens = scalar.type_tokens();
    quote! { __buf.extend_from_slice(&#ty_tokens::to_le_bytes(#read)); }
}

/// Generate the value-restore statement for a `#[persist]` field
/// (parses `__value: &[u8]` and stores it into the field; a malformed
/// slice leaves the field untouched).
pub(crate) fn gen_persist_read(ident: &syn::Ident, ty: &PersistType) -> TokenStream2 {
    let scalar = ty.inner;
    let store = |value: TokenStream2| match ty.wrapper {
        PersistWrapper::RwLock => {
            quote! { *self.#ident.write().unwrap_or_else(|e| e.into_inner()) = #value; }
        }
        PersistWrapper::Mutex => {
            quote! { *self.#ident.lock().unwrap_or_else(|e| e.into_inner()) = #value; }
        }
    };
    if scalar == PersistScalar::String {
        let s = store(quote! { __s.to_owned() });
        return quote! {
            if let Ok(__s) = ::std::str::from_utf8(__value) {
                #s
            }
        };
    }
    if scalar == PersistScalar::Bool {
        let b = store(quote! { *__b != 0 });
        return quote! {
            if let [__b] = __value {
                #b
            }
        };
    }
    let ty_tokens = scalar.type_tokens();
    let width = scalar.byte_width().expect("numeric scalars have a width");
    let v = store(quote! {
        #ty_tokens::from_le_bytes(__arr)
    });
    quote! {
        if let Ok(__arr) = <[u8; #width]>::try_from(__value) {
            #v
        }
    }
}
