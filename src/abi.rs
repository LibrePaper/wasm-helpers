//! The WebAssembly interface, minus the exports themselves.
//!
//! Plain exports over linear memory rather than wasm-bindgen: a renderer needs
//! nothing but cargo and a wasm32 target -- no bindgen CLI whose version has to
//! match the crate's -- and the loader on the other side is a few lines of
//! JavaScript against a module that imports nothing at all.
//!
//! One convention: the caller allocates, writes UTF-8 into the module's memory,
//! calls `compile` or `title_of`, and reads the result back out. Both return
//! the length; `output_ptr` says where it starts and `ok` whether there is a
//! document.
//!
//! A compile leaves a second result beside the first: `diagnostics` is the
//! length of the JSON list of what the compiler had to say and
//! `diagnostics_ptr` where it starts. Two results rather than one envelope,
//! because the document is a megabyte and the list is a hundred bytes, and
//! wrapping the first in JSON to carry the second would be an encode and a
//! decode of the wrong thing on every keystroke.
//!
//! Everything here is a plain function. A renderer wraps each one in the
//! `#[no_mangle] pub extern "C"` export a host actually calls, because an
//! export must be compiled into the `cdylib` that ships and cannot be inherited
//! from a dependency.

use std::path::Path;

use crate::diagnostic::{Compiled, Diagnostic, RenderedDocument};

/// Where the last result lives until the next call replaces it.
static mut OUTPUT: Option<Vec<u8>> = None;
static mut OK: bool = false;
/// The format of `OUTPUT`: 0 = no output, 1 = HTML text, 2 = PDF bytes.
static mut OUTPUT_KIND: u32 = 0;
/// What the last compile had to say, as JSON, beside the document.
static mut DIAGNOSTICS: Option<Vec<u8>> = None;
/// And the same, kept as it was, so the page shown where a document would be
/// can be built from it without the host sending it back.
static mut SAID: Option<Vec<Diagnostic>> = None;
/// What day it is, for a renderer with dates. Kept as numbers rather than as
/// any one renderer's type, because this crate knows about none of them.
static mut TODAY: Option<(i32, u32, u32)> = None;
/// The files the host has handed the module, which is the whole of what a
/// document may read. Filled from the document's own directory on the command
/// line, and in a browser from the shared document: a document is a directory,
/// and every text and figure in it is put here before a compile.
static mut FILES: Option<Vec<(String, Vec<u8>)>> = None;
/// What the main file is called, which is not decoration: a sibling resolves
/// relative to it, so a main file at `chapters/paper.typ` reaches `lib.typ`
/// beside it and not one at the root, and a diagnostic in an imported file is
/// named against it.
static mut MAIN: Option<String> = None;
/// Where each figure is, for a renderer whose output is HTML a browser will
/// fetch from. Empty for one that embeds its figures itself.
static mut ASSET_URLS: Option<Vec<(String, String)>> = None;

/// Reserves `len` bytes for the caller to write a source into.
pub fn alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::with_capacity(len);
    let pointer = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    pointer
}

/// Releases what `alloc` reserved. The caller frees the source it wrote; the
/// output belongs to the module and is replaced on the next call.
///
/// # Safety
/// `pointer` and `len` must be exactly what a previous `alloc` returned.
pub unsafe fn dealloc(pointer: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(pointer, 0, len));
}

/// The UTF-8 at a pointer the host wrote, or the empty string if there is none.
///
/// # Safety
/// The pointer and length must describe memory written into this module.
pub unsafe fn text_at<'a>(pointer: *const u8, len: usize) -> &'a str {
    if pointer.is_null() || len == 0 {
        return "";
    }
    std::str::from_utf8(std::slice::from_raw_parts(pointer, len)).unwrap_or("")
}

/// Leaves a plain text result where `output_ptr` will find it.
///
/// # Safety
/// Replaces the output buffer, so any pointer into it is dead after this.
pub unsafe fn answer(result: Result<String, String>) -> usize {
    let (ok, text) = match result {
        Ok(html) => (true, html),
        Err(message) => (false, message),
    };
    let bytes = text.into_bytes();
    let length = bytes.len();
    OUTPUT = Some(bytes);
    OK = ok;
    OUTPUT_KIND = u32::from(ok);
    length
}

/// A compile's two results: the document where `output_ptr` looks for it, and
/// the list where `diagnostics_ptr` does. A document that did not compile
/// leaves no output and is not an error of the caller's; the list says what
/// happened.
///
/// # Safety
/// Replaces both buffers, so any pointer into either is dead after this.
pub unsafe fn answer_compiled(compiled: Compiled) -> usize {
    DIAGNOSTICS = Some(compiled.diagnostics_json().into_bytes());
    SAID = Some(compiled.diagnostics.clone());
    let (ok, kind, bytes) = match compiled.output {
        Some(RenderedDocument::Html(html)) => (true, 1, html.into_bytes()),
        Some(RenderedDocument::Pdf(pdf)) => (true, 2, pdf),
        None => (false, 0, Vec::new()),
    };
    let length = bytes.len();
    OUTPUT = Some(bytes);
    OK = ok;
    OUTPUT_KIND = kind;
    length
}

/// The page to show where a document would be when the last compile produced
/// none: what it said, dressed as a document rather than as a crash.
///
/// # Safety
/// The pointer and length must describe UTF-8 written into this module.
pub unsafe fn failure_page(title: *const u8, len: usize) -> usize {
    let title = text_at(title, len);
    let said = (*std::ptr::addr_of!(SAID)).clone().unwrap_or_default();
    answer(Ok(crate::diagnostic::diagnostics_page(&said, title)))
}

/// The shared word-level diff, as a JSON array of `{at, delete, insert}` edits,
/// where `at` and `delete` are UTF-16 offsets in `old` and `insert` is text
/// from `new`. The history panel and the native side tokenise the same way
/// because they run the same code.
///
/// # Safety
/// The pointers and lengths must describe UTF-8 written into this module.
pub unsafe fn word_diff(old: *const u8, old_len: usize, new: *const u8, new_len: usize) -> usize {
    let edits = crate::text::diff(text_at(old, old_len), text_at(new, new_len));
    // Written out by hand, like the diagnostics list, so that a module which
    // needs a hundred bytes of JSON does not carry a serialiser to the browser.
    let entries: Vec<String> = edits
        .iter()
        .map(|edit| {
            format!(
                "{{\"at\":{},\"delete\":{},\"insert\":{}}}",
                edit.at,
                edit.delete,
                crate::diagnostic::quote(&edit.insert)
            )
        })
        .collect();
    answer(Ok(format!("[{}]", entries.join(","))))
}

/// Puts a file where the next compile can read it, under the path a document
/// would import it by. This is what `#import` and `#bibliography` need in a
/// host that has no directory.
///
/// # Safety
/// The pointers and lengths must describe memory written into this module.
pub unsafe fn add_file(path: *const u8, path_len: usize, body: *const u8, body_len: usize) {
    let name = text_at(path, path_len).to_string();
    let bytes = if body.is_null() || body_len == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(body, body_len).to_vec()
    };
    let files = (*std::ptr::addr_of_mut!(FILES)).get_or_insert_with(Vec::new);
    match files.iter_mut().find(|(known, _)| *known == name) {
        Some(slot) => slot.1 = bytes,
        None => files.push((name, bytes)),
    }
}

/// Empties the map, which a host does before every document it compiles: the
/// files of the last one are not the files of this one. The main file's name
/// goes with them, since it named a document no longer being compiled.
pub fn clear_files() {
    unsafe {
        FILES = None;
        MAIN = None;
        ASSET_URLS = None;
    }
}

/// Where a figure the document names actually is, for the renderer that needs
/// a URL rather than bytes.
///
/// # Safety
/// The pointers and lengths must describe UTF-8 written into this module.
pub unsafe fn set_asset_url(path: *const u8, path_len: usize, url: *const u8, url_len: usize) {
    let name = text_at(path, path_len).to_string();
    let where_it_is = text_at(url, url_len).to_string();
    let urls = (*std::ptr::addr_of_mut!(ASSET_URLS)).get_or_insert_with(Vec::new);
    match urls.iter_mut().find(|(known, _)| *known == name) {
        Some(slot) => slot.1 = where_it_is,
        None => urls.push((name, where_it_is)),
    }
}

/// Names the main file, so that what it imports resolves relative to it.
///
/// # Safety
/// The pointer and length must describe UTF-8 written into this module.
pub unsafe fn set_main(path: *const u8, len: usize) {
    let name = text_at(path, len).to_string();
    unsafe { MAIN = if name.is_empty() { None } else { Some(name) } }
}

/// Tells a renderer what day it is, for `datetime.today()` and its kind: a
/// module has no clock, so the host hands it one. A renderer without dates
/// never reads it.
pub fn set_today(year: i32, month: u32, day: u32) {
    unsafe { TODAY = Some((year, month, day)) }
}

/// What the host last said the date was.
pub fn today() -> Option<(i32, u32, u32)> {
    unsafe { *std::ptr::addr_of!(TODAY) }
}

/// What the main file is called, or the empty string if the host never said.
pub fn main_name() -> String {
    unsafe { (*std::ptr::addr_of!(MAIN)).clone() }.unwrap_or_default()
}

/// Reads a file the host put in the map, and nothing else: there is no
/// directory in a browser, and a document reaches only what it was given.
pub fn file(path: &Path) -> Option<Vec<u8>> {
    let wanted = path.to_string_lossy();
    unsafe {
        (*std::ptr::addr_of!(FILES))
            .as_ref()?
            .iter()
            .find(|(name, _)| name.as_str() == wanted)
            .map(|(_, bytes)| bytes.clone())
    }
}

/// Every file the host handed over, for a renderer that wants them all at once.
pub fn files() -> Vec<(String, Vec<u8>)> {
    unsafe { (*std::ptr::addr_of!(FILES)).clone() }.unwrap_or_default()
}

/// Where the host put a figure, for a renderer that rewrites image sources.
pub fn asset_url(path: &str) -> Option<String> {
    unsafe {
        (*std::ptr::addr_of!(ASSET_URLS))
            .as_ref()?
            .iter()
            .find(|(name, _)| name.as_str() == path)
            .map(|(_, url)| url.clone())
    }
}

/// Forgets what the last compile said. An answer that is not a compile -- a
/// parsed bibliography, say -- leaves no diagnostics rather than the previous
/// call's.
pub fn clear_diagnostics() {
    unsafe {
        DIAGNOSTICS = None;
        SAID = None;
    }
}

/// Where the last result starts.
pub fn output_ptr() -> *const u8 {
    unsafe {
        match &*std::ptr::addr_of!(OUTPUT) {
            Some(bytes) => bytes.as_ptr(),
            None => std::ptr::null(),
        }
    }
}

/// Whether the last result is a document (1) or nothing (0).
pub fn ok() -> u32 {
    unsafe { u32::from(*std::ptr::addr_of!(OK)) }
}

/// Which format `output_ptr` points to: 0 means no output, 1 HTML, 2 PDF.
pub fn output_kind() -> u32 {
    unsafe { OUTPUT_KIND }
}

/// The length of what the last compile had to say, as a JSON list.
pub fn diagnostics() -> usize {
    unsafe {
        match &*std::ptr::addr_of!(DIAGNOSTICS) {
            Some(bytes) => bytes.len(),
            None => 0,
        }
    }
}

/// Where that list starts.
pub fn diagnostics_ptr() -> *const u8 {
    unsafe {
        match &*std::ptr::addr_of!(DIAGNOSTICS) {
            Some(bytes) => bytes.as_ptr(),
            None => std::ptr::null(),
        }
    }
}
