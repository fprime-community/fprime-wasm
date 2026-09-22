//! How much of a dictionary name's qualifier the author wrote.

use fprime_dictionary::naming::str_to_ident;
use proc_macro2::{Ident, Span};
use syn::Path as SynPath;

pub(super) const COMPLETION_MARKER: &str = "raCompletionMarker";

/// A path an author wrote for a dictionary name, carrying as much of that name's qualifier
/// as they cared to: `RED`, `Choice::RED` and `Ref::Choice::RED` are all this.
///
/// Only the last segment can be one an editor is partway through typing, so it is the only
/// one whose marker state is tracked.
pub(super) struct WrittenPath {
    /// Each segment's text, with any editor marker stripped from the last. Never empty.
    lookup: Vec<String>,
    /// The last segment as written, marker included.
    written: Ident,
    span: Span,
    /// Whether an editor is mid-completion on the last segment.
    in_progress: bool,
}

impl WrittenPath {
    /// The last segment: the name itself, the rest being its qualifier.
    pub(super) fn leaf(&self) -> &str {
        self.lookup.last().expect("a path has a last segment")
    }

    /// Whether this names the dictionary name whose own segments are `full` — the same last
    /// segment, and a qualifier that is a tail of `full`'s. So `Choice::RED` and a bare `RED`
    /// both name `Ref.Choice.RED`, while `Other::Choice::RED` names nothing.
    ///
    /// A path longer than `full` never matches, which is what leaves the `crate::Defs::..`
    /// form this pass writes for [`is_qualified_path`] to recognise rather than rewrite.
    pub(super) fn names(&self, full: &[&str]) -> bool {
        self.lookup.len() <= full.len()
            && self
                .lookup
                .iter()
                .rev()
                .zip(full.iter().rev())
                .all(|(written, dictionary)| written == dictionary)
    }

    /// The identifier to emit for a dictionary name this resolved to.
    pub(super) fn emit(&self, dictionary_name: &str) -> Ident {
        if self.in_progress {
            return self.written.clone();
        }

        let mut ident = str_to_ident(dictionary_name);
        ident.set_span(self.span);

        ident
    }
}

/// The path `qself`/`path` spell, if it is a plain one: no `<T as Trait>`, no leading `::`,
/// no generic arguments — none of which a dictionary name has.
pub(super) fn written_path(qself: Option<&syn::QSelf>, path: &SynPath) -> Option<WrittenPath> {
    if qself.is_some() || path.leading_colon.is_some() {
        return None;
    }

    let mut lookup = Vec::with_capacity(path.segments.len());
    for segment in &path.segments {
        if !segment.arguments.is_none() {
            return None;
        }
        lookup.push(unraw(&segment.ident));
    }

    let last = path.segments.last()?;
    let written = unraw(&last.ident);
    let in_progress = written.contains(COMPLETION_MARKER);
    if in_progress {
        let leaf = lookup.last_mut().expect("`last` found a segment");
        *leaf = written.replace(COMPLETION_MARKER, "");
    }

    Some(WrittenPath {
        lookup,
        written: last.ident.clone(),
        span: last.ident.span(),
        in_progress,
    })
}

/// The dictionary's own segments for a name declared inside `qualified_name`, e.g.
/// `("Ref.Choice", "RED")` becomes `["Ref", "Choice", "RED"]`.
pub(super) fn declared_in<'a>(qualified_name: &'a str, member: &'a str) -> Vec<&'a str> {
    let mut segments: Vec<&str> = qualified_name.split('.').collect();
    segments.push(member);

    segments
}

/// The identifier's name with any `r#` prefix stripped.
pub(super) fn unraw(ident: &Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").map(str::to_string).unwrap_or(name)
}

/// The identifier's name as the dictionary would spell it: `r#` stripped, and the cursor an
/// editor left in the middle of it taken out.
///
/// Looking a name up by what the author has typed *so far* is the only way a name being edited
/// resolves at all, and resolving is what keeps the two expansions rust-analyzer builds
/// agreeing up to the cursor. The marker still has to reach the output, which is why only the
/// lookup uses this and the emitted token stays the author's — see [`WrittenPath::emit`] for
/// the same rule one level down.
pub(super) fn dictionary_name(ident: &Ident) -> String {
    unraw(ident).replace(COMPLETION_MARKER, "")
}
