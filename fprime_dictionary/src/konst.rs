//! Which command arguments can reach the wire by const evaluation, and how.

use crate::{Command, Dictionary, TypeDefinition, TypeName};

/// Aliases followed before concluding the dictionary is cyclic.
const MAX_ALIAS_HOPS: usize = 32;

/// Suffix on a generated size function, e.g. `CMD_NO_OP__size`.
pub const SIZE_SUFFIX: &str = "__size";

/// Suffix on a generated encoder, e.g. `CMD_NO_OP__encode`.
pub const ENCODE_SUFFIX: &str = "__encode";

/// Nesting descended before concluding a type is cyclic.
const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstArg {
    /// `put_<put>(&mut buf, offset, <access>)`
    Scalar {
        put: &'static str,
    },

    /// `put_<put>(&mut buf, offset, <access> as <cast>)`, for types whose Rust
    /// representation is not already the integer that goes on the wire.
    Cast {
        put: &'static str,
        cast: &'static str,
    },

    /// `put_str(&mut buf, offset, <access>, <capacity>)`.
    Str {
        capacity: u32,
    },

    Array {
        element: Box<ConstArg>,
        size: u32,
    },

    Struct {
        members: Vec<ConstMember>,
    },
}

/// One member of a [`ConstArg::Struct`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstMember {
    pub name: String,
    pub encoding: ConstArg,
}

impl Dictionary {
    /// Resolve `type_name` through any chain of aliases.
    pub fn resolved_type<'a>(&'a self, type_name: &'a TypeName) -> Option<&'a TypeName> {
        let mut current = type_name;

        for _ in 0..MAX_ALIAS_HOPS {
            let TypeName::QualifiedIdentifier { name } = current else {
                return Some(current);
            };

            match self.type_definitions.get(name) {
                Some(TypeDefinition::Alias(alias)) => current = &alias.underlying_type,
                Some(_) => return Some(current),
                // A name the dictionary does not define is not something we can
                // encode, and is not ours to diagnose here.
                None => return None,
            }
        }

        None
    }

    /// The definition an alias chain bottoms out in, for a type that has one.
    ///
    /// `None` for a primitive, for an alias to one, and for a name the dictionary does not
    /// define — none of which carries the enumerated constants or members a caller asking this
    /// is after. [`Dictionary::resolved_type`] is the same walk stopping one step earlier.
    pub fn definition(&self, type_name: &TypeName) -> Option<&TypeDefinition> {
        match self.resolved_type(type_name)? {
            TypeName::QualifiedIdentifier { name } => self.type_definitions.get(name),
            _ => None,
        }
    }

    /// How a whole formal parameter of type `type_name` is written by a const
    /// encoder, or `None` if it cannot be.
    pub fn const_arg(&self, type_name: &TypeName) -> Option<ConstArg> {
        self.const_arg_at(type_name, true, 0)
    }

    /// `strings` says whether a `string size N` is acceptable here; see
    /// [`ConstArg::Str`].
    fn const_arg_at(&self, type_name: &TypeName, strings: bool, depth: usize) -> Option<ConstArg> {
        if depth > MAX_DEPTH {
            return None;
        }

        match self.resolved_type(type_name)? {
            TypeName::Integer { name } => Some(ConstArg::Scalar {
                put: integer_put(*name),
            }),
            TypeName::Float { name } => Some(ConstArg::Scalar {
                put: match name {
                    crate::FloatKind::F32 => "put_f32",
                    crate::FloatKind::F64 => "put_f64",
                },
            }),
            // `Serializable for bool` writes a single 0 or 1 byte.
            TypeName::Bool => Some(ConstArg::Cast {
                put: "put_u8",
                cast: "u8",
            }),
            TypeName::String { size } => strings.then_some(ConstArg::Str { capacity: *size }),
            TypeName::QualifiedIdentifier { name } => {
                // `resolved_type` stops at the first non-alias, so this names a
                // struct, array or enum definition.
                match self.type_definitions.get(name)? {
                    TypeDefinition::Enum(ty) => {
                        let TypeName::Integer { name } =
                            self.resolved_type(&ty.representation_type)?
                        else {
                            return None;
                        };

                        Some(ConstArg::Cast {
                            put: integer_put(*name),
                            cast: integer_rust_name(*name),
                        })
                    }
                    TypeDefinition::Array(ty) => Some(ConstArg::Array {
                        element: Box::new(self.const_arg_at(&ty.element_type, false, depth + 1)?),
                        size: ty.size,
                    }),
                    TypeDefinition::Struct(ty) => {
                        let members = ty
                            .members
                            .iter()
                            .map(|member| {
                                let encoding =
                                    self.const_arg_at(&member.type_name, false, depth + 1)?;

                                // A member declared as an array is that many of
                                // its type, laid out in order.
                                let encoding = match member.size {
                                    None => encoding,
                                    Some(size) => ConstArg::Array {
                                        element: Box::new(encoding),
                                        size,
                                    },
                                };

                                Some(ConstMember {
                                    name: member.name.clone(),
                                    encoding,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?;

                        Some(ConstArg::Struct { members })
                    }
                    TypeDefinition::Alias(_) => None,
                }
            }
        }
    }

    /// Whether every argument of `command` can be encoded by const evaluation.
    pub fn const_encodable(&self, command: &Command) -> bool {
        command
            .formal_params
            .iter()
            .all(|param| self.const_arg(&param.type_name).is_some())
    }
}

fn integer_put(kind: crate::IntegerKind) -> &'static str {
    use crate::IntegerKind::*;

    match kind {
        U8 => "put_u8",
        I8 => "put_i8",
        U16 => "put_u16",
        I16 => "put_i16",
        U32 => "put_u32",
        I32 => "put_i32",
        U64 => "put_u64",
        I64 => "put_i64",
    }
}

fn integer_rust_name(kind: crate::IntegerKind) -> &'static str {
    use crate::IntegerKind::*;

    match kind {
        U8 => "u8",
        I8 => "i8",
        U16 => "u16",
        I16 => "i16",
        U32 => "u32",
        I32 => "i32",
        U64 => "u64",
        I64 => "i64",
    }
}
