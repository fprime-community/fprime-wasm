//! Section byte counts, read with `wasmparser`.

use std::ops::Range;
use wasmparser::{Parser, Payload};

/// The section byte counts of one module.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Sections {
    /// Every byte of the file, including the header and section framing.
    pub total: usize,
    /// Payload of the code section, i.e. function bodies.
    pub code: usize,
    /// Payload of the data section, i.e. initialised memory. Const-encoded
    /// command buffers live here.
    pub data: usize,
}

/// Length of a section range. `wasmparser` reports offsets as `u64`; the module is
/// already in memory as a slice, so any range within it fits a `usize`.
fn span(range: Range<u64>) -> usize {
    (range.end - range.start) as usize
}

/// Read the section sizes of a wasm module.
///
/// Returns `None` if `bytes` is not a readable wasm module, rather than guessing:
/// a size report that silently reads zero would be worse than no report.
pub fn sections(bytes: &[u8]) -> Option<Sections> {
    let mut out = Sections {
        total: bytes.len(),
        ..Sections::default()
    };

    for payload in Parser::new(0).parse_all(bytes) {
        // `range` spans the whole section payload, vector count included, which
        // is the convention this tool reports in. Note it is not
        // `CodeSectionStart::size`, which excludes that count and would
        // under-report the code section by the width of it.
        match payload.ok()? {
            Payload::CodeSectionStart { range, .. } => out.code += span(range),
            Payload::DataSection(reader) => out.data += span(reader.range()),
            _ => {}
        }
    }

    Some(out)
}

#[cfg(test)]
mod test {
    use super::*;

    const HEADER: [u8; 8] = [b'\0', b'a', b's', b'm', 1, 0, 0, 0];

    /// One `() -> ()` type, the function that uses it, and one page of memory.
    /// A code section without the first two, or a data section without the third,
    /// does not parse.
    const TYPE_UNIT: [u8; 4] = [1, 0x60, 0, 0];
    const FUNC_ONE: [u8; 2] = [1, 0];
    const MEMORY_ONE: [u8; 3] = [1, 0, 1];

    /// A module of `(id, payload)` sections.
    ///
    /// Section lengths are computed rather than written out, and the ids have to be
    /// passed in the order the core specification lays down — wasmparser rejects
    /// them out of order, which is not something this reader should paper over.
    fn module(sections: &[(u8, &[u8])]) -> Vec<u8> {
        let mut bytes = HEADER.to_vec();

        for (id, payload) in sections {
            bytes.push(*id);
            leb128(&mut bytes, payload.len());
            bytes.extend_from_slice(payload);
        }

        bytes
    }

    /// A code section payload carrying one function body.
    fn code(body: &[u8]) -> Vec<u8> {
        let mut payload = vec![1]; // one body
        leb128(&mut payload, body.len());
        payload.extend_from_slice(body);
        payload
    }

    fn leb128(out: &mut Vec<u8>, mut value: usize) {
        loop {
            let byte = (value & 0x7F) as u8;
            value >>= 7;

            match value {
                0 => return out.push(byte),
                _ => out.push(byte | 0x80),
            }
        }
    }

    #[test]
    fn reads_the_code_and_data_sections() {
        let code = code(&[0x00, 0x0B]); // no locals, then `end`
        // One active segment at offset 0 carrying two bytes.
        let data: &[u8] = &[1, 0, 0x41, 0, 0x0B, 2, b'h', b'i'];

        let bytes = module(&[
            (1, &TYPE_UNIT),
            (3, &FUNC_ONE),
            (5, &MEMORY_ONE),
            (10, &code),
            (11, data),
        ]);

        let sections = sections(&bytes).expect("should parse");

        // The vector count is part of the payload, so these are the full section
        // bodies rather than `size`, which would be one less on the code section.
        assert_eq!(sections.code, code.len());
        assert_eq!(sections.code, 4);
        assert_eq!(sections.data, data.len());
        assert_eq!(sections.total, bytes.len());
    }

    #[test]
    fn ignores_other_sections() {
        // A custom section named "a" should count toward the total only.
        let bytes = module(&[(0, &[1, b'a', b'b', b'c'])]);

        let sections = sections(&bytes).expect("should parse");

        assert_eq!(sections.code, 0);
        assert_eq!(sections.data, 0);
        assert_eq!(sections.total, bytes.len());
    }

    /// A section long enough to need two LEB128 length bytes, so the arithmetic is
    /// exercised past the one-byte case that every small module happens to hit.
    /// A section long enough to need two LEB128 length bytes, so the arithmetic is
    /// exercised past the one-byte case every small module happens to hit.
    #[test]
    fn reads_a_multi_byte_length() {
        let mut body = vec![0x00]; // no locals
        body.extend(std::iter::repeat_n(0x01, 200)); // 200 nops
        body.push(0x0B); // end

        let code = code(&body);
        let bytes = module(&[(1, &TYPE_UNIT), (3, &FUNC_ONE), (10, &code)]);

        assert!(code.len() > 0x7F, "should need a multi-byte length");
        assert_eq!(sections(&bytes).expect("should parse").code, code.len());
    }

    #[test]
    fn rejects_what_is_not_a_module() {
        assert_eq!(sections(b"not wasm at all"), None);
        assert_eq!(sections(b""), None);
    }

    /// Claims 40 bytes of payload but carries four.
    #[test]
    fn rejects_a_truncated_section() {
        let mut bytes = HEADER.to_vec();
        bytes.extend_from_slice(&[10, 40, 1, 2, 0x00, 0x0B]);

        assert_eq!(sections(&bytes), None);
    }
}
