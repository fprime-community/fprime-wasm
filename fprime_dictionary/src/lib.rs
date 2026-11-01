use serde::Deserialize;
use serde::de::Error;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

pub mod konst;
pub mod naming;

/// Environment variable that `fprime_build::generate` the dictionary.
pub const DICTIONARY_ENV: &str = "FPRIME_DICTIONARY";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub deployment_name: String,
    pub framework_version: String,
    pub project_version: String,
    pub library_versions: Vec<String>,
    pub dictionary_spec_version: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum IntegerKind {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum FloatKind {
    F32,
    F64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TypeName {
    Integer { name: IntegerKind },
    Float { name: FloatKind },
    Bool,
    String { size: u32 },
    QualifiedIdentifier { name: String },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Array(Vec<Value>),
    Struct(BTreeMap<String, Value>),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumConstant {
    pub name: String,
    pub value: i64,

    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StructMemberRaw {
    #[serde(rename = "type")]
    pub type_name: TypeName,
    pub index: u32,
    pub size: Option<u32>,
    pub format: Option<String>,

    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructMember {
    pub name: String,
    pub type_name: TypeName,
    pub index: u32,
    pub size: Option<u32>,
    pub format: Option<String>,

    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArrayType {
    pub qualified_name: String,
    pub size: u32,
    pub element_type: TypeName,
    pub default: Vec<Value>,

    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumType {
    pub qualified_name: String,
    pub representation_type: TypeName,
    pub enumerated_constants: Vec<EnumConstant>,
    pub default: String,

    pub annotation: Option<String>,
}

fn deserialize_struct_members<'de, D>(deserializer: D) -> Result<Vec<StructMember>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let initial_map: HashMap<String, StructMemberRaw> = HashMap::deserialize(deserializer)?;
    let mut initial: Vec<(String, StructMemberRaw)> = initial_map.into_iter().collect();
    initial.sort_by(|a, b| a.1.index.cmp(&b.1.index));

    initial
        .into_iter()
        .enumerate()
        .map(|(index, (name, member))| {
            if index != member.index as usize {
                Err(D::Error::custom(format!(
                    "Missing struct member with index {}",
                    index
                )))
            } else {
                Ok(StructMember {
                    name,
                    type_name: member.type_name,
                    index: member.index,
                    size: member.size,
                    format: member.format,
                    annotation: member.annotation,
                })
            }
        })
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructType {
    pub qualified_name: String,
    #[serde(deserialize_with = "deserialize_struct_members")]
    pub members: Vec<StructMember>,
    pub default: Option<Value>,

    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasType {
    pub qualified_name: String,
    #[serde(rename = "type")]
    pub type_name: TypeName,
    pub underlying_type: TypeName,

    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TypeDefinition {
    Array(ArrayType),
    Enum(EnumType),
    Struct(StructType),
    Alias(AliasType),
}

impl TypeDefinition {
    pub fn qualified_name(&self) -> &str {
        match self {
            TypeDefinition::Array(a) => &a.qualified_name,
            TypeDefinition::Enum(e) => &e.qualified_name,
            TypeDefinition::Struct(s) => &s.qualified_name,
            TypeDefinition::Alias(a) => &a.qualified_name,
        }
    }
}

fn deserialize_type_definitions<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, TypeDefinition>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let initial: Vec<TypeDefinition> = Vec::deserialize(deserializer)?;
    let mut out = BTreeMap::new();

    for definition in initial {
        let qualified_name = definition.qualified_name().to_string();
        if out.insert(qualified_name.clone(), definition).is_some() {
            return Err(D::Error::custom(format!(
                "Duplicate type definition {}",
                qualified_name
            )));
        }
    }

    Ok(out)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Constant {
    pub qualified_name: String,
    #[serde(rename = "type")]
    pub type_name: TypeName,
    pub value: Value,

    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum CommandKind {
    Async,
    Guarded,
    Sync,
    Set,
    Save,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum QueueFull {
    Assert,
    Block,
    Drop,
    Hook,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormalParam {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: TypeName,
    #[serde(rename = "ref")]
    pub is_ref: bool,

    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub name: String,
    #[serde(rename = "commandKind")]
    pub kind: CommandKind,
    pub opcode: u64,
    pub formal_params: Vec<FormalParam>,
    pub priority: Option<i64>,
    pub queue_full_behavior: Option<QueueFull>,

    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: TypeName,
    pub id: u64,
    pub default: Option<Value>,

    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryChannel {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: TypeName,
    pub id: u64,

    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dictionary {
    pub metadata: Metadata,
    #[serde(deserialize_with = "deserialize_type_definitions")]
    pub type_definitions: BTreeMap<String, TypeDefinition>,
    pub constants: Vec<Constant>,
    pub commands: Vec<Command>,
    pub parameters: Vec<Parameter>,
    pub telemetry_channels: Vec<TelemetryChannel>,
}

/// The dictionary type F Prime serializes a string's length as.
pub const SIZE_STORE_TYPE: &str = "FwSizeStoreType";

impl Dictionary {
    /// The integer a string's length prefix is serialized as: [`SIZE_STORE_TYPE`], or `U16`
    /// for a dictionary from before F Prime exported it, since that is what it was then.
    pub fn size_store_kind(&self) -> IntegerKind {
        use IntegerKind::*;

        let Some(definition) = self.type_definitions.get(SIZE_STORE_TYPE) else {
            return U16;
        };

        match definition {
            TypeDefinition::Alias(AliasType {
                underlying_type: TypeName::Integer { name },
                ..
            }) if matches!(name, U8 | U16 | U32 | U64) => *name,
            _ => panic!("{SIZE_STORE_TYPE} must be an alias of an unsigned integer"),
        }
    }

    /// Width in bytes of a string's length prefix; see [`Dictionary::size_store_kind`].
    pub fn size_store_bytes(&self) -> u32 {
        use IntegerKind::*;

        match self.size_store_kind() {
            U8 | I8 => 1,
            U16 | I16 => 2,
            U32 | I32 => 4,
            U64 | I64 => 8,
        }
    }

    pub fn underlying_type<'a>(&'a self, type_name: &'a TypeName) -> &'a TypeName {
        match type_name {
            TypeName::QualifiedIdentifier { name } => match self.type_definitions.get(name) {
                Some(TypeDefinition::Alias(alias)) => &alias.underlying_type,
                _ => type_name,
            },
            _ => type_name,
        }
    }
}

/// Load a dictionary
pub fn try_parse(json_file: &Path) -> Result<Dictionary, String> {
    let contents =
        fs::read_to_string(json_file).map_err(|err| format!("{}: {}", json_file.display(), err))?;

    serde_json::from_str(&contents).map_err(|err| {
        format!(
            "{}:{}:{} {}",
            json_file.display(),
            err.line(),
            err.column(),
            err
        )
    })
}

pub fn parse(json_file: &Path) -> Dictionary {
    match try_parse(json_file) {
        Ok(d) => d,
        Err(err) => panic!("{}", err),
    }
}

#[cfg(test)]
mod test {
    mod test;
}
