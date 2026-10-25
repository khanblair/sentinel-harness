//! `PairQr`: the pairing link as a QR code. The daemon sends the SVG markup.

use leptos::prelude::*;

/// The QR box stays at most 220 pixels wide, and it shrinks on a narrow screen.
#[component]
pub fn PairQr(#[prop(into)] svg: Signal<String>) -> impl IntoView {
    view! {
        <div
            style="flex:0 0 auto;width:min(220px,100%);aspect-ratio:1/1;background:#fff;border-radius:8px;padding:8px;box-sizing:border-box;line-height:0"
            inner_html=move || fit_svg(&svg.get())
        ></div>
    }
}

/// Lets the SVG fill its box. The fixed `width` and `height` would keep it at its drawn size, and
/// the flex row would let it spill over the text. An SVG without a `viewBox` cannot scale, so it
/// keeps its sizes.
pub fn fit_svg(svg: &str) -> String {
    let Some(start) = svg.find("<svg") else {
        return svg.to_owned();
    };
    let Some(end) = svg[start..].find('>').map(|offset| start + offset) else {
        return svg.to_owned();
    };
    let tag = &svg[start..end];
    if !tag.contains(" viewBox=\"") {
        return svg.to_owned();
    }
    let sized = strip_attribute(&strip_attribute(tag, "width"), "height");
    format!(
        "{}{} style=\"width:100%;height:100%;display:block\"{}",
        &svg[..start],
        sized,
        &svg[end..]
    )
}

/// Removes the first ` name="..."` attribute from a tag. The leading space keeps `stroke-width`
/// from matching `width`.
fn strip_attribute(tag: &str, name: &str) -> String {
    let needle = format!(" {name}=\"");
    let Some(at) = tag.find(&needle) else {
        return tag.to_owned();
    };
    let value_start = at + needle.len();
    let Some(close) = tag[value_start..].find('"') else {
        return tag.to_owned();
    };
    let after = value_start + close + 1;
    format!("{}{}", &tag[..at], &tag[after..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sized_svg_loses_its_width_and_height_and_fills_its_box() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="440" height="440" viewBox="0 0 41 41"><rect/></svg>"#;
        let fitted = fit_svg(svg);
        assert!(!fitted.contains("width=\"440\""));
        assert!(!fitted.contains("height=\"440\""));
        assert!(fitted.contains("style=\"width:100%;height:100%;display:block\""));
        assert!(fitted.contains("viewBox=\"0 0 41 41\""));
        assert!(fitted.ends_with("<rect/></svg>"));
    }

    #[test]
    fn stroke_width_is_not_mistaken_for_the_width_attribute() {
        let svg = r#"<svg width="10" stroke-width="2" viewBox="0 0 1 1"></svg>"#;
        let fitted = fit_svg(svg);
        assert!(fitted.contains("stroke-width=\"2\""));
        assert!(!fitted.contains(" width=\"10\""));
    }

    #[test]
    fn an_svg_without_a_viewbox_keeps_its_sizes() {
        let svg = r#"<svg width="440" height="440"><rect/></svg>"#;
        assert_eq!(fit_svg(svg), svg);
    }

    #[test]
    fn text_without_an_svg_tag_is_returned_unchanged() {
        assert_eq!(fit_svg("<div>no picture</div>"), "<div>no picture</div>");
    }
}
