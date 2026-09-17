use crate::generate_to_file;
use pretty_assertions::assert_eq;
use std::io::BufWriter;
use std::path::PathBuf;
use std::{env, fs};

pub(crate) fn run_test(json_dictionary: &str, file_path: &str) {
    // Compute the path to the FPP input and .ref.txt output
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("src/test");

    let mut json_file = path.clone();
    json_file.push(json_dictionary);
    json_file.set_extension("json");

    let mut ref_file = path.clone();
    ref_file.push(file_path);
    ref_file.set_extension("ref.rs");

    let mut buf = BufWriter::new(Vec::new());
    let dict = fprime_dictionary::parse(&json_file);

    generate_to_file(&dict, &mut buf);
    let output = String::from_utf8(buf.into_inner().expect("failed to get bytes"))
        .expect("failed to decode file");

    // Validate the diagnostic messages against the reference file
    match env::var("FPRIME_UPDATE_REF") {
        Ok(_) => {
            // Update the ref file
            fs::write(ref_file, output).expect("failed to write ref.rs")
        }
        Err(_) => {
            // Read and compare against the ref file
            let ref_txt = fs::read_to_string(ref_file).expect("failed to read ref.rs");
            assert_eq!(ref_txt, output)
        }
    }
}

#[test] // Marks this function as a test
fn ref_topology() {
    run_test(
        "../../../fprime_dictionary/src/test/RefTopologyDictionary",
        "RefTopologyDictionary",
    )
}
