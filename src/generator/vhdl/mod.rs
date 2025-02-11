//! VHDL back-end.
//!
//! This module contains functionality to convert hardware defined in the common hardware
//! representation to VHDL source files.

use std::collections::HashSet;
use std::path::Path;
use std::str::FromStr;

use indexmap::IndexMap;
use log::debug;
#[cfg(feature = "cli")]
use structopt::StructOpt;

use crate::cat;
use crate::design::implementation::composer::GenericComponent;
use crate::design::Project;
use crate::generator::common::convert::Packify;
use crate::generator::common::*;
use crate::generator::GenerateProject;
use crate::stdlib::utils::fancy_wrapper::generate_fancy_wrapper;
use crate::traits::Identify;
use crate::Name;
use crate::{Error, Result, Reversed};

mod impls;

/// Generate trait for generic VHDL declarations.
pub trait Declare {
    /// Generate a VHDL declaration from self.
    fn declare(&self) -> Result<String>;
}

/// Generate trait for VHDL type declarations.
pub trait DeclareType {
    /// Generate a VHDL declaration from self.
    fn declare(&self, is_root_type: bool) -> Result<String>;
}

/// Generate trait for VHDL package declarations.
pub trait DeclareLibrary {
    /// Generate a VHDL declaration from self.
    fn declare(&self, abstraction: AbstractionLevel) -> Result<String>;
}

/// A list of VHDL usings, indexed by library
#[derive(Debug, Clone)]
pub struct Usings(IndexMap<Name, HashSet<String>>);

impl Usings {
    pub fn new_empty() -> Usings {
        Usings(IndexMap::new())
    }

    /// If the set did not have this value present, `true` is returned.
    ///
    /// If the set did have this value present, `false` is returned.
    pub fn add_using(&mut self, library: Name, using: impl Into<String>) -> bool {
        self.0
            .entry(library)
            .or_insert(HashSet::new())
            .insert(using.into())
    }

    pub fn usings(&self) -> &IndexMap<Name, HashSet<String>> {
        &self.0
    }

    /// Combine two usings
    pub fn combine(&mut self, other: &Usings) {
        for (library, using) in other.usings() {
            self.0.insert(library.clone(), using.clone());
        }
    }
}

pub trait ListUsings {
    fn list_usings(&self) -> Result<Usings>;
}

pub trait DeclareUsings {
    fn declare_usings(&self) -> Result<String>;
}

/// Generate supertrait for VHDL with usings declarations. (E.g. use ieee.std_logic_1164.all;)
impl<T: ListUsings> DeclareUsings for T {
    fn declare_usings(&self) -> Result<String> {
        let mut result = String::new();

        for (lib, usings) in self.list_usings()?.0 {
            result.push_str(format!("library {};\n", lib).as_str());
            for using in usings {
                result.push_str(format!("use {}.{};\n", lib, using).as_str());
            }
            result.push_str("\n");
        }

        Ok(result)
    }
}

/// Generate trait for VHDL identifiers.
pub trait VHDLIdentifier {
    /// Generate a VHDL identifier from self.
    fn vhdl_identifier(&self) -> Result<String>;
}

/// Analyze trait for VHDL objects.
pub trait Analyze {
    /// List all nested types used.
    fn list_nested_types(&self) -> Vec<Type>;
}

/// Abstraction levels
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "cli", derive(StructOpt))]
pub enum AbstractionLevel {
    Canonical,
    Fancy,
}

impl Default for AbstractionLevel {
    fn default() -> Self {
        AbstractionLevel::Fancy
    }
}

impl FromStr for AbstractionLevel {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "canon" => Ok(AbstractionLevel::Canonical),
            "fancy" => Ok(AbstractionLevel::Fancy),
            _ => Err(Error::InvalidArgument(s.to_string())),
        }
    }
}

/// VHDL back-end configuration parameters.
#[derive(Debug)]
#[cfg_attr(feature = "cli", derive(StructOpt))]
pub struct VHDLConfig {
    /// Abstraction level of generated files.
    /// Possible options: canonical, fancy.
    ///   canonical: generates the canonical Tydi representation of streamlets as components in a
    ///              package.
    ///   fancy: generates the canonical components that wrap a more user-friendly version for the
    ///          user to implement.
    #[cfg_attr(feature = "cli", structopt(short, long))]
    abstraction: Option<AbstractionLevel>,

    /// Suffix of generated files. Default = "gen", such that
    /// generated files are named <name>.gen.vhd.
    #[cfg_attr(feature = "cli", structopt(short, long))]
    suffix: Option<String>,
}

impl VHDLConfig {
    pub fn abstraction(&self) -> AbstractionLevel {
        self.abstraction.unwrap_or_default()
    }
}

impl Default for VHDLConfig {
    fn default() -> Self {
        VHDLConfig {
            suffix: Some("gen".to_string()),
            abstraction: Some(AbstractionLevel::Fancy),
        }
    }
}

/// A configurable VHDL back-end entry point.
#[derive(Default)]
pub struct VHDLBackEnd {
    /// Configuration for the VHDL back-end.
    config: VHDLConfig,
}

impl VHDLBackEnd {
    pub fn config(&self) -> &VHDLConfig {
        &self.config
    }
}

impl From<VHDLConfig> for VHDLBackEnd {
    fn from(config: VHDLConfig) -> Self {
        VHDLBackEnd { config }
    }
}

impl GenerateProject for VHDLBackEnd {
    fn generate(&self, project: &Project, path: impl AsRef<Path>) -> Result<()> {
        // Create the project directory.
        let mut dir = path.as_ref().to_path_buf();
        dir.push(project.identifier());
        std::fs::create_dir_all(dir.as_path())?;

        for lib in project.libraries() {
            let mut pkg = dir.clone();
            pkg.push(format!("{}_pkg", lib.identifier()));
            pkg.set_extension(match self.config.suffix.clone() {
                None => "vhd".to_string(),
                Some(s) => format!("{}.vhd", s),
            });
            let pak = match self.config().abstraction() {
                AbstractionLevel::Canonical => lib.canonical(),
                AbstractionLevel::Fancy => lib.fancy(),
            };
            std::fs::write(pkg.as_path(), pak.declare()?)?;
            debug!("Wrote {}.", pkg.as_path().to_str().unwrap_or(""));
            if let AbstractionLevel::Fancy = self.config().abstraction() {
                for streamlet in lib.streamlets() {
                    let mut wrapper = dir.clone();
                    wrapper.push(format!("{}_wrapper", streamlet.identifier()));
                    wrapper.set_extension(match self.config.suffix.clone() {
                        None => "vhd".to_string(),
                        Some(s) => format!("{}.vhd", s),
                    });
                    let arch = generate_fancy_wrapper(&pak, &streamlet.key())?;
                    std::fs::write(wrapper.as_path(), arch.declare()?)?;
                }
            }
        }
        Ok(())
    }
}

/// Trait used to split types, ports, and record fields into a VHDL-friendly versions, since VHDL
/// does not support bundles of wires with opposite directions.
pub trait Split {
    /// Split up self into a (downstream/forward, upstream/reverse) version, if applicable.
    fn split(&self) -> (Option<Self>, Option<Self>)
    where
        Self: Sized;
}

impl Split for Type {
    fn split(&self) -> (Option<Self>, Option<Self>) {
        match self {
            Type::Record(rec) => {
                let (down_rec, up_rec) = rec.split();
                (down_rec.map(Type::Record), up_rec.map(Type::Record))
            }
            Type::Union(rec) => {
                let (down_rec, up_rec) = rec.split();
                (down_rec.map(Type::Union), up_rec.map(Type::Union))
            }
            _ => (Some(self.clone()), None),
        }
    }
}

impl Split for Field {
    fn split(&self) -> (Option<Self>, Option<Self>) {
        // Split the inner type.
        let (down_type, up_type) = self.typ().split();

        let result = (
            down_type.map(|t| Field::new(self.identifier(), t, false, None)),
            up_type.map(|t| Field::new(self.identifier(), t, false, None)),
        );

        if self.is_reversed() {
            // If this field itself is reversed, swap the result of splitting the field type.
            (result.1, result.0)
        } else {
            result
        }
    }
}

impl Split for Record {
    fn split(&self) -> (Option<Self>, Option<Self>) {
        let mut down_rec = Record::new_empty(self.identifier());
        let mut up_rec = Record::new_empty(self.identifier());

        for f in self.fields().into_iter() {
            let (down_field, up_field) = f.split();
            if let Some(df) = down_field {
                down_rec.insert(df)
            };
            if let Some(uf) = up_field {
                up_rec.insert(uf)
            };
        }

        let f = |r: Record| if r.is_empty() { None } else { Some(r) };

        (f(down_rec), f(up_rec))
    }
}

impl Split for Array {
    fn split(&self) -> (Option<Self>, Option<Self>) {
        let (dn, up) = self.typ().split();
        let down_arr = if let Some(df) = dn {
            Some(Array::new(self.identifier(), df, self.width()))
        } else {
            None
        };
        let up_arr = if let Some(uf) = up {
            Some(Array::new(self.identifier(), uf, self.width()))
        } else {
            None
        };

        (down_arr, up_arr)
    }
}

impl Split for Port {
    fn split(&self) -> (Option<Self>, Option<Self>) {
        let (type_down, type_up) = self.typ().split();
        (
            type_down.map(|t| {
                Port::new(
                    cat!(self.identifier(), "dn"),
                    self.mode(),
                    match t {
                        Type::Record(r) => Type::Record(r.append_name_nested("dn")),
                        Type::Union(r) => Type::Union(r.append_name_nested("dn")),
                        _ => t,
                    },
                )
            }),
            type_up.map(|t| {
                Port::new(
                    cat!(self.identifier(), "up"),
                    self.mode().reversed(),
                    match t {
                        Type::Record(r) => Type::Record(r.append_name_nested("up")),
                        Type::Union(r) => Type::Union(r.append_name_nested("up")),
                        _ => t,
                    },
                )
            }),
        )
    }
}

#[cfg(test)]
mod test {
    use std::fs;

    use crate::Reversed;

    use super::*;

    #[test]
    fn split_primitive() {
        assert_eq!(Type::bitvec(3).split(), (Some(Type::bitvec(3)), None));
    }

    #[test]
    fn split_field() {
        let f0 = Field::new("test", Type::bitvec(3), false, None);
        assert_eq!(f0.split(), (Some(f0), None));

        let f1 = Field::new("test", Type::bitvec(3), true, None);
        assert_eq!(f1.split(), (None, Some(f1.reversed())));
    }

    #[test]
    fn split_simple_rec() {
        let rec = Type::record(
            "ra",
            vec![
                Field::new("fc", Type::Bit, false, None),
                Field::new("fd", Type::Bit, true, None),
            ],
        );

        assert_eq!(
            rec.split().0.unwrap(),
            Type::record("ra", vec![Field::new("fc", Type::Bit, false, None)])
        );

        assert_eq!(
            rec.split().1.unwrap(),
            Type::record("ra", vec![Field::new("fd", Type::Bit, false, None)])
        );
    }

    #[test]
    fn split_nested_rec() {
        let rec = Type::record(
            "test",
            vec![
                Field::new(
                    "fa",
                    Type::record(
                        "ra",
                        vec![
                            Field::new("fc", Type::Bit, false, None),
                            Field::new("fd", Type::Bit, true, None),
                        ],
                    ),
                    false,
                    None,
                ),
                Field::new(
                    "fb",
                    Type::record(
                        "rb",
                        vec![
                            Field::new("fe", Type::Bit, false, None),
                            Field::new("ff", Type::Bit, true, None),
                        ],
                    ),
                    true,
                    None,
                ),
            ],
        );

        assert_eq!(
            rec.split().0.unwrap(),
            Type::record(
                "test",
                vec![
                    Field::new(
                        "fa",
                        Type::record("ra", vec![Field::new("fc", Type::Bit, false, None)]),
                        false,
                        None,
                    ),
                    Field::new(
                        "fb",
                        Type::record("rb", vec![Field::new("ff", Type::Bit, false, None)]),
                        false,
                        None,
                    )
                ]
            )
        );

        assert_eq!(
            rec.split().1.unwrap(),
            Type::record(
                "test",
                vec![
                    Field::new(
                        "fa",
                        Type::record("ra", vec![Field::new("fd", Type::Bit, false, None)]),
                        false,
                        None,
                    ),
                    Field::new(
                        "fb",
                        Type::record("rb", vec![Field::new("fe", Type::Bit, false, None)]),
                        false,
                        None,
                    )
                ]
            )
        );
    }

    #[test]
    fn split_port() {
        let (dn, up) = Port::new_documented(
            "test",
            Mode::Out,
            Type::record(
                "test",
                vec![
                    Field::new("a", Type::Bit, false, None),
                    Field::new("b", Type::Bit, true, None),
                ],
            ),
            None,
        )
        .split();

        assert_eq!(
            dn,
            Some(Port::new_documented(
                "test_dn",
                Mode::Out,
                Type::record("test_dn", vec![Field::new("a", Type::Bit, false, None)]),
                None
            ))
        );

        assert_eq!(
            up,
            Some(Port::new_documented(
                "test_up",
                Mode::In,
                Type::record("test_up", vec![Field::new("b", Type::Bit, false, None)]),
                None
            ))
        );
    }

    #[test]
    fn backend() -> Result<()> {
        let v = VHDLBackEnd::default();

        let tmpdir = tempfile::tempdir()?;
        let path = tmpdir.path().join("__test");

        assert!(v
            .generate(&crate::design::project::tests::proj::empty_proj(), &path)
            .is_ok());

        // Check if files were correctly generated.
        assert!(fs::metadata(&path).is_ok());
        assert!(fs::metadata(&path.join("proj")).is_ok());
        assert!(fs::metadata(&path.join("proj/lib_pkg.gen.vhd")).is_ok());

        Ok(())
    }
}
