//! Proc macros for AURA plugins.
//!
//! - [`#[derive(Params)`](macro@Params) generates the full
//!   `aura_params::Params` impl for a struct of `FloatParam` /
//!   `IntParam` / `BoolParam` / `EnumParam<E>` / `MeterSlot` fields,
//!   plus `#[nested]` sub-structs and `#[persist]` plain-data fields.
//!   It also emits a `<Struct>ParamId` companion enum (one variant per
//!   own param field) so editors can use `GainParamsParamId::Gain.id()`
//!   instead of scattering raw `u32` IDs.
//! - [`#[derive(ParamEnum)`](macro@ParamEnum) generates the
//!   `aura_params::ParamEnum` impl for a field-less enum.
//!
//! Selective port of `truce-derive` (MIT/Apache-2.0) adapted to the
//! `aura-params` trait surface. Dropped from the reference: the
//! `plugin_info!` / `truce.toml` machinery, the LV2 metadata sidecars,
//! `#[derive(State)]`, ID auto-assignment schemes (AURA params pin an
//! explicit `id = N`), and nested-ID rebasing (AURA nested structs keep
//! their declared IDs; collisions panic at construction via
//! `Params::assert_no_id_collisions`).

#![forbid(unsafe_code)]

use std::collections::HashSet;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Expr, Fields, Lit, Type, TypePath, UnOp};

use aura_params::METER_ID_BASE;

/// Recognized parameter field types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParamKind {
    Float,
    Bool,
    Int,
    Enum,
}

/// A parsed parameter field from the input struct.
struct ParamField {
    ident: syn::Ident,
    kind: ParamKind,
    attrs: ParamAttrs,
    /// For `EnumParam<T>`, the inner type `T`.
    enum_type: Option<syn::Type>,
}

impl ParamField {
    /// ID that the explicit-`id` validation in `derive_params` has
    /// guaranteed is populated. Calling this earlier is a logic error.
    fn id(&self) -> u32 {
        self.attrs
            .id
            .expect("ParamField::id called before the id validation ran")
    }
}

/// A nested Params field (delegates to the inner struct).
struct NestedField {
    ident: syn::Ident,
    /// Field type, retained so the derive can call associated functions
    /// on it without an instance - specifically
    /// `Params::param_infos_static` for the registration-time
    /// "no temp plugin" path.
    ty: syn::Type,
}

/// A meter slot field.
struct MeterField {
    ident: syn::Ident,
    id: Option<u32>,
}

impl MeterField {
    /// ID that the auto-assignment block has guaranteed is populated.
    /// Same invariant as [`ParamField::id`].
    fn id(&self) -> u32 {
        self.id
            .expect("MeterField::id called before the auto-assignment block ran")
    }
}

/// Derive-side mirror of `aura_params::MidiSource`, used to validate
/// bindings across params and emit the `ParamInfo::midi_map` tokens.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MidiBindKind {
    Cc(u8),
    PitchBend,
    ChannelPressure,
    ProgramChange,
}

const MIDI_BOTH_SET: &str = "set only one of `midi_cc` / `midi_source` on a parameter";

impl MidiBindKind {
    /// The `Some(::aura::params::MidiSource::…)` tokens for a binding,
    /// or `None`. CC values are emitted unsuffixed (`Cc(74)`) to keep
    /// the macro output rust-analyzer-friendly.
    fn to_tokens(self) -> TokenStream2 {
        match self {
            Self::Cc(n) => {
                let lit = proc_macro2::Literal::u8_unsuffixed(n);
                quote! { ::aura::params::MidiSource::Cc(#lit) }
            }
            Self::PitchBend => quote! { ::aura::params::MidiSource::PitchBend },
            Self::ChannelPressure => quote! { ::aura::params::MidiSource::ChannelPressure },
            Self::ProgramChange => quote! { ::aura::params::MidiSource::ProgramChange },
        }
    }
}

/// Parsed `#[param(...)]` attributes.
#[derive(Default)]
struct ParamAttrs {
    id: Option<u32>,
    name: Option<String>,
    short_name: Option<String>,
    group: Option<String>,
    range: Option<String>,
    default: Option<f64>,
    unit: Option<String>,
    flags: Option<String>,
    /// Set by `#[param(chunk = false)]` on parameters too expensive to
    /// retarget mid-block (FFT sizes, lookahead, etc.). `None` means
    /// "use the default" (`true`). Drives the `ParamFlags::CHUNKED`
    /// bit in `gen_param_info_literal`.
    chunk: Option<bool>,
    /// Default host MIDI-learn binding source, set by `midi_cc = N`
    /// or `midi_source = "pitchbend" | "pressure" | "program"` (at
    /// most one). Baked onto `ParamInfo::midi_map`.
    midi_map: Option<MidiBindKind>,
    /// Channel scope for `midi_map`, stored as the wire channel
    /// `0..=15` (the attribute is the user-facing `1..=16`).
    midi_channel: Option<u8>,
    smooth: Option<String>,
    format_fn: Option<String>,
    parse_fn: Option<String>,
    /// Compile-error tokens collected during parsing - emitted by the
    /// derive output so unknown keys and unexpected literal kinds
    /// surface at compile time instead of as silent default values.
    errors: Vec<TokenStream2>,
}

fn type_last_segment(ty: &Type) -> Option<String> {
    let Type::Path(TypePath { path, .. }) = ty else {
        return None;
    };
    path.segments.last().map(|seg| seg.ident.to_string())
}

/// Extract the generic type argument from `EnumParam<T>`.
fn extract_enum_type_arg(ty: &Type) -> Option<syn::Type> {
    if let Type::Path(TypePath { path, .. }) = ty {
        let seg = path.segments.last()?;
        if seg.ident == "EnumParam"
            && let syn::PathArguments::AngleBracketed(args) = &seg.arguments
            && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
        {
            return Some(inner.clone());
        }
    }
    None
}

fn classify_param_type(ty: &Type) -> Option<ParamKind> {
    let name = type_last_segment(ty)?;
    match name.as_str() {
        "FloatParam" => Some(ParamKind::Float),
        "BoolParam" => Some(ParamKind::Bool),
        "IntParam" => Some(ParamKind::Int),
        "EnumParam" => Some(ParamKind::Enum),
        _ => None,
    }
}

/// Parse the `midi_cc` / `midi_source` / `midi_channel` keys. Split out
/// of [`parse_param_attrs`]'s loop to keep it under the line limit;
/// errors are pushed onto `attrs.errors` like the other arms.
fn parse_midi_attr(
    key: &str,
    meta: &syn::meta::ParseNestedMeta,
    attrs: &mut ParamAttrs,
) -> syn::Result<()> {
    let push = |attrs: &mut ParamAttrs, e: syn::Error| attrs.errors.push(e.to_compile_error());
    let value: Lit = meta.value()?.parse()?;
    match (key, value) {
        ("midi_cc", Lit::Int(lit)) => match lit.base10_parse::<u16>() {
            Ok(cc) if cc <= 127 => {
                let cc = u8::try_from(cc).expect("checked <= 127");
                if attrs.midi_map.is_some() {
                    push(attrs, meta.error(MIDI_BOTH_SET));
                } else {
                    attrs.midi_map = Some(MidiBindKind::Cc(cc));
                }
            }
            _ => push(
                attrs,
                syn::Error::new_spanned(lit, "`#[param(midi_cc = …)]` must be 0..=127"),
            ),
        },
        ("midi_source", Lit::Str(s)) => {
            let kind = match s.value().as_str() {
                "pitchbend" => Some(MidiBindKind::PitchBend),
                "pressure" => Some(MidiBindKind::ChannelPressure),
                "program" => Some(MidiBindKind::ProgramChange),
                _ => None,
            };
            match kind {
                _ if attrs.midi_map.is_some() => push(attrs, meta.error(MIDI_BOTH_SET)),
                Some(k) => attrs.midi_map = Some(k),
                None => push(
                    attrs,
                    syn::Error::new_spanned(
                        s,
                        "`#[param(midi_source = …)]` expects \"pitchbend\", \"pressure\", \
                         or \"program\"",
                    ),
                ),
            }
        }
        ("midi_channel", Lit::Int(lit)) => match lit.base10_parse::<u16>() {
            Ok(ch) => {
                if (1..=16).contains(&ch) {
                    attrs.midi_channel = Some(u8::try_from(ch - 1).expect("checked 1..=16"));
                } else {
                    push(
                        attrs,
                        syn::Error::new_spanned(lit, "`#[param(midi_channel = …)]` must be 1..=16"),
                    );
                }
            }
            _ => push(
                attrs,
                syn::Error::new_spanned(lit, "`#[param(midi_channel = …)]` must be 1..=16"),
            ),
        },
        ("midi_source", other) => push(
            attrs,
            syn::Error::new_spanned(
                other,
                "`#[param(midi_source = …)]` expects a string literal",
            ),
        ),
        (k, other) => push(
            attrs,
            syn::Error::new_spanned(
                other,
                format!("`#[param({k} = …)]` expects an integer literal"),
            ),
        ),
    }
    Ok(())
}

/// Coerce a `default = ...` attribute expression into an `f64`.
///
/// Accepts numeric literals (positive and `-`-prefixed) and `true` /
/// `false` (for `BoolParam`). Anything else returns `None` and the
/// caller emits a `compile_error!` - the downstream range / shape
/// checks want a concrete `f64`.
///
// `i64 as f64` is an at-compile-time literal default whose magnitude
// is bounded by IntParam ranges; well-defined for the validation
// round-trip.
#[allow(clippy::cast_precision_loss)]
fn parse_default_expr(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Lit(syn::ExprLit { lit, .. }) => match lit {
            Lit::Float(lit) => lit.base10_parse::<f64>().ok(),
            Lit::Int(lit) => lit.base10_parse::<i64>().ok().map(|n| n as f64),
            // `default = true` / `default = false` for BoolParam map to
            // exactly 1.0 / 0.0; `BoolParam::new` panics on anything
            // else, so this is the only path that produces a valid
            // bool default.
            Lit::Bool(lit) => Some(if lit.value { 1.0 } else { 0.0 }),
            _ => None,
        },
        Expr::Unary(syn::ExprUnary {
            op: UnOp::Neg(_),
            expr: inner,
            ..
        }) => parse_default_expr(inner).map(|v| -v),
        _ => None,
    }
}

/// Parse `#[param(...)]` attributes from a field. Errors carried in
/// `attrs.errors` instead of bubbling out so the caller can keep
/// collecting (each malformed attribute should produce a separate
/// `compile_error!` rather than the first one short-circuiting).
#[allow(clippy::too_many_lines)]
fn parse_param_attrs(field: &syn::Field) -> ParamAttrs {
    let mut attrs = ParamAttrs::default();
    // Helper: turn a `syn::Error` into a `compile_error!` token stream
    // and stash it. Used both by the explicit "unknown key" / "wrong
    // literal kind" arms and by `parse_nested_meta`'s own bubbled
    // errors below.
    let push_err = |attrs: &mut ParamAttrs, e: syn::Error| {
        attrs.errors.push(e.to_compile_error());
    };
    for attr in &field.attrs {
        if !attr.path().is_ident("param") {
            continue;
        }
        // `parse_nested_meta`'s closure can only return one error per
        // call (it short-circuits the *current* attribute group on
        // first Err), so route per-key errors through `attrs.errors`
        // instead - each malformed key generates a `compile_error!`
        // and parsing continues.
        let parse_result = attr.parse_nested_meta(|meta| {
            let key = meta
                .path
                .get_ident()
                .map(std::string::ToString::to_string)
                .unwrap_or_default();
            // Two-step pattern for the string-typed keys: parse the
            // literal first, then either assign or stash a
            // compile_error. Avoids needing `&mut attrs` aliased with
            // `&mut attrs.<field>` inside a closure.
            let take_str_into = |slot: &mut Option<String>,
                                 errors: &mut Vec<TokenStream2>,
                                 key_name: &str|
             -> syn::Result<()> {
                let value: Lit = meta.value()?.parse()?;
                match value {
                    Lit::Str(lit) => {
                        *slot = Some(lit.value());
                    }
                    other => {
                        errors.push(
                            syn::Error::new_spanned(
                                other,
                                format!("`#[param({key_name} = ...)]` expects a string literal"),
                            )
                            .to_compile_error(),
                        );
                    }
                }
                Ok(())
            };
            match key.as_str() {
                "id" => {
                    let value: Lit = meta.value()?.parse()?;
                    match value {
                        Lit::Int(lit) => attrs.id = Some(lit.base10_parse()?),
                        other => push_err(
                            &mut attrs,
                            syn::Error::new_spanned(
                                other,
                                "`#[param(id = ...)]` expects an integer literal",
                            ),
                        ),
                    }
                }
                "name" => take_str_into(&mut attrs.name, &mut attrs.errors, "name")?,
                "short_name" => {
                    take_str_into(&mut attrs.short_name, &mut attrs.errors, "short_name")?;
                }
                "group" => take_str_into(&mut attrs.group, &mut attrs.errors, "group")?,
                "range" => take_str_into(&mut attrs.range, &mut attrs.errors, "range")?,
                "default" => {
                    // `meta.value()` returns the stream after `=`. Parse as
                    // an `Expr` so we accept negative literals like
                    // `default = -1` (which `Lit` alone refuses - `-1` is
                    // an `Expr::Unary(Neg, Lit::Int(1))`, not a literal).
                    let expr: Expr = meta.value()?.parse()?;
                    match parse_default_expr(&expr) {
                        Some(value) => attrs.default = Some(value),
                        None => push_err(
                            &mut attrs,
                            syn::Error::new_spanned(
                                &expr,
                                "expected a numeric or bool literal for `default` \
                                 (e.g. `default = 0.5`, `default = -1`, `default = true`)",
                            ),
                        ),
                    }
                }
                "unit" => take_str_into(&mut attrs.unit, &mut attrs.errors, "unit")?,
                "flags" => take_str_into(&mut attrs.flags, &mut attrs.errors, "flags")?,
                "smooth" => take_str_into(&mut attrs.smooth, &mut attrs.errors, "smooth")?,
                "format" => take_str_into(&mut attrs.format_fn, &mut attrs.errors, "format")?,
                "parse" => take_str_into(&mut attrs.parse_fn, &mut attrs.errors, "parse")?,
                "chunk" => {
                    let value: Lit = meta.value()?.parse()?;
                    match value {
                        Lit::Bool(lit) => attrs.chunk = Some(lit.value),
                        other => push_err(
                            &mut attrs,
                            syn::Error::new_spanned(
                                other,
                                "`#[param(chunk = ...)]` expects a bool literal (e.g. \
                                 `chunk = false` for params too expensive to retarget mid-block)",
                            ),
                        ),
                    }
                }
                "midi_cc" | "midi_source" | "midi_channel" => {
                    parse_midi_attr(key.as_str(), &meta, &mut attrs)?;
                }
                other => {
                    push_err(
                        &mut attrs,
                        meta.error(format!(
                            "unknown `#[param]` key `{other}` (expected one of: id, name, \
                             short_name, group, range, default, unit, flags, smooth, format, \
                             parse, chunk, midi_cc, midi_source, midi_channel)",
                        )),
                    );
                }
            }
            Ok(())
        });
        // `parse_nested_meta` itself can fail at the tokenizer level
        // (mis-typed `=`, stray punctuation). Surface those too.
        if let Err(e) = parse_result {
            push_err(&mut attrs, e);
        }
    }
    attrs
}

/// Check if a field has `#[nested]` attribute.
fn has_nested_attr(field: &syn::Field) -> bool {
    field.attrs.iter().any(|a| a.path().is_ident("nested"))
}

/// Check if a field has `#[meter]` attribute.
fn has_meter_attr(field: &syn::Field) -> bool {
    field.attrs.iter().any(|a| a.path().is_ident("meter"))
}

/// `#[skip]` — plain data field (e.g. `Arc<SharedMeters>`). Default-init in
/// `new()`, excluded from param ids / infos / state. Product plugins hold
/// DSP↔UI shared atomics here without host automation.
fn has_skip_attr(field: &syn::Field) -> bool {
    field.attrs.iter().any(|a| a.path().is_ident("skip"))
}

/// Check if a field type is `MeterSlot`.
fn is_meter_slot(ty: &Type) -> bool {
    type_last_segment(ty).is_some_and(|s| s == "MeterSlot")
}

/// A `#[persist]` field: a non-parameter value the host saves alongside
/// the param values (editor-editable config). `Default`-initialized in
/// `new()` and excluded from ids / infos / count, but its bytes are
/// round-tripped through the generated `serialize_persist` /
/// `load_persist`.
fn has_persist_attr(field: &syn::Field) -> bool {
    field.attrs.iter().any(|a| a.path().is_ident("persist"))
}

/// The `#[persist = "key"]` string key, or the field name when the
/// attribute is bare (`#[persist]`). The key identifies the field in the
/// saved blob so add / remove / reorder stays compatible.
fn persist_key(field: &syn::Field) -> String {
    for attr in &field.attrs {
        if attr.path().is_ident("persist")
            && let syn::Meta::NameValue(nv) = &attr.meta
            && let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
        {
            return s.value();
        }
    }
    field
        .ident
        .as_ref()
        .map_or_else(String::new, std::string::ToString::to_string)
}

/// Collect parameter fields, nested fields, meter fields, persist fields,
/// and `#[skip]` plain fields from a struct.
type CollectedFields = (
    Vec<ParamField>,
    Vec<NestedField>,
    Vec<MeterField>,
    Vec<PersistField>,
    Vec<syn::Ident>,
);

/// A `#[persist]` field: ident, blob key, and the parsed wrapper type.
struct PersistField {
    ident: syn::Ident,
    key: String,
    ty: PersistType,
}

/// The supported `#[persist]` field shapes. `load_persist` takes
/// `&self`, so a persist field must be writable through a shared
/// reference - hence the interior-mutability wrapper requirement.
/// `Cell` is excluded on purpose: it isn't `Sync`, and `Params`
/// requires `Sync`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PersistWrapper {
    RwLock,
    Mutex,
}

/// A validated `#[persist]` field type: wrapper + inner scalar.
struct PersistType {
    wrapper: PersistWrapper,
    inner: PersistScalar,
}

/// The scalar types the persist codec can read / write.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PersistScalar {
    Bool,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    String,
}

impl PersistScalar {
    fn from_ident(name: &str) -> Option<Self> {
        match name {
            "bool" => Some(Self::Bool),
            "u8" => Some(Self::U8),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "i8" => Some(Self::I8),
            "i16" => Some(Self::I16),
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            "String" => Some(Self::String),
            _ => None,
        }
    }

    /// Byte width of the little-endian encoding. `None` for the
    /// variable-width `String`.
    fn byte_width(self) -> Option<usize> {
        match self {
            Self::Bool | Self::U8 | Self::I8 => Some(1),
            Self::U16 | Self::I16 => Some(2),
            Self::U32 | Self::I32 | Self::F32 => Some(4),
            Self::U64 | Self::I64 | Self::F64 => Some(8),
            Self::String => None,
        }
    }

    /// The Rust type name tokens (`u32`, `f64`, …) for the decode
    /// expression. Not used for `Bool` / `String`, which have bespoke
    /// codecs.
    fn type_tokens(self) -> TokenStream2 {
        let ident = syn::Ident::new(
            match self {
                Self::U8 => "u8",
                Self::U16 => "u16",
                Self::U32 => "u32",
                Self::U64 => "u64",
                Self::I8 => "i8",
                Self::I16 => "i16",
                Self::I32 => "i32",
                Self::I64 => "i64",
                Self::F32 => "f32",
                Self::F64 => "f64",
                Self::Bool | Self::String => unreachable!("bespoke codecs"),
            },
            proc_macro2::Span::call_site(),
        );
        quote! { #ident }
    }
}

/// Validate a `#[persist]` field type. `Err` tokens are a
/// `compile_error!` naming the supported shapes.
fn parse_persist_type(field: &syn::Field) -> Result<PersistType, TokenStream2> {
    let err = || {
        syn::Error::new_spanned(
            &field.ty,
            "unsupported `#[persist]` field type - expected `RwLock<T>` or `Mutex<T>` \
             with `T` one of: bool, u8, u16, u32, u64, i8, i16, i32, i64, f32, f64, \
             String (the wrapper is required because `load_persist` restores through \
             `&self`; `Cell` isn't `Sync`, which `Params` requires)",
        )
        .to_compile_error()
    };
    let Type::Path(TypePath { path, .. }) = &field.ty else {
        return Err(err());
    };
    let Some(seg) = path.segments.last() else {
        return Err(err());
    };
    let wrapper = match seg.ident.to_string().as_str() {
        "RwLock" => PersistWrapper::RwLock,
        "Mutex" => PersistWrapper::Mutex,
        _ => return Err(err()),
    };
    let Some(inner_ty) = (|| {
        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments
            && let Some(syn::GenericArgument::Type(t)) = args.args.first()
        {
            return Some(t.clone());
        }
        None
    })() else {
        return Err(err());
    };
    let Some(inner_name) = type_last_segment(&inner_ty) else {
        return Err(err());
    };
    let Some(inner) = PersistScalar::from_ident(&inner_name) else {
        return Err(err());
    };
    Ok(PersistType { wrapper, inner })
}

fn collect_fields(fields: &Fields) -> Result<CollectedFields, TokenStream2> {
    let Fields::Named(named) = fields else {
        return Ok((Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()));
    };

    let mut params = Vec::new();
    let mut nested = Vec::new();
    let mut meters = Vec::new();
    let mut persist = Vec::new();
    let mut skipped = Vec::new();

    for f in &named.named {
        let Some(ident) = f.ident.clone() else {
            continue;
        };

        // Checked first: an explicit `#[persist]` saved-config field
        // always wins over whatever the field's type would otherwise
        // classify as. It's a non-param, `Default`-initialized in
        // `new()`, and round-tripped through serialize/load_persist.
        if has_persist_attr(f) {
            persist.push(PersistField {
                ident,
                key: persist_key(f),
                ty: parse_persist_type(f)?,
            });
            continue;
        }

        if has_skip_attr(f) {
            skipped.push(ident);
            continue;
        }

        if has_nested_attr(f) {
            nested.push(NestedField {
                ident,
                ty: f.ty.clone(),
            });
            continue;
        }

        if has_meter_attr(f) || is_meter_slot(&f.ty) {
            meters.push(MeterField { ident, id: None });
            continue;
        }

        if let Some(kind) = classify_param_type(&f.ty) {
            let attrs = parse_param_attrs(f);
            let enum_type = if kind == ParamKind::Enum {
                extract_enum_type_arg(&f.ty)
            } else {
                None
            };
            params.push(ParamField {
                ident,
                kind,
                attrs,
                enum_type,
            });
        }
    }

    Ok((params, nested, meters, persist, skipped))
}

/// Emit an `f64` as a decimal literal (`60.0`, `-60.0`) rather than the
/// suffixed-integer form `quote! { #f64 }` produces for whole numbers
/// (`60f64`). rustc accepts `60f64`, but rust-analyzer's proc-macro
/// expansion rejects it with "expected Expr". The decimal form
/// round-trips cleanly and matches the hand-written `0.0` / `1.0`
/// literals the range-less default path emits.
fn f64_lit(v: f64) -> TokenStream2 {
    if v < 0.0 {
        let abs = proc_macro2::Literal::f64_unsuffixed(-v);
        quote! { -#abs }
    } else {
        let lit = proc_macro2::Literal::f64_unsuffixed(v);
        quote! { #lit }
    }
}

/// Like [`f64_lit`] for `i64` - emits `12` / `-12`, not `12i64`.
fn i64_lit(v: i64) -> TokenStream2 {
    let abs = proc_macro2::Literal::u64_unsuffixed(v.unsigned_abs());
    if v < 0 {
        quote! { -#abs }
    } else {
        quote! { #abs }
    }
}

/// Like [`f64_lit`] for `usize` - emits `2`, not `2usize`.
fn usize_lit(v: usize) -> TokenStream2 {
    let lit = proc_macro2::Literal::usize_unsuffixed(v);
    quote! { #lit }
}

/// Parse the two power-law taper shapes (`skewed(min, max, factor)` and
/// `sym_skewed(min, max, factor, center)`). `None` when neither prefix
/// matches; `Some(compile_error!)` on a malformed match. Split out of
/// [`parse_range_tokens`] to keep that function under the line-length lint.
#[allow(clippy::too_many_lines)]
fn parse_skew_range(range: &str) -> Option<TokenStream2> {
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
fn parse_range_tokens(range: &str) -> TokenStream2 {
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
fn range_bounds(range: &str) -> Option<(f64, f64)> {
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
fn default_range_error(f: &ParamField) -> Option<String> {
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
fn parse_unit_tokens(unit: &str) -> TokenStream2 {
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
fn parse_flags_tokens(flags: &str) -> TokenStream2 {
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
fn parse_smooth_tokens(smooth: &str) -> TokenStream2 {
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
fn gen_param_info_literal(f: &ParamField) -> Option<TokenStream2> {
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
fn gen_field_constructor(f: &ParamField) -> TokenStream2 {
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

// ============================================================================
// #[derive(Params)]
// ============================================================================

/// Derive the `Params` implementation for a parameter struct.
///
/// Field kinds:
///
/// - `FloatParam` / `IntParam` / `BoolParam` / `EnumParam<E>` with
///   `#[param(...)]` - a real parameter.
/// - `MeterSlot` with `#[meter]` - a meter slot, auto-assigned an ID
///   from `METER_ID_BASE` upward, in declaration order.
/// - any `Params` type with `#[nested]` - a sub-struct whose params
///   merge into this one (IDs stay as declared in the child; a
///   parent/child collision panics in `new()` via
///   `Params::assert_no_id_collisions`).
/// - plain data with `#[persist]` (or `#[persist = "key"]`) - saved
///   alongside the params in the host state blob. The field type must
///   be `RwLock<T>` / `Mutex<T>` with `T` one of `bool`,
///   `u8`..`u64`, `i8`..`i64`, `f32`, `f64`, `String`, because
///   `load_persist` restores through `&self`.
///
/// `#[param(...)]` keys:
///
/// | key | value | default |
/// |-----|-------|---------|
/// | `id` | integer literal `< METER_ID_BASE` | required |
/// | `name` | string | `"Unnamed"` |
/// | `short_name` | string | `name` |
/// | `group` | string | `""` |
/// | `range` | `"linear(min, max)"`, `"log(min, max)"`, `"skewed(min, max, factor)"`, `"sym_skewed(min, max, factor, center)"`, `"discrete(min, max)"`, `"enum(count)"`, `"reversed(<range>)"` | `linear(0, 1)`; `discrete(0, 1)` for bools; `enum(variant_count)` for enums |
/// | `default` | numeric or bool literal | `0.0` |
/// | `unit` | `"dB"`, `"Hz"`, `"ms"`, `"s"`, `"%"`, `"st"`, `"pan"`, `"deg"`, `"none"` | `"none"` |
/// | `flags` | `"automatable \| hidden \| readonly \| bypass \| modulatable \| modulatable_per_note"` | `"automatable"` |
/// | `smooth` | `"none"`, `"linear(<ms>)"`, `"exp(<ms>)"`, `"log(<ms>)"` (float params only) | `"none"` |
/// | `chunk` | bool | `true` (`ParamFlags::CHUNKED` set) |
/// | `midi_cc` | `0..=127` | unset |
/// | `midi_source` | `"pitchbend"` \| `"pressure"` \| `"program"` | unset |
/// | `midi_channel` | `1..=16` | any channel |
/// | `format` / `parse` | method name string - custom display hooks | unit-aware defaults |
///
/// The derive also emits `__private::Sealed`, a `new()` that
/// collision-checks the whole tree, and `impl Default` calling it.
///
/// # Panics
///
/// Panics if `syn` fails to parse the input token stream. That only
/// happens on syntactically broken input (rustc would already be
/// rejecting the same file), so the panic surfaces a derive-internal
/// regression rather than user error.
#[proc_macro_derive(Params, attributes(param, nested, meter, persist, skip))]
#[allow(clippy::too_many_lines)]
pub fn derive_params(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).expect("Failed to parse input for Params derive");
    let struct_name = &ast.ident;

    if !ast.generics.params.is_empty() {
        return syn::Error::new_spanned(
            &ast.generics,
            "Params cannot be derived on a generic struct",
        )
        .to_compile_error()
        .into();
    }

    let fields = match &ast.data {
        Data::Struct(data) => &data.fields,
        _ => {
            return syn::Error::new_spanned(&ast, "Params can only be derived on structs")
                .to_compile_error()
                .into();
        }
    };

    let (param_fields, nested_fields, mut meter_fields, persist_fields, skip_fields) =
        match collect_fields(fields) {
            Ok(c) => c,
            Err(err_tokens) => return err_tokens.into(),
        };

    if param_fields.is_empty() && nested_fields.is_empty() && meter_fields.is_empty() {
        return syn::Error::new_spanned(
            &ast,
            "Params derive: no recognized fields (FloatParam, BoolParam, IntParam, EnumParam, MeterSlot, or #[nested])",
        )
        .to_compile_error()
        .into();
    }

    // --- Parameter IDs are explicit ---
    // AURA pins `id = N` on every param (wire-stable by construction);
    // there is no auto-assignment scheme. A missing id is a compile
    // error, spanned at the field.
    for f in &param_fields {
        if f.attrs.id.is_none() {
            return syn::Error::new(
                f.ident.span(),
                "missing `id = N` in `#[param(...)]` - AURA params pin an explicit ID",
            )
            .to_compile_error()
            .into();
        }
    }

    // --- Auto-assign meter IDs ---
    // Meters live in a dedicated high range starting at 2^24 so they
    // can never collide with param IDs. `METER_ID_BASE` is imported
    // from `aura_params` at proc-macro build time so the value can't
    // drift between crates.
    for (next_meter, m) in (METER_ID_BASE..).zip(meter_fields.iter_mut()) {
        m.id = Some(next_meter);
    }

    // --- Compile-time validation: duplicate IDs + meter range ---
    //
    // Checks (within this struct; the cross-nested-struct case is the
    // runtime `assert_no_id_collisions` in `new()`):
    //  1. No two params share an ID.
    //  2. No param ID lands in the meter range (≥ METER_ID_BASE).
    {
        let mut seen_ids = HashSet::new();
        for f in &param_fields {
            if let Some(id) = f.attrs.id {
                if id >= METER_ID_BASE {
                    let msg = format!(
                        "Parameter ID {id} is in the meter range (≥ {METER_ID_BASE}). \
                         Param IDs must be < {METER_ID_BASE}."
                    );
                    return syn::Error::new(f.ident.span(), msg).to_compile_error().into();
                }
                if !seen_ids.insert(id) {
                    let msg = format!("Duplicate parameter ID: {id}");
                    return syn::Error::new(f.ident.span(), msg).to_compile_error().into();
                }
            }
        }
    }

    // MIDI bindings must be unambiguous: two params can't claim the same
    // source on overlapping channels (an any-channel binding overlaps
    // every channel), or the host's mapping query would be
    // order-dependent.
    {
        let bound: Vec<(&str, MidiBindKind, Option<u8>)> = param_fields
            .iter()
            .filter_map(|f| {
                f.attrs.midi_map.map(|k| {
                    (
                        f.attrs.name.as_deref().unwrap_or("<unnamed>"),
                        k,
                        f.attrs.midi_channel,
                    )
                })
            })
            .collect();
        for (i, (na, ka, cha)) in bound.iter().enumerate() {
            for (nb, kb, chb) in bound.iter().skip(i + 1) {
                let overlap = cha.is_none() || chb.is_none() || cha == chb;
                if ka == kb && overlap {
                    let msg = format!(
                        "conflicting MIDI binding: parameters `{na}` and `{nb}` map the \
                         same source on overlapping channels"
                    );
                    return syn::Error::new_spanned(&ast, msg).to_compile_error().into();
                }
            }
        }
    }

    // `#[persist]` keys and `#[nested]` field names share one keyed
    // list in the saved state (`serialize_persist` / `load_persist`),
    // so a duplicate key would generate two match arms for the same
    // string and the later entry would silently never load. Bare
    // `#[persist]` uses the unique field name, so a collision needs an
    // explicit `#[persist = "key"]` - reject it here with the field
    // that introduced the duplicate.
    {
        let mut seen_keys = HashSet::new();
        let all_keys = persist_fields
            .iter()
            .map(|p| (&p.ident, p.key.clone()))
            .chain(
                nested_fields
                    .iter()
                    .map(|n| (&n.ident, n.ident.to_string())),
            );
        for (ident, key) in all_keys {
            if !seen_keys.insert(key.clone()) {
                let msg = format!(
                    "duplicate persist key `{key}`: `#[persist]` keys and `#[nested]` \
                     field names share one saved-state list and must be unique"
                );
                return syn::Error::new(ident.span(), msg).to_compile_error().into();
            }
        }
    }

    // --- Count ---
    let own_count = param_fields.len();
    let nested_idents: Vec<_> = nested_fields.iter().map(|n| &n.ident).collect();

    // --- `<Struct>ParamId` companion enum (G2, option A) ---
    // Own param fields only; nested structs carry their own enum.
    let param_id_enum = gen_param_id_enum(struct_name, &param_fields);
    let count_expr = if nested_fields.is_empty() {
        quote! { #own_count }
    } else {
        quote! { #own_count #(+ ::aura::params::Params::count(&self.#nested_idents))* }
    };

    // --- param_infos ---
    // `ParamInfo` is `Copy`, so push/read it by value.
    let own_infos: Vec<_> = param_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            quote! { self.#ident.info }
        })
        .collect();

    let infos_expr = if nested_fields.is_empty() {
        quote! { vec![#(#own_infos),*] }
    } else {
        // Recurse via `append_param_infos` so each nested struct
        // pushes directly into the shared buffer instead of building
        // its own `Vec` that the outer call then extends. Saves
        // O(depth) intermediate allocations per `param_infos()` call.
        quote! {
            let mut infos = vec![#(#own_infos),*];
            #(::aura::params::Params::append_param_infos(&self.#nested_idents, &mut infos);)*
            infos
        }
    };

    // Override `append_param_infos` so the buffer-based form
    // recurses without the extra `Vec` round-trip in nested cases.
    // Plain (non-nested) structs accept the default impl.
    let append_infos_impl = if nested_fields.is_empty() {
        quote! {}
    } else {
        quote! {
            fn append_param_infos(&self, into: &mut Vec<::aura::params::ParamInfo>) {
                #(into.push(#own_infos);)*
                #(::aura::params::Params::append_param_infos(&self.#nested_idents, into);)*
            }
        }
    };

    // --- param_infos_static ---
    // Same shape as `param_infos`, but each entry is the raw
    // `ParamInfo { ... }` literal (built by
    // `gen_param_info_literal`) rather than a runtime `self.<f>.info`
    // read. Lifted into a `LazyLock<Vec<ParamInfo>>` so format
    // wrappers' `register_*` paths can read parameter metadata
    // without constructing a plugin instance.
    let own_info_literals: Vec<TokenStream2> = param_fields
        .iter()
        .filter_map(gen_param_info_literal)
        .collect();
    let nested_static_calls: Vec<TokenStream2> = nested_fields
        .iter()
        .map(|n| {
            let ty = &n.ty;
            quote! {
                infos.extend(
                    <#ty as ::aura::params::Params>::param_infos_static(),
                );
            }
        })
        .collect();
    let static_infos_body = if nested_fields.is_empty() {
        quote! { vec![#(#own_info_literals),*] }
    } else {
        quote! {
            {
                let mut infos: Vec<::aura::params::ParamInfo> = vec![#(#own_info_literals),*];
                #(#nested_static_calls)*
                infos
            }
        }
    };
    let param_infos_static_impl = quote! {
        fn param_infos_static() -> Vec<::aura::params::ParamInfo>
        where
            Self: ::std::marker::Sized,
        {
            // `LazyLock` so the first call computes the metadata and
            // every later registration reads the cache. `clone()` is
            // a single Vec allocation - cheap relative to the avoided
            // plugin construction. (`ParamInfo` is `Clone`.)
            static INFOS: ::std::sync::LazyLock<Vec<::aura::params::ParamInfo>> =
                ::std::sync::LazyLock::new(|| #static_infos_body);
            INFOS.clone()
        }
    };

    // --- meter_ids ---
    let own_meter_ids: Vec<_> = meter_fields
        .iter()
        .map(|m| {
            let ident = &m.ident;
            quote! { self.#ident.id() }
        })
        .collect();
    let meter_ids_expr = if nested_fields.is_empty() {
        quote! { vec![#(#own_meter_ids),*] }
    } else {
        quote! {
            let mut ids = vec![#(#own_meter_ids),*];
            #(ids.extend(::aura::params::Params::meter_ids(&self.#nested_idents));)*
            ids
        }
    };

    // --- get_plain ---
    let get_plain_arms: Vec<_> = param_fields.iter().map(|f| {
        let ident = &f.ident;
        match f.kind {
            ParamKind::Float => quote! { x if x == self.#ident.id() => Some(self.#ident.raw_target()), },
            // `i64 as f64` is precision-lossy by spec (mantissa 53 < 63);
            // no `From<i64> for f64` exists, so the cast is the idiom.
            ParamKind::Int => quote! { x if x == self.#ident.id() => {
                #[allow(clippy::cast_precision_loss)]
                let v = self.#ident.value() as f64;
                Some(v)
            }, },
            ParamKind::Bool => quote! { x if x == self.#ident.id() => Some(if self.#ident.value() { 1.0 } else { 0.0 }), },
            // `u32 → f64` is lossless (u32::MAX < 2^53); use `From` for
            // consistency with the rest of the derive output.
            ParamKind::Enum => quote! { x if x == self.#ident.id() => Some(f64::from(self.#ident.index())), },
        }
    }).collect();

    let get_plain_fallthrough = if nested_fields.is_empty() {
        quote! { _ => None, }
    } else {
        quote! {
            _ => {
                #(if let Some(v) = ::aura::params::Params::get_plain(&self.#nested_idents, id) { return Some(v); })*
                None
            }
        }
    };

    // --- get_normalized ---
    //
    // Per-id match arms reach into the matching param's `info.range`
    // and call `normalize` / `denormalize` directly. Dispatching
    // through `self.param_infos()` would allocate a `Vec<ParamInfo>`
    // on every host-driven `set_normalized` / `get_normalized` round
    // trip and every editor paint frame.
    let get_normalized_arms: Vec<_> = param_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let plain_expr = match f.kind {
                ParamKind::Float => quote! { self.#ident.raw_target() },
                // i64 → f64 has no `From`; `as` with an explicit
                // allow is the idiom.
                ParamKind::Int => quote! {{
                    #[allow(clippy::cast_precision_loss)]
                    let v = self.#ident.value() as f64;
                    v
                }},
                ParamKind::Bool => quote! { if self.#ident.value() { 1.0 } else { 0.0 } },
                // u32 → f64 is lossless: use `From`.
                ParamKind::Enum => quote! { f64::from(self.#ident.index()) },
            };
            quote! {
                x if x == self.#ident.id() => Some(self.#ident.info.range.normalize(#plain_expr)),
            }
        })
        .collect();

    let get_normalized_fallthrough = if nested_fields.is_empty() {
        quote! { _ => None, }
    } else {
        quote! {
            _ => {
                #(if let Some(v) = ::aura::params::Params::get_normalized(&self.#nested_idents, id) { return Some(v); })*
                None
            }
        }
    };

    // --- set_plain ---
    let set_plain_arms: Vec<_> = param_fields.iter().map(|f| {
        let ident = &f.ident;
        match f.kind {
            ParamKind::Float => quote! { x if x == self.#ident.id() => self.#ident.set_value(value), },
            ParamKind::Bool => quote! { x if x == self.#ident.id() => self.#ident.set_value(value > 0.5), },
            ParamKind::Int => quote! { x if x == self.#ident.id() => {
                #[allow(clippy::cast_possible_truncation)]
                self.#ident.set_value(value.round() as i64);
            }, },
            ParamKind::Enum => quote! { x if x == self.#ident.id() => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                self.#ident.set_index(value.round() as u32);
            }, },
        }
    }).collect();

    let set_plain_fallthrough = if nested_fields.is_empty() {
        quote! { _ => {} }
    } else {
        quote! {
            _ => {
                #(::aura::params::Params::set_plain(&self.#nested_idents, id, value);)*
            }
        }
    };

    // --- set_normalized ---
    //
    // Per-id arms denormalize through the matching param's range, then
    // commit through the kind-specific atomic write. Same allocation
    // motivation as `get_normalized` above.
    //
    // The same commit/readback pair also drives the two
    // `set_normalized_returning_*` single-dispatch overrides below.
    let commit_readback: Vec<(TokenStream2, TokenStream2)> = param_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            match f.kind {
                ParamKind::Float => (
                    quote! {{ self.#ident.set_value(plain); }},
                    quote! { self.#ident.raw_target() },
                ),
                ParamKind::Bool => (
                    quote! {{ self.#ident.set_value(plain > 0.5); }},
                    quote! { if self.#ident.value() { 1.0 } else { 0.0 } },
                ),
                ParamKind::Int => (
                    quote! {{
                        #[allow(clippy::cast_possible_truncation)]
                        self.#ident.set_value(plain.round() as i64);
                    }},
                    quote! {{
                        #[allow(clippy::cast_precision_loss)]
                        let v = self.#ident.value() as f64;
                        v
                    }},
                ),
                ParamKind::Enum => (
                    quote! {{
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        self.#ident.set_index(plain.round() as u32);
                    }},
                    quote! { f64::from(self.#ident.index()) },
                ),
            }
        })
        .collect();

    let set_normalized_arms: Vec<_> = param_fields
        .iter()
        .zip(&commit_readback)
        .map(|(f, (commit, _))| {
            let ident = &f.ident;
            quote! {
                x if x == self.#ident.id() => {
                    let plain = self.#ident.info.range.denormalize(value);
                    #commit
                }
            }
        })
        .collect();

    let set_normalized_fallthrough = if nested_fields.is_empty() {
        quote! { _ => {} }
    } else {
        quote! {
            _ => {
                #(::aura::params::Params::set_normalized(&self.#nested_idents, id, value);)*
            }
        }
    };

    // --- set_normalized_returning_plain / _normalized ---
    //
    // Single match-arm walk: denormalize, commit, read back the
    // resulting plain value (post-clamp / post-step) from the same
    // arm - no second dispatch through `get_plain` / `get_normalized`
    // like the trait default would pay. The nested fallthrough probes
    // each child's `get_plain` to find the owner, then delegates to
    // the child's own single-dispatch override.
    let set_returning_plain_arms: Vec<_> = param_fields
        .iter()
        .zip(&commit_readback)
        .map(|(f, (commit, readback))| {
            let ident = &f.ident;
            quote! {
                x if x == self.#ident.id() => {
                    let plain = self.#ident.info.range.denormalize(value);
                    #commit
                    #readback
                }
            }
        })
        .collect();

    let set_returning_plain_fallthrough = if nested_fields.is_empty() {
        quote! { _ => 0.0, }
    } else {
        quote! {
            _ => {
                #(
                    if ::aura::params::Params::get_plain(&self.#nested_idents, id).is_some() {
                        return ::aura::params::Params::set_normalized_returning_plain(
                            &self.#nested_idents, id, value,
                        );
                    }
                )*
                0.0
            }
        }
    };

    let set_returning_normalized_arms: Vec<_> = param_fields
        .iter()
        .zip(&commit_readback)
        .map(|(f, (commit, readback))| {
            let ident = &f.ident;
            quote! {
                x if x == self.#ident.id() => {
                    let plain = self.#ident.info.range.denormalize(value);
                    #commit
                    self.#ident.info.range.normalize(#readback)
                }
            }
        })
        .collect();

    let set_returning_normalized_fallthrough = if nested_fields.is_empty() {
        quote! { _ => 0.0, }
    } else {
        quote! {
            _ => {
                #(
                    if ::aura::params::Params::get_plain(&self.#nested_idents, id).is_some() {
                        return ::aura::params::Params::set_normalized_returning_normalized(
                            &self.#nested_idents, id, value,
                        );
                    }
                )*
                0.0
            }
        }
    };

    // --- format_value ---
    let format_value_arms: Vec<_> = param_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            if let Some(ref fmt_fn) = f.attrs.format_fn {
                let fmt_ident = syn::Ident::new(fmt_fn, ident.span());
                quote! { x if x == self.#ident.id() => Some(self.#fmt_ident(value)), }
            } else {
                match f.kind {
                    ParamKind::Bool => quote! {
                        x if x == self.#ident.id() => {
                            Some(if value > 0.5 { "On".to_string() } else { "Off".to_string() })
                        }
                    },
                    ParamKind::Enum => {
                        let enum_ty = f
                            .enum_type
                            .as_ref()
                            .expect("ParamKind::Enum field must have enum_type populated");
                        quote! {
                            x if x == self.#ident.id() => {
                                Some(::aura::params::EnumParam::<#enum_ty>::format_by_index(value))
                            }
                        }
                    }
                    _ => quote! {
                        x if x == self.#ident.id() => {
                            Some(::aura::params::format_param_value(&self.#ident.info, value))
                        }
                    },
                }
            }
        })
        .collect();

    let format_fallthrough = if nested_fields.is_empty() {
        quote! { _ => None, }
    } else {
        quote! {
            _ => {
                #(if let Some(v) = ::aura::params::Params::format_value(&self.#nested_idents, id, value) { return Some(v); })*
                None
            }
        }
    };

    // --- parse_value ---
    //
    // Defaults per kind (overridable per field with `parse = "fn"`):
    // Float / Int trim the text, strip the unit suffix (exact and
    // lowercased, so `"dB"` also accepts `"db"`), and parse the rest as
    // a float. Bools accept true/false/1/0/on/off (case-insensitive,
    // so the parse round-trips the "On"/"Off" display). Enums match
    // variant names case-insensitively.
    let parse_value_arms: Vec<_> = param_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            if let Some(ref parse_fn) = f.attrs.parse_fn {
                let parse_ident = syn::Ident::new(parse_fn, ident.span());
                return quote! { x if x == self.#ident.id() => self.#parse_ident(text), };
            }
            match f.kind {
                ParamKind::Bool => quote! {
                    x if x == self.#ident.id() => {
                        match text.trim().to_lowercase().as_str() {
                            "true" | "1" | "on" => Some(1.0),
                            "false" | "0" | "off" => Some(0.0),
                            _ => None,
                        }
                    }
                },
                ParamKind::Enum => {
                    let enum_ty = f
                        .enum_type
                        .as_ref()
                        .expect("ParamKind::Enum field must have enum_type populated");
                    quote! {
                        x if x == self.#ident.id() => {
                            let __t = text.trim();
                            #[allow(clippy::cast_precision_loss)]
                            let __v = <#enum_ty as ::aura::params::ParamEnum>::variant_names()
                                .iter()
                                .position(|__n| __n.eq_ignore_ascii_case(__t))
                                .map(|__i| __i as f64);
                            __v
                        }
                    }
                }
                _ => quote! {
                    x if x == self.#ident.id() => {
                        let __unit: &str = self.#ident.info.unit.as_str();
                        let __t = text.trim();
                        let __t = if __unit.is_empty() {
                            __t
                        } else {
                            let __lower = __unit.to_lowercase();
                            __t.strip_suffix(__unit)
                                .or_else(|| __t.strip_suffix(__lower.as_str()))
                                .unwrap_or(__t)
                        };
                        __t.trim().parse::<f64>().ok()
                    }
                },
            }
        })
        .collect();

    let parse_fallthrough = if nested_fields.is_empty() {
        quote! { _ => None, }
    } else {
        quote! {
            _ => {
                #(if let Some(v) = ::aura::params::Params::parse_value(&self.#nested_idents, id, text) { return Some(v); })*
                None
            }
        }
    };

    // --- snap_smoothers ---
    let snap_stmts: Vec<_> = param_fields
        .iter()
        .filter(|f| f.kind == ParamKind::Float)
        .map(|f| {
            let ident = &f.ident;
            quote! { self.#ident.smoother.snap(self.#ident.raw_target()); }
        })
        .collect();

    // --- set_sample_rate ---
    let sr_stmts: Vec<_> = param_fields
        .iter()
        .filter(|f| f.kind == ParamKind::Float)
        .map(|f| {
            let ident = &f.ident;
            quote! { self.#ident.smoother.set_sample_rate(sample_rate); }
        })
        .collect();

    // --- collect_values ---
    let collect_ids: Vec<_> = param_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            quote! { self.#ident.id() }
        })
        .collect();

    // --- new() / Default ---
    let param_inits: Vec<_> = param_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let constructor = gen_field_constructor(f);
            quote! { #ident: #constructor }
        })
        .collect();

    let nested_inits: Vec<_> = nested_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            quote! { #ident: ::core::default::Default::default() }
        })
        .collect();

    let meter_inits: Vec<_> = meter_fields
        .iter()
        .map(|m| {
            let ident = &m.ident;
            let id = m.id();
            quote! { #ident: ::aura::params::MeterSlot { id: #id } }
        })
        .collect();

    let persist_inits: Vec<_> = persist_fields
        .iter()
        .map(|p| {
            let ident = &p.ident;
            quote! { #ident: ::core::default::Default::default() }
        })
        .collect();

    let skip_inits: Vec<_> = skip_fields
        .iter()
        .map(|ident| {
            quote! { #ident: ::core::default::Default::default() }
        })
        .collect();

    let new_impl = quote! {
        impl #struct_name {
            /// Construct with every parameter at its declared default.
            ///
            /// # Panics
            ///
            /// Panics when a parameter or meter ID is reachable twice
            /// (a parent / `#[nested]` collision the per-struct
            /// compile-time check can't see) - see
            /// `Params::assert_no_id_collisions`.
            #[must_use]
            pub fn new() -> Self {
                let me = Self {
                    #(#param_inits,)*
                    #(#nested_inits,)*
                    #(#meter_inits,)*
                    #(#persist_inits,)*
                    #(#skip_inits,)*
                };
                // The per-struct compile-time ID check can't see
                // across nested types; a parent id matching a nested
                // id would silently corrupt state round-trips. Surface
                // it as a construction panic.
                <Self as ::aura::params::Params>::assert_no_id_collisions(&me);
                me
            }
        }

        impl Default for #struct_name {
            fn default() -> Self {
                Self::new()
            }
        }
    };

    // Surface every `compile_error!` collected by `parse_param_attrs`
    // (unknown keys, wrong literal kinds, malformed `default = ...`).
    // Emitted alongside the impl rather than instead of it so the
    // diagnostics are precise; downstream type errors from a malformed
    // attribute aren't masked by a missing `Params` impl.
    let attr_errors: Vec<TokenStream2> = param_fields
        .iter()
        .flat_map(|f| f.attrs.errors.iter().cloned())
        .collect();

    // --- `#[persist]` codegen ---
    //
    // The blob is a self-delimiting keyed list so fields can be added /
    // removed / reordered without breaking older saves: a `u32` entry
    // count (LE), then per entry a `u32`-length-prefixed key and a
    // `u32`-length-prefixed value. Own `#[persist]` fields and
    // `#[nested]` sub-params share one list; a nested entry's value is
    // that sub-struct's own `serialize_persist` blob, so it recurses.
    let persist_write_stmts: Vec<TokenStream2> = persist_fields
        .iter()
        .map(|p| {
            let ident = &p.ident;
            let key = &p.key;
            // Key length is known at expansion time - no runtime cast.
            let key_len = u32::try_from(key.len()).expect("persist key fits u32");
            let write_value = gen_persist_write(ident, &p.ty);
            quote! {
                {
                    __buf.extend_from_slice(&#key_len.to_le_bytes());
                    __buf.extend_from_slice(#key.as_bytes());
                    let __start = __buf.len();
                    __buf.extend_from_slice(&0u32.to_le_bytes());
                    #write_value
                    #[allow(clippy::cast_possible_truncation)]
                    let __len = (__buf.len() - __start - 4) as u32;
                    __buf[__start..__start + 4].copy_from_slice(&__len.to_le_bytes());
                }
            }
        })
        .chain(nested_idents.iter().map(|ident| {
            let key = ident.to_string();
            let key_len = u32::try_from(key.len()).expect("nested name fits u32");
            quote! {
                {
                    __buf.extend_from_slice(&#key_len.to_le_bytes());
                    __buf.extend_from_slice(#key.as_bytes());
                    let __blob = ::aura::params::Params::serialize_persist(&self.#ident);
                    __buf.extend_from_slice(
                        &u32::try_from(__blob.len()).expect("persist blob fits u32").to_le_bytes(),
                    );
                    __buf.extend_from_slice(&__blob);
                }
            }
        }))
        .collect();

    let persist_read_arms: Vec<TokenStream2> = persist_fields
        .iter()
        .map(|p| {
            let ident = &p.ident;
            let key = syn::LitByteStr::new(p.key.as_bytes(), ident.span());
            let read_value = gen_persist_read(ident, &p.ty);
            quote! {
                #key => { #read_value }
            }
        })
        .chain(nested_idents.iter().map(|ident| {
            let key = syn::LitByteStr::new(ident.to_string().as_bytes(), ident.span());
            quote! {
                #key => ::aura::params::Params::load_persist(&self.#ident, __value),
            }
        }))
        .collect();

    #[allow(clippy::cast_possible_truncation)]
    let persist_count = (persist_fields.len() + nested_idents.len()) as u32;

    // Nothing to carry: leave the empty-`Vec` trait default so a plugin
    // without persisted config adds no bytes to its saved state.
    let persist_impl = if persist_fields.is_empty() && nested_idents.is_empty() {
        quote! {}
    } else {
        quote! {
        fn serialize_persist(&self) -> ::std::vec::Vec<u8> {
            let mut __buf: ::std::vec::Vec<u8> = ::std::vec::Vec::new();
            __buf.extend_from_slice(&#persist_count.to_le_bytes());
            #(#persist_write_stmts)*
            __buf
        }

        fn load_persist(&self, __data: &[u8]) {
            // Bounds-checked cursor over the blob. Any short / malformed
            // frame bails out, leaving the remaining fields at their
            // current values - state blobs are host-supplied and can be
            // truncated or corrupt.
            let mut __pos: usize = 0;
            let __take = |__pos: &mut usize, __n: usize| -> ::core::option::Option<&[u8]> {
                let __end = __pos.checked_add(__n)?;
                let __s = __data.get(*__pos..__end)?;
                *__pos = __end;
                ::core::option::Option::Some(__s)
            };
            let ::core::option::Option::Some(__count_b) = __take(&mut __pos, 4) else {
                return;
            };
            let __count = u32::from_le_bytes(
                __count_b.try_into().expect("__take(4) yields 4 bytes"),
            );
            for _ in 0..__count {
                let ::core::option::Option::Some(__klen_b) = __take(&mut __pos, 4) else {
                    return;
                };
                let __klen = u32::from_le_bytes(
                    __klen_b.try_into().expect("__take(4) yields 4 bytes"),
                );
                let Ok(__klen) = usize::try_from(__klen) else {
                    return;
                };
                let ::core::option::Option::Some(__key) = __take(&mut __pos, __klen) else {
                    return;
                };
                let ::core::option::Option::Some(__vlen_b) = __take(&mut __pos, 4) else {
                    return;
                };
                let __vlen = u32::from_le_bytes(
                    __vlen_b.try_into().expect("__take(4) yields 4 bytes"),
                );
                let Ok(__vlen) = usize::try_from(__vlen) else {
                    return;
                };
                let ::core::option::Option::Some(__value) = __take(&mut __pos, __vlen) else {
                    return;
                };
                match __key {
                    #(#persist_read_arms)*
                    _ => {}
                }
            }
        }
        }
    };

    let expanded = quote! {
        #(#attr_errors)*

        #param_id_enum

        #new_impl

        impl ::aura::params::__private::Sealed for #struct_name {}

        impl ::aura::params::Params for #struct_name {
            fn param_infos(&self) -> Vec<::aura::params::ParamInfo> {
                #infos_expr
            }

            #append_infos_impl

            #param_infos_static_impl

            fn count(&self) -> usize {
                #count_expr
            }

            fn meter_ids(&self) -> Vec<u32> {
                #meter_ids_expr
            }

            fn get_normalized(&self, id: u32) -> Option<f64> {
                match id {
                    #(#get_normalized_arms)*
                    #get_normalized_fallthrough
                }
            }

            fn set_normalized(&self, id: u32, value: f64) {
                match id {
                    #(#set_normalized_arms)*
                    #set_normalized_fallthrough
                }
            }

            fn set_normalized_returning_plain(&self, id: u32, value: f64) -> f64 {
                match id {
                    #(#set_returning_plain_arms)*
                    #set_returning_plain_fallthrough
                }
            }

            fn set_normalized_returning_normalized(&self, id: u32, value: f64) -> f64 {
                match id {
                    #(#set_returning_normalized_arms)*
                    #set_returning_normalized_fallthrough
                }
            }

            fn get_plain(&self, id: u32) -> Option<f64> {
                match id {
                    #(#get_plain_arms)*
                    #get_plain_fallthrough
                }
            }

            fn set_plain(&self, id: u32, value: f64) {
                match id {
                    #(#set_plain_arms)*
                    #set_plain_fallthrough
                }
            }

            fn format_value(&self, id: u32, value: f64) -> Option<String> {
                match id {
                    #(#format_value_arms)*
                    #format_fallthrough
                }
            }

            fn parse_value(&self, id: u32, text: &str) -> Option<f64> {
                match id {
                    #(#parse_value_arms)*
                    #parse_fallthrough
                }
            }

            fn snap_smoothers(&self) {
                #(#snap_stmts)*
                #(::aura::params::Params::snap_smoothers(&self.#nested_idents);)*
            }

            fn set_sample_rate(&self, sample_rate: f64) {
                #(#sr_stmts)*
                #(::aura::params::Params::set_sample_rate(&self.#nested_idents, sample_rate);)*
            }

            fn collect_values(&self) -> (Vec<u32>, Vec<f64>) {
                let mut ids: Vec<u32> = vec![#(#collect_ids),*];
                let mut values: Vec<f64> = ids
                    .iter()
                    .map(|id| self.get_plain(*id).expect("id was emitted by #[derive(Params)] and so must resolve"))
                    .collect();
                #({
                    let (nids, nvals) = ::aura::params::Params::collect_values(&self.#nested_idents);
                    ids.extend(nids);
                    values.extend(nvals);
                })*
                (ids, values)
            }

            fn restore_values(&self, values: &[(u32, f64)]) {
                for (id, value) in values {
                    self.set_plain(*id, *value);
                }
            }

            #persist_impl
        }
    };

    expanded.into()
}

/// `gain_db` / `r#type` → `GainDb` / `Type` for `*ParamId` variant names.
fn snake_to_pascal(ident: &str) -> String {
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
fn gen_param_id_enum(struct_name: &syn::Ident, param_fields: &[ParamField]) -> TokenStream2 {
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
fn gen_persist_write(ident: &syn::Ident, ty: &PersistType) -> TokenStream2 {
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
fn gen_persist_read(ident: &syn::Ident, ty: &PersistType) -> TokenStream2 {
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

// ============================================================================
// #[derive(ParamEnum)]
// ============================================================================

/// Derive `ParamEnum` for a C-like enum.
///
/// Generates `Clone`, `Copy`, `PartialEq`, `Eq`, the `Sealed` marker,
/// and all 5 `ParamEnum` methods: `from_index`, `to_index`, `name`,
/// `variant_count`, and `variant_names`.
///
/// Display names default to the variant identifier. Use
/// `#[name = "..."]` on a variant to override:
///
/// ```ignore
/// #[derive(ParamEnum)]
/// pub enum ArpPattern {
///     Up,
///     Down,
///     #[name = "Up/Down"]
///     UpDown,
///     Random,
/// }
/// ```
///
/// # Panics
///
/// Panics if `syn` fails to parse the input token stream - same
/// "rustc-already-rejected" condition as [`derive_params`].
#[proc_macro_derive(ParamEnum, attributes(name))]
#[allow(clippy::too_many_lines)]
pub fn derive_param_enum(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).expect("Failed to parse input for ParamEnum derive");
    let enum_name = &ast.ident;

    let variants = match &ast.data {
        Data::Enum(data) => &data.variants,
        _ => {
            return syn::Error::new_spanned(&ast, "ParamEnum can only be derived on enums")
                .to_compile_error()
                .into();
        }
    };

    if variants.is_empty() {
        return syn::Error::new_spanned(&ast, "ParamEnum needs at least one variant")
            .to_compile_error()
            .into();
    }

    // Ensure all variants are unit variants (no fields)
    for v in variants {
        if !matches!(v.fields, Fields::Unit) {
            return syn::Error::new_spanned(
                v,
                "ParamEnum variants must be unit variants (no fields)",
            )
            .to_compile_error()
            .into();
        }
    }

    let count = variants.len();

    let variant_idents: Vec<_> = variants.iter().map(|v| &v.ident).collect();

    // Parse #[name = "..."] attributes, falling back to the variant ident
    let variant_names: Vec<String> = variants
        .iter()
        .map(|v| {
            for attr in &v.attrs {
                if attr.path().is_ident("name")
                    && let Ok(syn::MetaNameValue {
                        value:
                            syn::Expr::Lit(syn::ExprLit {
                                lit: Lit::Str(lit), ..
                            }),
                        ..
                    }) = attr.meta.require_name_value()
                {
                    return lit.value();
                }
            }
            v.ident.to_string()
        })
        .collect();

    // from_index match arms
    let from_index_arms: Vec<_> = variant_idents
        .iter()
        .enumerate()
        .map(|(i, ident)| {
            quote! { #i => Self::#ident, }
        })
        .collect();
    let first_variant = &variant_idents[0];

    // to_index match arms
    let to_index_arms: Vec<_> = variant_idents
        .iter()
        .enumerate()
        .map(|(i, ident)| {
            quote! { Self::#ident => #i, }
        })
        .collect();

    // name match arms
    let name_arms: Vec<_> = variant_idents
        .iter()
        .zip(variant_names.iter())
        .map(|(ident, name)| {
            quote! { Self::#ident => #name, }
        })
        .collect();

    let name_strs: Vec<_> = variant_names
        .iter()
        .map(std::string::String::as_str)
        .collect();

    let expanded = quote! {
        #[allow(clippy::expl_impl_clone_on_copy)]
        impl Clone for #enum_name {
            fn clone(&self) -> Self { *self }
        }
        impl Copy for #enum_name {}
        impl PartialEq for #enum_name {
            fn eq(&self, other: &Self) -> bool {
                ::aura::params::ParamEnum::to_index(self) == ::aura::params::ParamEnum::to_index(other)
            }
        }
        impl Eq for #enum_name {}

        impl ::aura::params::__private::Sealed for #enum_name {}

        impl ::aura::params::ParamEnum for #enum_name {
            fn from_index(index: usize) -> Self {
                match index {
                    #(#from_index_arms)*
                    _ => Self::#first_variant,
                }
            }

            fn to_index(&self) -> usize {
                match self {
                    #(#to_index_arms)*
                }
            }

            fn name(&self) -> &'static str {
                match self {
                    #(#name_arms)*
                }
            }

            fn variant_count() -> usize {
                #count
            }

            fn variant_names() -> &'static [&'static str] {
                &[#(#name_strs),*]
            }
        }
    };

    expanded.into()
}

#[cfg(test)]
mod parse_default_tests {
    use super::parse_default_expr;

    fn eval(src: &str) -> Option<f64> {
        let expr: syn::Expr = syn::parse_str(src).expect("test input must parse as Expr");
        parse_default_expr(&expr)
    }

    #[test]
    fn numeric_and_bool_literals() {
        assert_eq!(eval("0.5"), Some(0.5));
        assert_eq!(eval("3"), Some(3.0));
        assert_eq!(eval("-1"), Some(-1.0));
        assert_eq!(eval("-0.25"), Some(-0.25));
        assert_eq!(eval("true"), Some(1.0));
        assert_eq!(eval("false"), Some(0.0));
    }

    #[test]
    fn rejected_shapes() {
        // Bare ident: ambiguous with crate-local consts, not accepted.
        assert_eq!(eval("PI"), None);
        // Arbitrary crate path.
        assert_eq!(eval("crate::FOO"), None);
        // Function calls and arithmetic expressions are not const-evaluated.
        assert_eq!(eval("some_fn()"), None);
        assert_eq!(eval("1 + 2"), None);
    }
}
