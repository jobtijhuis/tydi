

mod components {
    use std::sync::atomic::{AtomicU32, Ordering};
    use crate::{
        Name, cat, Result, Error, Identify, Natural
    };
    use crate::logical::{
        LogicalType
    };
    use crate::generator::{
        common::*,
        common::convert::ModeFor,
    };
    use crate::stdlib::common::architecture::*;
    use crate::stdlib::common::architecture::{
        statement::PortMapping,
        declaration::ObjectDeclaration,
        declaration::ObjectMode,
        object::ObjectType,
        assignment::Assign,
        assignment::FieldSelection,
        assignment::ObjectAssignment,
    };

    pub fn alphabet_sequence(num: u32) -> String {
        let parent = num / 26;
        if parent > 0 {
            return alphabet_sequence(parent - 1) + &alphabet_sequence(num % 26);
        } else {
            let alph_num = 97 + num % 26;
            return char::from_u32(alph_num).unwrap().to_string();
        }
    }

    pub fn slice_comp(typ: Type) -> Component {
        Component::new(
            "StreamSlice",
            vec![
                Parameter{name: String::from("DATA_WIDTH"), typ: Type::Natural}
            ],
            vec![
                Port::new_documented("clk", Mode::In, Type::Bit, None),
                Port::new_documented("reset", Mode::In, Type::Bit, None),
                Port::new_documented("in_valid", Mode::In, Type::Bit, None),
                Port::new_documented("in_ready", Mode::Out, Type::Bit, None),
                // in_data type and size determined from argument
                Port::new_documented("in_data", Mode::In, typ.clone(), None),
                Port::new_documented("out_valid", Mode::Out, Type::Bit, None),
                Port::new_documented("out_ready", Mode::In, Type::Bit, None),
                // out_data type and size determined from argument
                Port::new_documented("out_data", Mode::Out, typ.clone(), None)
            ],
            Some(String::from("test"))
        )
    }


    pub fn logical_slice<'a>(logical_type : LogicalType, package: &'a mut Package) -> Result<Architecture<'a>> {

        fn gen_ports (l_type: &LogicalType, mode: crate::design::Mode) -> Vec<Port> {
            let mut ports = vec![];
            let synth_logical = l_type.synthesize();
            let prefix = match mode {
                crate::design::Mode::In => "in",
                crate::design::Mode::Out => "out",
            };

            for (path, width) in synth_logical.signals() {
                ports.push(Port::new(
                    cat![prefix, path.to_string()],
                    match mode {
                        crate::design::Mode::In => Mode::In,
                        crate::design::Mode::Out => Mode::Out,
                    },
                    Type::bitvec(width.get()),
                ));
            }

            for (path, phys) in synth_logical.streams() {
                for s in phys.signal_list().into_iter() {
                    let port_name = cat!(prefix, path, s.identifier());
                    ports.push(Port::new(
                        port_name,
                        s.origin().mode_for(mode),
                        s.width().into(),
                    ));
                }
            }
            ports
        }

        // clk and rst are added manually because they are only added when a streamlet is synthesized
        // and we operate at two levels below a streamlet but still need clk and rst
        let mut entity_ports = vec![
            Port::new_documented("clk", Mode::In, Type::Bit, None),
            Port::new_documented("rst", Mode::In, Type::Bit, None),
        ];
        entity_ports.extend(gen_ports(&logical_type, crate::design::Mode::In));
        entity_ports.extend(gen_ports(&logical_type, crate::design::Mode::Out));

        // DATA_WIDTH spans all input ports except the valid/ready handshake signals
        let data_width: Natural = entity_ports
            .iter()
            .filter(|port| {
                port.identifier().starts_with("in_")
                    && port.identifier() != "in_valid"
                    && port.identifier() != "in_ready"
            })
            .map(|port| match port.typ() {
                Type::BitVec { width } => width,
                Type::Bit => 1,
                _ => 0,
            })
            .sum();

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

        // the slice's data ports carry the full concatenated payload of width DATA_WIDTH
        let slice = slice_comp(Type::bitvec(data_width));

        let mut slice_portmap = PortMapping::from_component(&slice, "canonical")?;

        let mut architecture = Architecture::new_default(
            package,
            entity_name
        )?;

        let mut slice_signals = vec![];
        let mut slice_assignments = vec![];

        let ent_ports = architecture.entity_ports().unwrap();

        slice_portmap.map_generic("DATA_WIDTH", &data_width)?;

        // create signals and assignments for StreamSlice component
        for (port_name, object) in slice_portmap.clone().ports() {
            let signal = ObjectDeclaration::signal(cat!(port_name, "wire"), object.typ().clone(), None);
            slice_signals.push(signal.clone());
            //let _assign_decl = signal.assign(object)?;

            let entity_port = ent_ports.get(
                match port_name.as_str() {
                    "reset" => "rst",
                    _ => port_name,
                }
            ).ok_or(
                Error::BackEndError(format!("Entity does not have a {} signal", port_name))
            )?;
            if port_name.as_str() == "in_data" {
                // drive the slice's input data from the concatenation of all
                // input ports, excluding the valid/ready handshake signals
                let in_ports: Vec<ObjectDeclaration> = ent_ports
                    .iter()
                    .filter(|(name, _)| {
                        name.starts_with("in_")
                            && name.as_str() != "in_valid"
                            && name.as_str() != "in_ready"
                    })
                    .map(|(_, port)| port.clone())
                    .collect();
                slice_assignments.push(signal.assign_concat(&in_ports)?);
            } else if port_name.as_str() == "out_data" {
                // split the slice's output data back out into the individual output
                // ports, in the same order they were concatenated on the input side
                let mut high = data_width as i32 - 1;
                for (_, out_port) in ent_ports.iter().filter(|(name, _)| {
                    name.starts_with("out_")
                        && name.as_str() != "out_valid"
                        && name.as_str() != "out_ready"
                }) {
                    let selection = match out_port.typ() {
                        ObjectType::Bit => {
                            let sel = FieldSelection::index(high);
                            high -= 1;
                            sel
                        }
                        ObjectType::Array(array) => {
                            let low = high - array.width() as i32 + 1;
                            let sel = FieldSelection::downto(high, low)?;
                            high = low - 1;
                            sel
                        }
                        _ => continue,
                    };
                    let sliced =
                        ObjectAssignment::from(signal.clone()).assign_from(&vec![selection])?;
                    slice_assignments.push(out_port.assign(&sliced)?);
                }
            } else if *entity_port.mode() == ObjectMode::Out {
                slice_assignments.push(entity_port.assign(&signal)?);
            } else {
                slice_assignments.push(signal.assign(entity_port)?);
            }

            slice_portmap.map_port(port_name, &signal)?;
            architecture.add_declaration(signal)?;
        }

        for assign in slice_assignments {
            architecture.add_statement(assign)?;
        }

        architecture.add_statement(slice_portmap)?;
        architecture.add_using(Name::try_new("work")?, "Stream_pkg.all");

        Ok(architecture)
    }
}


#[cfg(test)]
mod test {
    use std::convert::TryFrom;

    use crate::generator::{
        common::convert::Packify,
        common::convert::CANON_SUFFIX,
        common::*,
        vhdl::Declare,
    };
    use crate::stdlib::common::architecture::*;
    use crate::design::{
        library::Library,
        StreamletKey,
        implementation::composer::GenericComponent
    };
    use crate::{
        Name, Natural, Result, cat, parser
    };

    use crate::stdlib::common::{
        architecture::{
            statement::PortMapping,
            declaration::ObjectDeclaration
        },
    };

    use super::*;

    #[test]
    fn slice_complex_generation() -> Result<()> {
        let (_, streamlet) = parser::nom::streamlet(
            "Streamlet streamlet (in_pass : in Stream<Union<a: Bits<32>, b: Bits<8>>, d=0, t=8, c=8>,
                in_pass2 : in Stream<Group<op1: Bits<64>, op2: Bits<54>>, d=0, t=0.1, c=8>,
                out_pass : out Stream<Union<a: Bits<32>, b: Bits<8>>, d=0, t=8, c=8>)",
        )
        .unwrap();

        let interface = streamlet.interfaces().next().unwrap().clone();
        let logical_type = interface.typ();

        let library = Library::try_new(
            Name::try_from("test_library")?,
            vec![],
            vec![streamlet]
        )?;

        let mut package = library.canonical();

        let architecture = components::logical_slice(logical_type, &mut package)?;

        println!("architecture: \n{}", architecture.declare()?);

        // let arch = generate_fancy_wrapper(&pak, &StreamletKey::try_from("passthrough_stub")?)?;
        Ok(())
    }

    #[test]
    fn slice_simple_generation() -> Result<()> {
        let (_, streamlet) = parser::nom::streamlet(
            "Streamlet streamlet (a : out Stream<Bits<8>>)",
        ).unwrap();

        let interface = streamlet.interfaces().next().unwrap().clone();
        let logical_type = interface.typ();

        let library = Library::try_new(
            Name::try_from("test_library")?,
            vec![],
            vec![streamlet]
        )?;

        let mut package = library.canonical();

        let architecture = components::logical_slice(logical_type, &mut package)?;

        println!("architecture: \n{}", architecture.declare()?);

        // Convert Signal to Type from MatthijsR

        let _example_output = "
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

    architecture behavioral of slice_a is
    begin
    end behavioral;
    ";

        Ok(())
    }


    #[test]
    fn streamlet_simple_gen() -> Result<()> {
        let (_, streamlet) = parser::nom::streamlet(
            "Streamlet streamlet (
                a : out Stream<Group<op1: Bits<8>, op2: Bits<8>>>,
                b : in Stream<Group<op1: Bits<4>, op2: Bits<4>>>
            )",
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
                Parameter{name: String::from("DATA_WIDTH"), typ: Type::Natural}
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

        slice_portmap.map_generic("DATA_WIDTH", &(32 as Natural))?;

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
