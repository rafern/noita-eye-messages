use std::error::Error;

use fasteval::{Compiler, Evaler, Instruction, Parser, Slab, eval_compiled_ref};

pub struct UserCondition {
    slab: Slab,
    compiled: Instruction,
}

impl UserCondition {
    pub fn new(expr_str: &String) -> Result<Self, Box<dyn Error>> {
        let parser = Parser::new();
        let mut slab = Slab::new();
        let compiled = parser.parse(expr_str, &mut slab.ps)?.from(&slab.ps).compile(&slab.ps, &mut slab.cs);
        Ok(UserCondition { slab, compiled })
    }

    pub fn eval(&self, var_cb: &mut impl FnMut(&str, Vec<f64>) -> Option<f64>) -> Result<f64, Box<dyn Error>> {
        #[allow(unexpected_cfgs)] // FIXME why is this warning showing up?
        let val = eval_compiled_ref!(&self.compiled, &self.slab, var_cb);
        Ok(val)
    }

    pub fn eval_condition(&self, var_cb: &mut impl FnMut(&str, Vec<f64>) -> Option<f64>) -> Result<bool, Box<dyn Error>> {
        #[allow(unexpected_cfgs)] // FIXME why is this warning showing up?
        let val = eval_compiled_ref!(&self.compiled, &self.slab, var_cb);
        Ok(val > 0.0)
    }
}