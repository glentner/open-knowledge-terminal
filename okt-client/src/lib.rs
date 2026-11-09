//! The Open Knowledge Terminal browser client, in Rust/WebAssembly, with the graph drawn on a
//! canvas and the lists updated in place, so panning, zooming and arrowing through lists stay at
//! frame rate.

mod actions;
mod api;
mod arrange;
mod chrome;
mod dom;
mod fmt;
mod go;
mod graph;
mod insp;
mod js;
mod keys;
mod live;
mod model;
mod nav;
mod panes;
mod state;
mod theme;
mod views;

use wasm_bindgen::prelude::*;

use crate::api::{ApiError, spawn};
use crate::state::{s, s_mut};

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    spawn(boot());
}

/// Boot: apply the theme, wire the views, the arrangement and the keys, then load the first index.
async fn boot() {
    theme::apply_theme();
    graph::wire();
    views::tree::wire();
    views::log::wire();
    views::table::wire();
    views::list::wire();
    go::wire();
    chrome::wire_settings();
    panes::wire();
    keys::wire();
    chrome::register_sw();
    let text = match api::get_text("/api/index").await {
        Ok(t) => t,
        Err(ApiError::Network(_)) => {
            // no server: the app shell came from the service worker
            chrome::wait_for_server();
            return;
        }
        Err(e) => {
            dom::set_html(
                &dom::id("ibody"),
                &format!(
                    "<div class=\"empty\">Could not load the index: {}</div>",
                    js::esc(&e.message())
                ),
            );
            chrome::set_live(state::Live::Err);
            return;
        }
    };
    let idx = match model::Idx::parse(&text) {
        Ok(i) => i,
        Err(e) => {
            dom::set_html(
                &dom::id("ibody"),
                &format!(
                    "<div class=\"empty\">Could not load the index: {}</div>",
                    js::esc(&e)
                ),
            );
            return;
        }
    };
    s_mut().idx = Some(idx);
    graph::build(&[]);
    chrome::render_status();
    spawn(async {
        let _ = views::log::ensure(false).await;
    });
    nav::boot_view().await;
    live::connect();
    let _ = s().view;
}
