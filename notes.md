# Tokenizer — Code Notes

## Step 1: Token enum

An enum lists every possible kind of token. A token is a labelled chunk of the input — like "this is a number" or "this is a plus sign".

| Variant | Symbol | Notes |
|---|---|---|
| `Number(f64)` | `3`, `2.5` | Stores the actual numeric value inside |
| `Identifier(String)` | `x`, `xy2` | Stores the variable name inside |
| `Plus` | `+` | |
| `Minus` | `-` | |
| `Asterisk` | `*` | |
| `ForwardSlash` | `/` | |
| `Caret` | `^` | |
| `LeftParenthesis` | `(` | |
| `RightParenthesis` | `)` | |
| `EndOfFile` | — | Signals that there is nothing left to read |

`#[derive(Debug, Clone, PartialEq)]` automatically generates three trait implementations:
- `Debug` — allows printing with `{:?}`
- `Clone` — allows copying a value with `.clone()`
- `PartialEq` — allows comparing two values with `==`

---

## Step 2: Tokenizer struct

A struct groups related data together. The tokenizer needs to remember two things:
- `input` — the full expression split into individual characters (`Vec<char>`)
- `position` — where it currently is in that list, starting at 0

---

## Step 3: Tokenizer methods

### `new(input)`
Creates a fresh Tokenizer. Turns the input string into a `Vec<char>` — for example `"x+2"` becomes `['x', '+', '2']` — and sets `position` to 0.

### `current()`
Peeks at the character at the current position without moving forward. Returns `Some(character)` if there is one, or `None` if the end has been reached.

### `advance()`
Reads the current character and moves `position` forward by one. Returns the character it just passed over.

### `tokenize()`
The main loop. Repeatedly calls `current()` to see what character is next, then decides what to do:
- Whitespace (space, tab) — skip it, advance, produce no token
- A digit or dot — call `read_number()` to consume the whole number
- A letter — call `read_identifier()` to consume the whole variable name
- An operator or parenthesis — advance once and push the matching token
- Anything else — panic with an error message
- `None` — push `EndOfFile` and stop

### `read_number()`
Called when a digit is first seen. Keeps looping and appending characters to `accumulated_text` for as long as the current character is a digit (`0`–`9`) or a dot (`.`). Once a non-digit appears, it stops and calls `.parse()` to convert the collected text (e.g. `"3.14"`) into an actual `f64` number.

This is why `10` is treated as one token and not two — by the time `tokenize()` moves on, `read_number()` has already consumed both the `1` and the `0` as a single unit.

### `read_identifier()`
Called when a letter is first seen. Works the same way as `read_number()` but keeps going as long as the character is alphanumeric (letter or digit). Returns the collected name as a `String`.

---

## Step 4: main()

1. Prints a prompt asking the user to type an expression
2. Reads a line from standard input into `expression`
3. Calls `.trim()` to remove the trailing newline that the Enter key adds
4. Creates a `Tokenizer`, calls `.tokenize()`, and prints each token with `{:?}`