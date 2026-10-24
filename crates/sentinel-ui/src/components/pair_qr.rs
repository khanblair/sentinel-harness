//! `PairQr`: the pairing link as a QR code. The daemon sends the SVG markup.

use leptos::prelude::*;

#[component]
pub fn PairQr(#[prop(into)] svg: Signal<String>) -> impl IntoView {
    view! {
        <div style="flex:0 0 auto" inner_html=move || svg.get()></div>
    }
}
