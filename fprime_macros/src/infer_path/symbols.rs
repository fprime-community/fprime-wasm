//! The dictionary index a scan resolves names against.

use fprime_dictionary::{Command, Dictionary};
use std::collections::HashMap;

/// Which kind of point a dotted name is.
pub(super) enum Point<'a> {
    Command(&'a Command),
    Channel,
    Parameter,
}

/// What [`Symbols`] stores per name: a [`Point`] with its command as an index, since the
/// command lives in the same struct.
#[derive(Clone, Copy)]
enum Indexed {
    Command(usize),
    Channel,
    Parameter,
}

/// The parsed dictionary, plus a name lookup over its points — which [`Dictionary`]'s `Vec`s
/// don't offer.
pub(super) struct Symbols {
    pub(super) dictionary: Dictionary,
    points: HashMap<String, Indexed>,
}

impl Symbols {
    pub(super) fn new(dictionary: Dictionary) -> Self {
        let channels = dictionary
            .telemetry_channels
            .iter()
            .map(|channel| (channel.name.clone(), Indexed::Channel));
        let parameters = dictionary
            .parameters
            .iter()
            .map(|parameter| (parameter.name.clone(), Indexed::Parameter));
        // Last, so a command wins a name it shares with a channel or parameter.
        let commands = dictionary
            .commands
            .iter()
            .enumerate()
            .map(|(index, command)| (command.name.clone(), Indexed::Command(index)));

        let points = channels.chain(parameters).chain(commands).collect();

        Self { dictionary, points }
    }

    /// The point `name` refers to, if any.
    ///
    /// A name with no `.` is never one: an F Prime point is at least `Component.NAME`, and
    /// matching a single segment would rewrite the author's own locals.
    pub(super) fn point(&self, name: &str) -> Option<Point<'_>> {
        if !name.contains('.') {
            return None;
        }

        Some(match *self.points.get(name)? {
            Indexed::Command(index) => Point::Command(&self.dictionary.commands[index]),
            Indexed::Channel => Point::Channel,
            Indexed::Parameter => Point::Parameter,
        })
    }
}
