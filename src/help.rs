use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Grid, Label, Orientation, ScrolledWindow};

pub fn show_help_window(app: &Application) {
    let vbox = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(15)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(20)
        .margin_end(20)
        .build();

    let title = Label::builder()
        .label("<span size='x-large' weight='bold'>Tastenkombinationen</span>")
        .use_markup(true)
        .halign(gtk4::Align::Start)
        .build();
    vbox.append(&title);

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .build();

    let content_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(20)
        .build();

    // --- Sektion: Hauptfenster ---
    let main_title = Label::builder()
        .label("<span size='large' weight='bold'>Hauptfenster</span>")
        .use_markup(true)
        .halign(gtk4::Align::Start)
        .build();
    content_box.append(&main_title);

    let grid_main = Grid::builder().row_spacing(10).column_spacing(20).build();

    let shortcuts_main = [
        ("j / k", "Nächste / Vorherige Mail auswählen"),
        ("Shift+Klick", "Mehrere Mails am Stück markieren"),
        ("Ctrl+Klick", "Einzelne Mails markieren/abwählen"),
        ("Ctrl+a", "Alle Mails im Ordner auswählen"),
        ("a", "Auswahl archivieren (in Archive)"),
        ("d", "Auswahl löschen (in TRASH)"),
        ("v", "Auswahl verifizieren (Toggle INBOX / Quarantäne)"),
        ("m", "Auswahl verschieben (Interaktiver Ordner-Dialog)"),
        ("/", "Sucheingabe fokussieren"),
        ("Esc", "Suche abbrechen / Fokus zurück zur Liste"),
        ("?", "Diese Hilfe anzeigen"),
    ];

    for (i, &(key, desc)) in shortcuts_main.iter().enumerate() {
        let key_label = Label::builder()
            .label(&format!("<tt><b>{}</b></tt>", key))
            .use_markup(true)
            .halign(gtk4::Align::End)
            .build();
        let desc_label = Label::builder()
            .label(desc)
            .halign(gtk4::Align::Start)
            .build();

        grid_main.attach(&key_label, 0, i as i32, 1, 1);
        grid_main.attach(&desc_label, 1, i as i32, 1, 1);
    }
    content_box.append(&grid_main);

    // --- Sektion: Adressbuch ---
    let ab_title = Label::builder()
        .label("<span size='large' weight='bold'>Adressbuch</span>")
        .use_markup(true)
        .halign(gtk4::Align::Start)
        .margin_top(10)
        .build();
    content_box.append(&ab_title);

    let grid_ab = Grid::builder().row_spacing(10).column_spacing(20).build();

    let shortcuts_ab = [
        ("j / k", "Nächsten / Vorherigen Kontakt auswählen"),
        ("Enter", "Neue Mail an ausgewählten Kontakt verfassen"),
        ("r", "Kontakt umbenennen (Inline-Edit)"),
        ("v", "Verifizierungs-Status umschalten"),
        ("d", "Kontakt aus dem Adressbuch verstecken/löschen"),
    ];

    for (i, &(key, desc)) in shortcuts_ab.iter().enumerate() {
        let key_label = Label::builder()
            .label(&format!("<tt><b>{}</b></tt>", key))
            .use_markup(true)
            .halign(gtk4::Align::End)
            .build();
        let desc_label = Label::builder()
            .label(desc)
            .halign(gtk4::Align::Start)
            .build();

        grid_ab.attach(&key_label, 0, i as i32, 1, 1);
        grid_ab.attach(&desc_label, 1, i as i32, 1, 1);
    }
    content_box.append(&grid_ab);

    scroll.set_child(Some(&content_box));
    vbox.append(&scroll);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Hilfe")
        .default_width(450)
        .default_height(550)
        .modal(false)
        .destroy_with_parent(true)
        .child(&vbox)
        .build();

    window.present();
}
