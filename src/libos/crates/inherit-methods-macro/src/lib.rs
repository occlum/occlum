extern crate proc_macro;

use darling::FromMeta;
use proc_macro2::{Punct, Spacing, Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::parse::{Parse, ParseStream};
use syn::{AttributeArgs, Block, Expr, FnArg, Ident, ImplItem, Item, ItemImpl, Pat, Stmt};

#[derive(Debug, FromMeta)]
struct MacroAttr {
    #[darling(default = "default_from_val")]
    from_field: String,
}

fn default_from_val() -> String {
    "self.0".to_string()
}

#[proc_macro_attribute]
pub fn inherit_methods(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attr = {
        let attr_tokens = syn::parse_macro_input!(attr as AttributeArgs);
        match MacroAttr::from_list(&attr_tokens) {
            Ok(attr) => attr,
            Err(e) => {
                return TokenStream::from(e.write_errors()).into();
            }
        }
    };
    let item_impl = syn::parse_macro_input!(item as syn::ItemImpl);
    do_inherit_methods(attr, item_impl).into()
}

fn do_inherit_methods(attr: MacroAttr, mut item_impl: ItemImpl) -> TokenStream {
    for impl_item in &mut item_impl.items {
        let impl_item_method = if let ImplItem::Method(method) = impl_item {
            method
        } else {
            continue;
        };
        let sig = &impl_item_method.sig;
        let fn_name = &sig.ident;
        let fn_args: Vec<&Ident> = sig
            .inputs
            .iter()
            .filter_map(|fn_arg| match fn_arg {
                FnArg::Receiver(_) => None,
                FnArg::Typed(pat_type) => Some(pat_type),
            })
            .filter_map(|pat_type| match &*pat_type.pat {
                Pat::Ident(pat_ident) => Some(&pat_ident.ident),
                _ => None,
            })
            .collect();

        let mut args_tokens = TokenStream::new();
        for fn_arg in fn_args {
            let fn_arg: Ident = fn_arg.clone();
            args_tokens.append(fn_arg);
            args_tokens.append(Punct::new(',', Spacing::Alone));
        }

        let field: Expr = syn::parse_str(&attr.from_field).unwrap();
        let new_fn_tokens = quote! {
            {
                #field.#fn_name(#args_tokens)
            }
        };
        let new_fn_block: Block = syn::parse(new_fn_tokens.into()).unwrap();

        if impl_item_method.block.is_empty() {
            impl_item_method.block = new_fn_block;
        }
    }
    item_impl.into_token_stream()
}

trait BlockExt {
    /// Check if a block is empty, which means only contains a ";".
    fn is_empty(&self) -> bool;
}

impl BlockExt for Block {
    fn is_empty(&self) -> bool {
        if self.stmts.len() == 0 {
            return true;
        }
        if self.stmts.len() > 1 {
            return false;
        }

        if let Stmt::Item(item) = &self.stmts[0] {
            if let Item::Verbatim(token_stream) = item {
                token_stream.to_string().trim() == ";"
            } else {
                false
            }
        } else {
            false
        }
    }
}
