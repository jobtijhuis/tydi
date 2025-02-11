use crate::stdlib::common::architecture::ArchitectureDeclare;
use crate::{stdlib::common::architecture::declaration::ObjectKind, Document, Result};

use super::AssignDeclaration;

impl ArchitectureDeclare for AssignDeclaration {
    fn declare(&self, pre: &str, post: &str) -> Result<String> {
        let mut result = pre.to_string();
        if let Some(doc) = self.doc() {
            result.push_str("--");
            result.push_str(doc.replace("\n", &format!("\n{}--", pre)).as_str());
            result.push_str("\n");
            result.push_str(pre);
        }
        result.push_str(&self.object_string());
        result.push_str(match self.object.kind() {
            ObjectKind::Signal => " <= ",
            ObjectKind::Variable => " := ",
            ObjectKind::Constant => " := ",
            ObjectKind::EntityPort => " <= ",
            ObjectKind::ComponentPort => " => ",
        });
        result.push_str(
            &self
                .assignment()
                .declare_for(self.object_string(), pre, post)?,
        );
        result.push_str(post);
        Ok(result)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::convert::{TryFrom, TryInto};

    use indexmap::IndexMap;

    use crate::generator::common::test::records;
    use crate::generator::common::Mode;
    use crate::stdlib::common::architecture::assignment::{
        Assign, Assignment, AssignmentKind, ObjectAssignment, StdLogicValue,
    };
    use crate::stdlib::common::architecture::declaration::ObjectDeclaration;
    use crate::stdlib::common::architecture::object::ObjectType;
    use crate::stdlib::common::architecture::{
        assignment::bitvec::BitVecValue, declaration::tests::test_complex_signal,
    };
    use crate::Result;

    use super::*;

    pub(crate) fn bit_signal_object() -> Result<ObjectDeclaration> {
        Ok(ObjectDeclaration::signal(
            "test_signal".to_string(),
            ObjectType::Bit,
            None,
        ))
    }

    pub(crate) fn bit_variable_object() -> Result<ObjectDeclaration> {
        Ok(ObjectDeclaration::variable(
            "test_variable".to_string(),
            ObjectType::Bit,
            None,
        ))
    }

    pub(crate) fn bit_component_port_object() -> Result<ObjectDeclaration> {
        Ok(ObjectDeclaration::component_port(
            "test_component_port".to_string(),
            ObjectType::Bit,
            Mode::In,
        ))
    }

    pub(crate) fn record_signal(
        typename: impl Into<String>,
        identifier: impl Into<String>,
    ) -> Result<ObjectDeclaration> {
        let rec_type = records::rec(typename);
        Ok(ObjectDeclaration::signal(
            identifier,
            rec_type.try_into()?,
            None,
        ))
    }

    pub(crate) fn nested_record_signal(
        typename: impl Into<String>,
        identifier: impl Into<String>,
    ) -> Result<ObjectDeclaration> {
        let rec_type = records::rec_nested(typename);
        Ok(ObjectDeclaration::signal(
            identifier,
            rec_type.try_into()?,
            None,
        ))
    }

    pub(crate) fn bitvec_signal(
        identifier: impl Into<String>,
        high: i32,
        low: i32,
    ) -> Result<ObjectDeclaration> {
        Ok(ObjectDeclaration::signal(
            identifier,
            ObjectType::bit_vector(high, low)?,
            None,
        ))
    }

    pub(crate) fn complex_array_signal(
        identifier: impl Into<String>,
        high: i32,
        low: i32,
        typename: impl Into<String>,
        rectypename: impl Into<String>,
    ) -> Result<ObjectDeclaration> {
        Ok(ObjectDeclaration::signal(
            identifier,
            ObjectType::array(
                high,
                low,
                ObjectType::try_from(records::rec(rectypename))?,
                typename,
            )?,
            None,
        ))
    }

    #[test]
    fn test_bit_assign() -> Result<()> {
        let sig = bit_signal_object()?.assign(&StdLogicValue::Logic(false))?;
        let var = bit_variable_object()?.assign(&StdLogicValue::Logic(true))?;
        let port = bit_component_port_object()?
            .assign(&StdLogicValue::DontCare)?
            .with_doc("This is\nSome neat documentation");
        assert_eq!(sig.declare("", ";\n")?, "test_signal <= '0';\n");
        assert_eq!(var.declare("", ";\n")?, "test_variable := '1';\n");
        assert_eq!(
            port.declare("   ", ",\n")?,
            r#"   --This is
   --Some neat documentation
   test_component_port => '-',
"#
        );
        Ok(())
    }

    #[test]
    fn test_bitvec_assign() -> Result<()> {
        let a_others = BitVecValue::Others(StdLogicValue::Logic(true));
        let a_unsigned = BitVecValue::Unsigned(32);
        let a_unsigned_range = BitVecValue::Unsigned(32);
        let a_signed = BitVecValue::Signed(-32);
        let a_signed_range = BitVecValue::Signed(-32);
        let a_str = BitVecValue::from_str("1-XUL0H")?;
        assert_eq!(
            "test_signal <= (others => '1');\n",
            AssignDeclaration::new(test_complex_signal()?, a_others.into()).declare("", ";\n")?
        );
        assert_eq!(
            "test_signal <= std_logic_vector(to_unsigned(32, test_signal'length));\n",
            AssignDeclaration::new(test_complex_signal()?, a_unsigned.into()).declare("", ";\n")?
        );
        assert_eq!(
            "test_signal(10 downto 0) <= std_logic_vector(to_unsigned(32, 11));\n",
            AssignDeclaration::new(
                test_complex_signal()?,
                Assignment::from(a_unsigned_range).to_downto(10, 0)?
            )
            .declare("", ";\n")?
        );
        assert_eq!(
            "test_signal <= std_logic_vector(to_signed(-32, test_signal'length));\n",
            AssignDeclaration::new(test_complex_signal()?, a_signed.clone().into())
                .declare("", ";\n")?
        );
        assert_eq!(
            "test_signal.a(4 downto -3) <= std_logic_vector(to_signed(-32, 8));\n",
            test_complex_signal()?
                .assign(
                    &Assignment::from(a_signed.clone())
                        .to_named("a")
                        .to_downto(4, -3)?
                )?
                .declare("", ";\n")?
        );
        assert_eq!(
            "test_signal(0 to 10) <= std_logic_vector(to_signed(-32, 11));\n",
            AssignDeclaration::new(
                test_complex_signal()?,
                Assignment::from(a_signed_range).to_to(0, 10)?
            )
            .declare("", ";\n")?
        );
        assert_eq!(
            "test_signal <= \"1-XUL0H\";\n",
            AssignDeclaration::new(test_complex_signal()?, a_str.into()).declare("", ";\n")?
        );
        Ok(())
    }

    #[test]
    fn test_record_assign() -> Result<()> {
        let a_single = BitVecValue::Others(StdLogicValue::H);
        let mut multifields = IndexMap::new();
        multifields.insert(
            "c".to_string(),
            BitVecValue::Others(StdLogicValue::H).into(),
        );
        multifields.insert("d".to_string(), BitVecValue::Signed(-55).into());
        let a_full = AssignmentKind::full_record(multifields);
        assert_eq!(
            "recname.c <= (others => 'H');\n",
            record_signal("rectype", "recname")?
                .assign(&Assignment::from(a_single.clone()).to_named("c"))?
                .declare("", ";\n")?
        );
        assert_eq!(
            "recname2.c(40 downto 30) <= (others => 'H');\n",
            record_signal("rectype", "recname2")?
                .assign(
                    &Assignment::from(a_single.clone())
                        .to_named("c")
                        .to_downto(40, 30)?
                )?
                .declare("", ";\n")?
        );
        assert_eq!(
            r#"  recname3 <= (
    c => (others => 'H'),
    d => std_logic_vector(to_signed(-55, recname3.d'length))
  );
"#,
            record_signal("rectype", "recname3")?
                .assign(&a_full)?
                .declare("  ", ";\n")?
        );

        let a_rec = nested_record_signal("a_rec_type", "a_rec")?;
        let a_rec_assign = AssignmentKind::to_direct(&ObjectAssignment::from(a_rec), true)?;
        assert_eq!(
            r#"recname4 <= (
  a => (
    c => a_rec.a.c,
    d => a_rec.a.d
  ),
  b => (
    c => a_rec.b.c,
    d => a_rec.b.d
  )
);
"#,
            nested_record_signal("nestedrectype", "recname4")?
                .assign(&a_rec_assign)?
                .declare("", ";\n")?
        );

        Ok(())
    }

    #[test]
    fn test_array_assign() -> Result<()> {
        assert_eq!(
            "arr(4) <= 'U';\n",
            bitvec_signal("arr", 10, 0)?
                .assign(&Assignment::from(StdLogicValue::U).to_index(4))?
                .declare("", ";\n")?
        );
        assert_eq!(
            "arr <= \"10ZWUHLX-\";\n",
            bitvec_signal("arr", 8, 0)?
                .assign(&BitVecValue::from_str("10ZWUHLX-")?)?
                .declare("", ";\n")?
        );

        assert_eq!(
            "arr <= arr2;\n",
            complex_array_signal("arr", 7, 0, "arrtype", "rectype")?
                .assign(&complex_array_signal("arr2", 7, 0, "arrtype", "rectype")?)?
                .declare("", ";\n")?
        );
        assert_eq!(
            "arr <= ( arr2(0), arr2(1), arr2(2), arr2(3), arr2(4), arr2(5), arr2(6), arr2(7) );\n",
            complex_array_signal("arr", 7, 0, "arrtype", "rectype")?
                .assign(&AssignmentKind::to_direct(
                    &complex_array_signal("arr2", 7, 0, "difftype", "rectype")?,
                    false
                )?)?
                .declare("", ";\n")?
        );
        assert_eq!(
            r#"arr <= ( (
  c => arr2(0).c,
  d => arr2(0).d
), (
  c => arr2(1).c,
  d => arr2(1).d
), (
  c => arr2(2).c,
  d => arr2(2).d
), (
  c => arr2(3).c,
  d => arr2(3).d
), (
  c => arr2(4).c,
  d => arr2(4).d
), (
  c => arr2(5).c,
  d => arr2(5).d
), (
  c => arr2(6).c,
  d => arr2(6).d
), (
  c => arr2(7).c,
  d => arr2(7).d
) );
"#,
            complex_array_signal("arr", 7, 0, "arrtype", "rectype")?
                .assign(&AssignmentKind::to_direct(
                    &complex_array_signal("arr2", 7, 0, "difftype", "diffrectype")?,
                    true
                )?)?
                .declare("", ";\n")?
        );

        Ok(())
    }
}
