//! `#[derive(Params)]` expansion: validates the fields
//! [`crate::parse::collect_fields`] collected, then assembles the
//! `Params` trait impl from the [`crate::codegen`] codegen helpers.

use std::collections::HashSet;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput};

// Mirrors `aura_params::METER_ID_BASE` so the derive does not depend on
// `aura-params` at proc-macro build time (avoids linking the parameter crate
// into the proc-macro DLL). Keep in sync with `aura_params::METER_ID_BASE`.
const METER_ID_BASE: u32 = 1 << 24;

use crate::codegen::{
    gen_field_constructor, gen_param_id_enum, gen_param_info_literal, gen_persist_read,
    gen_persist_write,
};
use crate::parse::{MidiBindKind, ParamKind, collect_fields};

#[allow(clippy::too_many_lines)]
pub(crate) fn expand(input: TokenStream) -> TokenStream {
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
                    return syn::Error::new(f.ident.span(), msg)
                        .to_compile_error()
                        .into();
                }
                if !seen_ids.insert(id) {
                    let msg = format!("Duplicate parameter ID: {id}");
                    return syn::Error::new(f.ident.span(), msg)
                        .to_compile_error()
                        .into();
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
    let set_plain_arms: Vec<_> = param_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            match f.kind {
                ParamKind::Float => {
                    quote! { x if x == self.#ident.id() => self.#ident.set_value(value), }
                }
                ParamKind::Bool => {
                    quote! { x if x == self.#ident.id() => self.#ident.set_value(value > 0.5), }
                }
                ParamKind::Int => quote! { x if x == self.#ident.id() => {
                    #[allow(clippy::cast_possible_truncation)]
                    self.#ident.set_value(value.round() as i64);
                }, },
                ParamKind::Enum => quote! { x if x == self.#ident.id() => {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    self.#ident.set_index(value.round() as u32);
                }, },
            }
        })
        .collect();

    let set_plain_fallthrough = if nested_fields.is_empty() {
        quote! { _ => {} }
    } else {
        quote! {
            _ => {
                #(::aura::params::Params::set_plain(&self.#nested_idents, id, value);)*
            }
        }
    };

    // --- set_mod (FloatParam mono modulation; other kinds ignore) ---
    let set_mod_arms: Vec<_> = param_fields
        .iter()
        .filter(|f| matches!(f.kind, ParamKind::Float))
        .map(|f| {
            let ident = &f.ident;
            quote! { x if x == self.#ident.id() => self.#ident.set_mod_amount(amount), }
        })
        .collect();

    let set_mod_fallthrough = if nested_fields.is_empty() {
        quote! { _ => {} }
    } else {
        quote! {
            _ => {
                #(::aura::params::Params::set_mod(&self.#nested_idents, id, amount);)*
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
                        ::aura::params::parse_param_value(&self.#ident.info, text)
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

            fn set_mod(&self, id: u32, amount: f64) {
                match id {
                    #(#set_mod_arms)*
                    #set_mod_fallthrough
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
