use crate::composer;
use crate::db;
use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box, Entry, HeaderBar, Label, ListBox, ListBoxRow, Orientation,
    ScrolledWindow, SearchEntry,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub fn open_addressbook_window(app: &Application) {
    let contacts = Rc::new(RefCell::new(db::get_all_contacts().unwrap_or_default()));
    let editing_idx = Rc::new(Cell::new(None::<usize>));

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

    // Wir nutzen einen Weak-Pointer für die Closure, um Speicherlecks (Rc Cycles) zu vermeiden
    type RenderFn = Rc<dyn Fn(Option<usize>)>;
    let render_list_rc: Rc<RefCell<Option<RenderFn>>> = Rc::new(RefCell::new(None));
    let render_list_weak = Rc::downgrade(&render_list_rc);

    {
        let list_box = list_box.clone();
        let contacts = contacts.clone();
        let editing_idx = editing_idx.clone();

        *render_list_rc.borrow_mut() = Some(Rc::new(move |select_idx: Option<usize>| {
            while let Some(child) = list_box.first_child() {
                list_box.remove(&child);
            }

            for (idx, (name, email, is_verified, has_pub_key)) in
                contacts.borrow().iter().enumerate()
            {
                let row = ListBoxRow::new();
                let row_box = Box::builder()
                    .orientation(Orientation::Horizontal)
                    .spacing(10)
                    .margin_start(10)
                    .margin_end(10)
                    .margin_top(5)
                    .margin_bottom(5)
                    .build();

                // Inline-Editier-Feld
                if editing_idx.get() == Some(idx) {
                    let name_entry = Entry::builder()
                        .text(name)
                        .xalign(0.0)
                        .width_request(200)
                        .build();

                    let rl_weak = render_list_weak.clone();
                    let contacts_c = contacts.clone();
                    let email_c = email.clone();
                    let ed_idx = editing_idx.clone();

                    name_entry.connect_activate(move |e| {
                        let new_name = e.text().to_string();
                        if db::update_contact_name(&email_c, &new_name).is_ok() {
                            if let Some(c) = contacts_c.borrow_mut().get_mut(idx) {
                                c.0 = new_name;
                            }
                        }
                        ed_idx.set(None);
                        if let Some(rl_rc) = rl_weak.upgrade() {
                            if let Some(render) = rl_rc.borrow().as_ref() {
                                render(Some(idx));
                            }
                        }
                    });

                    row_box.append(&name_entry);

                    let entry_focus = name_entry.clone();
                    gtk4::glib::timeout_add_local(
                        std::time::Duration::from_millis(10),
                        move || {
                            entry_focus.grab_focus();
                            gtk4::glib::ControlFlow::Break
                        },
                    );
                } else {
                    let display_name = if name.is_empty() { "Unbekannt" } else { name };
                    let name_label = Label::builder()
                        .label(display_name)
                        .xalign(0.0)
                        .width_request(200)
                        .ellipsize(gtk4::pango::EllipsizeMode::End)
                        .build();
                    row_box.append(&name_label);
                }

                let email_label = Label::builder()
                    .label(email)
                    .xalign(0.0)
                    .hexpand(true)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .build();
                row_box.append(&email_label);

                if *has_pub_key {
                    let key_label = Label::builder()
                        .use_markup(true)
                        .label("<span foreground='#1e90ff' weight='bold'>[K]</span>")
                        .tooltip_text("Public Key vorhanden")
                        .build();
                    row_box.append(&key_label);
                }

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

            // Fokus wiederherstellen
            if let Some(idx) = select_idx {
                if let Some(row) = list_box.row_at_index(idx as i32) {
                    list_box.select_row(Some(&row));
                    if editing_idx.get().is_none() {
                        row.grab_focus();
                    }
                } else if let Some(row) = list_box.row_at_index((idx as i32) - 1) {
                    list_box.select_row(Some(&row));
                    if editing_idx.get().is_none() {
                        row.grab_focus();
                    }
                }
            }
        }));
    }

    if let Some(render) = render_list_rc.borrow().as_ref() {
        render(None);
    }

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
            if let Some((name, email, _, _)) = contacts_ref.borrow().get(idx) {
                let search_text = format!("{} {}", name, email).to_lowercase();
                search_text.contains(&query)
            } else {
                false
            }
        });
    });

    // --- NEU: Expliziter Doppelklick-Handler statt row_activated ---
    let click_gesture = gtk4::GestureClick::new();
    click_gesture.set_button(gdk::BUTTON_PRIMARY);
    let app_click = app.clone();
    let contacts_click = contacts.clone();
    let ed_idx_click = editing_idx.clone();
    let list_click = list_box.clone();

    click_gesture.connect_pressed(move |_, n_press, _, y| {
        if n_press == 2 {
            if ed_idx_click.get().is_some() {
                return;
            }

            // Finde die Zeile unter dem Cursor
            if let Some(row) = list_click.row_at_y(y as i32) {
                let idx = row.index() as usize;
                if let Some((name, email, _, _)) = contacts_click.borrow().get(idx) {
                    let to_str = if name.is_empty() {
                        email.clone()
                    } else {
                        format!("{} <{}>", name, email)
                    };
                    composer::open_composer_window(&app_click, Some(&to_str), None, None);
                }
            }
        }
    });
    list_box.add_controller(click_gesture);

    let header = HeaderBar::new();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Adressbuch")
        .default_width(550)
        .default_height(600)
        .child(&vbox)
        .build();

    let key_controller = gtk4::EventControllerKey::new();
    let list_nav = list_box.clone();
    let contacts_keys = contacts.clone();
    let render_keys = render_list_rc.clone();
    let search_focus = search_entry.clone();
    let ed_idx_keys = editing_idx.clone();
    let app_keys = app.clone();
    let win_close = window.clone(); // NEU: Fenster-Referenz für ESC

    key_controller.connect_key_pressed(move |_, keyval, _, _| {
        let current_idx = list_nav.selected_row().map(|r| r.index()).unwrap_or(-1);

        if ed_idx_keys.get().is_some() {
            if keyval == gdk::Key::Escape {
                ed_idx_keys.set(None);
                if let Some(render) = render_keys.borrow().as_ref() {
                    render(Some(current_idx.max(0) as usize));
                }
                list_nav.grab_focus();
                return gtk4::glib::Propagation::Stop;
            }
            return gtk4::glib::Propagation::Proceed;
        }

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
            gdk::Key::Return | gdk::Key::KP_Enter => {
                if current_idx >= 0 {
                    let idx = current_idx as usize;
                    if let Some((name, email, _, _)) = contacts_keys.borrow().get(idx) {
                        let to_str = if name.is_empty() {
                            email.clone()
                        } else {
                            format!("{} <{}>", name, email)
                        };
                        composer::open_composer_window(&app_keys, Some(&to_str), None, None);
                    }
                }
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::d => {
                if current_idx >= 0 {
                    let idx = current_idx as usize;
                    let email_opt = contacts_keys.borrow().get(idx).map(|c| c.1.clone());
                    if let Some(email) = email_opt {
                        // Soft-Delete statt echtem Löschen
                        if db::hide_contact(&email).is_ok() {
                            contacts_keys.borrow_mut().remove(idx);
                            if let Some(render) = render_keys.borrow().as_ref() {
                                render(Some(idx));
                            }
                        }
                    }
                }
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::v => {
                if current_idx >= 0 {
                    let idx = current_idx as usize;
                    let email_opt = contacts_keys.borrow().get(idx).map(|c| c.1.clone());
                    if let Some(email) = email_opt {
                        if db::verify_contact(&email).is_ok() {
                            if let Some(contact) = contacts_keys.borrow_mut().get_mut(idx) {
                                contact.2 = true;
                            }
                            if let Some(render) = render_keys.borrow().as_ref() {
                                render(Some(idx));
                            }
                        }
                    }
                }
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::r => {
                if current_idx >= 0 {
                    ed_idx_keys.set(Some(current_idx as usize));
                    if let Some(render) = render_keys.borrow().as_ref() {
                        render(Some(current_idx as usize));
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
                    // NEU: Fenster schließen
                    win_close.close();
                    gtk4::glib::Propagation::Stop
                }
            }
            _ => gtk4::glib::Propagation::Proceed,
        }
    });

    window.add_controller(key_controller);
    window.set_titlebar(Some(&header));
    window.present();
}
