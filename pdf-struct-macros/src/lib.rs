use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, Ident, ItemStruct, Token, bracketed, parenthesized,
    parse::Parse,
    parse_macro_input,
    punctuated::Punctuated,
    token::{Bracket, Paren},
};

struct ObjectArgs {
    args: Vec<ObjectArg>,
}

enum ObjectArg {
    Children(Vec<Ident>),
    Metadata(Vec<Ident>),
    Parent(Ident),
    Pair(Ident),
    PageType(PageType),
    PairSequence(PairSequenceType),
    Patterns(Vec<Expr>),
}

#[derive(PartialEq)]
enum PageType {
    Key,
    Inferred,
}

enum PairSequenceType {
    First,
    Last,
    None,
}

impl Parse for ObjectArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut args = Vec::new();

        while !input.is_empty() {
            let name: Ident = input.parse()?;

            match name.to_string().as_str() {
                "children" => {
                    input.parse::<Token![=]>()?;
                    if input.peek(Paren) {
                        // tuple: children = (Type1, Type2)
                        let content;
                        parenthesized!(content in input);
                        let child_types: Punctuated<Ident, Token![,]> =
                            content.parse_terminated(Ident::parse, Token![,])?;
                        args.push(ObjectArg::Children(child_types.into_iter().collect()));
                    } else {
                        // single type: children = Type
                        let child_type: Ident = input.parse()?;
                        args.push(ObjectArg::Children(vec![child_type]));
                    }
                }
                "metadata" => {
                    input.parse::<Token![=]>()?;
                    if input.peek(Paren) {
                        let content;
                        parenthesized!(content in input);
                        let metadata_types: Punctuated<Ident, Token![,]> =
                            content.parse_terminated(Ident::parse, Token![,])?;
                        args.push(ObjectArg::Metadata(metadata_types.into_iter().collect()));
                    } else {
                        let metadata_type: Ident = input.parse()?;
                        args.push(ObjectArg::Metadata(vec![metadata_type]));
                    }
                }
                "parent" => {
                    input.parse::<Token![=]>()?;
                    let parent_type: Ident = input.parse()?;
                    args.push(ObjectArg::Parent(parent_type));
                }
                "pair" => {
                    input.parse::<Token![=]>()?;
                    let pair_type: Ident = input.parse()?;
                    args.push(ObjectArg::Pair(pair_type));
                }
                "page_type" => {
                    input.parse::<Token![=]>()?;
                    let page_type_str: Ident = input.parse()?;
                    let page_type = match page_type_str.to_string().as_str() {
                        "Key" => PageType::Key,
                        "Inferred" => PageType::Inferred,
                        _ => {
                            return Err(syn::Error::new(
                                page_type_str.span(),
                                "Expected 'Key' or 'Inferred'",
                            ));
                        }
                    };
                    args.push(ObjectArg::PageType(page_type));
                }
                "sequence" => {
                    input.parse::<Token![=]>()?;
                    let seq_str: Ident = input.parse()?;
                    let seq = match seq_str.to_string().as_str() {
                        "First" => PairSequenceType::First,
                        "Last" => PairSequenceType::Last,
                        "None" => PairSequenceType::None,
                        _ => {
                            return Err(syn::Error::new(
                                seq_str.span(),
                                "Expected 'First', 'Last', or 'None'",
                            ));
                        }
                    };
                    args.push(ObjectArg::PairSequence(seq));
                }
                "patterns" => {
                    input.parse::<Token![=]>()?;
                    // patterns = [ Pattern::Pair { first: A::TYPE, second: B::TYPE }, ... ]
                    if input.peek(Bracket) {
                        let content;
                        bracketed!(content in input);
                        let pattern_exprs: Punctuated<Expr, Token![,]> =
                            content.parse_terminated(Expr::parse, Token![,])?;
                        args.push(ObjectArg::Patterns(pattern_exprs.into_iter().collect()));
                    } else {
                        return Err(syn::Error::new(
                            name.span(),
                            "Unexpected token expected bracket(s)",
                        ));
                    }
                }
                _ => return Err(syn::Error::new(name.span(), "Unknown argument")),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(ObjectArgs { args })
    }
}

#[proc_macro_attribute]
pub fn object(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as ObjectArgs);
    let input_struct = parse_macro_input!(input as ItemStruct);
    let struct_name = &input_struct.ident;
    let mut generated_impls = Vec::new();
    let mut children_types = Vec::new();
    let mut metadata_types = Vec::new();
    let mut parent_type = quote! { () };
    let mut pair_type = quote! { () };
    let mut pair_sequence = quote! { PairSequence::None };
    let mut patterns = Vec::new();
    let mut page_type = PageType::Inferred;

    for arg in args.args {
        match arg {
            ObjectArg::Children(child_types) => {
                children_types = child_types;
                generated_impls.push(quote! {
                    impl Parent for #struct_name {}
                });
            }
            ObjectArg::Metadata(meta_types) => {
                metadata_types = meta_types;
            }
            ObjectArg::Parent(parent) => {
                parent_type = quote! { #parent };

                generated_impls.push(quote! {
                    impl Child for #struct_name {}
                });
            }
            ObjectArg::Pair(pair) => {
                pair_type = quote! { #pair };
                if matches!(pair_sequence.to_string().as_str(), "PairSequence :: None") {
                    pair_sequence = quote! { PairSequence::First };
                }
            }
            ObjectArg::PageType(pt) => {
                page_type = pt;
            }
            ObjectArg::PairSequence(seq) => {
                pair_sequence = match seq {
                    PairSequenceType::First => quote! { PairSequence::First },
                    PairSequenceType::Last => quote! { PairSequence::Last },
                    PairSequenceType::None => quote! { PairSequence::None },
                };
            }
            ObjectArg::Patterns(pattern_types) => {
                patterns = pattern_types;
            }
        }
    }

    // (combine metadata and children, with metadata first)
    let all_children: Vec<_> = metadata_types.iter().chain(children_types.iter()).collect();
    let children_array = if all_children.is_empty() {
        quote! { &[] }
    } else {
        quote! { &[#(#all_children::TYPE),*] }
    };

    let pattern_items = patterns.iter().map(|expr| quote! { #expr });
    let patterns_array = if patterns.is_empty() {
        quote! { &[] }
    } else {
        quote! { &[#(#pattern_items),*] }
    };

    if !matches!(pair_type.to_string().as_str(), "()") {
        generated_impls.push(quote! {
            impl PairWith<#pair_type> for #struct_name {
                const SEQUENCE: PairSequence = #pair_sequence;
                const PATTERNS: &'static [Pattern] = #patterns_array;
            }
        });
    }

    let key = page_type == PageType::Key;
    let inferred = page_type == PageType::Inferred;

    let object_impl = quote! {
        impl Object for #struct_name {
            const CHILDREN: &'static [TypeInformation] = #children_array;
            const TYPE: TypeInformation = TypeInformation {
                id: std::any::TypeId::of::<Self>(),
                ident: stringify!(#struct_name),
            };
            const INFERRED_PAGE: bool = #key;
            const KEY_PAGE: bool = #inferred;



            type Parent = #parent_type;
            type Pair = #pair_type;
        }
    };

    let expanded = quote! {
        #input_struct

        #object_impl

        #(#generated_impls)*
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn root(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input_struct = parse_macro_input!(input as ItemStruct);
    let struct_name = &input_struct.ident;

    let expanded = quote! {
        #input_struct

        impl Root for #struct_name {}
    };

    TokenStream::from(expanded)
}
