

mod components {

}

pub mod records {
    use crate::generator::{common::*};
    use crate::cat;

    #[allow(dead_code)]
    fn prim(bits: u32) -> Type {
        Type::bitvec(bits)
    }

    fn rec(name: impl Into<String>) -> Type {
        Type::record(
            name.into(),
            vec![
                Field::new("c", Type::bitvec(42), false, None),
                Field::new("d", Type::bitvec(1337), false, None),
            ],
        )
    }

    pub fn rec_rev(name: impl Into<String>) -> Type {
        Type::record(
            name.into(),
            vec![
                Field::new("c", Type::bitvec(42), false, None),
                Field::new("d", Type::bitvec(1337), true, None),
            ],
        )
    }

    #[allow(dead_code)]
    fn rec_of_single(name: impl Into<String>) -> Type {
        Type::record(
            name.into(),
            vec![Field::new("a", Type::bitvec(42), false, None)],
        )
    }

    pub fn rec_rev_nested(name: impl Into<String>) -> Type {
        let n: String = name.into();
        Type::record(
            n.clone(),
            vec![
                Field::new("a", rec(cat!(n, "a")), false, None),
                Field::new("b", rec_rev(cat!(n, "b")), false, None),
            ],
        )
    }

    #[allow(dead_code)]
    fn rec_nested(name: impl Into<String>) -> Type {
        let n: String = name.into();
        Type::record(
            n.clone(),
            vec![
                Field::new("a", rec(cat!(n, "a")), false, None),
                Field::new("b", rec(cat!(n, "b")), false, None),
            ],
        )
    }

    #[allow(dead_code)]
    fn union(name: impl Into<String>) -> Type {
        Type::union(
            name,
            vec![
                Field::new("tag", Type::bitvec(2), false, None),
                Field::new("c", Type::bitvec(42), false, None),
                Field::new("d", Type::bitvec(1337), false, None),
            ],
        )
    }

    #[allow(dead_code)]
    fn union_nested(name: impl Into<String>) -> Type {
        let n: String = name.into();
        Type::union(
            n.clone(),
            vec![
                Field::new("tag", Type::bitvec(2), false, None),
                Field::new("a", union(cat!(n, "a")), false, None),
                Field::new("b", union(cat!(n, "b")), false, None),
            ],
        )
    }
}

#[cfg(test)]
mod test {
    use std::convert::TryFrom;
    
    use crate::generator::{
        common::convert::Packify,
        common::convert::Componentify,
        common::convert::CANON_SUFFIX,
        common::*,
        vhdl::Declare,
    };
    use crate::generator::components::records;
    use crate::logical::LogicalType;
    use crate::stdlib::common::architecture::*;
    use crate::design::{
        project::Project,
        library::Library,
        StreamletKey,
    };
    use crate::{
        Name, cat, Result, parser, Error
    };

    use crate::stdlib::common::{
        architecture::{
            statement::PortMapping,
            assignment::Assign,
            declaration::ObjectDeclaration
        },
    };

    fn logical_slice<'a>(l_type : LogicalType, package: &'a Package) -> Result<Architecture<'a>> {

        let streamlet_key = StreamletKey::try_from("simple_stream")?;

        let architecture = Architecture::new_default(
            &package,
            cat!(streamlet_key, CANON_SUFFIX.unwrap()) 
        )?;
        let portmap =
        PortMapping::from_component(&package.get_component(streamlet_key.clone())?, "canonical")?;

        Ok(architecture)
    }

    pub fn test_comp() -> Component {
        Component::new (
            "test_comp",
            vec![],
            vec![
                Port::new_documented("a", Mode::In, records::rec_rev("a"), None),
                Port::new_documented("b", Mode::Out, records::rec_rev_nested("b"), None),
            ],
            None
        )
    }

    #[test]
    fn comp_decl() {
        let c = test_comp().with_doc(" My awesome\n Component".to_string());
        assert_eq!(
            c.declare().unwrap(),
            concat!(
                "-- My awesome
-- Component
component test_comp
  port(
    a_dn : in a_dn_type;
    a_up : out a_up_type;
    b_dn : out b_dn_type;
    b_up : in b_up_type
  );
end component;"
            )
        );
    }

    #[test]
    fn test_logical_slice() -> Result<()>{
        let my_type = LogicalType::try_new_bits(8).unwrap();

        let (_, streamlet) = parser::nom::streamlet(
            "Streamlet streamlet (a : in Bits<1>, b : out Bits<1>)",
        ).unwrap();

        let (_, complex_streamlet) = parser::nom::streamlet(
            "Streamlet complex_streamlet (a : in Stream<Bits<1>>, b : out Stream<Bits<1>>)",
        ).unwrap();

        let library = Library::try_new(
            Name::try_from("test_library")?,
            vec![],
            vec![streamlet, complex_streamlet]
        )?;

        let streamlet_key = StreamletKey::try_from("complex_streamlet")?;

        let package = library.canonical();

        let mut architecture = Architecture::new_default(
            &package,
            cat!(streamlet_key, CANON_SUFFIX.unwrap())
        )?;
        // let mut portmap =
        // PortMapping::from_component(
        //     &package.get_component(cat!(streamlet_key, CANON_SUFFIX.unwrap()))?, 
        //     "canonical"
        // )?;

        let streamslice_comp = Component::new(
            "StreamSlice",
            vec![
                // TODO: Implement natural
                Parameter{name: String::from("DATA_WIDTH"), typ: Type::Bit}
            ],
            vec![
                Port::new_documented("clk", Mode::In, Type::Bit, None),
                Port::new_documented("reset", Mode::In, Type::Bit, None),
                Port::new_documented("in_valid", Mode::In, Type::Bit, None),
                Port::new_documented("in_ready", Mode::In, Type::Bit, None),
                Port::new_documented("in_data", Mode::In, Type::Bit, None),
                Port::new_documented("out_valid", Mode::Out, Type::Bit, None),
                Port::new_documented("out_ready", Mode::Out, Type::Bit, None),
                Port::new_documented("out_data", Mode::Out, Type::Bit, None)
            ],
            Some(String::from("test"))
        );

        let mut slice_portmap =
        PortMapping::from_component(
            &streamslice_comp,
            "streamslice"
        )?;

        // create signals and assignments for StreamSlice component
        for (port_name, object) in slice_portmap.clone().ports() {
            let signal = ObjectDeclaration::signal(cat!(port_name, "wire"), object.typ().clone(), None);
            let _assign_decl = signal.assign(object)?;
            slice_portmap.map_port(port_name, &signal)?;
            architecture.add_declaration(signal)?;
        }

        architecture.add_statement(slice_portmap)?;

        // // create signals and assignments for component from entity
        // for (port_name, object) in portmap.clone().ports() {
        //     let signal = ObjectDeclaration::signal(cat!(port_name, "wire"), object.typ().clone(), None);
        //     let _assign_decl = signal.assign(architecture.entity_ports()?.get(port_name).ok_or(
        //         Error::BackEndError(format!("Entity does not have a {} signal", port_name)),
        //     )?)?;
        //     portmap.map_port(port_name, &signal)?;
        //     architecture.add_declaration(signal)?;
        // }

        // architecture.add_statement(portmap)?;

        println!("{}", architecture.declare()?);

        Ok(())

    }

    // Things to add for using external component:
    // use work.Stream_pkg.all;
    // PortMapping of Component
    // add generics to Portmapping
    // use ObjectDeclaration::constant for defining generics
    // Find a way to do sub assign of bitvector


}
