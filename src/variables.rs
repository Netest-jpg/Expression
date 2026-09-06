use crate::parser::Node;
pub const VARIABLE_LIMIT: usize = 64;

#[cold]
#[inline(never)]
fn variable_limit_reached_err() -> String {
    format!("Variable limit ({VARIABLE_LIMIT}) reached; clear some variables first")
}

pub struct VariableBank {
    entries: [(u64, f64); VARIABLE_LIMIT], // stores (hash, value)
    // fixed stack/inline array — no heap
    len: usize,
}

impl VariableBank {
    pub fn new() -> Self {
        VariableBank {
            entries: [(0u64, 0.0f64); VARIABLE_LIMIT],
            len: 0,
        }
    }

    /// Inserts a new binding or updates an existing one.
    ///
    /// If `hash` is already present, its value is overwritten in
    /// place. Otherwise a new entry is appended. Returns `Err` if the
    /// `VariableBank` is already at [`VARIABLE_LIMIT`].
    /// # Examples
    ///
    /// ```ignore
    /// let mut bank = VariableBank::new();
    /// assert!(bank.set(1, 10.0).is_ok());
    /// assert!(bank.set(1, 20.0).is_ok()); // update, not a new slot
    /// assert_eq!(bank.get(1), Some(20.0));
    /// ```
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
            return Err(variable_limit_reached_err());
        }
        self.entries[self.len] = (hash, value);
        self.len += 1;
        Ok(())
    }

    /// Looks up the value bound to `hash`, if any.
    ///
    /// Performs a linear scan over the filled portion of the inline
    /// array.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut bank = VariableBank::new();
    /// bank.set(42, 3.14).unwrap();
    /// assert_eq!(bank.get(42), Some(3.14));
    /// assert_eq!(bank.get(7), None);
    /// ```
    #[inline(always)]
    pub fn get(&self, hash: u64) -> Option<f64> {
        self.entries[..self.len]
            .iter()
            .find(|(h, _)| *h == hash)
            .map(|(_, v)| *v)
    }

    /// Removes all the bindings inside `VariableBank` **lazily**.
    ///
    /// This just resets the length counter; the underlying array
    /// slots are left as-is and get overwritten lazily by future
    /// `set` calls, so `clear` is O(1).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut bank = VariableBank::new();
    /// bank.set(1, 5.0).unwrap();
    /// bank.clear();
    /// assert_eq!(bank.get(1), None);
    /// ```
    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Clones all current bindings and appends one extra slot for the
    /// free variable `hash`, used to probe a single unknown.
    ///
    /// The new slot's value starts at `0.0`; call [`set_last`] each
    /// iteration to update it. This is the allocation strategy the
    /// Newton solver relies on: one stack copy of the bank before the
    /// loop starts, then a single `f64` write per iteration inside
    /// the loop — zero heap allocations for the whole solve.
    ///
    /// If `hash` already exists in the bank, its existing slot is
    /// reset to `0.0` instead of appending a duplicate.
    ///
    /// [`set_last`]: VariableBank::set_last
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let bank = VariableBank::new();
    /// let mut probe = bank.clone_for_probe(99);
    /// probe.set_last(1.5);
    /// assert_eq!(probe.get(99), Some(1.5));
    /// ```
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

    /// Updates the value of the last entry — the unknown slot created
    /// by [`clone_for_probe`] — in place.
    ///
    /// Intended to be called once per Newton iteration on a bank
    /// produced by `clone_for_probe`, so the solver can update the
    /// single free variable without touching the rest of the bindings
    /// or allocating.
    ///
    /// # Panics
    ///
    /// Panics if the bank is empty (`len == 0`).
    ///
    /// [`clone_for_probe`]: VariableBank::clone_for_probe
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let bank = VariableBank::new();
    /// let mut probe = bank.clone_for_probe(1);
    /// probe.set_last(2.0);
    /// assert_eq!(probe.get(1), Some(2.0));
    /// ```
    #[inline(always)]
    pub fn set_last(&mut self, value: f64) {
        self.entries[self.len - 1].1 = value;
    }
}

impl Default for VariableBank {
    fn default() -> Self {
        Self::new()
    }
}

pub struct VariableList {
    entries: [(u64, u32, u32); VARIABLE_LIMIT],
    len: usize,
    overflowed: bool,
}

impl VariableList {
    pub fn new() -> Self {
        VariableList {
            entries: [(0, 0, 0); VARIABLE_LIMIT],
            len: 0,
            overflowed: false,
        }
    }

    /// Returns the number of distinct variables currently stored.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let list = VariableList::new();
    /// assert_eq!(list.len(), 0);
    /// ```
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if no variables have been recorded.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let list = VariableList::new();
    /// assert!(list.is_empty());
    /// ```
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `true` if the list is full — i.e. a distinct variable
    /// was encountered after [`VARIABLE_LIMIT`] slots were already
    /// taken, and was dropped rather than stored.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let list = VariableList::new();
    /// assert!(!list.is_full());
    /// ```
    #[inline(always)]
    pub fn is_full(&self) -> bool {
        self.overflowed
    }

    /// Returns an iterator over the `(hash, start, end)` entries
    /// currently stored, in insertion order.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let list = VariableList::new();
    /// assert_eq!(list.iter().count(), 0);
    /// ```
    #[inline(always)]
    pub fn iter(&self) -> std::slice::Iter<'_, (u64, u32, u32)> {
        self.entries[..self.len].iter()
    }

    /// Keeps only the entries for which `f` returns `true`, removing
    /// the rest in place and compacting the array (like
    /// `Vec::retain`, but without any allocation or shifting beyond a
    /// single in-place pass).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut list = VariableList::new();
    /// // ... populated via collect_variables ...
    /// list.retain(|(hash, _, _)| *hash != 0);
    /// ```
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

    /// Appends `(hash, start, end)` if `hash` isn't already present.
    ///
    /// If the list is already full, silently sets `overflowed` and
    /// drops the entry instead of appending — this is a fixed-size
    /// list with no heap to grow into.
    ///
    /// This is a private helper, so it can't carry a doctest that
    /// `cargo test` will run (doctests only execute against a crate's
    /// public API); its behavior is exercised indirectly through
    /// [`collect_variables`].
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

impl Default for VariableList {
    fn default() -> Self {
        Self::new()
    }
}

// `VariableList` is a fixed-size, append-only collection rather than a
// `Vec`, so it can't offer `Deref<Target = [_]>` for free; this `Index`
// impl gives it the familiar `list[i]` syntax anyway, scoped to just
// the filled `..len` prefix of the backing array.
impl std::ops::Index<usize> for VariableList {
    type Output = (u64, u32, u32);

    /// Returns the entry at `index` within the filled portion of the
    /// list.
    ///
    /// # Panics
    ///
    /// Panics if `index >= self.len()`, same as indexing a slice out
    /// of bounds.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let list = VariableList::new();
    /// // list[0] would panic here since the list is empty;
    /// // see collect_variables for a populated example.
    /// ```
    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[..self.len][index]
    }
}

/// Walks the expression tree rooted at `root` within `arena` and
/// collects every distinct variable referenced, in first-occurrence
/// order.
///
/// # Examples
///
/// ```ignore
/// // Given an arena encoding the expression `x + 1`:
/// let vars = collect_variables(&arena, root);
/// assert_eq!(vars.len(), 1);
/// ```
pub fn collect_variables(arena: &[Node], root: u32) -> VariableList {
    let mut out = VariableList::new();
    collect_vars_inner(arena, root, &mut out);
    out
}

/// Recursive worker behind [`collect_variables`]: walks a single node
/// and its children, recording any `Node::Variable` it finds into
/// `out`.
fn collect_vars_inner(arena: &[Node], idx: u32, out: &mut VariableList) {
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
        | Node::LogBase(a, b)
        | Node::Equation(a, b) => {
            collect_vars_inner(arena, *a, out);
            collect_vars_inner(arena, *b, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Tokenizer;
    use crate::parser::Parser;

    #[test]
    fn test_collect_variables_from_log_base() {
        let src = "log_b(k)";
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();

        let vars = collect_variables(&arena, root);
        assert_eq!(vars.len(), 2);
        assert_eq!(&src[vars[0].1 as usize..vars[0].2 as usize], "b");
        assert_eq!(&src[vars[1].1 as usize..vars[1].2 as usize], "k");
    }
}
