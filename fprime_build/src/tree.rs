use crate::util::str_to_ident;
use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::BTreeMap;

pub type Qualifier = Vec<String>;

pub struct Definition {
    pub qualifier: Qualifier,
    pub tokens: TokenStream,
}

pub struct CodeTree {
    leafs: Vec<TokenStream>,
    modules: BTreeMap<String, CodeTree>,
}

/// Consolidate definitions under the same module qualifier into a tree structure
impl From<Vec<Definition>> for CodeTree {
    fn from(value: Vec<Definition>) -> Self {
        let mut root = CodeTree {
            leafs: vec![],
            modules: Default::default(),
        };

        for def in value {
            let mut current_node = &mut root;

            // Traverse the tree from the root to the qualified point
            for q in def.qualifier {
                match current_node.modules.get_mut(&q) {
                    None => {
                        // Ensure we can traverse down the tree
                        current_node.modules.insert(
                            q.clone(),
                            CodeTree {
                                leafs: vec![],
                                modules: Default::default(),
                            },
                        );
                    }
                    Some(_) => {}
                }

                current_node = current_node.modules.get_mut(&q).unwrap()
            }

            current_node.leafs.push(def.tokens);
        }

        root
    }
}

impl CodeTree {
    fn module_nesting_impl(self) -> TokenStream {
        let modules: Vec<TokenStream> = self
            .modules
            .into_iter()
            .map(|(q, g)| {
                let inner: TokenStream = g.module_nesting_impl();
                let mod_name = str_to_ident(&q);
                quote! {
                    pub mod #mod_name {
                        #inner
                    }
                }
            })
            .collect();

        let leafs = self.leafs;
        quote! {
            #[allow(unused_imports)]
            use fprime_core::*;

            #(#leafs)*
            #(#modules)*
        }
    }

    pub fn module_nesting(self, root: &str) -> TokenStream {
        let inner = self.module_nesting_impl();
        let root = str_to_ident(root);

        quote! {
            pub mod #root {
                #inner
            }
        }
    }

    fn struct_nesting_impl(self, scope: Qualifier) -> TokenStream {
        let members = self.modules.keys().cloned().collect::<Vec<_>>();

        let inner: Vec<_> = self
            .modules
            .into_iter()
            .map(|(next_ident, next_tree)| {
                let mut next_scope = scope.clone();
                next_scope.push(next_ident);

                next_tree.struct_nesting_impl(next_scope)
            })
            .collect();

        if scope.is_empty() {
            if !self.leafs.is_empty() {
                let leafs = self.leafs;
                let leafs = quote! { #(#leafs)* };
                assert!(false, "{}", leafs.to_string());
            }

            quote! {
                #(#inner)*
            }
        } else {
            // Build a private struct with `_` concatenated naming for holding the impls
            let name = str_to_ident(&scope.join("_").to_case(Case::Pascal));

            let member_name_ty: Vec<_> = members
                .iter()
                .map(|member_name| {
                    let member_name = str_to_ident(&member_name);
                    let ty_name = str_to_ident(
                        &format!("{}_{}", scope.join("_"), member_name.to_string())
                            .to_case(Case::Pascal),
                    );

                    (member_name, ty_name)
                })
                .collect();

            let member_defs = member_name_ty.iter().map(|(name, ty)| {
                quote! {
                    pub #name: #ty,
                }
            });

            let default_const_members = member_name_ty.iter().map(|(name, ty)| {
                quote! {
                    #name: #ty::DEFAULT,
                }
            });

            let leafs = self.leafs;
            quote! {
                #(#inner)*

                pub struct #name {
                    #(#member_defs)*
                }

                impl #name {
                    pub const DEFAULT: Self = Self {
                        #(#default_const_members)*
                    };

                    #(#leafs)*
                }
            }
        }
    }

    pub fn struct_nesting(self, ns: &str) -> TokenStream {
        // Encode the top-level defs as ZST-consts
        let consts: Vec<_> = self
            .modules
            .keys()
            .map(|top_level_mod| {
                let type_name = str_to_ident(top_level_mod);
                let var_name = str_to_ident(top_level_mod);

                quote! {
                    pub const #var_name: #type_name = #type_name::DEFAULT;
                }
            })
            .collect();

        let defs = self.struct_nesting_impl(vec![]);
        let ns = str_to_ident(ns);

        quote! {
            pub mod #ns {
                #[allow(unused_imports)]
                use fprime_core::*;

                #[allow(unused_imports)]
                use super::Dict::FwOpcodeType;

                #defs

                #(#consts)*
            }
        }
    }
}
