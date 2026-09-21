use crate::{generate_descriptors_to_file, generate_to_file};
use pretty_assertions::assert_eq;
use std::io::BufWriter;
use std::path::PathBuf;
use std::{env, fs};

/// Compares generated output against a checked-in reference file.
fn run_test(
    json_dictionary: &str,
    file_path: &str,
    emit: fn(&fprime_dictionary::Dictionary, &mut BufWriter<Vec<u8>>),
) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("src/test");

    let mut json_file = path.clone();
    json_file.push(json_dictionary);
    json_file.set_extension("json");

    // Appended, not `set_extension`: a ref name may itself contain a dot
    // (`RefTopologyDictionary.descriptors`).
    let mut ref_file = path.clone();
    ref_file.push(format!("{file_path}.ref.rs"));

    let mut buf = BufWriter::new(Vec::new());
    let dict = fprime_dictionary::parse(&json_file);

    emit(&dict, &mut buf);
    let output = String::from_utf8(buf.into_inner().expect("failed to get bytes"))
        .expect("failed to decode file");

    match env::var("FPRIME_UPDATE_REF") {
        Ok(_) => fs::write(ref_file, output).expect("failed to write ref.rs"),
        Err(_) => {
            let ref_txt = fs::read_to_string(ref_file).expect("failed to read ref.rs");
            assert_eq!(ref_txt, output)
        }
    }
}

#[test]
fn ref_topology() {
    run_test(
        "../../../fprime_dictionary/src/test/RefTopologyDictionary",
        "RefTopologyDictionary",
        generate_to_file,
    )
}

/// The `Desc` tree, which only a host build compiles.
#[test]
fn ref_topology_descriptors() {
    run_test(
        "../../../fprime_dictionary/src/test/RefTopologyDictionary",
        "RefTopologyDictionary.descriptors",
        generate_descriptors_to_file,
    )
}

/// The flight API must never mention the test descriptors.
#[test]
fn flight_api_has_no_descriptors() {
    let dict = fprime_dictionary::parse(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../fprime_dictionary/src/test/RefTopologyDictionary.json"),
    );

    let mut buf = BufWriter::new(Vec::new());
    generate_to_file(&dict, &mut buf);
    let flight = String::from_utf8(buf.into_inner().expect("bytes")).expect("utf8");

    for forbidden in ["pub mod Desc", "desc::Chan", "desc::Prm", "desc::CmdDesc"] {
        assert!(
            !flight.contains(forbidden),
            "the flight API contains `{forbidden}`, which belongs in descriptors.rs"
        );
    }

    // And the descriptors must actually contain them, or this test passes vacuously.
    let mut buf = BufWriter::new(Vec::new());
    generate_descriptors_to_file(&dict, &mut buf);
    let descriptors = String::from_utf8(buf.into_inner().expect("bytes")).expect("utf8");
    assert!(descriptors.contains("pub mod Desc"), "{descriptors:.200}");
    assert!(descriptors.contains("desc::Chan"));
}
