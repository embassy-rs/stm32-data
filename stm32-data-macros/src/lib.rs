use proc_macro2::TokenStream;
use quote::quote;
use syn;
use syn::Data;

#[proc_macro_derive(EnumDebug)]
pub fn enum_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse(input).unwrap();

    impl_enum_derive(&ast).into()
}

fn impl_enum_derive(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let enumm = match &ast.data {
        Data::Enum(e) => e,
        _ => unreachable!(),
    };

    let match_variants: TokenStream = enumm
        .variants
        .iter()
        .map(|v| {
            let variant_name = &v.ident;
            let variant_debug = format!("{}::{}", name, variant_name);

            quote! {
                #name::#variant_name => #variant_debug,
            }
        })
        .collect();

    quote! {
        #[automatically_derived]
        impl ::core::fmt::Debug for #name {
            fn fmt(self: &Self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::write_str(f, match self {
                    #match_variants
                })
            }
        }
    }
    .into()
}
