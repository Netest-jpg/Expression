use expression::parser::Node;
use expression::variables::{VariableValues, collect_variables};
use std::io::Write;
pub struct Pending {
    pub src: String,
    pub arena: Vec<Node>,
    pub root: u32,
}

impl Pending {
    /// Returns variables that are unbound.
    pub fn unbound_variables<'a>(&'a self, vars: &VariableValues) -> Vec<&'a str> {
        collect_variables(&self.arena, self.root)
            .iter()
            .filter(|(h, _, _)| vars.get(*h).is_none())
            .map(|(_, s, e)| &self.src[*s as usize..*e as usize])
            .collect()
    }

    /// Prints pending equation and variables.
    pub fn print_pending<W: Write>(&self, out: &mut W, vars: &VariableValues) {
        let free = self.unbound_variables(vars);
        if free.is_empty() {
            writeln!(
                out,
                "Pending equation: '{}'. All variables bound, type 'evaluate'",
                self.src
            )
            .ok();
        } else {
            writeln!(
                out,
                "Pending equation: '{}'. Still need: {}",
                self.src,
                free.join(", ")
            )
            .ok();
        }
    }
}

pub fn store_pending(
    old: Option<Pending>,
    pending_src: &mut String,
    arena: &mut Vec<Node>,
    expression: &str,
    root: u32,
) -> Pending {
    let mut pending_arena = match old {
        Some(old) => {
            *pending_src = old.src;
            old.arena
        }
        None => Vec::with_capacity(32),
    };

    std::mem::swap(&mut pending_arena, arena);

    pending_src.clear();
    pending_src.push_str(expression);

    Pending {
        src: std::mem::take(pending_src),
        arena: pending_arena,
        root,
    }
}
