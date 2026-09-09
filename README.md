# wasm-helpers

What every [LibrePaper](https://github.com/LibrePaper) renderer shares: the page
a document is stored as, the shape a compile answers in, the word diff, and the
WebAssembly interface itself.

This crate builds no WebAssembly of its own. It is linked into the renderers,
which do:

```
wasm-helpers
  ├── wasm-markdown       → markdown.wasm
  │     └── wasm-bibliography → bibliography.wasm, citations.wasm
  └── wasm-typst          → typst.wasm
```

## Why it exists

Every renderer exports the same sixteen functions, and the bodies were
identical in all of them — only two private functions, `render` and `heading`,
ever differ. Copied per repository, a fix to one of those bodies would have to
be made in four places by hand.

## The version is the ABI's version

Change what a host must call, change it here. Every renderer that has not been
rebuilt then fails to compile, rather than shipping a surface the loader no
longer speaks to. That is the one thing separate repositories otherwise give
up: a coupling the compiler enforces.

## What a renderer still writes

A `#[no_mangle]` export has to be compiled into the `cdylib` that ships and
cannot be inherited from a dependency, so each renderer declares its own
sixteen — each one a wrapper of two or three lines around
`librepaper_wasm_helpers::abi`. See `src/abi.rs` in any renderer.

Sixteen readable signatures rather than a macro that writes them. A macro would
have saved about forty lines per repository and cost every compile error a
useful location; this is a project that hand-writes its JSON rather than carry
a serialiser, and the same taste applies.

## Contents

| Module | What it is |
| --- | --- |
| `abi` | the interface, minus the exports themselves: the buffers, the file map, the date, the word diff |
| `diagnostic` | `Compiled`, `Diagnostic`, `Severity`, and the page shown where a document would be |
| `page` | the standalone HTML page a document is stored as, and `document.css` |
| `text` | the word diff and three-way merge used by the browser and application |

`src/text.rs` is the canonical implementation. The editor's history panel and
the native application both call it, so they tokenise and merge identically.
The pure merge fuzz target lives under `fuzz/` beside this implementation.
Run it with `make fuzz FUZZ_SECONDS=60` after installing a nightly Rust
toolchain and `cargo-fuzz`. CI runs it briefly on each push and pull request.

## Licence

MIT. See [LICENSE](LICENSE).
