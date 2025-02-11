use indexmap::IndexMap;

use crate::Result;
use crate::{
    generator::{
        common::Package,
        vhdl::{ListUsings, Usings},
    },
    Identify, Name,
};

use super::entity::Entity;

use self::declaration::{ArchitectureDeclaration, ObjectDeclaration};
use self::statement::Statement;

pub mod assignment;
pub mod declaration;
pub mod impls;
pub mod object;
pub mod statement;

// NOTE: One of the main things to consider is probably how to handle multiple element lanes. Probably as a check on the number of lanes,
// then wrapping in a generate statement. Need to consider indexes at that point.
// This'd be easier if I simply always made it an array, even when the number of lanes is 1, but that gets real ugly, real fast.

/// An architecture
#[derive(Debug, Clone)]
pub struct Architecture<'a> {
    /// Name of the architecture
    identifier: Name,
    /// Entity which this architecture is for
    entity: Entity,
    /// Additional usings beyond the Package and those within it
    usings: Usings,
    /// Documentation.
    doc: Option<String>,
    /// The declaration part of the architecture
    declaration: Vec<ArchitectureDeclaration<'a>>,
    /// The statement part of the architecture
    statement: Vec<Statement>,
}

pub trait ArchitectureDeclare {
    /// Returns a string for the declaration, pre is useful for tabs/spaces, post is useful for closing characters (','/';')
    fn declare(&self, pre: &str, post: &str) -> Result<String>;
}

impl<'a> Architecture<'a> {
    /// Create the architecture based on a component contained within a package, assuming the library (project) is "work" and the architecture's identifier is "Behavioral"
    pub fn new_default(package: &Package, component_id: impl Into<String>) -> Result<Architecture> {
        Architecture::new(
            Name::try_new("work")?,
            Name::try_new("Behavioral")?,
            package,
            component_id,
        )
    }

    /// Create the architecture based on a component contained within a package, specify the library (project) in which the package is contained
    pub fn new(
        library_id: Name,
        identifier: Name,
        package: &Package,
        component_id: impl Into<String>,
    ) -> Result<Architecture> {
        let mut usings = package.list_usings()?;
        usings.add_using(library_id, format!("{}.all", package.identifier));
        Ok(Architecture {
            identifier,
            entity: Entity::from(package.get_component(component_id)?),
            usings: usings,
            doc: None,
            declaration: vec![],
            statement: vec![],
        })
    }

    /// Add additional usings which weren't already part of the package
    pub fn add_using(&mut self, library: Name, using: impl Into<String>) -> bool {
        self.usings.add_using(library, using)
    }

    /// Return this architecture with documentation added.
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    /// Set the documentation of this architecture.
    pub fn set_doc(&mut self, doc: impl Into<String>) {
        self.doc = Some(doc.into())
    }

    pub fn add_declaration(
        &mut self,
        declaration: impl Into<ArchitectureDeclaration<'a>>,
    ) -> Result<()> {
        let declaration = declaration.into();
        match &declaration {
            ArchitectureDeclaration::Object(object) => {
                self.usings.combine(&object.list_usings()?);
            }
            ArchitectureDeclaration::Alias(alias) => {
                self.usings.combine(&alias.object().list_usings()?);
            }
            ArchitectureDeclaration::Type(_)
            | ArchitectureDeclaration::SubType(_)
            | ArchitectureDeclaration::Procedure(_)
            | ArchitectureDeclaration::Function(_)
            | ArchitectureDeclaration::Component(_)
            | ArchitectureDeclaration::Custom(_) => (),
        }
        self.declaration.push(declaration);
        Ok(())
    }

    pub fn add_statement(&mut self, statement: impl Into<Statement>) -> Result<()> {
        let statement = statement.into();
        match &statement {
            Statement::Assignment(assignment) => self.usings.combine(&assignment.list_usings()?),
            Statement::PortMapping(pm) => {
                for (_, object) in pm.ports() {
                    self.usings.combine(&object.list_usings()?);
                }
            }
        }
        self.statement.push(statement);
        Ok(())
    }

    pub fn statements(&self) -> &Vec<Statement> {
        &self.statement
    }

    pub fn declarations(&self) -> &Vec<ArchitectureDeclaration> {
        &self.declaration
    }

    pub fn entity_ports(&self) -> Result<IndexMap<String, ObjectDeclaration>> {
        let mut result = IndexMap::new();
        for port in self.entity.ports() {
            let objs = ObjectDeclaration::from_port(port, true)?;
            for obj in objs {
                result.insert(obj.identifier().to_string(), obj);
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::generator::{common::convert::Packify, vhdl::Declare};

    use super::*;

    pub fn test_package() -> Package {
        let (_, streamlet) = crate::parser::nom::streamlet(
            "Streamlet test (a : in Stream<Bits<1>>, b : out Stream<Bits<2>, d=2>)",
        )
        .unwrap();
        let lib = crate::design::library::Library::try_new(
            Name::try_new("test").unwrap(),
            vec![],
            vec![streamlet],
        );
        let lib: crate::generator::common::Package = lib.unwrap().fancy();
        lib
    }

    #[test]
    fn test_architecture() {
        let package = test_package();
        let architecture =
            Architecture::new_default(&package, Name::try_new("test").unwrap()).unwrap();

        assert_eq!(
            r#"library ieee;
use ieee.std_logic_1164.all;

library work;
use work.test.all;

entity test is
  port(
    clk : in std_logic;
    rst : in std_logic;
    a_dn : in test_a_dn_type;
    a_up : out test_a_up_type;
    b_dn : out test_b_dn_type;
    b_up : in test_b_up_type
  );
end test;

architecture Behavioral of test is
begin
end Behavioral;
"#,
            architecture.declare().unwrap()
        );
    }
}
