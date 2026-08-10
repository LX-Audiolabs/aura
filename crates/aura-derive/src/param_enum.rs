//! `#[derive(ParamEnum)]` expansion. Self-contained - doesn't share
//! any parsing/codegen helpers with the `Params` derive
//! ([`crate::parse`] / [`crate::codegen`] / [`crate::params`]).

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Lit};

#[allow(clippy::too_many_lines)]
pub(crate) fn expand(input: TokenStream) -> TokenStream {
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

    let expanded: TokenStream2 = quote! {
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
