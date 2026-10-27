//! The dictionary index a scan resolves names against.

use fprime_dictionary::naming::Namespace;
use fprime_dictionary::{Command, Dictionary};
use std::collections::HashMap;

/// Which kind of point a dotted name is.
pub(super) enum Point<'a> {
    Command(&'a Command),
    Channel,
    Parameter,
}

impl Point<'_> {
    /// The `Desc` namespace this point's descriptor is in.
    pub(super) fn namespace(&self) -> Namespace {
        match self {
            Point::Command(_) => Namespace::Cmd,
            Point::Channel => Namespace::Tlm,
            Point::Parameter => Namespace::Prm,
        }
    }
}

/// What [`Symbols`] stores per name: a [`Point`] with its command as an index, since the
/// command lives in the same struct.
#[derive(Clone, Copy)]
enum Indexed {
    Command(usize),
    Channel,
    Parameter,
}

impl Indexed {
    fn namespace(self) -> Namespace {
        match self {
            Indexed::Command(_) => Namespace::Cmd,
            Indexed::Channel => Namespace::Tlm,
            Indexed::Parameter => Namespace::Prm,
        }
    }
}

/// The parsed dictionary, plus a name lookup over its points — which [`Dictionary`]'s `Vec`s
/// don't offer.
pub(super) struct Symbols {
    pub(super) dictionary: Dictionary,
    /// Every point of each name, a command first: a command, channel and parameter may share
    /// one, being in separate namespaces.
    points: HashMap<String, Vec<Indexed>>,
}

impl Symbols {
    pub(super) fn new(dictionary: Dictionary) -> Self {
        let commands = dictionary
            .commands
            .iter()
            .enumerate()
            .map(|(index, command)| (&command.name, Indexed::Command(index)));
        let channels = dictionary
            .telemetry_channels
            .iter()
            .map(|channel| (&channel.name, Indexed::Channel));
        let parameters = dictionary
            .parameters
            .iter()
            .map(|parameter| (&parameter.name, Indexed::Parameter));

        let mut points: HashMap<String, Vec<Indexed>> = HashMap::new();
        for (name, point) in commands.chain(channels).chain(parameters) {
            points.entry(name.clone()).or_default().push(point);
        }

        Self { dictionary, points }
    }

    /// The point `name` refers to in `namespace`, if any. With no namespace written, a
    /// command wins a name it shares, as an unqualified name is a command in the flight API.
    ///
    /// A name with no `.` is never one: an F Prime point is at least `Component.NAME`, and
    /// matching a single segment would rewrite the author's own locals.
    pub(super) fn point(&self, namespace: Option<Namespace>, name: &str) -> Option<Point<'_>> {
        if !name.contains('.') {
            return None;
        }

        let found = self
            .points
            .get(name)?
            .iter()
            .find(|point| namespace.is_none_or(|namespace| point.namespace() == namespace))?;

        Some(match *found {
            Indexed::Command(index) => Point::Command(&self.dictionary.commands[index]),
            Indexed::Channel => Point::Channel,
            Indexed::Parameter => Point::Parameter,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fprime_dictionary::{TelemetryChannel, TypeName};

    /// The reference dictionary plus a channel named after `CdhCore.cmdDisp.CMD_NO_OP`.
    fn shared_name() -> Symbols {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fprime_dictionary/src/test/RefTopologyDictionary.json");
        let mut dictionary = fprime_dictionary::parse(&path);
        dictionary.telemetry_channels.push(TelemetryChannel {
            name: "CdhCore.cmdDisp.CMD_NO_OP".to_string(),
            type_name: TypeName::Bool,
            id: 0,
            annotation: None,
        });

        Symbols::new(dictionary)
    }

    #[test]
    fn a_shared_name_is_one_point_per_namespace() {
        let symbols = shared_name();
        let name = "CdhCore.cmdDisp.CMD_NO_OP";

        let namespace = |written| symbols.point(written, name).map(|point| point.namespace());
        assert_eq!(namespace(None), Some(Namespace::Cmd));
        assert_eq!(namespace(Some(Namespace::Cmd)), Some(Namespace::Cmd));
        assert_eq!(namespace(Some(Namespace::Tlm)), Some(Namespace::Tlm));
        assert_eq!(namespace(Some(Namespace::Prm)), None);
    }
}
