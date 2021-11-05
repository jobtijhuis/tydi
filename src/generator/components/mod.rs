

mod components {
    use std::convert::TryFrom;
    use std::sync::atomic::{AtomicU32, Ordering};
    use crate::{
        Name, cat, Result, Error, Identify
    };
    use crate::logical::{
        LogicalType, Direction
    };
    use crate::design::{
        StreamletKey,
    };
    use crate::generator::{
        common::*,
        common::convert::CANON_SUFFIX,
        vhdl::Declare,
    };
    use crate::stdlib::common::architecture::*;

    pub fn alphabet_sequence(num: u32) -> String {
        let parent = num / 26;
        if parent > 0 {
            return alphabet_sequence(parent - 1) + &alphabet_sequence(num % 26);
        } else {
            let alph_num = 97 + num % 26;
            return char::from_u32(alph_num).unwrap().to_string();
        }
    }

    pub fn logical_slice<'a>(logical_type : LogicalType, package: &'a mut Package) -> Result<Architecture<'a>> {

        fn gen_ports (l_type: &LogicalType, gen_clk_rst: bool) -> Vec<Port> {
            let mut all_ports = if gen_clk_rst {
                vec![
                    Port::new_documented("clk", Mode::In, Type::Bit, None),
                    Port::new_documented("rst", Mode::In, Type::Bit, None),
                ]
            } else { vec![] };
            static COUNTER: AtomicU32 = AtomicU32::new(0);

            match &l_type {
                LogicalType::Null => todo!(), // implement later
                LogicalType::Bits(width) => {
                    let data_in_count = COUNTER.fetch_add(1, Ordering::Relaxed);
                    all_ports.push(Port::new_documented(cat!["in_data", alphabet_sequence(data_in_count)], Mode::In, Type::bitvec(width.get()), None));
                    all_ports
                }
                LogicalType::Group(_) => todo!(), // needs implementation
                LogicalType::Union(_) => todo!(), // needs implementation
                // Nested streams currently not supported
                LogicalType::Stream(stream) => {
                    if stream.dimensionality() > 1 { todo!() }
                    if stream.direction() == Direction::Reverse { todo!() }
                    all_ports.push(Port::new_documented("in_valid", Mode::In, Type::Bit, None));
                    all_ports.push(Port::new_documented("in_ready", Mode::Out, Type::Bit, None));
                    all_ports.extend(gen_ports(stream.data(), false));

                    all_ports.push(Port::new_documented("out_valid", Mode::Out, Type::Bit, None));
                    all_ports.push(Port::new_documented("out_ready", Mode::In, Type::Bit, None));
                    // TODO: Add all output ports that have been sliced
                    all_ports
                }
            }
        }

        let entity_ports = gen_ports(&logical_type, true);

        static SLICE_COUNTER: AtomicU32 = AtomicU32::new(0);
        let slice_count = SLICE_COUNTER.fetch_add(1, Ordering::Relaxed);

        let entity_name = cat!["slice", alphabet_sequence(slice_count)];
        let slice_entity = Component::new(
            entity_name.clone(),
            vec![],
            entity_ports,
            None
        );

        package.components.push(slice_entity);

        // let portmap = PortMapping::from_component(&package.get_component(streamlet_key.clone())?, "canonical")?;

        let architecture = Architecture::new_default(
            package,
            entity_name
        )?;

        Ok(architecture)
    }
}


#[cfg(test)]
mod test {
    use std::convert::TryFrom;
    use std::sync::atomic::{AtomicU32, Ordering};
    
    use crate::generator::{
        common::convert::Packify,
        common::convert::Portify,
        common::convert::CANON_SUFFIX,
        common::*,
        vhdl::Declare,
    };
    use crate::logical::{
        LogicalType, Direction
    };
    use crate::stdlib::common::architecture::*;
    use crate::design::{
        project::Project,
        library::Library,
        StreamletKey,
        ComponentKey, IFKey, Interface, Streamlet,
        implementation::composer::GenericComponent
    };
    use crate::{
        Name, cat, Result, parser, Error, Identify
    };

    use crate::stdlib::common::{
        architecture::{
            statement::PortMapping,
            assignment::Assign,
            declaration::ObjectDeclaration
        },
    };

    use super::*;

    #[test]
    fn slice_simple_generation() -> Result<()> {
        // let (_, streamlet) = parser::nom::streamlet(
        //     "Streamlet streamlet (b : out Stream<Bits<8>>)",
        // ).unwrap();
        let (_, streamlet) = parser::nom::streamlet(
            "Streamlet streamlet (a : out Stream<Bits<8>>, b : out Stream<Bits<8>>)",
        ).unwrap();

        let interface = streamlet.interfaces().next().unwrap().clone();
        let int_ports = interface.canonical(interface.identifier());
        let logical_type = interface.typ();

        let library = Library::try_new(
            Name::try_from("test_library")?,
            vec![],
            vec![streamlet]
        )?;

        let mut package = library.canonical();

        let architecture = components::logical_slice(logical_type, &mut package)?;

        println!("architecture: \n {}", architecture.declare()?);

        // Convert Signal to Type from MatthijsR

        let example_output = "
    library ieee;
    use ieee.std_logic_1164.all;

    library work;
    use work.test_library.all;

    entity slice_a is
    port(
        clk : in std_logic;
        rst : in std_logic;
        in_valid : in std_logic;
        in_ready : out std_logic;
        in_data_a : in std_logic_vector(7 downto 0);
        out_valid : out std_logic;
        out_ready : in std_logic;
        out_data_a : out std_logic_vector(7 downto 0);
    );
    end slice_a;

    architecture Behavioral of slice_a is
    begin
    end Behavioral;
    ";

        Ok(())
    }


    #[test]
    fn streamlet_simple_gen() -> Result<()> {
        let (_, streamlet) = parser::nom::streamlet(
            "Streamlet streamlet (a : out Stream<Group<op1: Bits<8>, op2: Bits<8>>>)",
        ).unwrap();

        let library = Library::try_new(
            Name::try_from("test_library")?,
            vec![],
            vec![streamlet]
        )?;

        let package = library.fancy();

        let architecture = Architecture::new_default(
            &package,
            cat!("streamlet", CANON_SUFFIX.unwrap())
        )?;

        println!("{}", architecture.declare()?);

        Ok(())
    }

    #[test]
    fn test_slice() -> Result<()>{
        let my_type = LogicalType::try_new_bits(8).unwrap();

        let (_, streamlet) = parser::nom::streamlet(
            "Streamlet streamlet (a : in Bits<1>, b : out Bits<1>)",
        ).unwrap();

        let (_, complex_streamlet) = parser::nom::streamlet(
            "Streamlet complex_streamlet (a : in Stream<Bits<8>>, b : out Stream<Bits<8>>)",
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

        //let mut conc_assigns = vec![];

        // create signals and assignments for StreamSlice component
        for (port_name, object) in slice_portmap.clone().ports() {
            let signal = ObjectDeclaration::signal(cat!(port_name, "wire"), object.typ().clone(), None);
            //let _assign_decl = signal.assign(object)?;
            slice_portmap.map_port(port_name, &signal)?;
            architecture.add_declaration(signal)?;
        }

        // for assign in conc_assigns {
        //     architecture.add_statement(assign)?;
        // }

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

    #[test]
    fn test_alph_seq() {
        println!("{}", components::alphabet_sequence(701));
        assert_eq!(components::alphabet_sequence(0),   String::from("a"));
        assert_eq!(components::alphabet_sequence(25),  String::from("z"));
        assert_eq!(components::alphabet_sequence(26),  String::from("aa"));
        assert_eq!(components::alphabet_sequence(30),  String::from("ae"));
        assert_eq!(components::alphabet_sequence(701), String::from("zz"));
        assert_eq!(components::alphabet_sequence(702), String::from("aaa"));
    }

}
