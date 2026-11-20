use crate::util::str_to_ident;
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

/// Name of the generated struct holding the definitions under `scope`
fn scope_type_name(scope: &[String]) -> proc_macro2::Ident {
    str_to_ident(&scope.join("_"))
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
            let name = scope_type_name(&scope);

            let member_name_ty: Vec<_> = members
                .iter()
                .map(|member_name| {
                    let mut member_scope = scope.clone();
                    member_scope.push(member_name.clone());
                    let ty_name = scope_type_name(&member_scope);
                    let member_name = str_to_ident(&member_name);

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
                let type_name = scope_type_name(&[top_level_mod.clone()]);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Struct names keep the dictionary's spelling, so the top-level const
    /// names the same struct as its definition.
    #[test]
    fn struct_names_keep_dictionary_spelling() {
        let tree: CodeTree = vec![Definition {
            qualifier: vec!["Widget2x".into(), "mgr".into()],
            tokens: quote! {},
        }]
        .into();

        let out = tree.struct_nesting("Cmd").to_string();
        assert!(out.contains("pub struct Widget2x "), "{out}");
        assert!(out.contains("pub mgr : Widget2x_mgr"), "{out}");
        assert!(
            out.contains("pub const Widget2x : Widget2x = Widget2x :: DEFAULT"),
            "{out}"
        );
    }
}
