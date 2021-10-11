

mod components {

}

pub mod records {
    use crate::generator::{common::*};
    use crate::cat;

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
    
    use crate::generator::{common::*, vhdl::Declare};
    use crate::generator::components::records;
    use crate::logical::LogicalType;

    

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

    fn logical_slice(l_type : LogicalType) -> Component {
        let comp = Component::new (
            "test_comp",
            vec![],
            vec![
                Port::new_documented("a", Mode::In, records::rec_rev("a"), None),
                Port::new_documented("b", Mode::Out, records::rec_rev_nested("b"), None),
            ],
            None
        );

        return comp;        
    }

    #[test]
    fn test_logical_slice() {
        let my_type = LogicalType::try_new_bits(8).unwrap();
        let _result = logical_slice(my_type);

    }

}
