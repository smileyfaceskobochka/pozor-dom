#[cfg(any(feature = "server", feature = "wasm"))]
use wasm_bindgen::prelude::*;

#[cfg(any(feature = "server", feature = "wasm"))]
use crate::dashboard::yew_app::YewDashboardApp;

#[cfg(any(feature = "server", feature = "wasm"))]
#[wasm_bindgen(start)]
pub fn run_app() {
    yew::Renderer::<YewDashboardApp>::new().render();
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[wasm_bindgen]
pub fn yew_dashboard_app(element: web_sys::Element) {
    yew::Renderer::<YewDashboardApp>::with_root(element).render();
}
