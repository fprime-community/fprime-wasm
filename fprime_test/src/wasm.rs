//! A read-only view of a Wasm binary, for what `spacewasm` cannot report.
//!
//! `spacewasm` resolves imports against the host store as it decodes, so by the time
//! a [`spacewasm::Module`] exists its `module.field` names are gone — and those names
//! are exactly what a link failure has to name. So the module is read again with
//! `wasmparser`, lifting only what the reports use: section sizes, import and export
//! names, an import's signature, and the memory limits.

use anyhow::{Context, Result, bail};
use std::fmt;
use wasmparser::{CompositeInnerType, ExternalKind, Parser, Payload, TypeRef, ValType};

/// A `(params) -> results` signature, in the `iIfd` spelling `spacewasm` uses, so a
/// mismatch lines up with `HostFunction::new` character for character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub params: String,
    pub returns: String,
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"{}\" -> \"{}\"", self.params, self.returns)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportKind {
    Func(Signature),
    Table,
    Memory,
    Global,
    /// From the exception-handling proposal. Cannot link on board; kept distinct so
    /// the diagnostic can say what it actually is.
    Tag,
}

impl fmt::Display for ImportKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportKind::Func(sig) => write!(f, "func {sig}"),
            ImportKind::Table => f.write_str("table"),
            ImportKind::Memory => f.write_str("memory"),
            ImportKind::Global => f.write_str("global"),
            ImportKind::Tag => f.write_str("tag"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub module: String,
    pub name: String,
    pub kind: ImportKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Func,
    Table,
    Memory,
    Global,
    Tag,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Memory {
    pub initial_pages: u64,
    pub max_pages: Option<u64>,
    pub page_size: u64,
}

impl Memory {
    /// Declared size in bytes, which is what `Config::guestMemorySize` must hold.
    pub fn initial_bytes(&self) -> u64 {
        self.initial_pages.saturating_mul(self.page_size)
    }
}

/// Byte sizes of the sections reported on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Sizes {
    pub total: usize,
    pub code: usize,
    pub data: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Module {
    pub sizes: Sizes,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub memory: Option<Memory>,
}

impl Module {
    /// Imports of `module`, in declaration order.
    pub fn imports_from<'a>(&'a self, module: &str) -> impl Iterator<Item = &'a Import> {
        self.imports.iter().filter(move |i| i.module == module)
    }

    /// Imports from anywhere else. `WasmSequencer` registers exactly one host
    /// module, so on a sequence these are the ones that will not link on board.
    pub fn imports_outside<'a>(&'a self, module: &str) -> impl Iterator<Item = &'a Import> {
        self.imports.iter().filter(move |i| i.module != module)
    }

    pub fn export(&self, name: &str) -> Option<&Export> {
        self.exports.iter().find(|e| e.name == name)
    }
}

fn valtype(ty: ValType) -> char {
    match ty {
        ValType::I32 => 'i',
        ValType::I64 => 'I',
        ValType::F32 => 'f',
        ValType::F64 => 'd',
        ValType::V128 | ValType::Ref(_) => '?',
    }
}

pub fn read(bytes: &[u8]) -> Result<Module> {
    if bytes.len() < 8 || &bytes[..4] != b"\0asm" {
        bail!("not a Wasm module: it does not start with the \\0asm header");
    }

    let mut module = Module {
        sizes: Sizes {
            total: bytes.len(),
            ..Sizes::default()
        },
        ..Module::default()
    };
    // Function signatures, indexed by type index, for resolving imports.
    let mut types: Vec<Signature> = Vec::new();

    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.context("not a readable Wasm module")?;
        match payload {
            Payload::TypeSection(reader) => {
                for group in reader {
                    for ty in group.context("bad type section")?.into_types() {
                        // Only function types are indexable by an import; the GC
                        // proposal's struct and array types are placeholders, so
                        // later indices still line up.
                        let signature = match &ty.composite_type.inner {
                            CompositeInnerType::Func(func) => Signature {
                                params: func.params().iter().copied().map(valtype).collect(),
                                returns: func.results().iter().copied().map(valtype).collect(),
                            },
                            _ => Signature {
                                params: "?".into(),
                                returns: "?".into(),
                            },
                        };
                        types.push(signature);
                    }
                }
            }
            Payload::ImportSection(reader) => {
                for group in reader {
                    for entry in group.context("bad import section")? {
                        let (_, import) = entry.context("bad import section")?;
                        let kind = match import.ty {
                            // `FuncExact` still names a function type index, so it
                            // resolves the same way.
                            TypeRef::Func(index) | TypeRef::FuncExact(index) => {
                                let signature =
                                    types.get(index as usize).cloned().with_context(|| {
                                        format!(
                                            "import {}.{} refers to type {index}, but the module \
                                             declares only {}",
                                            import.module,
                                            import.name,
                                            types.len()
                                        )
                                    })?;
                                ImportKind::Func(signature)
                            }
                            TypeRef::Table(_) => ImportKind::Table,
                            TypeRef::Memory(_) => ImportKind::Memory,
                            TypeRef::Global(_) => ImportKind::Global,
                            TypeRef::Tag(_) => ImportKind::Tag,
                        };
                        module.imports.push(Import {
                            module: import.module.to_owned(),
                            name: import.name.to_owned(),
                            kind,
                        });
                    }
                }
            }
            Payload::MemorySection(reader) => {
                // `spacewasm` addresses one memory, so a multi-memory module could
                // not run anyway.
                if let Some(memory) = reader.into_iter().next() {
                    let memory = memory.context("bad memory section")?;
                    // Sequences are linked `--page-size=1` (custom-page-sizes);
                    // absent, a page is 64 KiB. Wrong here is 65536x wrong.
                    let exponent = memory.page_size_log2.unwrap_or(16);
                    if exponent >= 64 {
                        bail!(
                            "memory declares a page size of 2^{exponent}, which does not fit in \
                             64 bits"
                        );
                    }
                    module.memory = Some(Memory {
                        initial_pages: memory.initial,
                        max_pages: memory.maximum,
                        page_size: 1u64 << exponent,
                    });
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.context("bad export section")?;
                    module.exports.push(Export {
                        name: export.name.to_owned(),
                        kind: match export.kind {
                            ExternalKind::Func | ExternalKind::FuncExact => ExportKind::Func,
                            ExternalKind::Table => ExportKind::Table,
                            ExternalKind::Memory => ExportKind::Memory,
                            ExternalKind::Global => ExportKind::Global,
                            ExternalKind::Tag => ExportKind::Tag,
                        },
                    });
                }
            }
            // Summed, not assigned: the spec allows one of each, but summing cannot
            // silently report only the last.
            Payload::CodeSectionStart { range, .. } => {
                module.sizes.code += usize::try_from(range.end - range.start).unwrap_or(usize::MAX);
            }
            Payload::DataSection(reader) => {
                let range = reader.range();
                module.sizes.data += usize::try_from(range.end - range.start).unwrap_or(usize::MAX);
            }
            _ => {}
        }
    }

    Ok(module)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(module (import "m" "f" (func (param i32 i64) (result i32))))`
    const IMPORT_MODULE: &[u8] = &[
        b'\0', b'a', b's', b'm', 1, 0, 0, 0, //
        1, 7, 1, 0x60, 2, 0x7F, 0x7E, 1, 0x7F, // type section
        2, 7, 1, 1, b'm', 1, b'f', 0x00, 0, // import section
    ];

    #[test]
    fn reads_an_import_with_its_signature() {
        let module = read(IMPORT_MODULE).expect("valid module");
        assert_eq!(
            module.imports,
            vec![Import {
                module: "m".into(),
                name: "f".into(),
                kind: ImportKind::Func(Signature {
                    params: "iI".into(),
                    returns: "i".into(),
                }),
            }]
        );
    }

    #[test]
    fn partitions_imports_by_module() {
        let module = read(IMPORT_MODULE).expect("valid module");
        assert_eq!(module.imports_from("m").count(), 1);
        assert_eq!(module.imports_outside("m").count(), 0);
        assert_eq!(module.imports_from("other").count(), 0);
        assert_eq!(module.imports_outside("other").count(), 1);
    }

    /// Pointing the tool at something that is not a module: the common mistake, and
    /// the one worth a single readable line.
    #[test]
    fn rejects_a_non_wasm_file() {
        let err = read(b"#!/bin/sh\n").expect_err("should not parse");
        assert!(err.to_string().contains("not a Wasm module"), "{err}");
        assert!(read(b"").is_err(), "an empty file is not a module");
        assert!(read(b"\0asm").is_err(), "a header alone is not a module");
    }

    /// `wasmparser` reports truncation with an offset — more than the header check
    /// here could say alone.
    #[test]
    fn rejects_a_truncated_module() {
        let err = read(&IMPORT_MODULE[..12]).expect_err("should not parse");
        assert!(
            format!("{err:#}").contains("end-of-file"),
            "the cause should name the truncation: {err:#}"
        );
    }

    /// Sequences are linked `--page-size=1`; unhonoured, every guest memory reads
    /// 65536x too large.
    #[test]
    fn reads_a_custom_page_size() {
        // flags = 0x08 (page size present), min = 941, page size = 2^0.
        let bytes = &[
            b'\0', b'a', b's', b'm', 1, 0, 0, 0, //
            5, 5, 1, 0x08, 0xAD, 0x07, 0x00,
        ];
        let memory = read(bytes).expect("valid module").memory.expect("a memory");
        assert_eq!(memory.page_size, 1);
        assert_eq!(memory.initial_pages, 941);
        assert_eq!(memory.max_pages, None);
        assert_eq!(memory.initial_bytes(), 941);
    }

    /// Without the bit a page is 64 KiB: the same declaration, very different memory.
    #[test]
    fn defaults_to_64_kib_pages() {
        let bytes = &[
            b'\0', b'a', b's', b'm', 1, 0, 0, 0, //
            5, 3, 1, 0x00, 0x02,
        ];
        let memory = read(bytes).expect("valid module").memory.expect("a memory");
        assert_eq!(memory.page_size, 65536);
        assert_eq!(memory.initial_bytes(), 131072);
    }

    /// A count past the end of the section must error, not allocate or truncate.
    #[test]
    fn rejects_an_implausible_vector_length() {
        let bytes = &[
            b'\0', b'a', b's', b'm', 1, 0, 0, 0, //
            7, 2, 0xFF, 0x01,
        ];
        assert!(read(bytes).is_err());
    }

    /// Sizes count the whole payload, count byte included, to match `wasm_size` —
    /// which is why section ranges are used and not `CodeSectionStart::size`.
    #[test]
    fn section_sizes_count_the_whole_payload() {
        let bytes = &[
            b'\0', b'a', b's', b'm', 1, 0, 0, 0, //
            0, 4, 1, b'a', b'b', b'c', // custom section "a", ignored
            3, 2, 1, 0, // function section: one function of type 0
            10, 4, 1, 2, 0x0B, 0x0B, // code section: 4 payload bytes
        ];
        let sizes = read(bytes).expect("valid module").sizes;
        assert_eq!(sizes.total, bytes.len());
        assert_eq!(sizes.code, 4, "the count byte is part of the payload");
        assert_eq!(sizes.data, 0);
    }

    #[test]
    fn skips_unknown_sections() {
        let bytes = &[
            b'\0', b'a', b's', b'm', 1, 0, 0, 0, //
            0, 4, 1, b'a', b'b', b'c', // custom section named "a"
        ];
        let module = read(bytes).expect("valid module");
        assert_eq!(module.sizes.total, bytes.len());
        assert!(module.imports.is_empty());
        assert!(module.exports.is_empty());
        assert!(module.memory.is_none());
    }
}
