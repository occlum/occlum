extern crate proc_macro;

use proc_macro2::{Punct, Spacing, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::parse::{Parse, ParseStream};
use syn::{Block, FnArg, Ident, ImplItem, Item, ItemImpl, Pat, Stmt};

#[proc_macro_attribute]
pub fn inherit_methods(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item_impl = syn::parse_macro_input!(item as syn::ItemImpl);
    do_inherit_methods(item_impl).into()
}

fn do_inherit_methods(mut item_impl: ItemImpl) -> TokenStream {
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

        let new_fn_tokens = quote! {
            {
                self.0.#fn_name(#args_tokens)
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
