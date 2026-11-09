//! WordCraft Preview: a browser-only, read-only document viewer.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_arch = "wasm32")]
mod preview_api;
#[cfg(target_arch = "wasm32")]
mod preview_session;
#[cfg(target_arch = "wasm32")]
mod preview_ui;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("wordcraft-preview only runs in the browser: build it with `trunk build --release` in apps/wordcraft-preview");
}
