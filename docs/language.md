# The CCore language

This page describes what the compiler actually accepts today. Anything not listed here doesn't exist yet.

## Programs

A program is a single `.ccr` file made of functions. Execution starts at `main`.

```rust
fn main() {
    println("Hello, world!");
}
```

`main` takes no parameters and either returns nothing or returns `i32`. The returned value becomes the process exit code (on Linux and macOS only the low 8 bits survive, so keep it in 0..255).

```rust
fn main() -> i32 {
    return 0;
}
```

## Comments

Line comments only:

```rust
// this is a comment
let x = 1; // so is this
```

## Types

| Type | Meaning |
|---|---|
| `i32` | 32-bit signed integer |
| `i64` | 64-bit signed integer |
| `bool` | `true` or `false` |

There are no implicit conversions. `i32` and `i64` can't be mixed in one expression.

### Integer literals

Underscores can be used as separators: `1_000_000`.

A literal is `i32` unless the context needs `i64`:

```rust
let a = 5;                      // i32
let b: i64 = 5_000_000_000;     // i64, because of the annotation
let c = b + 1;                  // 1 becomes i64 because b is i64
```

A literal that doesn't fit in `i32` without such context is an error:

```
error: literal `3000000000` doesn't fit in `i32`; annotate the variable as `i64`
```

### Strings

String literals exist, but for now they can only be passed directly to `println`. You can't store them in variables yet. Supported escapes: `\n`, `\t`, `\r`, `\0`, `\\`, `\"`. UTF-8 is fine.

## Variables

```rust
let x = 1;          // immutable
let mut y = 2;      // mutable
y = y + x;
let z: i64 = 10;    // explicit type
```

Assigning to an immutable variable is a compile error:

```
error: cannot assign twice to immutable variable `x`; declare it with `let mut`
```

### Shadowing

A new `let` with an existing name creates a new variable that hides the old one. The new one can have a different type.

```rust
let x = 5;
let x = x + 1;      // 6
let x = x == 6;     // now a bool
```

A variable declared inside a block disappears at the end of it:

```rust
let x = 1;
{
    let x = 10;
    println(x);     // 10
}
println(x);         // 1
```

## Operators

From lowest to highest precedence:

| Operators | Operand types | Notes |
|---|---|---|
| `\|\|` | `bool` | short-circuit |
| `&&` | `bool` | short-circuit |
| `==` `!=` `<` `<=` `>` `>=` | integers; `==` and `!=` also `bool` | can't be chained |
| `+` `-` | integers | checked |
| `*` `/` `%` | integers | checked |
| `-x` `!x` | integers / `bool` | unary |

Both sides of a binary operator must have the same type. Use parentheses to group.

Comparisons can't be chained, so `a < b < c` is an error instead of meaning something surprising like it does in C:

```
error: comparison operators cannot be chained; use parentheses
```

There's no `+=`, `++` or `--` yet. Write `x = x + 1`.

## Control flow

```rust
if n < 0 {
    println("negative");
} else if n == 0 {
    println("zero");
} else {
    println("positive");
}

while i < 10 {
    i = i + 1;
}
```

Conditions must be `bool`. There's no implicit "non-zero means true".

Blocks `{ ... }` can also be used on their own to limit the scope of variables.

## Functions

```rust
fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

fn greet() {
    println("hi");
}
```

- Functions can be called before they are defined in the file, and they can be recursive.
- Parameters are immutable. Copy one into a `let mut` if you need to change it.
- A function with a return type must `return` on every path. This is rejected:

```rust
fn sign(x: i32) -> i32 {
    if x > 0 {
        return 1;
    }
}
```

```
error: not all paths in `sign` return a value of type `i32`
```

## println

`println` is built in. It takes exactly one argument (a string literal, an integer or a bool) and prints it followed by a newline.

```rust
println("text");
println(42);
println(1 == 2);    // false
```

## Runtime errors

Things that are undefined behavior in C stop a CCore program instead. The program prints where it happened and exits with code 101:

```
panic at example.ccr:12: integer overflow
```

This covers:

- integer overflow in `+`, `-`, `*` and unary `-`
- division and remainder by zero
- `MIN / -1` and `MIN % -1`

## Not there yet

Structs, arrays, pointers, string variables, floats, enums, `match`, modules, imports, generics. See the roadmap in the README.
