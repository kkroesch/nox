use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Button, HeaderBar, Label, ListBox,
    Orientation, Paned, ScrolledWindow, SelectionMode, TextView,
};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use mailparse::MailHeaderMap;

mod composer;

const APP_ID: &str = "app.noxmail.Nox";

fn main() -> gtk4::glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    // 1. Linkes Panel: Ordnerstruktur aus ~/.Mail
    let folder_list = ListBox::builder()
        .selection_mode(SelectionMode::Single)
        .css_classes(["navigation-sidebar"]) // Nutzt das native GTK-Styling für Seitenleisten
        .build();

    let folders = get_maildir_folders();
    for folder in &folders {
        let label = Label::builder()
            .label(folder)
            .halign(gtk4::Align::Start)
            .margin_start(10)
            .margin_end(10)
            .margin_top(5)
            .margin_bottom(5)
            .build();
        folder_list.append(&label);
    }

    let folder_scroll = ScrolledWindow::builder()
        .child(&folder_list)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();

    // 2. Rechtes Panel (Oben): Nachrichtenliste
    let mail_list = ListBox::builder()
        .selection_mode(SelectionMode::Single)
        .build();

    let mail_scroll = ScrolledWindow::builder().child(&mail_list).build();

    // 3. Rechtes Panel (Unten): E-Mail Inhalt (Der Viewer)
    let text_buffer = gtk4::TextBuffer::new(None);
    let mail_viewer = TextView::builder()
        .buffer(&text_buffer)
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk4::WrapMode::Word)
        .left_margin(15)
        .right_margin(15)
        .top_margin(15)
        .bottom_margin(15)
        .build();

    let viewer_scroll = ScrolledWindow::builder().child(&mail_viewer).build();

    // --- NEU: State für die aktuell geladenen Mails ---
    let current_mail_paths = Rc::new(RefCell::new(Vec::new()));

    // --- EVENT: Klick auf einen Ordner (Links) ---
    let mail_list_clone = mail_list.clone();
    let text_buffer_clone = text_buffer.clone();
    let paths_clone = current_mail_paths.clone();
    let folders_clone = folders.clone();

    folder_list.connect_row_activated(move |_, row| {
        let idx = row.index() as usize;
        if let Some(folder_name) = folders_clone.get(idx) {
            // UI leeren
            while let Some(child) = mail_list_clone.first_child() {
                mail_list_clone.remove(&child);
            }
            paths_clone.borrow_mut().clear();
            text_buffer_clone.set_text("");

            // Pfad zum Maildir bauen
            let maildir_path = if folder_name == "INBOX" && dirs::home_dir().unwrap().join(".Mail/cur").exists() {
                dirs::home_dir().unwrap().join(".Mail") // ~/.Mail ist selbst das Maildir
            } else {
                dirs::home_dir().unwrap().join(".Mail").join(folder_name)
            };

            let md = maildir::Maildir::from(maildir_path);
            let mut paths = paths_clone.borrow_mut();

            // Mails aus new/ und cur/ laden
            for entry in md.list_new().chain(md.list_cur()) {
                if let Ok(mail) = entry {
                    let path = mail.path().to_path_buf();
                    paths.push(path.clone());

                    // Betreff mit mailparse auslesen
                    let subject = if let Ok(data) = std::fs::read(&path) {
                        if let Ok(parsed) = mailparse::parse_mail(&data) {
                            parsed.get_headers().get_first_value("Subject").unwrap_or_else(|| "Kein Betreff".to_string())
                        } else {
                            "Konnte nicht geparst werden".to_string()
                        }
                    } else {
                        "Lesefehler".to_string()
                    };

                    let label = Label::builder()
                        .label(&subject)
                        .halign(gtk4::Align::Start)
                        .margin_start(10)
                        .margin_top(5)
                        .margin_bottom(5)
                        .build();
                    mail_list_clone.append(&label);
                }
            }
        }
    });

    // --- EVENT: Klick auf eine Mail (Rechts oben) ---
    let text_buffer_clone2 = text_buffer.clone();
    let paths_clone2 = current_mail_paths.clone();

    mail_list.connect_row_activated(move |_, row| {
        let idx = row.index() as usize;
        if let Some(path) = paths_clone2.borrow().get(idx) {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(parsed) = mailparse::parse_mail(&data) {
                    let body = parsed.get_body().unwrap_or_else(|_| "Konnte Body nicht lesen".to_string());
                    text_buffer_clone2.set_text(&body);
                }
            }
        }
    });

    // --- HeaderBar und Button ---
    let header_bar = HeaderBar::new();
    let btn_new_mail = Button::from_icon_name("document-new-symbolic");
    btn_new_mail.set_tooltip_text(Some("Neue Mail verfassen"));
    header_bar.pack_start(&btn_new_mail);

    // Klick-Event für den Button
    let app_clone = app.clone();
    btn_new_mail.connect_clicked(move |_| {
        composer::open_composer_window(&app_clone);
    });

    // Layout zusammensetzen: 2x GtkPaned (Trennlinien)
    let right_pane = Paned::builder()
        .orientation(Orientation::Vertical)
        .start_child(&mail_scroll)
        .end_child(&viewer_scroll)
        .build();
    
    right_pane.set_position(250); // Initiale Höhe der Mail-Liste

    let main_pane = Paned::builder()
        .orientation(Orientation::Horizontal)
        .start_child(&folder_scroll)
        .end_child(&right_pane)
        .build();
    
    main_pane.set_position(200); // Initiale Breite der Ordner-Liste

    // Hauptfenster konfigurieren
    let window = ApplicationWindow::builder()
        .application(app)
        .title("nox")
        .default_width(1100)
        .default_height(700)
        .child(&main_pane)
        .build();

    // HeaderBar als Titelleiste setzen (ersetzt die Standard-OS-Leiste durch die GTK-Leiste)
    window.set_titlebar(Some(&header_bar));

    window.present();
}

// Helfer: Liest die physischen Ordner aus ~/.Mail
fn get_maildir_folders() -> Vec<String> {
    let mut folders = Vec::new();
    
    if let Some(mut path) = dirs::home_dir() {
        path.push(".Mail");
        
        // Wenn ~/.Mail selbst cur/new/tmp enthält, ist es das Root-Maildir (INBOX)
        if path.join("cur").exists() {
            folders.push("INBOX".to_string());
        } else if let Ok(entries) = fs::read_dir(&path) {
            // Ansonsten suche nach Unterordnern, die Maildirs sind
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() && p.join("cur").exists() {
                    if let Ok(name) = entry.file_name().into_string() {
                        folders.push(name);
                    }
                }
            }
        }
    }
    
    // Fallback
    if folders.is_empty() {
        folders.push("INBOX".to_string());
    } else {
        folders.sort();
    }
    
    folders
}
