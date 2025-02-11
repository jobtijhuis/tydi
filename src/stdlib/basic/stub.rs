use std::borrow::Borrow;

use crate::design::implementation::composer::{
    impl_backend::ImplementationBackend, GenericComponent,
};
use crate::design::implementation::Implementation;
use crate::design::{Interface, Project, Streamlet, StreamletHandle, StreamletKey};

use crate::logical::LogicalType;

use crate::{Error, Name, Result, UniqueKeyBuilder};
use std::convert::TryFrom;

/// Stub construct, this can be used to prototype a dependency graph
/// or as a basis for custom components.
/// * If input and output match, passes inputs directly to outputs
/// * If no input exists, acts as a source and drives defaults values to output.
/// * If no output exists, acts as a sink.
#[derive(Clone, Debug)]
pub enum Stub {
    Source(Streamlet),
    Sink(Streamlet),
    Passthrough(Streamlet),
}

impl GenericComponent for Stub {
    fn streamlet(&self) -> &Streamlet {
        match self {
            Stub::Source(s) => s.borrow(),
            Stub::Sink(s) => s.borrow(),
            Stub::Passthrough(s) => s.borrow(),
        }
    }
}

impl Stub {
    pub fn try_new(project: &Project, name: Name, op: StreamletHandle) -> Result<Self> {
        let op = project.get_lib(op.lib())?.get_streamlet(op.streamlet())?;
        let mut is_source: bool = false;
        let mut is_sink: bool = false;

        let mut stream_in = op
            .inputs()
            .filter_map(|x| match x.typ() {
                LogicalType::Stream(_) => Some(x),
                _ => None,
            })
            .peekable();

        let mut stream_out = op
            .outputs()
            .filter_map(|x| match x.typ() {
                LogicalType::Stream(_) => Some(x),
                _ => None,
            })
            .peekable();

        let mut ifaces: Vec<Interface> = vec![];
        if stream_in.peek().is_some() {
            for i in stream_in {
                ifaces.push(i.clone());
            }
        } else {
            log::info!("Attempting to implement as source.");
            is_source = true;
        }

        if stream_out.peek().is_some() {
            for i in stream_out {
                ifaces.push(i.clone());
            }
        } else if !is_source {
            log::info!("Implementing as sink.");
            is_sink = true;
        } else {
            return Err(Error::ComposerError(format!(
                "No input or output Stream defined."
            )));
        }

        let streamlet = Streamlet::from_builder(
            StreamletKey::try_from(name).unwrap(),
            UniqueKeyBuilder::new().with_items(ifaces),
            None,
        )?;

        Ok(if is_source {
            Stub::Source(streamlet)
        } else if is_sink {
            Stub::Sink(streamlet)
        } else {
            Stub::Passthrough(streamlet)
        })
    }

    pub fn with_backend(&mut self, name: Name, streamlet_handle: StreamletHandle) -> Result<()> {
        match self {
            Stub::Source(s) => {
                s.attach_implementation(Implementation::Backend(Box::new(SourceStubBackend {
                    name,
                    streamlet_handle,
                })))?
            }
            Stub::Sink(s) => {
                s.attach_implementation(Implementation::Backend(Box::new(SinkStubBackend {
                    name,
                    streamlet_handle,
                })))?
            }
            Stub::Passthrough(s) => s.attach_implementation(Implementation::Backend(Box::new(
                PassthroughStubBackend {
                    name,
                    streamlet_handle,
                },
            )))?,
        }
        Ok(())
    }

    pub fn finish(self) -> Stub {
        self
    }
}

pub struct SourceStubBackend {
    name: Name,
    streamlet_handle: StreamletHandle,
}

pub struct SinkStubBackend {
    name: Name,
    streamlet_handle: StreamletHandle,
}

pub struct PassthroughStubBackend {
    name: Name,
    streamlet_handle: StreamletHandle,
}

impl ImplementationBackend for SourceStubBackend {
    fn name(&self) -> Name {
        self.name.clone()
    }

    fn streamlet_handle(&self) -> StreamletHandle {
        self.streamlet_handle.clone()
    }

    fn connect_action(&self) -> Result<()> {
        unimplemented!()
    }
}

impl ImplementationBackend for SinkStubBackend {
    fn name(&self) -> Name {
        self.name.clone()
    }

    fn streamlet_handle(&self) -> StreamletHandle {
        self.streamlet_handle.clone()
    }

    fn connect_action(&self) -> Result<()> {
        unimplemented!()
    }
}

impl ImplementationBackend for PassthroughStubBackend {
    fn name(&self) -> Name {
        self.name.clone()
    }

    fn streamlet_handle(&self) -> StreamletHandle {
        self.streamlet_handle.clone()
    }

    fn connect_action(&self) -> Result<()> {
        unimplemented!()
    }
}

#[cfg(test)]
pub mod tests {
    use std::convert::TryFrom;
    use std::fs;

    use crate::design::Library;

    use crate::generator::vhdl::VHDLBackEnd;
    use crate::generator::GenerateProject;
    use crate::Result;
    use crate::{parser, Name};

    use super::*;
    use crate::design::Project;

    pub(crate) fn parsed_stub_project() -> Result<Project> {
        let mut prj = Project::new(Name::try_from("test_project")?);
        let (_, source_stub) = parser::nom::streamlet(
            "Streamlet source_stub (
                out_source : out Stream<Union<a: Bits<32>, b: Bits<8>>, d=0, t=8, c=8>, 
                out_source2 : out Stream<Union<a: Stream<Bits<32>, d=0, t=8, c=8>, b: Stream<Bits<8>, d=0, t=3, c=8>>, d=0, t=8, c=8>
            )",
        )
        .unwrap();
        let (_, passthrough_stub) = parser::nom::streamlet(
            "Streamlet passthrough_stub (in_pass : in Stream<Union<a: Bits<32>, b: Bits<8>>, d=0, t=8, c=8>, 
                in_pass2 : in Stream<Group<op1: Bits<64>, op2: Bits<54>>, d=0, t=0.1, c=8>, 
                out_pass : out Stream<Union<a: Bits<32>, b: Bits<8>>, d=0, t=8, c=8>)",
        )
        .unwrap();
        let (_, sink_stub) = parser::nom::streamlet(
            "Streamlet sink_stub (in_sink : in Stream<Union<a: Bits<32>, b: Bits<8>>, d=0, t=8, c=8>)",
        )
        .unwrap();
        let (_, invalid_stub) = parser::nom::streamlet("Streamlet invalid_stub ()").unwrap();
        let lib = Library::try_new(
            Name::try_from("test_library")?,
            vec![],
            vec![source_stub, passthrough_stub, sink_stub, invalid_stub],
        )?;
        prj.add_lib(lib)?;
        Ok(prj)
    }

    #[test]
    fn source_stub_interfaces() -> Result<()> {
        let lib_key = Name::try_from("test_library")?;
        let prj = parsed_stub_project()?;
        let stub = Stub::try_new(
            &prj,
            Name::try_from("source")?,
            StreamletHandle {
                lib: lib_key.clone(),
                streamlet: Name::try_from("source_stub")?,
            },
        )?;

        println!("Stub interface\n {:?}\n", stub);

        Ok(())
    }

    #[test]
    fn passthrough_stub_interfaces() -> Result<()> {
        let lib_key = Name::try_from("test_library")?;
        let prj = parsed_stub_project()?;
        let stub = Stub::try_new(
            &prj,
            Name::try_from("passthrough")?,
            StreamletHandle {
                lib: lib_key.clone(),
                streamlet: Name::try_from("passthrough_stub")?,
            },
        )?;

        println!("Stub interface\n {:?}\n", stub);

        Ok(())
    }

    #[test]
    fn sink_stub_interfaces() -> Result<()> {
        let lib_key = Name::try_from("test_library")?;
        let prj = parsed_stub_project()?;
        let stub = Stub::try_new(
            &prj,
            Name::try_from("sink")?,
            StreamletHandle {
                lib: lib_key.clone(),
                streamlet: Name::try_from("sink_stub")?,
            },
        )?;

        println!("Stub interface\n {:?}\n", stub);

        Ok(())
    }

    #[test]
    fn invalid_stub_returns_composition_error() -> Result<()> {
        let lib_key = Name::try_from("test_library")?;
        let expected_err_string = "No input or output Stream defined.";
        let expected_error = Error::ComposerError(expected_err_string.to_string());
        let prj = parsed_stub_project()?;
        match Stub::try_new(
            &prj,
            Name::try_from("invalid")?,
            StreamletHandle {
                lib: lib_key.clone(),
                streamlet: Name::try_from("invalid_stub")?,
            },
        ) {
            Err(Error::ComposerError(err_str)) if err_str == expected_err_string => (),
            actual => panic!("Expected {:?}, got {:?}", expected_error, actual),
        };

        Ok(())
    }

    #[test]
    fn vhdl_generation() -> Result<()> {
        let lib_key = Name::try_from("test_library")?;
        let mut prj = parsed_stub_project()?;
        let source_stub = Stub::try_new(
            &prj,
            Name::try_from("source")?,
            StreamletHandle {
                lib: lib_key.clone(),
                streamlet: Name::try_from("source_stub")?,
            },
        )?;
        let sink_stub = Stub::try_new(
            &prj,
            Name::try_from("sink")?,
            StreamletHandle {
                lib: lib_key.clone(),
                streamlet: Name::try_from("sink_stub")?,
            },
        )?;
        let passthrough_stub = Stub::try_new(
            &prj,
            Name::try_from("passthrough")?,
            StreamletHandle {
                lib: lib_key.clone(),
                streamlet: Name::try_from("passthrough_stub")?,
            },
        )?;

        let lib = prj.get_lib_mut(lib_key.clone())?;
        lib.add_streamlet(source_stub.streamlet().clone())?;
        lib.add_streamlet(sink_stub.streamlet().clone())?;
        lib.add_streamlet(passthrough_stub.streamlet().clone())?;

        let tmpdir = tempfile::tempdir()?;
        let path = tmpdir.path().join("__test");
        let vhdl = VHDLBackEnd::default();
        assert!(vhdl.generate(&prj, &path).is_ok());

        // Assert files are generated
        assert!(fs::metadata(&path).is_ok());
        assert!(fs::metadata(&path.join("test_project")).is_ok());
        assert!(fs::metadata(&path.join("test_project/test_library_pkg.gen.vhd")).is_ok());
        // Interface/stream files
        assert!(fs::metadata(&path.join("test_project/invalid_stub_wrapper.gen.vhd")).is_ok());
        assert!(fs::metadata(&path.join("test_project/passthrough_stub_wrapper.gen.vhd")).is_ok());
        assert!(fs::metadata(&path.join("test_project/sink_stub_wrapper.gen.vhd")).is_ok());
        assert!(fs::metadata(&path.join("test_project/source_stub_wrapper.gen.vhd")).is_ok());
        // Implementation files
        assert!(fs::metadata(&path.join("test_project/passthrough_wrapper.gen.vhd")).is_ok());
        assert!(fs::metadata(&path.join("test_project/sink_wrapper.gen.vhd")).is_ok());
        assert!(fs::metadata(&path.join("test_project/source_wrapper.gen.vhd")).is_ok());

        Ok(())
    }
}
