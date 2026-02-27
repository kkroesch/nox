use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box, Button, Entry, HeaderBar, Orientation, ScrolledWindow,
    TextView,
};

pub fn open_composer_window(app: &Application) {
    let to_entry = Entry::builder().placeholder_text("Empfänger").build();

    let subject_entry = Entry::builder().placeholder_text("Betreff").build();

    let text_view = TextView::builder()
        .wrap_mode(gtk4::WrapMode::Word)
        .left_margin(10)
        .right_margin(10)
        .top_margin(10)
        .bottom_margin(10)
        .build();

    let text_scroll = ScrolledWindow::builder()
        .child(&text_view)
        .vexpand(true) // Sorgt dafür, dass der Texteditor den restlichen Platz einnimmt
        .build();

    let vbox = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .margin_start(10)
        .margin_end(10)
        .margin_top(10)
        .margin_bottom(10)
        .build();

    vbox.append(&to_entry);
    vbox.append(&subject_entry);
    vbox.append(&text_scroll);

    // Eigene HeaderBar für den Composer mit "Senden"-Button
    let composer_header = HeaderBar::new();
    let send_btn = Button::with_label("Senden");
    send_btn.add_css_class("suggested-action"); // Gibt dem Button einen blauen/primären Akzent
    composer_header.pack_end(&send_btn);

    let composer_window = ApplicationWindow::builder()
        .application(app)
        .title("Neue Mail")
        .default_width(600)
        .default_height(500)
        .child(&vbox)
        .build();

    composer_window.set_titlebar(Some(&composer_header));
    composer_window.present();
}
