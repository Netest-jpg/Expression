use crate::parser::Node;
pub const VARIABLE_LIMIT: usize = 64;

// TODO: Rename -> VariableBank
pub struct VariableBank {
    entries: [(u64, f64); VARIABLE_LIMIT], // fixed stack/inline array — no heap
    len: usize,
}

impl VariableBank {
    pub fn new() -> Self {
        VariableBank {
            entries: [(0u64, 0.0f64); VARIABLE_LIMIT],
            len: 0,
        }
    }

    // TODO: write docstring and doctest.
    // Also I am not sure if the rust compiler will inline this
    #[inline(always)]
    pub fn get(&self, hash: u64) -> Option<f64> {
        self.entries[..self.len]
            .iter()
            .find(|(h, _)| *h == hash)
            .map(|(_, v)| *v)
    }

    // TODO: Improve the docstring and write a doctest
    /// Insert or update. Returns Err if the store is full and the key is new.
    #[inline(always)]
    pub fn set(&mut self, hash: u64, value: f64) -> Result<(), String> {
        if let Some(entry) = self.entries[..self.len]
            .iter_mut()
            .find(|(h, _)| *h == hash)
        {
            entry.1 = value;
            return Ok(());
        }
        if self.len >= VARIABLE_LIMIT {
            return Err(format!(
                "variable limit ({VARIABLE_LIMIT}) reached; clear some variables first"
            ));
        }
        self.entries[self.len] = (hash, value);
        self.len += 1;
        Ok(())
    }

    /// TODO: write a docstring and doctest
    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    // TODO: improve the docstring and write a doctest.
    /// Clone all current bindings and add one extra slot for `hash`.
    /// The unknown's value is left as 0.0; call `set_last` to update it.
    /// Used by the Newton solver: one stack copy before the loop, then
    /// `set_last` updates the single f64 each iteration — zero heap allocs.
    pub fn clone_for_probe(&self, hash: u64) -> VariableBank {
        let mut probe = VariableBank {
            entries: self.entries,
            len: self.len,
        };
        // If the hash already exists (unlikely for a free var), update it;
        // otherwise append a new slot that set_last will overwrite.
        if let Some(e) = probe.entries[..probe.len]
            .iter_mut()
            .find(|(h, _)| *h == hash)
        {
            e.1 = 0.0;
        } else {
            // len < VARIABLE_LIMIT is guaranteed: the free var is unbound,
            // so it cannot already occupy one of the len filled slots.
            probe.entries[probe.len] = (hash, 0.0);
            probe.len += 1;
        }
        probe
    }

    // TODO: improve the docstring and write a doctest.
    /// Update the value of the last entry (the unknown slot created by
    /// `clone_for_probe`).  Panics if len is 0.
    #[inline(always)]
    pub fn set_last(&mut self, value: f64) {
        self.entries[self.len - 1].1 = value;
    }
}

impl Default for VariableBank {
    // TODO: write a docstring
    fn default() -> Self {
        Self::new()
    }
}

// TODO: rename -> VariableList
pub struct VarList {
    entries: [(u64, u32, u32); VARIABLE_LIMIT],
    len: usize,
    overflowed: bool,
}

impl VarList {
    // TODO: write a docstring
    pub fn new() -> Self {
        VarList {
            entries: [(0, 0, 0); VARIABLE_LIMIT],
            len: 0,
            overflowed: false,
        }
    }

    // TODO: write a docstring and doctest
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    // TODO: write a docstring and doctest
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    // TODO: Rename -> is_full , write a docstring and doctest
    #[inline(always)]
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    // TODO: write docstring and doctest
    #[inline(always)]
    pub fn iter(&self) -> std::slice::Iter<'_, (u64, u32, u32)> {
        self.entries[..self.len].iter()
    }

    // TODO: write a docstring and doctest
    #[inline(always)]
    pub fn retain(&mut self, mut f: impl FnMut(&(u64, u32, u32)) -> bool) {
        let mut out = 0;
        for i in 0..self.len {
            let entry = self.entries[i];
            if f(&entry) {
                self.entries[out] = entry;
                out += 1;
            }
        }
        self.len = out;
    }

    // TODO: write a docstring and doctest
    #[inline(always)]
    fn push_unique(&mut self, hash: u64, start: u32, end: u32) {
        if self.iter().any(|(h, _, _)| *h == hash) {
            return;
        }
        if self.len < VARIABLE_LIMIT {
            self.entries[self.len] = (hash, start, end);
            self.len += 1;
        } else {
            self.overflowed = true;
        }
    }
}

impl Default for VarList {
    fn default() -> Self {
        Self::new()
    }
}

// TODO: write a short 2-3 line comment explaining what we are doing here.
impl std::ops::Index<usize> for VarList {
    type Output = (u64, u32, u32);

    // TODO: write a docstring and doctest
    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[..self.len][index]
    }
}

// TODO: write a docstring and doctest
pub fn collect_variables(arena: &[Node], root: u32) -> VarList {
    let mut out = VarList::new();
    collect_vars_inner(arena, root, &mut out);
    out
}

// TODO: write a docstring and doctest, check the assembly and see if it gets converted
// into a jump table.
fn collect_vars_inner(arena: &[Node], idx: u32, out: &mut VarList) {
    match &arena[idx as usize] {
        Node::Variable(start, end, hash) => {
            out.push_unique(*hash, *start, *end);
        }
        Node::Number(_) | Node::Constant(_) => {}
        Node::Neg(a)
        | Node::Sin(a)
        | Node::Cos(a)
        | Node::Tan(a)
        | Node::Sec(a)
        | Node::Csc(a)
        | Node::Cot(a)
        | Node::Asin(a)
        | Node::Acos(a)
        | Node::Atan(a)
        | Node::Acsc(a)
        | Node::Asec(a)
        | Node::Acot(a)
        | Node::Sinh(a)
        | Node::Cosh(a)
        | Node::Tanh(a)
        | Node::Sech(a)
        | Node::Csch(a)
        | Node::Coth(a)
        | Node::Asinh(a)
        | Node::Acosh(a)
        | Node::Atanh(a)
        | Node::Asech(a)
        | Node::Acsch(a)
        | Node::Acoth(a)
        | Node::Ln(a)
        | Node::Log(a)
        | Node::Sqrt(a) => {
            collect_vars_inner(arena, *a, out);
        }
        Node::Add(a, b)
        | Node::Sub(a, b)
        | Node::Mul(a, b)
        | Node::Div(a, b)
        | Node::Pow(a, b)
        | Node::Equation(a, b) => {
            collect_vars_inner(arena, *a, out);
            collect_vars_inner(arena, *b, out);
        }
    }
}
