//! The parts every LibrePaper renderer shares.
//!
//! A renderer repository holds one thing: how its language becomes a document.
//! Everything around that -- the page template, the diagnostics a compile comes
//! back with, the word diff the editor's history panel asks for, and the
//! WebAssembly interface a host drives all of them through -- is the same in
//! every one of them, and lives here so that it is written once.
//!
//! This crate exports no WebAssembly itself. A `#[no_mangle]` export has to be
//! compiled into the `cdylib` that ships, so each renderer declares its own
//! sixteen exports; what they all call is [`abi`]. Sixteen short functions per
//! renderer rather than a macro that writes them: this is a project that
//! hand-writes its JSON, and a compile error you can read is worth more than
//! the forty lines a macro would save.
//!
//! The version of this crate is the ABI's version. Change what a host must
//! call, change it here, and every renderer that has not been rebuilt fails to
//! compile rather than exporting a surface a loader no longer speaks to.

pub mod abi;
pub mod diagnostic;
pub mod page;

/// The word diff, vendored from the LibrePaper application repository so that
/// this side and the native side compute the same one. Only `diff` is reached
/// from [`abi`]; the rest comes along so that a diff against upstream is empty.
pub mod text;
