use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{
    Ident, ItemStruct, Token, parenthesized, parse::Parse, parse_macro_input,
    punctuated::Punctuated,
};
struct ObjectArgs {
    args: Vec<ObjectArg>,
}

enum ObjectArg {
    Children(Vec<Ident>),
    Metadata(Vec<Ident>),
    Parent(Ident),
    Pair(Ident),
    Root,
    Type(Ident),
}

impl Parse for ObjectArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut args = Vec::new();

        while !input.is_empty() {
            let name: Ident = input.parse()?;

            match name.to_string().as_str() {
                "root" => {
                    args.push(ObjectArg::Root);
                }
                "children" | "metadata" | "parent" | "pair" | "object_type" => {
                    input.parse::<Token![=]>()?;

                    match name.to_string().as_str() {
                        "children" => {
                            if input.peek(syn::token::Paren) {
                                // Handle tuple: children = (Type1, Type2)
                                let content;
                                parenthesized!(content in input);
                                let child_types: Punctuated<Ident, Token![,]> =
                                    content.parse_terminated(Ident::parse, Token![,])?;
                                args.push(ObjectArg::Children(child_types.into_iter().collect()));
                            } else {
                                // Handle single type: children = Type
                                let child_type: Ident = input.parse()?;
                                args.push(ObjectArg::Children(vec![child_type]));
                            }
                        }
                        "metadata" => {
                            let content;
                            parenthesized!(content in input);
                            let metadata_types: Punctuated<Ident, Token![,]> =
                                content.parse_terminated(Ident::parse, Token![,])?;
                            args.push(ObjectArg::Metadata(metadata_types.into_iter().collect()));
                        }
                        "parent" => {
                            let parent_type: Ident = input.parse()?;
                            args.push(ObjectArg::Parent(parent_type));
                        }
                        "pair" => {
                            let pair_type: Ident = input.parse()?;
                            args.push(ObjectArg::Pair(pair_type));
                        }
                        "object_type" => {
                            let object_type: Ident = input.parse()?;
                            args.push(ObjectArg::Type(object_type));
                        }
                        _ => unreachable!(),
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
    let mut generated_fields = Vec::new();
    let mut generated_impls = Vec::new();
    let mut generated_structs = Vec::new();
    let mut object_type_variant = quote! { ObjectType::Inferred };
    let mut has_explicit_type = false;

    for arg in args.args {
        match arg {
            ObjectArg::Root => {
                object_type_variant = quote! { ObjectType::Root };
                has_explicit_type = true;
                generated_impls.push(quote! {
                    impl Root for #struct_name {}
                });
            }
            ObjectArg::Children(child_types) => {
                if child_types.len() == 1 {
                    let child_type = &child_types[0];
                    generated_fields.push(quote! {
                        pub children: Vec<#child_type>,
                    });
                } else {
                    let child_struct_name = quote::format_ident!("{}Children", struct_name);
                    let fields = child_types.iter().enumerate().map(|(i, child_type)| {
                        let field_name = quote::format_ident!("_{}", i);
                        quote! { pub #field_name: #child_type }
                    });

                    generated_structs.push(quote! {
                        pub struct #child_struct_name {
                            #(#fields),*
                        }
                    });

                    generated_fields.push(quote! {
                        pub children: Vec<#child_struct_name>,
                    });

                    for child_type in &child_types {
                        generated_impls.push(quote! {
                            impl Child<#child_type> for #struct_name {}
                        });
                    }
                }
            }
            ObjectArg::Type(object_type) => {
                object_type_variant = match object_type.to_string().as_str() {
                    "Key" => quote! { ObjectType::Key },
                    "Inferred" => quote! { ObjectType::Inferred },
                    _ => {
                        return syn::Error::new(
                            object_type.span(),
                            "Invalid object type. Expected 'Key' or 'Inferred'.",
                        )
                        .into_compile_error()
                        .to_token_stream()
                        .into();
                    }
                };

                has_explicit_type = true;

                match object_type.to_string().as_str() {
                    "Key" => {
                        generated_impls.push(quote! {
                            impl KeyPage for #struct_name {}
                        });
                    }
                    "Inferred" => {
                        generated_impls.push(quote! {
                            impl InferredPage for #struct_name {}
                        });
                    }
                    _ => unreachable!(),
                }
            }
            ObjectArg::Metadata(metadata_types) => {
                if metadata_types.len() > 2 {
                    return syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "Too much metadata, expected only 2 types.",
                    )
                    .into_compile_error()
                    .to_token_stream()
                    .into();
                }

                let struct_name = quote::format_ident!("{}Metadata", struct_name);
                let fields = metadata_types.iter().enumerate().map(|(i, metadata_type)| {
                    let field_name = quote::format_ident!("_{}", i);
                    quote! { pub #field_name: #metadata_type }
                });

                generated_structs.push(quote! {
                    pub struct #struct_name {
                        #(#fields),*
                    }
                });

                generated_fields.push(quote! {
                    pub metadata: Vec<#struct_name>,
                });
            }
            ObjectArg::Parent(parent_type) => {
                generated_fields.push(quote! {
                    pub parent: Box<#parent_type>,
                });

                generated_impls.push(quote! {
                    impl Parent<#struct_name> for #parent_type {}
                });
            }
            ObjectArg::Pair(pair_type) => {
                generated_impls.push(quote! {
                    impl PairWith<#pair_type> for #struct_name {
                        const SEQUENCE: PairSequence = PairSequence::First;
                    }
                });

                generated_impls.push(quote! {
                    impl PairWith<#struct_name> for #pair_type {
                        const SEQUENCE: PairSequence = PairSequence::Last;
                    }
                });
            }
        }
    }

    if !has_explicit_type {
        generated_impls.push(quote! {
            impl InferredPage for #struct_name {}
        });
    }

    let expanded = quote! {
        #(#generated_structs)*

        pub struct #struct_name {
            #(#generated_fields)*
        }

        #(#generated_impls)*

        impl Object for #struct_name {
            const OBJECT_TYPE: ObjectType = #object_type_variant;

            fn type_id() -> std::any::TypeId {
                std::any::TypeId::of::<#struct_name>()
            }
        }
    };

    TokenStream::from(expanded)
}
