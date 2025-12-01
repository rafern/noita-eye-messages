use std::error::Error;

use cel::{Context, Program};

pub struct UserCondition {
    program: Program,
}

impl UserCondition {
    pub fn new(expr_str: &String) -> Result<Self, Box<dyn Error>> {
        Ok(UserCondition { program: Program::compile(expr_str)? })
    }

    pub fn eval_condition(&self, ctx: &Context) -> Result<bool, Box<dyn Error>> {
        match self.program.execute(ctx)? {
            cel::Value::Int(x) => Ok(x > 0),
            cel::Value::UInt(x) => Ok(x > 0),
            cel::Value::Float(x) => Ok(x > 0.0),
            cel::Value::Bool(x) => Ok(x),
            _ => Err("Must evaluate to a boolean or number".into()),
        }
    }
}