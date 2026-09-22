//! The dictionary index a scan resolves names against.

use fprime_dictionary::{Command, Dictionary, TypeDefinition, TypeName};
use std::collections::HashMap;

/// Aliases followed before concluding the dictionary is cyclic.
const MAX_ALIAS_HOPS: usize = 32;

/// Which kind of point a dotted name is.
pub(super) enum Point<'a> {
    Command(&'a Command),
    Channel,
    Parameter,
}

/// The parsed dictionary, plus one name index per kind of point. The indices are the only
/// thing this adds over [`Dictionary`], whose commands, channels and parameters are `Vec`s
/// with no name lookup of their own.
pub(super) struct Symbols {
    /// Read directly by the passes that ask the dictionary about a *type* rather than a
    /// point — `resolved_type`, `const_arg`, `const_encodable`.
    pub(super) dictionary: Dictionary,
    commands: HashMap<String, usize>,
    /// Channel and parameter points — needed by `#[fprime_test]`, not `#[fprime_main]`.
    channels: HashMap<String, usize>,
    parameters: HashMap<String, usize>,
}

impl Symbols {
    pub(super) fn new(dictionary: Dictionary) -> Self {
        let commands = dictionary
            .commands
            .iter()
            .enumerate()
            .map(|(index, command)| (command.name.clone(), index))
            .collect();
        let channels = dictionary
            .telemetry_channels
            .iter()
            .enumerate()
            .map(|(index, channel)| (channel.name.clone(), index))
            .collect();
        let parameters = dictionary
            .parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.name.clone(), index))
            .collect();

        Self {
            dictionary,
            commands,
            channels,
            parameters,
        }
    }

    /// The point `name` refers to, if any (commands checked first).
    pub(super) fn point(&self, name: &str) -> Option<Point<'_>> {
        if let Some(command) = self.command(name) {
            return Some(Point::Command(command));
        }
        if self.channels.contains_key(name) {
            return Some(Point::Channel);
        }
        if self.parameters.contains_key(name) {
            return Some(Point::Parameter);
        }
        None
    }

    pub(super) fn command(&self, name: &str) -> Option<&Command> {
        self.commands
            .get(name)
            .map(|index| &self.dictionary.commands[*index])
    }

    /// Follow aliases down to the definition that carries the constants or members.
    pub(super) fn definition(&self, ty: &TypeName) -> Option<&TypeDefinition> {
        let TypeName::QualifiedIdentifier { name } = ty else {
            return None;
        };

        let mut definition = self.dictionary.type_definitions.get(name)?;
        for _ in 0..MAX_ALIAS_HOPS {
            let TypeDefinition::Alias(alias) = definition else {
                return Some(definition);
            };

            let TypeName::QualifiedIdentifier { name } = &alias.underlying_type else {
                return None;
            };

            definition = self.dictionary.type_definitions.get(name)?;
        }

        None
    }
}
