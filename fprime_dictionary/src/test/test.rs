use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::{env, fs};

pub(crate) fn run_test(file_path: &str) {
    // Compute the path to the FPP input and .ref.txt output
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("src/test");

    let mut json_file = path.clone();
    json_file.push(file_path);
    json_file.set_extension("json");

    let mut ref_file = path.clone();
    ref_file.push(file_path);
    ref_file.set_extension("ref.txt");

    let output = format!("{:#?}", crate::parse(&json_file));

    // Validate the diagnostic messages against the reference file
    match env::var("FPRIME_UPDATE_REF") {
        Ok(_) => {
            // Update the ref file
            fs::write(ref_file, output).expect("failed to write ref.txt")
        }
        Err(_) => {
            // Read and compare against the ref file
            let ref_txt = fs::read_to_string(ref_file).expect("failed to read ref.txt");
            assert_eq!(ref_txt, output)
        }
    }
}

#[test] // Marks this function as a test
fn ref_topology() {
    run_test("RefTopologyDictionary")
}

/// A dictionary from before F Prime exported `FwSizeStoreType` prefixed strings with a `U16`.
#[test]
fn size_store_defaults_to_u16() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/test/RefTopologyDictionary.json");
    let dict = crate::parse(&path);

    assert!(!dict.type_definitions.contains_key(crate::SIZE_STORE_TYPE));
    assert_eq!(dict.size_store_kind(), crate::IntegerKind::U16);
    assert_eq!(dict.size_store_bytes(), 2);
}

/// Current F Prime aliases `FwSizeStoreType` to `FwSizeType`, a `U64` by default.
#[test]
fn size_store_follows_the_alias() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/test/RefTopologyDictionary.json");
    let mut dict = crate::parse(&path);
    dict.type_definitions.insert(
        crate::SIZE_STORE_TYPE.to_string(),
        crate::TypeDefinition::Alias(crate::AliasType {
            qualified_name: crate::SIZE_STORE_TYPE.to_string(),
            type_name: crate::TypeName::QualifiedIdentifier {
                name: "FwSizeType".to_string(),
            },
            underlying_type: crate::TypeName::Integer {
                name: crate::IntegerKind::U64,
            },
            annotation: None,
        }),
    );

    assert_eq!(dict.size_store_kind(), crate::IntegerKind::U64);
    assert_eq!(dict.size_store_bytes(), 8);
}
