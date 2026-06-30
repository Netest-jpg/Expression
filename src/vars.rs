use crate::parser::{Node, NodeKind};

// -----------------------------------------------------------------------
// Variable store — linear scan over (hash, value) pairs.
//
// Up to 64 bindings; the FNV-1a hash (precomputed by the lexer) means
// lookup never touches the source string.
// -----------------------------------------------------------------------
pub const VAR_STORE_LIMIT: usize = 64;

pub struct VarStore {
    entries: [(u64, f64); VAR_STORE_LIMIT], // fixed stack/inline array — no heap
    len: usize,
}

impl VarStore {
    pub fn new() -> Self {
        VarStore {
            entries: [(0u64, 0.0f64); VAR_STORE_LIMIT],
            len: 0,
        }
    }

    #[inline(always)]
    pub fn get(&self, hash: u64) -> Option<f64> {
        self.entries[..self.len]
            .iter()
            .find(|(h, _)| *h == hash)
            .map(|(_, v)| *v)
    }

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
        if self.len >= VAR_STORE_LIMIT {
            return Err(format!(
                "variable limit ({VAR_STORE_LIMIT}) reached; clear some variables first"
            ));
        }
        self.entries[self.len] = (hash, value);
        self.len += 1;
        Ok(())
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Clone all current bindings and add one extra slot for `hash`.
    /// The unknown's value is left as 0.0; call `set_last` to update it.
    /// Used by the Newton solver: one stack copy before the loop, then
    /// `set_last` updates the single f64 each iteration — zero heap allocs.
    pub fn clone_for_probe(&self, hash: u64) -> VarStore {
        let mut probe = VarStore {
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
            // len < VAR_STORE_LIMIT is guaranteed: the free var is unbound,
            // so it cannot already occupy one of the len filled slots.
            probe.entries[probe.len] = (hash, 0.0);
            probe.len += 1;
        }
        probe
    }

    /// Update the value of the last entry (the unknown slot created by
    /// `clone_for_probe`).  Panics if len is 0.
    #[inline(always)]
    pub fn set_last(&mut self, value: f64) {
        self.entries[self.len - 1].1 = value;
    }
}

impl Default for VarStore {
    fn default() -> Self {
        Self::new()
    }
}

// -----------------------------------------------------------------------
// Variable collection — walk the tree and gather unique identifiers.
// Returns a fixed-size stack/inline list deduplicated by hash.
// -----------------------------------------------------------------------
pub struct VarList {
    entries: [(u64, u32, u32); VAR_STORE_LIMIT],
    len: usize,
    overflowed: bool,
}

impl VarList {
    pub fn new() -> Self {
        VarList {
            entries: [(0, 0, 0); VAR_STORE_LIMIT],
            len: 0,
            overflowed: false,
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    #[inline(always)]
    pub fn iter(&self) -> std::slice::Iter<'_, (u64, u32, u32)> {
        self.entries[..self.len].iter()
    }

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

    #[inline(always)]
    fn push_unique(&mut self, hash: u64, start: u32, end: u32) {
        if self.iter().any(|(h, _, _)| *h == hash) {
            return;
        }
        if self.len < VAR_STORE_LIMIT {
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

impl std::ops::Index<usize> for VarList {
    type Output = (u64, u32, u32);

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[..self.len][index]
    }
}

pub fn collect_vars(arena: &[Node], root: u32) -> VarList {
    let mut out = VarList::new();
    collect_vars_inner(arena, root, &mut out);
    out
}

fn collect_vars_inner(arena: &[Node], idx: u32, out: &mut VarList) {
    match &arena[idx as usize].kind {
        NodeKind::Variable(start, end, hash) => {
            out.push_unique(*hash, *start, *end);
        }
        NodeKind::Number(_) | NodeKind::Constant(_) => {}
        NodeKind::Neg(a)
        | NodeKind::Sin(a)
        | NodeKind::Cos(a)
        | NodeKind::Tan(a)
        | NodeKind::Sec(a)
        | NodeKind::Csc(a)
        | NodeKind::Cot(a)
        | NodeKind::Asin(a)
        | NodeKind::Acos(a)
        | NodeKind::Atan(a)
        | NodeKind::Acsc(a)
        | NodeKind::Asec(a)
        | NodeKind::Acot(a)
        | NodeKind::Sinh(a)
        | NodeKind::Cosh(a)
        | NodeKind::Tanh(a)
        | NodeKind::Sech(a)
        | NodeKind::Csch(a)
        | NodeKind::Coth(a)
        | NodeKind::Asinh(a)
        | NodeKind::Acosh(a)
        | NodeKind::Atanh(a)
        | NodeKind::Asech(a)
        | NodeKind::Acsch(a)
        | NodeKind::Acoth(a)
        | NodeKind::Ln(a)
        | NodeKind::Log(a)
        | NodeKind::Sqrt(a) => {
            collect_vars_inner(arena, *a, out);
        }
        NodeKind::Add(a, b)
        | NodeKind::Sub(a, b)
        | NodeKind::Mul(a, b)
        | NodeKind::Div(a, b)
        | NodeKind::Pow(a, b)
        | NodeKind::Equation(a, b) => {
            collect_vars_inner(arena, *a, out);
            collect_vars_inner(arena, *b, out);
        }
    }
}
