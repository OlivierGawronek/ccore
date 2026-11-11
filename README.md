# CCore

CCore is a small systems programming language with Rust-like syntax and C's memory model. No borrow checker, no garbage collector, no hidden runtime. The compiler is written in Rust and generates C, so it runs anywhere you have clang or gcc.

>**Status: early.** Only the core of the language works right now: integers, booleans, functions and loops. Structs, arrays, enums and pretty much everything else are still on the roadmap. Expect breaking changes.

```rust
fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        return a;
    }
    return gcd(b, a % b);
}

fn is_prime(n: i32) -> bool {
    if n < 2 {
        return false;
    }
    let mut d = 2;
    while d <= n / d {
        if n % d == 0 {
            return false;
        }
        d = d + 1;
    }
    return true;
}

fn main() {
    println(gcd(1_071_000_000_000, 462_000_000_000));

    let mut n = 0;
    let mut found = 0;
    while found < 10 {
        if is_prime(n) {
            println(n);
            found = found + 1;
        }
        n = n + 1;
    }
}
```

## Why

I've been writing C and Rust for a while. C is missing a lot: sum types, a sane way to handle errors, any protection from undefined behavior. Rust fixes all of that, but on top of new syntax you also have to learn a completely different way of thinking about memory (ownership, borrowing, lifetimes).

CCore tries to sit in between. It takes Rust's syntax and type system and keeps C's model underneath: you manage memory yourself, a pointer is just a pointer, and the code does what it looks like it does.

The rules every feature has to follow:

1. Nothing hidden: no exceptions, no hidden allocations, no hidden costs.
2. Safe by default: variables are immutable, there is no null. Unsafe things have to be written out explicitly.
3. Abstractions shouldn't cost more than the C you'd write by hand.
4. No undefined behavior in safe code. Every operation either has a defined result or is an error.
5. Keep the language small.

## Getting started

You need Rust (stable) and a C compiler (clang or gcc).

```sh
cargo install --path .
ccore run examples/hello.ccr
```

`cargo install` puts the `ccore` binary in `~/.cargo/bin`. If your shell can't find it, add that directory to your `PATH`.

| Command | What it does |
|---|---|
| `ccore run file.ccr` | compile and run |
| `ccore build file.ccr [-o out]` | compile to an executable |
| `ccore emit-c file.ccr` | print the generated C |
| `ccore tokens file.ccr` | print the token stream (mostly for debugging) |

ccore looks for `clang`, then `gcc`, then `cc`. Set `CCORE_CC` to use something else.

## The language

See [docs/language.md](docs/language.md) for everything that works today. In short:

- types: `i32`, `i64`, `bool`
- `let` and `let mut`, shadowing, type inference for locals
- `if` / `else if` / `else`, `while`, blocks
- functions, recursion, calls in any order
- `println` for strings, integers and bools
- integer overflow and division by zero stop the program with the file and line instead of being UB

## How it works

```
.ccr -> lexer -> parser -> type checker -> C codegen -> clang/gcc -> executable
```

Arithmetic in the generated C goes through small checked helpers, because signed overflow in C is undefined behavior. That costs a little performance and buys a program that never silently computes garbage.

## Roadmap
### Done
- integers, bools, functions, control flow, C backend
### TODO
- structs, arrays, slices, pointers, strings, floats
- enums with data and `match`
-  `Option`, `Result`, `?`, `defer`
-  modules, `extern "C"`, importing C headers directly
-  generics and interfaces
-  lightweight ownership model and `unsafe` blocks (no full borrow checker)

Smaller things that are coming sooner: `+=` and friends, `/* */` comments, `mut` parameters, `const`.

This project is developed in my free time. Reaching this point has taken 9 months, so I am not providing any specific release dates or deadlines for upcoming features. Updates will be released gradually as development progresses.

## Contributing

Feedback is highly appreciated! Feel free to open an issue to report bugs, suggest new features, or ask questions.

- **Found a bug?** Open an [issue](../../issues). Please include the `.ccr` code that triggers it, what you expected, what happened instead, your OS and which C compiler you use (`clang --version` or `gcc --version`).
- **Have an idea for the language?** Start a thread in [Discussions](../../discussions) instead of an issue. Language changes affect everything else, so it's better to talk them through first.
- **Want to send a pull request?** For small fixes, just go ahead. For anything bigger, open an issue or a discussion first so we can agree on the approach before you spend time on it. Run `cargo fmt` and `cargo test` before submitting.

Replies might take a few days. Decisions about where the language goes are mine in the end, but I read everything and I'm happy to be convinced.git c

## Known issues

- The order in which function arguments are evaluated depends on the C compiler. `f(a(), b())` runs `a()` first with clang and `b()` first with gcc.
- Deep recursion overflows the stack and the program dies without a message.
- `ccore run` exits with 1 both for compile errors and when the program itself crashes.
- Only the first compile error is reported.
- A function that only returns from inside `while true { ... }` is rejected with "not all paths return a value".
- `"\0"` inside a string cuts off the rest of the string when printed.
- The smallest `i64` value can't be written as a literal.
- No warnings yet (unused variables, unreachable code).

## Tests

```sh
cargo test
```

Unit tests cover the lexer, parser, type checker and codegen. The end-to-end tests in `tests/` compile and run real programs; they are skipped if no C compiler is found.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
