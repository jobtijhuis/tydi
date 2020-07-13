/// Composition examples
extern crate tydi;

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;
    use std::fs;
    use tydi::design::*;
    use tydi::generator::chisel::ChiselBackEnd;
    use tydi::generator::dot::DotBackend;
    use tydi::generator::GenerateProject;
    
    use tydi::parser::nom::interface;
    use tydi::{Name, Result, UniqueKeyBuilder};
    use tydi::design::implementation::composer::parser::ImplParser;
    use tydi::generator::vhdl::VHDLBackEnd;

    pub fn spark_example_prj() -> Result<Project> {
        let key1 = LibKey::try_new("primitives").unwrap();
        let key2 = LibKey::try_new("compositions").unwrap();
        let mut lib = Library::new(key1.clone());

        let mut lib_comp = Library::new(key2.clone());

        let _top = lib_comp
            .add_streamlet(
                Streamlet::from_builder(
                    StreamletKey::try_from("Top_level").unwrap(),
                    UniqueKeyBuilder::new().with_items(vec![
                        interface("numbers: in Stream<Bits<32>, d=1>").unwrap().1,
                        interface("strings: in Stream<Bits<8>, d=1>").unwrap().1,
                        interface("out: out Stream<Bits<32>, d=0>").unwrap().1,
                    ]),
                    None,
                )
                    .unwrap(),
            )
            .unwrap();

        let _matcher = lib
            .add_streamlet(
                Streamlet::from_builder(
                    StreamletKey::try_from("RegexMatcher").unwrap(),
                    UniqueKeyBuilder::new().with_items(vec![
                        interface("in: in Stream<Bits<8>, d=1>").unwrap().1,
                        interface("out: out Stream<Bits<1>, d=0>").unwrap().1,
                    ]),
                    None,
                )
                    .unwrap(),
            )
            .unwrap();

        let _test_op = lib
            .add_streamlet(
                Streamlet::from_builder(
                    StreamletKey::try_from("Sum").unwrap(),
                    UniqueKeyBuilder::new().with_items(vec![
                        interface("in: in Stream<Group<op1: Bits<64>, op2: Bits<64>>, d=0>").unwrap().1,
                        interface("out: out Stream<Bits<64>, d=0>").unwrap().1,
                    ]),
                    None,
                )
                    .unwrap(),
            )
            .unwrap();

        let mut prj = Project::new(Name::try_new("TestProj").unwrap());
        prj.add_lib(lib)?;
        prj.add_lib(lib_comp)?;

        let top_impl = include_str!("implementations/spark_example.impl");

        let mut builder = ImplParser::try_new(&mut prj, &top_impl)?;
        builder.transform_body().unwrap();
        let imp = builder.finish();
        prj.add_streamlet_impl(
            StreamletHandle {
                lib: Name::try_from("compositions")?,
                streamlet: Name::try_from("Top_level")?,
            },
            imp,
        )?;
        Ok(prj)
    }

    #[test]
    fn spark_example_dot() {
        let tmpdir = tempfile::tempdir().unwrap();

        let prj = spark_example_prj().unwrap();
        //let prj = pow2_example().unwrap();
        let dot = DotBackend {};
        // TODO: implement actual test.

        assert!(dot.generate(&prj, tmpdir).is_ok());
    }

    #[test]
    fn spark_example_vhdl() {
        let _tmpdir = tempfile::tempdir().unwrap();

        //let prj = impl_parser_test().unwrap();
        let prj = spark_example_prj().unwrap();
        let vhdl = VHDLBackEnd::default();

        let _folder = fs::create_dir_all("output").unwrap();

        assert!(vhdl.generate(&prj, "output").is_ok());
    }

    #[test]
    fn spark_example_chisel() {
        let _tmpdir = tempfile::tempdir().unwrap();

        //let prj = impl_parser_test().unwrap();
        let prj = spark_example_prj().unwrap();
        let chisel = ChiselBackEnd::default();

        let _folder = fs::create_dir_all("output").unwrap();

        assert!(chisel.generate(&prj, "output").is_ok());
    }
}
