//! Inherit methods from a field automatically (via procedural macros).
//!
//! # Motivation
//!
//! While Rust is partially inspired by the object-oriented programming (OOP) paradigm
//! and has some typical OOP features (like objects, encapsulation, and polymorphism),
//! it is not an OOP language. One piece of evidence is the lack of _inheritance_, which an
//! important pillar of OOP. But don't take me wrong: this lack of inheritance is actually a
//! good thing since it promotes the practice of
//! [_composition over inheritance_](https://en.wikipedia.org/wiki/Composition_over_inheritance)
//! in Rust programs. Despite all the benefits of composition, Rust programmers
//! have to write trivial [fowarding methods](https://en.wikipedia.org/wiki/Forwarding_(object-oriented_programming),
//! which is a tedious task, especially when you have to write many of them.
//!
//! To address this pain point of using composition in Rust, the crate provides a convenient
//! procedure macro that generates forwarding methods automatically for you. In other words,
//! your structs can now "inherit" methods from their fields, enjoying the best of both worlds:
//! the convenience of inheritance and the flexibility of composition.
//!
//! # Examples
//!
//! ## Implementing the new type idiom
//!
//! Suppose that you want to create new struct named `Stack<T>`, which can be implemented by
//! simply wrapping around `Vec<T>` and exposing only a subset of the APIs of `Vec`. Here is
//! how this crate can help you do it easily.
//!
//! ```rust
//! use inherit_methods_macro::inherit_methods;
//!
//! pub struct Stack<T>(Vec<T>);
//!
//! // Annotate an impl block with #[inherit_methods(from = "...")] to enable automatically
//! // inheriting methods from a field, which is specifiedd by the from attribute.
//! #[inherit_methods(from = "self.0")]
//! impl<T> Stack<T> {
//!     // Normal methods can be implemented with inherited methods in the same impl block.
//!     pub fn new() -> Self {
//!         Self(Vec::new())
//!     }
//!
//!     // All methods without code blocks will "inherit" the implementation of Vec by
//!     // forwarding their method calls to self.0.
//!     fn push(&mut self, value: T);
//!     fn pop(&mut self) -> Option<T>;
//!     fn len(&self) -> usize;
//! }
//! ```
//!
//! If you want to derive common traits (like `AsRef` and `Deref`) for a new type, check out
//! the [shrinkwraprs](https://crates.io/crates/shrinkwraprs) crate.
//!
//! ## Emulating the classic OOP inheritance
//!
//! In many OOP frameworks or applications, it is useful to have a base class from which all objects
//! inherit. In this example, we would like to do the same thing, creating a base class
//! (the `Object` trait for the interface and the `ObjectBase` struct for the implementation).
//! that all objects should "inherit".
//!
//! ```rust
//! use std::sync::atomic::{AtomicU64, Ordering};
//! use std::sync::Mutex;
//!
//! use inherit_methods_macro::inherit_methods;
//!
//! pub trait Object {
//!     fn type_name(&self) -> &'static str;
//!     fn object_id(&self) -> u64;
//!     fn name(&self) -> String;
//!     fn set_name(&self, new_name: String);
//! }
//!
//! struct ObjectBase {
//!     object_id: u64,
//!     name: Mutex<String>,
//! }
//!
//! impl ObjectBase {
//!     pub fn new() -> Self {
//!         static NEXT_ID: AtomicU64 = AtomicU64::new(0);
//!         Self {
//!             object_id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
//!             name: Mutex::new(String::new()),
//!         }
//!     }
//!
//!     pub fn object_id(&self) -> u64 {
//!         self.object_id
//!     }
//!
//!     pub fn name(&self) -> String {
//!         self.name.lock().unwrap().clone()
//!     }
//!
//!     pub fn set_name(&self, new_name: String) {
//!         *self.name.lock().unwrap() = new_name;
//!     }
//! }
//!
//! struct DummyObject {
//!     base: ObjectBase,
//! }
//!
//! impl DummyObject {
//!     pub fn new() -> Self {
//!         Self {
//!             base: ObjectBase::new(),
//!         }
//!     }
//! }
//!
//! #[inherit_methods(from = "self.base")]
//! impl Object for DummyObject {
//!     fn type_name(&self) -> &'static str {
//!         "DummyObject"
//!     }
//!
//!     // Inherit methods from the base class
//!     fn object_id(&self) -> u64;
//!     fn name(&self) -> String;
//!     fn set_name(&self, new_name: String);
//! }
//! ```

extern crate proc_macro;

use darling::FromMeta;
use proc_macro2::{Punct, Spacing, Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::parse::{Parse, ParseStream};
use syn::{AttributeArgs, Block, Expr, FnArg, Ident, ImplItem, Item, ItemImpl, Pat, Stmt};

#[derive(Debug, FromMeta)]
struct MacroAttr {
    #[darling(default = "default_from_val")]
    from: String,
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

        let field: Expr = syn::parse_str(&attr.from).unwrap();
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
