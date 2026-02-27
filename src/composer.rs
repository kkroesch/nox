use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box, Button, Entry, HeaderBar, Orientation, ScrolledWindow,
    TextView,
};

pub fn open_composer_window(app: &Application, to: Option<&str>, subject: Option<&str>, body: Option<&str>) {
    let to_entry = Entry::builder().placeholder_text("Empfänger").build();
    if let Some(t) = to {
        to_entry.set_text(t);
    }

    let subject_entry = Entry::builder().placeholder_text("Betreff").build();
    if let Some(s) = subject {
        subject_entry.set_text(s);
    }

    let text_buffer = gtk4::TextBuffer::new(None);
    if let Some(b) = body {
        text_buffer.set_text(b);
    }

    let text_view = TextView::builder()
        .buffer(&text_buffer)
        .wrap_mode(gtk4::WrapMode::Word)
        .left_margin(10)
        .right_margin(10)
        .top_margin(10)
        .bottom_margin(10)
        .build();

    let text_scroll = ScrolledWindow::builder()
        .child(&text_view)
        .vexpand(true)
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

    let composer_header = HeaderBar::new();
    let send_btn = Button::with_label("Senden");
    send_btn.add_css_class("suggested-action");
    composer_header.pack_end(&send_btn);

    let composer_window = ApplicationWindow::builder()
        .application(app)
        .title("Neue Mail")
        .default_width(600)
        .default_height(500)
        .child(&vbox)
        .build();

    // Klone für den Button-Closure
    let win_clone = composer_window.clone();
    let to_entry_clone = to_entry.clone();
    let subject_entry_clone = subject_entry.clone();
    let text_buffer_clone = text_buffer.clone();

    send_btn.connect_clicked(move |_| {
        let to = to_entry_clone.text().to_string();
        let subj = subject_entry_clone.text().to_string();

        let (start, end) = text_buffer_clone.bounds();
        let body = text_buffer_clone.text(&start, &end, false).to_string();

        // Minimalistischer RFC 2822 Header (MTA wie msmtp ergänzt Date und Message-ID)
        let raw_mail = format!(
            "To: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}",
            to, subj, body
        );

        let outbox_dir = dirs::home_dir().unwrap().join(".Mail").join("Outbox").join("new");
        if let Err(e) = std::fs::create_dir_all(&outbox_dir) {
            eprintln!("Fehler beim Erstellen des Outbox-Ordners: {}", e);
        } else {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();

            let file_path = outbox_dir.join(format!("{}.nox", timestamp));

            if let Err(e) = std::fs::write(&file_path, raw_mail) {
                eprintln!("Fehler beim Speichern der Mail: {}", e);
            } else {
                println!("Mail für Versand gepuffert: {:?}", file_path);
            }
        }

        win_clone.close();
    });

    composer_window.set_titlebar(Some(&composer_header));
    composer_window.present();
}
