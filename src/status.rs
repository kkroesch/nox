use gtk4::prelude::*;
use gtk4::{Align, Box, Label, Orientation};

pub fn build_status_bar() -> (Box, Label) {
    let container = Box::builder()
        .orientation(Orientation::Horizontal)
        .margin_start(10)
        .margin_end(10)
        .margin_top(4)
        .margin_bottom(4)
        .build();

    let label = Label::builder()
        .label("Bereit")
        .halign(Align::Start)
        .build();

    container.append(&label);

    (container, label)
}
