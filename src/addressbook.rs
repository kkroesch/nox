use crate::composer;
use crate::db;
use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box, HeaderBar, Label, ListBox, ListBoxRow, Orientation,
    ScrolledWindow, SearchEntry,
};
use std::cell::RefCell;
use std::rc::Rc;

pub fn open_addressbook_window(app: &Application) {
    // State für Kontakte, veränderbar für Delete/Verify
    let contacts = Rc::new(RefCell::new(db::get_all_contacts().unwrap_or_default()));

    let vbox = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(0)
        .build();

    let search_entry = SearchEntry::builder()
        .margin_start(10)
        .margin_end(10)
        .margin_top(10)
        .margin_bottom(10)
        .placeholder_text("Name oder E-Mail suchen...")
        .build();
    vbox.append(&search_entry);

    let list_box = ListBox::builder()
        .selection_mode(gtk4::SelectionMode::Single)
        .build();

    // Dynamisches Rendern der Liste
    let render_list = {
        let list_box = list_box.clone();
        let contacts = contacts.clone();
        Rc::new(move |select_idx: Option<usize>| {
            while let Some(child) = list_box.first_child() {
                list_box.remove(&child);
            }

            for (name, email, is_verified) in contacts.borrow().iter() {
                let row = ListBoxRow::new();
                let row_box = Box::builder()
                    .orientation(Orientation::Horizontal)
                    .spacing(10)
                    .margin_start(10)
                    .margin_end(10)
                    .margin_top(5)
                    .margin_bottom(5)
                    .build();

                let display_name = if name.is_empty() { "Unbekannt" } else { name };
                let name_label = Label::builder()
                    .label(display_name)
                    .xalign(0.0)
                    .width_request(200)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .build();
                let email_label = Label::builder()
                    .label(email)
                    .xalign(0.0)
                    .hexpand(true)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .build();

                row_box.append(&name_label);
                row_box.append(&email_label);

                // FIX: Unicode-Checkmark statt unzuverlässigem GTK-Icon
                if *is_verified {
                    let verified_label = Label::builder()
                        .use_markup(true)
                        .label("<span foreground='green' weight='bold'>✓</span>")
                        .tooltip_text("Verifiziert")
                        .build();
                    row_box.append(&verified_label);
                }

                row.set_child(Some(&row_box));
                list_box.append(&row);
            }

            // Fokus wiederherstellen (nach Delete/Verify)
            if let Some(idx) = select_idx {
                if let Some(row) = list_box.row_at_index(idx as i32) {
                    list_box.select_row(Some(&row));
                    row.grab_focus();
                } else if let Some(row) = list_box.row_at_index((idx as i32) - 1) {
                    list_box.select_row(Some(&row));
                    row.grab_focus();
                }
            }
        })
    };

    render_list(None);

    let scroll = ScrolledWindow::builder()
        .child(&list_box)
        .vexpand(true)
        .build();
    vbox.append(&scroll);

    // Such-Filter Logik
    let list_box_filter = list_box.clone();
    let contacts_filter = contacts.clone();
    search_entry.connect_search_changed(move |entry| {
        let query = entry.text().to_lowercase();
        let contacts_ref = contacts_filter.clone();
        list_box_filter.set_filter_func(move |row| {
            if query.is_empty() {
                return true;
            }
            let idx = row.index() as usize;
            if let Some((name, email, _)) = contacts_ref.borrow().get(idx) {
                let search_text = format!("{} {}", name, email).to_lowercase();
                search_text.contains(&query)
            } else {
                false
            }
        });
    });

    // Doppelklick / Enter (Composer öffnen)
    let app_clone = app.clone();
    let contacts_clone = contacts.clone();
    list_box.connect_row_activated(move |_, row| {
        let idx = row.index() as usize;
        if let Some((name, email, _)) = contacts_clone.borrow().get(idx) {
            let to_str = if name.is_empty() {
                email.clone()
            } else {
                format!("{} <{}>", name, email)
            };
            composer::open_composer_window(&app_clone, Some(&to_str), None, None);
        }
    });

    let header = HeaderBar::new();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Adressbuch")
        .default_width(500)
        .default_height(600)
        .child(&vbox)
        .build();

    // Vi-Navigation und Shortcuts
    let key_controller = gtk4::EventControllerKey::new();
    let list_nav = list_box.clone();
    let contacts_keys = contacts.clone();
    let render_keys = render_list.clone();
    let search_focus = search_entry.clone();

    key_controller.connect_key_pressed(move |_, keyval, _, _| {
        let current_idx = list_nav.selected_row().map(|r| r.index()).unwrap_or(-1);

        match keyval {
            gdk::Key::j => {
                if let Some(row) = list_nav.row_at_index(current_idx + 1) {
                    list_nav.select_row(Some(&row));
                    row.grab_focus();
                }
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::k => {
                if current_idx > 0 {
                    if let Some(row) = list_nav.row_at_index(current_idx - 1) {
                        list_nav.select_row(Some(&row));
                        row.grab_focus();
                    }
                }
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::d => {
                // Kontakt löschen
                if current_idx >= 0 {
                    let idx = current_idx as usize;
                    let email_opt = contacts_keys.borrow().get(idx).map(|c| c.1.clone());
                    if let Some(email) = email_opt {
                        if db::delete_contact(&email).is_ok() {
                            contacts_keys.borrow_mut().remove(idx);
                            render_keys(Some(idx));
                        }
                    }
                }
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::v => {
                // Kontakt verifizieren (Panic fix)
                if current_idx >= 0 {
                    let idx = current_idx as usize;
                    // 1. E-Mail extrahieren und Borrow sofort wieder schließen
                    let email_opt = contacts_keys.borrow().get(idx).map(|c| c.1.clone());

                    if let Some(email) = email_opt {
                        // 2. Jetzt sicher in die DB schreiben und den lokalen State mutieren
                        if db::verify_contact(&email).is_ok() {
                            if let Some(contact) = contacts_keys.borrow_mut().get_mut(idx) {
                                contact.2 = true;
                            }
                            render_keys(Some(idx));
                        }
                    }
                }
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::slash => {
                search_focus.grab_focus();
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::Escape => {
                if search_focus.has_focus() {
                    search_focus.set_text("");
                    list_nav.grab_focus();
                    gtk4::glib::Propagation::Stop
                } else {
                    gtk4::glib::Propagation::Proceed
                }
            }
            _ => gtk4::glib::Propagation::Proceed,
        }
    });

    window.add_controller(key_controller);
    window.set_titlebar(Some(&header));
    window.present();
}
