use std::convert::TryInto;
use std::fmt;

use crate::generator::common::{Component, Mode, Port, Type};
use crate::generator::vhdl::Split;
use crate::{Error, Identify, Result};

use super::assignment::{AssignmentKind, FieldSelection};
use super::object::ObjectType;

pub mod architecturedeclaration_from;
pub mod declare;
pub mod impls;

// Declarations may typically be any of the following: type, subtype, signal, constant, file, alias, component, attribute, function, procedure, configuration specification. (per: https://www.ics.uci.edu/~jmoorkan/vhdlref/architec.html)
// Per: https://insights.sigasi.com/tech/vhdl2008.ebnf/#block_declarative_item
//     subprogram_declaration
// | subprogram_body
// | subprogram_instantiation_declaration
// | package_declaration
// | package_body
// | package_instantiation_declaration
// | type_declaration
// | subtype_declaration
// | constant_declaration
// | signal_declaration
// | shared_variable_declaration
// | file_declaration
// | alias_declaration
// | component_declaration
// | attribute_declaration
// | attribute_specification
// | configuration_specification
// | disconnection_specification
// | use_clause
// | group_template_declaration
// | group_declaration
// | PSL_Property_Declaration
// | PSL_Sequence_Declaration
// | PSL_Clock_Declaration
/// Architecture declaration.
#[derive(Debug, Clone)]
pub enum ArchitectureDeclaration<'a> {
    /// Type declarations within the architecture
    Type(Type),
    SubType(String), // TODO: Do we want subtypes, or should these just be (part of) types?

    Procedure(String), // TODO: Procedure
    Function(String),  // TODO: Function
    /// Object declaration, covering signals, variables, constants and ports*
    ///
    /// *Ports cannot be declared within the architecture itself, but can be used in the statement part,
    /// as such, the ports of the entity implemented are treated as inferred declarations.
    Object(ObjectDeclaration),
    /// Alias for an object declaration, with optional range constraint
    Alias(AliasDeclaration<'a>),
    /// Component declarations within the architecture
    Component(Component),
    Custom(String), // TODO: Custom (templates?)
}

/// The kind of object declared (signal, variable, constant, ports)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Signal,
    Variable,
    Constant,
    /// Represents ports declared on the entity this architecture is describing
    EntityPort,
    /// Represents ports on components within the architecture
    ComponentPort,
}

impl fmt::Display for ObjectKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ObjectKind::Signal => write!(f, "Signal"),
            ObjectKind::Variable => write!(f, "Variable"),
            ObjectKind::Constant => write!(f, "Constant"),
            ObjectKind::EntityPort => write!(f, "EntityPort"),
            ObjectKind::ComponentPort => write!(f, "ComponentPort"),
        }
    }
}

// TODO: Currently unused and hard to implement, as the modes of hence undefined objects needs to be changed when they are assigned, which requires persistent mutable references to their declarations.
// Consider first declaring a signal, then assigning one of the entity's "in" ports to that signal, then assigning that signal to an "in" port of a component.
// This requires changing the mode of the signal with the first (declared) assignment, and keeping track of it for each subsequent assignment. (To make sure you don't accidentally connect the "in" of the entity to the "out" of the component)
// Now consider that there can be multiple signals between these steps, and that these signals can consist of multiple fields (records, arrays), each of which can also be assigned objects...
// So the challenge will be making sure that assigning something like some_record <= (a => some_other_record.a.b.c.d, b => some_array(4 to 8)) forces all objects to which the fields belong into appropriate modes.
// Basically, this only works if each object declaration is only ever a single mutable reference, which requires some significant rewrites at this point, and dealing with Rust's lifetimes.
/// The state of the object, with respect to the architecture
///
/// (E.g., an "in" port on the entity is "Assigned", but so is an "out" port of a component inside the architecture)
///
/// "Out" objects can be assigned "Assigned" objects or "Undefined" objects (which then become "Out" objects themselves)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectMode {
    /// The object does not have a defined mode yet
    Undefined,
    /// The object is carrying a value (the "in" port of an entity and the "out" port of a component, or a signal which was assigned a value)
    Assigned,
    /// The object is used to carry a value out of the architecture (the "out" port of an entity and the "in" port of a component)
    Out,
}

/// Struct describing the identifier of the object, its type, its kind, and a potential default value
#[derive(Debug, Clone)]
pub struct ObjectDeclaration {
    /// Name of the signal
    identifier: String,
    /// (Sub-)Type of the object
    typ: ObjectType,
    mode: ObjectMode,
    /// Default value assigned to the object (required for constants, cannot be used for ports)
    default: Option<AssignmentKind>,
    /// The kind of object
    kind: ObjectKind,
}

impl ObjectDeclaration {
    pub fn signal(
        identifier: impl Into<String>,
        typ: ObjectType,
        default: Option<AssignmentKind>,
    ) -> ObjectDeclaration {
        ObjectDeclaration {
            identifier: identifier.into(),
            typ,
            mode: ObjectMode::Undefined,
            default,
            kind: ObjectKind::Signal,
        }
    }

    pub fn variable(
        identifier: impl Into<String>,
        typ: ObjectType,
        default: Option<AssignmentKind>,
    ) -> ObjectDeclaration {
        ObjectDeclaration {
            identifier: identifier.into(),
            typ,
            mode: if let Some(_) = default {
                ObjectMode::Assigned
            } else {
                ObjectMode::Undefined
            },
            default,
            kind: ObjectKind::Variable,
        }
    }

    pub fn constant(
        identifier: impl Into<String>,
        typ: ObjectType,
        value: impl Into<AssignmentKind>,
    ) -> ObjectDeclaration {
        ObjectDeclaration {
            identifier: identifier.into(),
            typ,
            mode: ObjectMode::Assigned,
            default: Some(value.into()),
            kind: ObjectKind::Constant,
        }
    }

    /// Entity Ports serve as a way to represent the ports of an entity the architecture is describing.
    /// They are not declared within the architecture itself, but can drive or be driven by other objects.
    pub fn entity_port(
        identifier: impl Into<String>,
        typ: ObjectType,
        mode: Mode,
    ) -> ObjectDeclaration {
        ObjectDeclaration {
            identifier: identifier.into(),
            typ,
            mode: match mode {
                Mode::In => ObjectMode::Assigned,
                Mode::Out => ObjectMode::Out,
            },
            default: None,
            kind: ObjectKind::EntityPort,
        }
    }

    pub fn component_port(
        identifier: impl Into<String>,
        typ: ObjectType,
        mode: Mode,
    ) -> ObjectDeclaration {
        ObjectDeclaration {
            identifier: identifier.into(),
            typ,
            mode: match mode {
                Mode::In => ObjectMode::Out, // An "in" port requires an object going out of the architecture
                Mode::Out => ObjectMode::Assigned, // An "out" port is already assigned a value
            },
            default: None,
            kind: ObjectKind::ComponentPort,
        }
    }

    pub fn set_default(mut self, default: AssignmentKind) -> Result<()> {
        // TODO: Verify mode as well
        match self.kind() {
            ObjectKind::Signal | ObjectKind::Variable | ObjectKind::ComponentPort => {
                // self.can_assign(&default, None);
                self.default = Some(default);
                Ok(())
            }
            ObjectKind::Constant | ObjectKind::EntityPort => Err(Error::InvalidTarget(format!(
                "Default cannot be assigned to {} object",
                self.kind()
            ))),
        }
    }

    pub fn kind(&self) -> ObjectKind {
        self.kind
    }

    pub fn typ(&self) -> &ObjectType {
        &self.typ
    }

    pub fn identifier(&self) -> &str {
        self.identifier.as_str()
    }

    pub fn default(&self) -> &Option<AssignmentKind> {
        &self.default
    }

    pub fn mode(&self) -> &ObjectMode {
        &self.mode
    }

    pub fn from_port(port: &Port, is_entity: bool) -> Result<Vec<ObjectDeclaration>> {
        let ent_obj = |p: &Port| -> Result<ObjectDeclaration> {
            Ok(ObjectDeclaration::entity_port(
                p.identifier(),
                p.typ().try_into()?,
                p.mode(),
            ))
        };
        let comp_obj = |p: &Port| -> Result<ObjectDeclaration> {
            Ok(ObjectDeclaration::component_port(
                p.identifier(),
                p.typ().try_into()?,
                p.mode(),
            ))
        };
        let sel_obj = |p: &Port| -> Result<ObjectDeclaration> {
            if is_entity {
                ent_obj(p)
            } else {
                comp_obj(p)
            }
        };
        if port.has_reversed() {
            let (dn, up) = port.split();
            let mut results = vec![];
            if let Some(p) = dn {
                results.push(sel_obj(&p)?);
            }
            if let Some(p) = up {
                results.push(sel_obj(&p)?);
            }
            Ok(results)
        } else {
            Ok(vec![sel_obj(port)?])
        }
    }
}

/// Aliases an existing object, with optional field constraint
#[derive(Debug, Clone)]
pub struct AliasDeclaration<'a> {
    identifier: String,
    /// Reference to an existing object declaration
    object: &'a ObjectDeclaration,
    /// Optional field selection(s) - when assigning to or from the alias, this is used to determine the fields it represents
    field_selection: Vec<FieldSelection>,
}

impl<'a> AliasDeclaration<'a> {
    pub fn new(
        object: &'a ObjectDeclaration,
        identifier: impl Into<String>,
        fields: Vec<FieldSelection>,
    ) -> Result<AliasDeclaration<'a>> {
        AliasDeclaration::from_object(object, identifier).with_selection(fields)
    }

    pub fn from_object(
        object: &'a ObjectDeclaration,
        identifier: impl Into<String>,
    ) -> AliasDeclaration<'a> {
        AliasDeclaration {
            identifier: identifier.into(),
            object,
            field_selection: vec![],
        }
    }

    /// Apply one or more field selections to the alias
    pub fn with_selection(mut self, fields: Vec<FieldSelection>) -> Result<Self> {
        let mut object = self.object().typ().clone();
        for field in self.field_selection() {
            object = object.get_field(field)?;
        }
        for field in fields {
            object = object.get_field(&field)?;
            self.field_selection.push(field)
        }

        Ok(self)
    }

    /// Returns the actual object this is aliasing
    pub fn object(&self) -> &'a ObjectDeclaration {
        self.object
    }

    /// Returns the optional field selection of this alias
    pub fn field_selection(&self) -> &Vec<FieldSelection> {
        &self.field_selection
    }

    /// Returns the alias's identifier
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Returns the object type of the alias (after fields have been selected)
    pub fn typ(&self) -> Result<ObjectType> {
        let mut object = self.object().typ().clone();
        for field in self.field_selection() {
            object = object.get_field(field)?;
        }
        Ok(object)
    }
}

impl<'a> TryInto<ObjectDeclaration> for AliasDeclaration<'a> {
    type Error = Error;

    fn try_into(self) -> Result<ObjectDeclaration> {
        Ok(ObjectDeclaration {
            identifier: self.identifier().to_string(),
            typ: self.typ()?,
            mode: self.object().mode().clone(),
            default: None,
            kind: self.object().kind().clone(),
        })
    }
}

#[cfg(test)]
pub mod tests {
    use std::convert::TryFrom;

    use indexmap::IndexMap;

    use crate::{stdlib::common::architecture::object::RecordObject, Name};

    use super::*;

    pub(crate) fn test_bit_signal() -> Result<ObjectDeclaration> {
        Ok(ObjectDeclaration::signal(
            "test_signal".to_string(),
            ObjectType::Bit,
            None,
        ))
    }

    pub(crate) fn test_complex_signal() -> Result<ObjectDeclaration> {
        let mut fields: IndexMap<String, ObjectType> = IndexMap::new();
        fields.insert("a".to_string(), ObjectType::bit_vector(10, -4)?);
        Ok(ObjectDeclaration::signal(
            "test_signal",
            ObjectType::Record(RecordObject::new("record_typ".to_string(), fields)),
            None,
        ))
    }

    #[test]
    fn alias_verification_success() -> Result<()> {
        AliasDeclaration::from_object(&test_bit_signal()?, Name::try_from("test_signal_alias")?);
        AliasDeclaration::from_object(&test_complex_signal()?, "test_signal_alias")
            .with_selection(vec![FieldSelection::name("a")])?;
        AliasDeclaration::from_object(&test_complex_signal()?, "test_signal_alias")
            .with_selection(vec![
                FieldSelection::name("a"),
                FieldSelection::downto(10, -4)?,
            ])?;
        AliasDeclaration::from_object(&test_complex_signal()?, "test_signal_alias")
            .with_selection(vec![FieldSelection::name("a")])?
            .with_selection(vec![FieldSelection::downto(10, -4)?])?;
        AliasDeclaration::from_object(&test_complex_signal()?, "test_signal_alias")
            .with_selection(vec![
                FieldSelection::name("a"),
                FieldSelection::downto(4, -1)?,
            ])?;
        AliasDeclaration::from_object(&test_complex_signal()?, "test_signal_alias")
            .with_selection(vec![FieldSelection::name("a"), FieldSelection::to(-4, 10)?])?;
        AliasDeclaration::from_object(&test_complex_signal()?, "test_signal_alias")
            .with_selection(vec![FieldSelection::name("a"), FieldSelection::index(10)])?;
        AliasDeclaration::from_object(&test_complex_signal()?, "test_signal_alias")
            .with_selection(vec![FieldSelection::name("a"), FieldSelection::index(-4)])?;
        Ok(())
    }

    #[test]
    fn alias_verification_error() -> Result<()> {
        is_invalid_target(
            AliasDeclaration::from_object(&test_bit_signal()?, "test_signal_alias")
                .with_selection(vec![FieldSelection::name("a")]),
        )?;
        is_invalid_target(
            AliasDeclaration::from_object(&test_bit_signal()?, "test_signal_alias")
                .with_selection(vec![FieldSelection::index(1)]),
        )?;
        is_invalid_target(
            AliasDeclaration::from_object(&test_complex_signal()?, "test_signal_alias")
                .with_selection(vec![FieldSelection::index(1)]),
        )?;
        is_invalid_argument(
            AliasDeclaration::from_object(&test_complex_signal()?, "test_signal_alias")
                .with_selection(vec![FieldSelection::name("b")]),
        )?;
        is_invalid_target(
            AliasDeclaration::from_object(&test_complex_signal()?, "test_signal_alias")
                .with_selection(vec![FieldSelection::name("a"), FieldSelection::name("a")]),
        )?;
        is_invalid_argument(
            AliasDeclaration::from_object(&test_complex_signal()?, "test_signal_alias")
                .with_selection(vec![
                    FieldSelection::name("a"),
                    FieldSelection::downto(11, -4)?,
                ]),
        )?;
        Ok(())
    }

    fn is_invalid_target<T>(result: Result<T>) -> Result<()> {
        match result {
            Err(Error::InvalidTarget(_)) => Ok(()),
            _ => Err(Error::UnknownError),
        }
    }

    fn is_invalid_argument<T>(result: Result<T>) -> Result<()> {
        match result {
            Err(Error::InvalidArgument(_)) => Ok(()),
            _ => Err(Error::UnknownError),
        }
    }
}
