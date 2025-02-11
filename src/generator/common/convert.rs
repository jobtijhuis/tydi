//! Generator methods and implementations for Tydi types.
//!
//! The generator module is enabled by the `generator` feature flag.

use std::borrow::Borrow;
use std::cell::Ref;

use crate::design::implementation::composer::GenericComponent;
use crate::design::{Interface, Streamlet};
pub use crate::error::{Error, Result};
use crate::generator::common::{Component, Mode, Package, Port, Project, Record, Type};
use crate::logical::{Group, LogicalType, Stream, Union};
use crate::physical::{Origin, Signal, Width};
use crate::traits::Identify;
use crate::{cat, Document, NonZeroReal};

// Generator-global constants:

// TODO(johanpel): agree on a suffix that immediately makes users understand
//                 to preferably not touch the canonical component.
/// Suffix provided to the canonical representation of streamlet components.
pub const CANON_SUFFIX: Option<&str> = Some("com");

/// Trait to create common representation types from things in the canonical
/// way and user-friendly way.
pub trait Typify {
    fn canonical(&self, prefix: impl Into<String>) -> Vec<Signal>;
    fn fancy(&self, _prefix: impl Into<String>) -> Option<Type> {
        None
    }
}

/// Trait to create common representation ports from things in the canonical
/// way and user-friendly way.
pub trait Portify {
    fn canonical(&self, name: impl Into<String>) -> Vec<Port>;
    fn fancy(
        &self,
        _port_name: impl Into<String>,
        _port_type_prefix: impl Into<String>,
    ) -> Vec<Port> {
        Vec::new()
    }
}

/// Trait to create common representation components from things in the canonical
/// way and user-friendly way.
pub trait Componentify {
    fn canonical(&self, suffix: Option<&str>) -> Component;
    fn fancy(&self, _suffix: Option<&str>) -> Option<Component> {
        None
    }
}

/// Trait to create common representation of a package.
pub trait Packify {
    fn canonical(&self) -> Package;
    fn fancy(&self) -> Package;
}

/// Trait to create common representation of a project.
pub trait Projectify {
    fn canonical(&self) -> Project;
    fn fancy(&self) -> Project;
}

pub trait Multilane {
    fn with_throughput(
        &self,
        identity: impl Into<String>,
        throughput: NonZeroReal<f64>,
    ) -> Result<Type>;
}

impl Typify for LogicalType {
    fn canonical(&self, prefix: impl Into<String>) -> Vec<Signal> {
        // This implementation for LogicalType assumes the LogicalType has already been
        // flattened through synthesize.
        match self {
            LogicalType::Null => Vec::new(),
            LogicalType::Bits(width) => vec![Signal::vec(prefix.into(), Origin::Source, *width)],
            LogicalType::Group(group) => group.canonical(prefix),
            LogicalType::Stream(stream) => stream.canonical(prefix),
            LogicalType::Union(union) => union.canonical(prefix),
        }
    }

    fn fancy(&self, prefix: impl Into<String>) -> Option<Type> {
        // This implementation for LogicalType assumes the LogicalType has already been
        // flattened through synthesize.
        match self {
            LogicalType::Null => None,
            LogicalType::Bits(width) => Some(Type::bitvec(width.get())),
            LogicalType::Group(group) => group.fancy(prefix),
            LogicalType::Stream(stream) => stream.fancy(prefix),
            LogicalType::Union(union) => union.fancy(prefix),
        }
    }
}

impl Typify for Group {
    fn canonical(&self, prefix: impl Into<String>) -> Vec<Signal> {
        let n: String = prefix.into();
        let mut result = Vec::new();
        for (field_name, field_logical) in self.iter() {
            let field_result = field_logical.canonical(cat!(n.clone(), field_name));
            result.extend(field_result);
        }
        result
    }

    fn fancy(&self, prefix: impl Into<String>) -> Option<Type> {
        let n: String = prefix.into();
        let mut rec = Record::new_empty(n.clone());
        for (field_name, field_logical) in self.iter() {
            if let Some(field_common_type) = field_logical.fancy(cat!(n.clone(), field_name)) {
                rec.insert_new_field(field_name.to_string(), field_common_type, false, None)
            }
        }
        Some(Type::Record(rec))
    }
}

impl Typify for Union {
    fn canonical(&self, prefix: impl Into<String>) -> Vec<Signal> {
        let n: String = prefix.into();
        let mut result = Vec::new();
        if let Some((tag_name, tag_bc)) = self.tag() {
            result.push(Signal::vec(cat!(n, tag_name), Origin::Source, tag_bc));
        }
        for (field_name, field_logical) in self.iter() {
            let field_result = field_logical.canonical(cat!(n.clone(), field_name));
            result.extend(field_result);
        }
        result
    }

    fn fancy(&self, prefix: impl Into<String>) -> Option<Type> {
        let n: String = prefix.into();
        let mut rec = Record::new_empty(n.clone());
        if let Some((tag_name, tag_bc)) = self.tag() {
            let mut variants_doc = vec![];
            for (field_name, _) in self.iter() {
                variants_doc.push(field_name.to_string());
            }
            rec.insert_new_field(
                tag_name,
                Type::bitvec(tag_bc.get()),
                false,
                Some(format!(" Variants: {}", variants_doc.join(", "))),
            );
        }
        for (field_name, field_logical) in self.iter() {
            if let Some(field_common_type) = field_logical.fancy(cat!(n.clone(), field_name)) {
                rec.insert_new_field(field_name, field_common_type, false, None);
            }
        }
        Some(Type::Union(rec))
    }
}

impl Typify for Stream {
    fn canonical(&self, prefix: impl Into<String>) -> Vec<Signal> {
        // This implementation for Stream assumes the parent LogicalType has already been
        // flattened through synthesize.
        let n: String = prefix.into();
        let mut result = Vec::new();

        let logical = LogicalType::from(self.clone());
        assert!(logical.is_element_only());
        if !logical.is_null() {
            let synth = logical.synthesize();
            let (path, phys) = synth.streams().next().unwrap();
            for signal in phys.signal_list().into_iter() {
                let n = cat!(n.clone(), path, signal.identifier());
                result.push(signal.with_name(n));
            }
        }

        result
    }

    fn fancy(&self, prefix: impl Into<String>) -> Option<Type> {
        // This implementation for Stream assumes the parent LogicalType has already been
        // flattened through synthesize.
        let pre: String = prefix.into();
        // We need to wrap the Stream back into a LogicalType
        // to be able to use various methods for checks and synthesize.
        let logical = LogicalType::from(self.clone());

        // At this point, it should not be possible that this is a
        // non-element-only LogicalType.
        assert!(logical.is_element_only());

        // Check if the logical stream is null.
        if !logical.is_null() {
            // Synthesize the logical stream into physical streams.
            let synth = logical.synthesize();

            // Obtain the path name and signal map from the physical stream.
            // There should only be one, since it is an element only stream.
            // Therefore, it should be safe to unwrap.
            let (name, physical) = synth.streams().next().unwrap();
            let signals = physical.signal_list();

            // Set up the resulting record.
            let mut rec = Record::new_empty_stream(match name.len() {
                0 => pre.clone(),
                _ => cat!(pre, name),
            });

            let prefix = cat!(pre, name, "data");
            let data = self.data().fancy(&prefix).unwrap();
            // Insert data record. There must be something there since it is not null.
            // TODO: The fancy version doesn't account for throughput.
            rec.insert_new_field(
                "data",
                data.with_throughput(&prefix, self.throughput()).unwrap(),
                false,
                None,
            );

            // Check signals related to dimensionality, complexity, etc.
            if let Some(sig) = signals.last() {
                rec.insert_new_field("last", sig.width().into(), sig.reversed(), None);
            }
            if let Some(sig) = signals.stai() {
                rec.insert_new_field("stai", sig.width().into(), sig.reversed(), None);
            }
            if let Some(sig) = signals.endi() {
                rec.insert_new_field("endi", sig.width().into(), sig.reversed(), None);
            }
            if let Some(sig) = signals.strb() {
                rec.insert_new_field("strb", sig.width().into(), sig.reversed(), None);
            }

            Some(Type::Record(rec))
        } else {
            None
        }
    }
}

impl From<Width> for Type {
    fn from(width: Width) -> Self {
        match width {
            Width::Scalar => Type::Bit,
            Width::Vector(w) => Type::bitvec(w),
        }
    }
}

/// Trait that helps to determine the common representation port mode given a streamlet interface
/// mode.
pub trait ModeFor {
    /// Return the port mode of self, given a streamlet interface mode.
    fn mode_for(&self, streamlet_mode: crate::design::Mode) -> Mode;
}

impl ModeFor for Origin {
    /// Return the common representation port mode for this signal origin, given the interface mode.
    fn mode_for(&self, streamlet_mode: crate::design::Mode) -> Mode {
        match self {
            Origin::Sink => match streamlet_mode {
                crate::design::Mode::In => Mode::Out,
                crate::design::Mode::Out => Mode::In,
            },
            Origin::Source => match streamlet_mode {
                crate::design::Mode::In => Mode::In,
                crate::design::Mode::Out => Mode::Out,
            },
        }
    }
}

impl Portify for Interface {
    fn canonical(&self, prefix: impl Into<String>) -> Vec<Port> {
        let n: String = prefix.into();
        let mut ports = Vec::new();

        let synth = self.typ().synthesize();

        for (path, width) in synth.signals() {
            ports.push(Port::new(
                cat!(n.clone(), path.to_string()),
                match self.mode() {
                    crate::design::Mode::Out => Mode::Out,
                    crate::design::Mode::In => Mode::In,
                },
                Type::bitvec(width.get()),
            ));
        }

        for (path, phys) in synth.streams() {
            for s in phys.signal_list().into_iter() {
                let port_name = cat!(n.clone(), path, s.identifier());
                ports.push(Port::new(
                    port_name,
                    s.origin().mode_for(self.mode()),
                    s.width().into(),
                ));
            }
        }

        ports
    }

    fn fancy(&self, name: impl Into<String>, type_name: impl Into<String>) -> Vec<Port> {
        let n: String = name.into();
        let tn: String = type_name.into();

        let mut result = Vec::new();

        let split = self.typ().split_streams();

        if let Some(sig_type) = split.signal().fancy(tn.clone()) {
            result.push(Port::new(cat!(n), self.mode().into(), sig_type));
        }

        // Split the LogicalType up into discrete, simple streams.
        for (path, simple_stream) in self.typ().split_streams().streams() {
            if let Some(typ) = simple_stream.fancy(cat!(tn.clone(), path)) {
                result.push(Port::new(cat!(n, path), self.mode().into(), typ));
            }
        }

        result
    }
}

impl From<crate::design::Mode> for Mode {
    fn from(m: crate::design::Mode) -> Self {
        match m {
            crate::design::Mode::Out => Mode::Out,
            crate::design::Mode::In => Mode::In,
        }
    }
}

impl Componentify for Streamlet {
    fn canonical(&self, suffix: Option<&str>) -> Component {
        Component::new(
            cat!(self.identifier().to_string(), suffix.unwrap_or("")),
            vec![],
            {
                // Always add clock and reset for now.
                // TODO(johanpel): at some point we need to associate interfaces with clock domains.
                let mut all_ports = vec![
                    Port::new_documented("clk", Mode::In, Type::Bit, None),
                    Port::new_documented("rst", Mode::In, Type::Bit, None),
                ];
                self.inputs().for_each(|interface| {
                    all_ports.extend(interface.borrow().canonical(interface.identifier()));
                });
                self.outputs().for_each(|interface| {
                    all_ports.extend(interface.borrow().canonical(interface.identifier()));
                });
                all_ports
            },
            self.doc(),
        )
    }

    fn fancy(&self, suffix: Option<&str>) -> Option<Component> {
        Some(Component::new(
            cat!(self.identifier().to_string(), suffix.unwrap_or("")),
            vec![],
            {
                let collect_ports =
                    |interfaces: Box<(dyn Iterator<Item = Ref<Interface>>)>| -> Vec<Port> {
                        interfaces
                            .flat_map(|interface| {
                                interface.borrow().fancy(
                                    interface.identifier(),
                                    cat!(self.identifier().to_string(), interface.identifier()),
                                )
                            })
                            .collect::<Vec<Port>>()
                    };

                let mut all_ports: Vec<Port> = vec![
                    Port::new_documented("clk", Mode::In, Type::Bit, None),
                    Port::new_documented("rst", Mode::In, Type::Bit, None),
                ];
                all_ports.extend(collect_ports(self.inputs()));
                all_ports.extend(collect_ports(self.outputs()));
                all_ports
            },
            self.doc(),
        ))
    }
}

impl Packify for crate::design::Library {
    fn canonical(&self) -> Package {
        Package {
            identifier: self.identifier().to_string(),
            components: self
                .streamlets()
                .into_iter()
                .map(|s| s.canonical(CANON_SUFFIX))
                .collect(),
        }
    }

    fn fancy(&self) -> Package {
        Package {
            identifier: self.identifier().to_string(),
            components: self
                .streamlets()
                .into_iter()
                .flat_map(|s| {
                    let mut result = vec![s.canonical(CANON_SUFFIX)];
                    if let Some(user) = s.fancy(None) {
                        result.push(user);
                    }
                    result
                })
                .collect(),
        }
    }
}

impl Projectify for crate::design::Project {
    fn canonical(&self) -> Project {
        Project {
            identifier: self.identifier().to_string(),
            libraries: self.libraries().map(|l| l.canonical()).collect(),
        }
    }

    fn fancy(&self) -> Project {
        Project {
            identifier: self.identifier().to_string(),
            libraries: self.libraries().map(|l| l.fancy()).collect(),
        }
    }
}

impl Multilane for Type {
    fn with_throughput(
        &self,
        identity: impl Into<String>,
        throughput: NonZeroReal<f64>,
    ) -> Result<Type> {
        if throughput.0 > u32::MAX as f64 {
            return Err(Error::InvalidArgument(format!(
                "Throughput exceeds {}",
                u32::MAX
            )));
        }
        let element_lanes = throughput.0.ceil() as u32;
        if element_lanes > 1 {
            match self {
                Type::Bit => Ok(Type::BitVec {
                    width: element_lanes,
                }),
                Type::Natural => unimplemented!("natural currently not supported outside of generics"),
                Type::Positive => unimplemented!("positive currently not supported outside of generics"),
                Type::BitVec { width: _ } | Type::Record(_) | Type::Union(_) | Type::Array(_) => {
                    Ok(Type::array(
                        format!("{}_array", identity.into()),
                        self.clone(),
                        element_lanes,
                    ))
                }
            }
        } else {
            return Ok(self.clone());
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::design::{Interface, Streamlet};
    use crate::generator::common::test::records;
    use crate::generator::vhdl::Declare;
    use crate::logical::tests::{elements, streams};
    use crate::{Name, Positive, Result, UniqueKeyBuilder};

    use super::*;

    #[test]
    fn test_cat() {
        assert_eq!(cat!("ok"), "ok");
        assert_eq!(cat!("ok", "tydi"), "ok_tydi");
        assert_eq!(cat!("ok", "tydi", ""), "ok_tydi");
        assert_eq!(cat!("", ""), "");
    }

    mod canonical {
        use super::*;

        #[test]
        fn logical_to_common_prim() {
            let typ = elements::prim(8).canonical("test");
            assert_eq!(
                typ,
                vec![Signal::vec(
                    "test".to_string(),
                    Origin::Source,
                    Positive::new(8).unwrap()
                )]
            )
        }

        #[test]
        fn logical_to_common_groups() {
            let typ0 = elements::group().canonical("test");
            assert_eq!(
                typ0,
                vec![
                    Signal::vec(
                        "test_c".to_string(),
                        Origin::Source,
                        Positive::new(42).unwrap()
                    ),
                    Signal::vec(
                        "test_d".to_string(),
                        Origin::Source,
                        Positive::new(1337).unwrap()
                    )
                ]
            );

            let typ1 = elements::group_nested().canonical("test");
            assert_eq!(
                typ1,
                vec![
                    Signal::vec(
                        "test_a_c".to_string(),
                        Origin::Source,
                        Positive::new(42).unwrap()
                    ),
                    Signal::vec(
                        "test_a_d".to_string(),
                        Origin::Source,
                        Positive::new(1337).unwrap()
                    ),
                    Signal::vec(
                        "test_b_c".to_string(),
                        Origin::Source,
                        Positive::new(42).unwrap()
                    ),
                    Signal::vec(
                        "test_b_d".to_string(),
                        Origin::Source,
                        Positive::new(1337).unwrap()
                    ),
                ]
            );

            let typ2 = elements::group_of_single().canonical("test");
            assert_eq!(
                typ2,
                vec![Signal::vec(
                    "test_a".to_string(),
                    Origin::Source,
                    Positive::new(42).unwrap()
                ),]
            );
        }

        #[test]
        fn logical_to_common_streams() {
            let typ0 = streams::prim(8).canonical("test");
            dbg!(&typ0);

            let typ1 = streams::group().canonical("test");
            dbg!(&typ1);
            // TODO(johanpel): implement actual test
        }

        #[test]
        fn interface_to_port() {
            let if0 = Interface::try_new("test", crate::design::Mode::In, streams::prim(8), None)
                .unwrap();
            dbg!(if0.canonical("test"));
            let if1 = Interface::try_new("test", crate::design::Mode::Out, streams::group(), None)
                .unwrap();
            dbg!(if1.canonical("test"));
            // TODO(johanpel): implement actual test
        }
    }

    mod fancy {
        use crate::generator::common::Field;

        use super::*;

        #[test]
        fn logical_to_common_prim() {
            let typ: Type = elements::prim(8).fancy("test").unwrap();
            assert_eq!(typ, records::prim(8));
        }

        #[test]
        fn logical_to_common_groups() {
            let typ0: Type = elements::group().fancy("test").unwrap();
            assert_eq!(typ0, records::rec("test"));

            let typ1: Type = elements::group_nested().fancy("test").unwrap();
            assert_eq!(typ1, records::rec_nested("test"));

            let typ2: Type = elements::group_of_single().fancy("test").unwrap();
            assert_eq!(typ2, records::rec_of_single("test"));
        }

        #[test]
        fn logical_to_common_streams() {
            let typ0: Type = streams::prim(8).fancy("test").unwrap();
            assert_eq!(
                typ0,
                Type::record(
                    "test",
                    vec![
                        Field::new("valid", Type::Bit, false, None),
                        Field::new("ready", Type::Bit, true, None),
                        Field::new("data", Type::bitvec(8), false, None)
                    ]
                )
            );

            let typ1: Type = streams::group().fancy("test").unwrap();
            assert_eq!(
                typ1,
                Type::record(
                    "test",
                    vec![
                        Field::new(
                            "a",
                            Type::record(
                                "test_a",
                                vec![
                                    Field::new("valid", Type::Bit, false, None),
                                    Field::new("ready", Type::Bit, true, None),
                                    Field::new("data", Type::bitvec(42), false, None)
                                ]
                            ),
                            false,
                            None,
                        ),
                        Field::new(
                            "b",
                            Type::record(
                                "test_b",
                                vec![
                                    Field::new("valid", Type::Bit, false, None),
                                    Field::new("ready", Type::Bit, true, None),
                                    Field::new("data", Type::bitvec(1337), false, None)
                                ]
                            ),
                            false,
                            None,
                        )
                    ]
                )
            );
        }

        #[test]
        fn interface_to_port() {
            let if0 = Interface::try_new("test", crate::design::Mode::In, streams::prim(8), None)
                .unwrap();
            dbg!(if0.fancy("test", "test"));
            let if1 = Interface::try_new("test", crate::design::Mode::Out, streams::group(), None)
                .unwrap();
            dbg!(if1.fancy("test", "test"));
            // TODO(johanpel): write actual test
        }
    }

    #[test]
    pub(crate) fn simple_streamlet() -> Result<()> {
        let streamlet = Streamlet::from_builder(
            Name::try_new("test")?,
            UniqueKeyBuilder::new().with_items(vec![
                Interface::try_new("x", crate::design::Mode::In, streams::prim(8), None)?,
                Interface::try_new("y", crate::design::Mode::Out, streams::group(), None)?,
            ]),
            None,
        )?;
        // TODO(johanpel): write actual test
        let common_streamlet = streamlet.fancy(None).unwrap();
        let pkg = Package {
            identifier: "boomer".to_string(),
            components: vec![common_streamlet],
        };
        println!("{}", pkg.declare()?);
        Ok(())
    }

    #[test]
    pub(crate) fn nested_streams_streamlet() -> Result<()> {
        let streamlet = Streamlet::from_builder(
            Name::try_new("test")?,
            UniqueKeyBuilder::new().with_items(vec![
                Interface::try_new("x", crate::design::Mode::In, streams::prim(8), None)?,
                Interface::try_new("y", crate::design::Mode::Out, streams::nested(), None)?,
            ]),
            None,
        )?;
        // TODO(johanpel): write actual test
        let common_streamlet = streamlet.fancy(None).unwrap();
        let pkg = Package {
            identifier: "testing".to_string(),
            components: vec![common_streamlet],
        };
        println!("{}", pkg.declare()?);
        Ok(())
    }
}
