#[cfg(test)]
mod tests {
    use super::*;
    use crate::logical::LogicalType;
    use crate::design::Library;

    /// net x : LogicalType::Bits<8>
    /// net y : LogicalType::Bits<8> = x.slice()
    /// x - - - > | - - - > y

    fn logical_slice(typ : LogicalType) -> Result<MatthijsR> {
        // eventually produces "slice_x_y_whatever.vhd"

        // for each physical stream, a vhlib slice is inserted
        // the top-level ports are routed through these slices correctly
        // e.g. some logical stream has 2 physical streams a b
        // at the top level of this logical slice, i will find the ports for a and b
        // the implementation of this logical slice:
        // [x_a]--- slice a ---[y_a]
        // [x_b]--- slice b ---[y_b]
    }

    fn buffer(typ : LogicalType) -> Result<MatthijsR> {

    }

    #[test]
    fn foo() {
        // Logical Stream: Bits<8>
        let my_type = LogicalType::try_new_bits(8).unwrap();

    }
}