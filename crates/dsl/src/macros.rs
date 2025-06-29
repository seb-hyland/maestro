use proc_macro::TokenStream;
use quote::quote;
use syn::{LitStr, parse::Parse, parse_macro_input, punctuated::Punctuated, token::Comma};

struct MacroLits(Punctuated<LitStr, Comma>);
impl Parse for MacroLits {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(MacroLits(Punctuated::parse_terminated(input)?))
    }
}

#[proc_macro]
pub fn paths(input: TokenStream) -> TokenStream {
    let MacroLits(path_lits) = parse_macro_input!(input as MacroLits);
    let path_lits = path_lits.iter();
    quote! {
        vec![
            #(
                ::std::path::PathBuf::from(#path_lits)
            ),*
        ]
    }
    .into()
}
