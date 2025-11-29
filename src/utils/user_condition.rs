use std::error::Error;

use evalexpr::{DefaultNumericTypes, Node, build_operator_tree};

pub struct UserCondition {
    pub node: Node,
}

impl UserCondition {
    pub fn new(expr_str: &String) -> Result<Self, Box<dyn Error>> {
        Ok(UserCondition {
            node: build_operator_tree::<DefaultNumericTypes>(expr_str)?,
        })
    }
}