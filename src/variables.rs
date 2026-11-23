use crate::parser::Node;
pub const VARIABLE_LIMIT: usize = 64;

pub struct VariableValues {
    entries: [(u64, f64); VARIABLE_LIMIT],
    len: usize,
}

impl Default for VariableValues {
    fn default() -> Self {
        Self::new()
    }
}

impl VariableValues {
    pub fn new() -> Self {
        VariableValues {
            entries: [(0u64, 0.0f64); VARIABLE_LIMIT],
            len: 0,
        }
    }

    /// Inserts a new binding or updates an existing one.
    ///
    /// If `hash` is already present, its value is overwritten in place. Otherwise a new entry is appended.
    ///
    /// Returns `Err` if `VariableValues` has already reached `VARIABLE_LIMIT`.
    ///
    /// # Examples
    /// ```ignore
    /// let mut values = VariableValues::new();
    ///
    /// assert!(values.set(1, 10.0).is_ok());
    /// assert!(values.set(1, 20.0).is_ok()); // updates, doesn't create a new slot
    /// assert_eq!(values.get(1), Some(20.0));
    /// ```
    #[inline]
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

    /// Returns the value bound to `hash`; otherwise, returns `None`.
    ///
    /// Performs a linear scan over the filled portion of the inline array.
    ///
    /// # Examples
    /// ```ignore
    /// let mut values = VariableValues::new();
    ///
    /// values.set(42, 3.14).unwrap();
    /// assert_eq!(values.get(42), Some(3.14));
    /// assert_eq!(values.get(7), None);
    /// ```
    #[inline]
    pub fn get(&self, hash: u64) -> Option<f64> {
        self.entries[..self.len]
            .iter()
            .find(|(h, _)| *h == hash)
            .map(|(_, v)| *v)
    }

    /// Removes all the bindings inside `VariableValues` lazily.
    ///
    /// This just resets the length counter to 0; the underlying array slots are left as-is and get overwritten lazily by future `set()` calls.
    ///
    /// # Examples
    /// ```ignore
    /// let mut values = VariableValues::new();
    /// values.set(1, 5.0).unwrap();
    /// values.clear();
    /// assert_eq!(values.get(1), None);
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Returns a copy of `self` with a new entry `(hash, 0.0)` appended.
    ///
    /// Use [`set_last`] to update the new entry's value in place.
    ///
    /// `hash` must not already be bound in `self`.
    ///
    /// # Panics
    /// Panics if `self` is already at `VARIABLE_LIMIT` capacity.
    ///
    /// # Examples
    /// ```ignore
    /// let bank = VariableValues::new();
    /// let mut probe = bank.fork_probe(99);
    /// probe.set_last(1.5);
    /// assert_eq!(probe.get(99), Some(1.5));
    /// ```
    pub fn fork_probe(&self, hash: u64) -> VariableValues {
        let mut probe = VariableValues {
            entries: self.entries,
            len: self.len,
        };
        probe.entries[probe.len] = (hash, 0.0);
        probe.len += 1;
        probe
    }

    /// Sets the value of the last entry in place.
    ///
    /// Intended for use on a probe returned by [`fork_probe`], whose last entry is the newly appended one.
    ///
    /// # Panics
    /// Panics if `self` is empty.
    ///
    /// # Examples
    /// ```ignore
    /// let bank = VariableValues::new();
    /// let mut probe = bank.fork_probe(1);
    /// probe.set_last(2.0);
    /// assert_eq!(probe.get(1), Some(2.0));
    /// ```
    #[inline]
    pub fn set_last(&mut self, value: f64) {
        self.entries[self.len - 1].1 = value;
    }
}

pub struct VariablePos {
    entries: [(u64, u32, u32); VARIABLE_LIMIT],
    len: usize,
    overflow: bool,
}

impl Default for VariablePos {
    fn default() -> Self {
        Self::new()
    }
}

impl VariablePos {
    pub fn new() -> Self {
        VariablePos {
            entries: [(0, 0, 0); VARIABLE_LIMIT],
            len: 0,
            overflow: false,
        }
    }

    /// Returns the number of distinct variables currently stored.
    ///
    /// # Examples
    /// ```ignore
    /// let list = VariablePos::new();
    /// assert_eq!(list.len(), 0);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if no variables have been recorded.
    ///
    /// # Examples
    /// ```ignore
    /// let list = VariablePos::new();
    /// assert!(list.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `true` if the list is full — i.e. if a variable was dropped because `VARIABLE_LIMIT` was already reached when it was encountered.
    ///
    /// # Examples
    /// ```ignore
    /// let list = VariablePos::new();
    /// assert!(!list.overflow());
    /// ```
    #[inline]
    pub fn overflow(&self) -> bool {
        self.overflow
    }

    /// Returns an iterator over the `(hash, start, end)` entries currently stored, in insertion order.
    ///
    /// # Examples
    /// ```ignore
    /// let list = VariablePos::new();
    /// assert_eq!(list.iter().count(), 0);
    /// ```
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, (u64, u32, u32)> {
        self.entries[..self.len].iter()
    }

    /// Keeps only the entries for which `f` (Condition) returns `true`, lazily.
    ///
    /// Entries are compacted in place: retained entries are moved to the front `(index 0)` in their original order, and `len` is shrunk to match.
    #[inline]
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
    /// If the `VariablePos` has reached `VARIABLE_LIMIT`, sets `overflow` and drops the entry instead of appending.
    #[inline]
    fn push_unique(&mut self, hash: u64, start: u32, end: u32) {
        if self.iter().any(|(h, _, _)| *h == hash) {
            return;
        }
        if self.len < VARIABLE_LIMIT {
            self.entries[self.len] = (hash, start, end);
            self.len += 1;
        } else {
            self.overflow = true;
        }
    }
}

/// Walks the expression tree rooted at `root` within `arena` and collects every distinct variable referenced, in first-occurrence order.
///
/// # Examples
/// ```ignore
/// // Given an arena encoding the expression `x + 1`:
/// let vars = collect_variables(&arena, root);
/// assert_eq!(vars.len(), 1);
/// ```
pub fn collect_variables(arena: &[Node], root: u32) -> VariablePos {
    let mut out = VariablePos::new();
    visit(arena, root, &mut out);
    out
}

/// Recursive worker behind `collect_variables`: walks a single node and its children, recording any `Node::Variable` it finds into `out`.
fn visit(arena: &[Node], idx: u32, out: &mut VariablePos) {
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
        | Node::Sqrt(a)
        | Node::Cbrt(a)
        | Node::Factorial(a)
        | Node::Deg(a)
        | Node::Rad(a) => {
            visit(arena, *a, out);
        }
        Node::Add(a, b)
        | Node::Sub(a, b)
        | Node::Mul(a, b)
        | Node::Div(a, b)
        | Node::Pow(a, b)
        | Node::LogBase(a, b)
        | Node::Root(a, b)
        | Node::Equation(a, b) => {
            visit(arena, *a, out);
            visit(arena, *b, out);
        }
    }
}

// Error Handles--------------------------------------------------------------------
#[cold]
#[inline(never)]
fn variable_limit_reached_err() -> String {
    format!("Variable limit ({VARIABLE_LIMIT}) reached. Type 'clear'.")
}
//----------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Tokenizer;
    use crate::parser::Parser;

    #[test]
    fn test_collect_variables() {
        let src = "log_b(k)";
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();

        let vars = collect_variables(&arena, root);
        let mut iter = vars.iter();
        let &(_, s0, e0) = iter.next().unwrap();
        let &(_, s1, e1) = iter.next().unwrap();
        assert_eq!(vars.len(), 2);
        assert_eq!(&src[s0 as usize..e0 as usize], "b");
        assert_eq!(&src[s1 as usize..e1 as usize], "k");
    }
}
