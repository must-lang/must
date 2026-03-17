
# must

[![CI](https://github.com/must-lang/must/actions/workflows/ci.yml/badge.svg)](https://github.com/must-lang/must/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

To avoid the "AI slop" look, the key is to strip out all the emojis (no rockets, no stars), drop the overly enthusiastic marketing adjectives ("revolutionary", "blazing fast"), and write it exactly how a senior systems engineer would write it: dry, factual, scannable, and focused strictly on the code and the developer experience.

Here is a clean, professional README tailored exactly to the architecture we just set up. You can copy and paste this directly into your `README.md`.

***

`must` is a compiled, statically typed programming language. It is designed as a modern reimagination of C, offering safer and more ergonomic syntax while maintaining predictable performance and low-level control.

## A quick look

```rust
// example.must
fn calculate_sum(a: int, b: int) -> int {
    let result = a + b;
    return result;
}

fn main() {
    let x = 10;
    let y = 20;
    let z = calculate_sum(x, y);
}
```

*what a banger code, isn't it?*

## Architecture

The `must` compiler is written entirely in Rust. It relies on a few core technologies:

* **LALRPOP** for LR(1) parsing.
* **Salsa** for incremental, query-driven compilation.
* **Insta** for AST and diagnostic snapshot testing.

## Getting Started

To build the compiler from source, you will need the standard Rust toolchain.

```bash
git clone [https://github.com/must-lang/must.git](https://github.com/must-lang/must.git)
cd must
cargo build --release
```

### Running Tests

Because the compiler relies heavily on snapshot testing for the AST, you will need to use `cargo-insta` alongside the standard test runner.

```bash
# 1. Install the snapshot review tool (one-time setup)
cargo install cargo-insta

# 2. Run the test suite
cargo test

# 3. If you modified the parser or AST, review and accept the new snapshots
cargo insta review
```

## Contributing

We welcome contributions. To safely manage intellectual property, this project enforces a standard Developer Certificate of Origin (DCO).

When making a commit, please sign off your work using the `-s` flag. This legally affirms that you wrote the code or have the right to contribute it.

```bash
git commit -s -m "parser: add support for while loops"
```

## License

This project is dual-licensed to protect both users and contributors. You may choose to use this software under the terms of either the [MIT License](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE).

***

This format signals immediately to other developers that you know what you are doing. It provides the "what," the "how," and the "rules" without wasting their time.

Once you have this committed, your repo is completely set up. Are you ready to dive back into the Rust code and write those `Debug` wrappers for the rest of your AST (`PatternData` and `TypeExprData`), or do you want to start scaffolding the type-checker module?
