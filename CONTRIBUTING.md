# Contributing to KMark

Thank you for your interest in contributing to **KMark**!

## Architecture & Safety Rules

1. **Layered Workspace (`L0` -> `L5`)**:
   - Keep pure editing, parsing, and 2D layout logic inside the Rust crates (`crates/buffer`, `crates/syntax`, `crates/layout`, `crates/engine`) so both the headless CLI (`apps/kmark-cli`) and unit tests exercise the exact same code paths as the KDE GUI (`qt-shell/`).
2. **Memory & Crash Safety (`unsafe_code = "forbid"`)**:
   - All pure-Rust crates forbid `unsafe` code and deny `unwrap()`, `expect()`, `panic!()`, `todo!()`, and `unreachable!()` via workspace lints.
   - Only `crates/ffi` may use `unsafe` to expose the panic-isolated (`catch_unwind`) C ABI (`kmark_ffi.h`) to Qt6/KF6.
   - Always clamp byte indices to UTF-8 codepoint boundaries (`Rope::clamp_char_boundary`) and use checked/saturating arithmetic.
3. **Bilingual UI**:
   - Keep all user-visible strings in `qt-shell/src/` and `crates/engine/` localized in both **English** and **German** (`trBi(m_lang, "English", "Deutsch")`).

## Pre-Submit Checklist

Before opening a pull request, run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cmake -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j
```

## License

By contributing to KMark, you agree that your contributions will be licensed under [`GPL-3.0-or-later`](LICENSE).
