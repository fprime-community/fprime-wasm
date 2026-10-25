//! How much of a dictionary name's qualifier the author wrote.

use proc_macro2::Ident;
use syn::Path as SynPath;

pub(super) const COMPLETION_MARKER: &str = "raCompletionMarker";

/// A path an author wrote for a dictionary name, carrying as much of that name's qualifier
/// as they cared to: `RED`, `Choice::RED` and `Ref::Choice::RED` are all this.
pub(super) struct WrittenPath {
    /// Each segment's text, with any editor marker stripped from the last. Never empty.
    lookup: Vec<String>,
    /// The last segment as written, marker and span included.
    leaf: Ident,
}

impl WrittenPath {
    /// The last segment: the name itself, the rest being its qualifier.
    pub(super) fn leaf(&self) -> &str {
        self.lookup.last().expect("a path has a last segment")
    }

    /// Whether this names the dictionary name whose own segments are `full`: the same last
    /// segment, and a qualifier that is a tail of `full`'s. So `Choice::RED` and `RED` both
    /// name `Ref.Choice.RED`, while `Other::Choice::RED` names nothing.
    ///
    /// A path longer than `full` never matches, which leaves the `crate::Defs::..` form this
    /// pass writes unrewritten.
    pub(super) fn names(&self, full: &[&str]) -> bool {
        self.lookup.len() <= full.len()
            && self
                .lookup
                .iter()
                .rev()
                .zip(full.iter().rev())
                .all(|(written, dictionary)| written == dictionary)
    }

    /// The last segment to emit. Always the author's token: it matched the dictionary's name
    /// already, and only the author's carries an editor's marker through — see
    /// [`dictionary_name`].
    pub(super) fn emit(&self) -> Ident {
        self.leaf.clone()
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

    // Only the last segment can be the one an editor is typing.
    let leaf = path.segments.last()?.ident.clone();
    *lookup.last_mut()? = dictionary_name(&leaf);

    Some(WrittenPath { lookup, leaf })
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

/// The identifier's name as the dictionary would spell it: `r#` and any editor marker
/// stripped.
///
/// For lookup only. A name being edited has to resolve as what is typed so far, so that the
/// two expansions rust-analyzer builds agree up to the cursor. The emitted token stays the
/// author's, since rust-analyzer discards a mapped token that lost its marker.
pub(super) fn dictionary_name(ident: &Ident) -> String {
    unraw(ident).replace(COMPLETION_MARKER, "")
}
