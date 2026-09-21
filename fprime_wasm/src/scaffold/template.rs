//! The templates, and substitution into them.

use anyhow::{Result, bail};

/// One template, and where its output goes.
pub struct Template {
    /// Destination, relative to the generated crate root; also substituted.
    pub path: &'static str,
    pub body: &'static str,
    pub executable: bool,
}

impl Template {
    /// A file to be read.
    pub const fn plain(path: &'static str, body: &'static str) -> Self {
        Template {
            path,
            body,
            executable: false,
        }
    }

    /// A file to be run, so it needs the executable bit.
    pub const fn program(path: &'static str, body: &'static str) -> Self {
        Template {
            path,
            body,
            executable: true,
        }
    }
}

/// The starter sequence, also used by `add` for every later one.
pub const SEQUENCE: Template = Template::plain(
    "src/bin/{{sequence}}.rs",
    include_str!("../../templates/sequence.rs.tmpl"),
);

/// A sequence's tests, written beside it by both `init` and `add`.
pub const TEST: Template = Template::plain(
    "tests/{{sequence}}.rs",
    include_str!("../../templates/test.rs.tmpl"),
);

/// The limits a project is verified against, named here so [`fprime_test::config`] can hold
/// the generated file to its own defaults.
pub const SEQUENCER: Template = Template::plain(
    "sequencer.toml",
    include_str!("../../templates/sequencer.toml.tmpl"),
);

/// The release linker, named here so the generated cargo config can be held to the
/// path it is actually written at.
pub const WASM_LINK: Template = Template::program(
    ".cargo/wasm-link",
    include_str!("../../templates/wasm-link.tmpl"),
);

/// Everything `init` writes, in the order it reports them.
pub const PROJECT: &[Template] = &[
    Template::plain(
        "Cargo.toml",
        include_str!("../../templates/Cargo.toml.tmpl"),
    ),
    Template::plain(
        ".cargo/config.toml",
        include_str!("../../templates/cargo-config.toml.tmpl"),
    ),
    WASM_LINK,
    SEQUENCER,
    Template::plain("build.rs", include_str!("../../templates/build.rs.tmpl")),
    Template::plain("src/lib.rs", include_str!("../../templates/lib.rs.tmpl")),
    SEQUENCE,
    TEST,
    // Named without the leading dot; a real `.gitignore` here would apply to this repo.
    Template::plain(".gitignore", include_str!("../../templates/gitignore.tmpl")),
    // Same rust-analyzer settings this repo uses, tuned for a no_std crate.
    Template::plain(
        ".vscode/settings.json",
        include_str!("../../templates/vscode-settings.json.tmpl"),
    ),
    Template::plain(
        ".vscode/extensions.json",
        include_str!("../../templates/vscode-extensions.json.tmpl"),
    ),
    Template::plain("README.md", include_str!("../../templates/README.md.tmpl")),
];

/// Every placeholder any template may use; [`render`] rejects anything outside it.
pub const KEYS: &[&str] = &[
    "crate_name",
    "lib_name",
    "sequence",
    "target",
    "stack_size",
    "dictionary",
    "dictionary_slashes",
    "crate_version",
];

/// Substitutes `{{key}}` occurrences in `text`.
pub fn render(text: &str, values: &[(&str, String)]) -> Result<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            bail!("unterminated `{{{{` in a template");
        };
        let key = &after[..end];

        if !KEYS.contains(&key) {
            bail!(
                "template uses an unknown placeholder `{{{{{key}}}}}`; known placeholders are {}",
                KEYS.join(", ")
            );
        }
        let Some((_, value)) = values.iter().find(|(name, _)| *name == key) else {
            bail!("template placeholder `{{{{{key}}}}}` was not given a value");
        };
        out.push_str(value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);

    Ok(out)
}

/// Placeholders a template uses, in first-appearance order.
#[cfg(test)]
fn placeholders(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else { break };
        let key = &after[..end];
        if !found.contains(&key) {
            found.push(key);
        }
        rest = &after[end + 2..];
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> Vec<(&'static str, String)> {
        KEYS.iter().map(|key| (*key, format!("<{key}>"))).collect()
    }

    #[test]
    fn generated_sequencer_is_the_defaults() {
        let generated =
            fprime_test::config::parse(SEQUENCER.body).expect("the template should parse");
        assert_eq!(generated, fprime_test::interpreter::Limits::default());
    }

    #[test]
    fn substitutes_a_placeholder() {
        let values = [("sequence", "startup".to_string())];
        assert_eq!(
            render("bin/{{sequence}}.rs", &values).expect("renders"),
            "bin/startup.rs"
        );
    }

    #[test]
    fn substitutes_every_occurrence() {
        let values = [("target", "wasm32v1-none".to_string())];
        assert_eq!(
            render("[target.{{target}}] for {{target}}", &values).expect("renders"),
            "[target.wasm32v1-none] for wasm32v1-none"
        );
    }

    #[test]
    fn leaves_text_without_placeholders_alone() {
        let body = "pub fn main() {\n    let x = [[1]];\n}\n";
        assert_eq!(render(body, &[]).expect("renders"), body);
    }

    #[test]
    fn single_braces_are_not_placeholders() {
        let body = "fn main() { let m = HashMap::new(); }";
        assert_eq!(render(body, &[]).expect("renders"), body);
    }

    #[test]
    fn rejects_unknown_placeholder() {
        let err = render("{{sequenc}}", &values()).expect_err("should not render");
        assert!(err.to_string().contains("unknown placeholder"), "{err}");
        assert!(err.to_string().contains("sequenc"), "{err}");
    }

    #[test]
    fn rejects_known_placeholder_with_no_value() {
        let err = render("{{sequence}}", &[]).expect_err("should not render");
        assert!(err.to_string().contains("not given a value"), "{err}");
    }

    #[test]
    fn rejects_unterminated_placeholder() {
        assert!(render("{{sequence", &values()).is_err());
    }

    #[test]
    fn every_template_uses_only_known_placeholders() {
        for template in PROJECT {
            for text in [template.path, template.body] {
                for key in placeholders(text) {
                    assert!(
                        KEYS.contains(&key),
                        "template {} uses `{{{{{key}}}}}`, which is not in KEYS",
                        template.path
                    );
                }
            }
        }
    }

    #[test]
    fn every_template_renders_with_no_placeholder_left() {
        let values = values();
        for template in PROJECT {
            for (what, text) in [("path", template.path), ("body", template.body)] {
                let rendered = render(text, &values)
                    .unwrap_or_else(|err| panic!("{} {what}: {err}", template.path));
                assert!(
                    !rendered.contains("{{"),
                    "{} {what} still contains a placeholder:\n{rendered}",
                    template.path
                );
            }
        }
    }

    #[test]
    fn every_key_is_used_by_some_template() {
        for key in KEYS {
            let used = PROJECT.iter().any(|template| {
                placeholders(template.path).contains(key)
                    || placeholders(template.body).contains(key)
            });
            assert!(used, "`{{{{{key}}}}}` is in KEYS but no template uses it");
        }
    }

    #[test]
    fn project_includes_starter_sequence() {
        assert!(PROJECT.iter().any(|t| t.path == SEQUENCE.path));
    }

    #[test]
    fn template_paths_use_forward_slashes() {
        for template in PROJECT {
            assert!(
                !template.path.contains('\\'),
                "{} should use forward slashes",
                template.path
            );
        }
    }
}
