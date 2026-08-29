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
//! AURA owns this derive. Param IDs are always explicit (`id = N`);
//! nested structs keep their declared IDs (collisions panic in `new()`
//! via `Params::assert_no_id_collisions`). There is no compile-time
//! `plugin_info!`, no framework `#[derive(State)]`, and no silent
//! auto-numbering.
//!
//! Implementation split across modules ([`parse`] collects fields /
//! attributes, [`codegen`] turns them into `TokenStream`s, [`params`] and
//! [`param_enum`] assemble the two derives) - the functions actually
//! tagged `#[proc_macro_derive]` stay here since that attribute is
//! only accepted on functions defined at the crate root.

#![forbid(unsafe_code)]

use proc_macro::TokenStream;

mod codegen;
mod param_enum;
mod params;
mod parse;

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
/// - plain data with `#[persist]` (or `#[persist = "key"]`) — saved in the
///   host state blob via `encode_state` / `decode_state` (v2 envelope:
///   params + persist). Field type must be `RwLock<T>` / `Mutex<T>` with
///   `T` one of `bool`, `u8`..`u64`, `i8`..`i64`, `f32`, `f64`, `String`.
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
pub fn derive_params(input: TokenStream) -> TokenStream {
    params::expand(input)
}

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
pub fn derive_param_enum(input: TokenStream) -> TokenStream {
    param_enum::expand(input)
}
