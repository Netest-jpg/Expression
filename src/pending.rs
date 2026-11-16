use expression::parser::Node;
use expression::variables::{VariableBank, collect_variables};
use std::io::Write;
pub struct Pending {
    pub src: String,
    pub arena: Vec<Node>,
    pub root: u32,
}

impl Pending {
    /// Return the names of variables in this equation that are still unbound.
    pub fn free_var_names<'a>(&'a self, vars: &VariableBank) -> Vec<&'a str> {
        collect_variables(&self.arena, self.root)
            .iter()
            .filter(|(h, _, _)| vars.get(*h).is_none())
            .map(|(_, s, e)| &self.src[*s as usize..*e as usize])
            .collect()
    }

    /// Print the current state of this pending equation relative to vars.
    #[rustfmt::skip]
    pub fn print_status<W: Write>(&self, out: &mut W, vars: &VariableBank) {
        let free = self.free_var_names(vars);
        if free.is_empty() {
            writeln!(out, "  pending \"{}\" — all variables bound, type 'evaluate'", self.src).ok();
        } else {
            writeln!(out, "  pending \"{}\" — still need: {}", self.src, free.join(", ")).ok();
        }
    }
}
