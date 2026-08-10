//! Field / attribute parsing for `#[derive(Params)]`.
//!
//! Turns the input struct's fields and their `#[param(...)]` /
//! `#[nested]` / `#[meter]` / `#[persist]` / `#[skip]` attributes into
//! the typed [`ParamField`] / [`NestedField`] / [`MeterField`] /
//! [`PersistField`] lists [`params::expand`](crate::params::expand)
//! codegens from. No `TokenStream` assembly happens here beyond what's
//! needed to carry parse errors as `compile_error!` tokens.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Expr, Fields, Lit, Type, TypePath, UnOp};

/// Recognized parameter field types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParamKind {
    Float,
    Bool,
    Int,
    Enum,
}

/// A parsed parameter field from the input struct.
pub(crate) struct ParamField {
    pub(crate) ident: syn::Ident,
    pub(crate) kind: ParamKind,
    pub(crate) attrs: ParamAttrs,
    /// For `EnumParam<T>`, the inner type `T`.
    pub(crate) enum_type: Option<syn::Type>,
}

impl ParamField {
    /// ID that the explicit-`id` validation in `derive_params` has
    /// guaranteed is populated. Calling this earlier is a logic error.
    pub(crate) fn id(&self) -> u32 {
        self.attrs
            .id
            .expect("ParamField::id called before the id validation ran")
    }
}

/// A nested Params field (delegates to the inner struct).
pub(crate) struct NestedField {
    pub(crate) ident: syn::Ident,
    /// Field type, retained so the derive can call associated functions
    /// on it without an instance - specifically
    /// `Params::param_infos_static` for the registration-time
    /// "no temp plugin" path.
    pub(crate) ty: syn::Type,
}

/// A meter slot field.
pub(crate) struct MeterField {
    pub(crate) ident: syn::Ident,
    pub(crate) id: Option<u32>,
}

impl MeterField {
    /// ID that the auto-assignment block has guaranteed is populated.
    /// Same invariant as [`ParamField::id`].
    pub(crate) fn id(&self) -> u32 {
        self.id
            .expect("MeterField::id called before the auto-assignment block ran")
    }
}

/// Derive-side mirror of `aura_params::MidiSource`, used to validate
/// bindings across params and emit the `ParamInfo::midi_map` tokens.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MidiBindKind {
    Cc(u8),
    PitchBend,
    ChannelPressure,
    ProgramChange,
}

pub(crate) const MIDI_BOTH_SET: &str = "set only one of `midi_cc` / `midi_source` on a parameter";

impl MidiBindKind {
    /// The `Some(::aura::params::MidiSource::…)` tokens for a binding,
    /// or `None`. CC values are emitted unsuffixed (`Cc(74)`) to keep
    /// the macro output rust-analyzer-friendly.
    pub(crate) fn to_tokens(self) -> TokenStream2 {
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
pub(crate) struct ParamAttrs {
    pub(crate) id: Option<u32>,
    pub(crate) name: Option<String>,
    pub(crate) short_name: Option<String>,
    pub(crate) group: Option<String>,
    pub(crate) range: Option<String>,
    pub(crate) default: Option<f64>,
    pub(crate) unit: Option<String>,
    pub(crate) flags: Option<String>,
    /// Set by `#[param(chunk = false)]` on parameters too expensive to
    /// retarget mid-block (FFT sizes, lookahead, etc.). `None` means
    /// "use the default" (`true`). Drives the `ParamFlags::CHUNKED`
    /// bit in `gen_param_info_literal`.
    pub(crate) chunk: Option<bool>,
    /// Default host MIDI-learn binding source, set by `midi_cc = N`
    /// or `midi_source = "pitchbend" | "pressure" | "program"` (at
    /// most one). Baked onto `ParamInfo::midi_map`.
    pub(crate) midi_map: Option<MidiBindKind>,
    /// Channel scope for `midi_map`, stored as the wire channel
    /// `0..=15` (the attribute is the user-facing `1..=16`).
    pub(crate) midi_channel: Option<u8>,
    pub(crate) smooth: Option<String>,
    pub(crate) format_fn: Option<String>,
    pub(crate) parse_fn: Option<String>,
    /// Compile-error tokens collected during parsing - emitted by the
    /// derive output so unknown keys and unexpected literal kinds
    /// surface at compile time instead of as silent default values.
    pub(crate) errors: Vec<TokenStream2>,
}

pub(crate) fn type_last_segment(ty: &Type) -> Option<String> {
    let Type::Path(TypePath { path, .. }) = ty else {
        return None;
    };
    path.segments.last().map(|seg| seg.ident.to_string())
}

/// Extract the generic type argument from `EnumParam<T>`.
pub(crate) fn extract_enum_type_arg(ty: &Type) -> Option<syn::Type> {
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

pub(crate) fn classify_param_type(ty: &Type) -> Option<ParamKind> {
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
pub(crate) fn parse_midi_attr(
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
pub(crate) fn parse_default_expr(expr: &Expr) -> Option<f64> {
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
pub(crate) fn parse_param_attrs(field: &syn::Field) -> ParamAttrs {
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
pub(crate) fn has_nested_attr(field: &syn::Field) -> bool {
    field.attrs.iter().any(|a| a.path().is_ident("nested"))
}

/// Check if a field has `#[meter]` attribute.
pub(crate) fn has_meter_attr(field: &syn::Field) -> bool {
    field.attrs.iter().any(|a| a.path().is_ident("meter"))
}

/// `#[skip]` — plain data field (e.g. `Arc<SharedMeters>`). Default-init in
/// `new()`, excluded from param ids / infos / state. Product plugins hold
/// DSP↔UI shared atomics here without host automation.
pub(crate) fn has_skip_attr(field: &syn::Field) -> bool {
    field.attrs.iter().any(|a| a.path().is_ident("skip"))
}

/// Check if a field type is `MeterSlot`.
pub(crate) fn is_meter_slot(ty: &Type) -> bool {
    type_last_segment(ty).is_some_and(|s| s == "MeterSlot")
}

/// A `#[persist]` field: a non-parameter value the host saves alongside
/// the param values (editor-editable config). `Default`-initialized in
/// `new()` and excluded from ids / infos / count, but its bytes are
/// round-tripped through the generated `serialize_persist` /
/// `load_persist`.
pub(crate) fn has_persist_attr(field: &syn::Field) -> bool {
    field.attrs.iter().any(|a| a.path().is_ident("persist"))
}

/// The `#[persist = "key"]` string key, or the field name when the
/// attribute is bare (`#[persist]`). The key identifies the field in the
/// saved blob so add / remove / reorder stays compatible.
pub(crate) fn persist_key(field: &syn::Field) -> String {
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
pub(crate) type CollectedFields = (
    Vec<ParamField>,
    Vec<NestedField>,
    Vec<MeterField>,
    Vec<PersistField>,
    Vec<syn::Ident>,
);

/// A `#[persist]` field: ident, blob key, and the parsed wrapper type.
pub(crate) struct PersistField {
    pub(crate) ident: syn::Ident,
    pub(crate) key: String,
    pub(crate) ty: PersistType,
}

/// The supported `#[persist]` field shapes. `load_persist` takes
/// `&self`, so a persist field must be writable through a shared
/// reference - hence the interior-mutability wrapper requirement.
/// `Cell` is excluded on purpose: it isn't `Sync`, and `Params`
/// requires `Sync`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PersistWrapper {
    RwLock,
    Mutex,
}

/// A validated `#[persist]` field type: wrapper + inner scalar.
pub(crate) struct PersistType {
    pub(crate) wrapper: PersistWrapper,
    pub(crate) inner: PersistScalar,
}

/// The scalar types the persist codec can read / write.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PersistScalar {
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
    pub(crate) fn from_ident(name: &str) -> Option<Self> {
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
    pub(crate) fn byte_width(self) -> Option<usize> {
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
    pub(crate) fn type_tokens(self) -> TokenStream2 {
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
pub(crate) fn parse_persist_type(field: &syn::Field) -> Result<PersistType, TokenStream2> {
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

pub(crate) fn collect_fields(fields: &Fields) -> Result<CollectedFields, TokenStream2> {
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

#[cfg(test)]
mod tests {
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
