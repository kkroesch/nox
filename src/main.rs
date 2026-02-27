use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Button, HeaderBar, Label, ListBox, Orientation, Paned,
    ScrolledWindow, SelectionMode, TextView, Spinner, SearchEntry, ToggleButton,
};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};
use mailparse::MailHeaderMap;
use gtk4::gdk;

mod composer;
mod db;
mod addressbook;

const APP_ID: &str = "app.noxmail.Nox";

#[derive(Clone)]
struct MailEntry {
    path: PathBuf,
    timestamp: i64,
    date_short: String,
    date_full: String,
    from: String,
    return_path: String,
    subject: String,
    is_read: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum SortCol { Date, Sender, Subject }

fn perform_search(entries: &[MailEntry], query: &str) -> Vec<MailEntry> {
    if query.trim().is_empty() {
        return entries.to_vec();
    }
    let q = query.to_lowercase();
    entries
        .iter()
        .filter(|e| {
            e.subject.to_lowercase().contains(&q)
                || e.from.to_lowercase().contains(&q)
                || e.date_full.to_lowercase().contains(&q)
        })
        .cloned()
        .collect()
}

fn main() -> gtk4::glib::ExitCode {
    if let Err(e) = db::init_db() {
        eprintln!("Fehler bei der DB-Initialisierung: {}", e);
    }

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(".unread { font-weight: bold; }");
    gtk4::style_context_add_provider_for_display(
        &gdk::Display::default().expect("Konnte Display nicht laden"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let quit_action = gtk4::gio::SimpleAction::new("quit", None);
    let app_clone = app.clone();
    quit_action.connect_activate(move |_, _| {
        app_clone.quit();
    });
    app.add_action(&quit_action);
    app.set_accels_for_action("app.quit", &["<Primary>q"]);

    let folder_list = ListBox::builder()
        .selection_mode(SelectionMode::Single)
        .css_classes(["navigation-sidebar"])
        .build();

    let folders = get_maildir_folders();
    for folder in &folders {
        let label = Label::builder()
            .label(folder)
            .halign(gtk4::Align::Start)
            .margin_start(10).margin_end(10).margin_top(5).margin_bottom(5)
            .build();
        folder_list.append(&label);
    }

    let folder_scroll = ScrolledWindow::builder()
        .child(&folder_list)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();

    let mail_list_vbox = gtk4::Box::builder().orientation(Orientation::Vertical).build();
    let header_box = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .margin_start(10).margin_end(10).margin_top(5).margin_bottom(5)
        .build();

    let btn_sort_date = Button::builder().label("Datum").width_request(130).css_classes(["flat"]).build();
    let btn_sort_sender = Button::builder().label("Absender").width_request(200).css_classes(["flat"]).build();
    let btn_sort_subject = Button::builder().label("Betreff").hexpand(true).halign(gtk4::Align::Start).css_classes(["flat"]).build();

    header_box.append(&btn_sort_date);
    header_box.append(&btn_sort_sender);
    header_box.append(&btn_sort_subject);

    let mail_list = ListBox::builder().selection_mode(SelectionMode::Single).build();
    let mail_scroll = ScrolledWindow::builder().child(&mail_list).vexpand(true).build();

    mail_list_vbox.append(&header_box);
    mail_list_vbox.append(&gtk4::Separator::new(Orientation::Horizontal));
    mail_list_vbox.append(&mail_scroll);

    let viewer_vbox = gtk4::Box::builder().orientation(Orientation::Vertical).build();
    let viewer_header_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .margin_start(15).margin_end(15).margin_top(15).margin_bottom(15)
        .build();

    let lbl_viewer_subj = Label::builder().halign(gtk4::Align::Start).use_markup(true).selectable(true).build();
    let lbl_viewer_date = Label::builder().halign(gtk4::Align::Start).selectable(true).build();
    let lbl_viewer_return = Label::builder().halign(gtk4::Align::Start).selectable(true).css_classes(["dim-label"]).build();

    viewer_header_box.append(&lbl_viewer_subj);
    viewer_header_box.append(&lbl_viewer_date);
    viewer_header_box.append(&lbl_viewer_return);

    let text_buffer = gtk4::TextBuffer::new(None);
    let mail_viewer = TextView::builder()
        .buffer(&text_buffer)
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk4::WrapMode::Word)
        .left_margin(15).right_margin(15).top_margin(15).bottom_margin(15)
        .build();

    let viewer_scroll = ScrolledWindow::builder().child(&mail_viewer).vexpand(true).build();

    viewer_vbox.append(&viewer_header_box);
    viewer_vbox.append(&gtk4::Separator::new(Orientation::Horizontal));
    viewer_vbox.append(&viewer_scroll);

    // --- State ---
    let current_mail_entries = Rc::new(RefCell::new(Vec::<MailEntry>::new()));
    let displayed_mail_entries = Rc::new(RefCell::new(Vec::<MailEntry>::new()));
    let sort_state = Rc::new(RefCell::new((SortCol::Date, true)));
    let selected_mail = Rc::new(RefCell::new(None::<MailEntry>));
    let current_search_query = Rc::new(RefCell::new(String::new()));

    // --- Helfer: Rendern ---
    let do_sort_and_render = {
        let list_box = mail_list.clone();
        let all_entries = current_mail_entries.clone();
        let disp_entries = displayed_mail_entries.clone();
        let state = sort_state.clone();
        let search_query = current_search_query.clone();

        Rc::new(move || {
            let query = search_query.borrow().clone();

            let mut display_list = perform_search(&all_entries.borrow(), &query);

            let (col, desc) = *state.borrow();
            display_list.sort_by(|a, b| {
                let cmp = match col {
                    SortCol::Date => a.timestamp.cmp(&b.timestamp),
                    SortCol::Sender => a.from.to_lowercase().cmp(&b.from.to_lowercase()),
                    SortCol::Subject => a.subject.to_lowercase().cmp(&b.subject.to_lowercase()),
                };
                if desc { cmp.reverse() } else { cmp }
            });

            while let Some(child) = list_box.first_child() { list_box.remove(&child); }

            for entry in display_list.iter() {
                let hbox = gtk4::Box::builder()
                    .orientation(Orientation::Horizontal)
                    .spacing(10)
                    .margin_start(10).margin_end(10).margin_top(5).margin_bottom(5)
                    .build();

                let (name, email) = crate::db::parse_from(&entry.from);
                let display_from = if name.is_empty() { email } else { name };

                let lbl_date = Label::builder().label(&entry.date_short).xalign(0.0).width_request(130).build();
                let lbl_from = Label::builder()
                    .label(&display_from)
                    .xalign(0.0)
                    .width_request(200)
                    .max_width_chars(25)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .build();
                let lbl_subj = Label::builder().label(&entry.subject).xalign(0.0).hexpand(true).ellipsize(gtk4::pango::EllipsizeMode::End).build();

                if !entry.is_read {
                    lbl_from.add_css_class("unread");
                    lbl_subj.add_css_class("unread");
                }

                hbox.append(&lbl_date);
                hbox.append(&lbl_from);
                hbox.append(&lbl_subj);
                list_box.append(&hbox);
            }

            *disp_entries.borrow_mut() = display_list;
        })
    };

    let r1 = do_sort_and_render.clone();
    let st1 = sort_state.clone();
    btn_sort_date.connect_clicked(move |_| {
        let mut s = st1.borrow_mut();
        if s.0 == SortCol::Date { s.1 = !s.1; } else { s.0 = SortCol::Date; s.1 = true; }
        drop(s); r1();
    });

    let r2 = do_sort_and_render.clone();
    let st2 = sort_state.clone();
    btn_sort_sender.connect_clicked(move |_| {
        let mut s = st2.borrow_mut();
        if s.0 == SortCol::Sender { s.1 = !s.1; } else { s.0 = SortCol::Sender; s.1 = false; }
        drop(s); r2();
    });

    let r3 = do_sort_and_render.clone();
    let st3 = sort_state.clone();
    btn_sort_subject.connect_clicked(move |_| {
        let mut s = st3.borrow_mut();
        if s.0 == SortCol::Subject { s.1 = !s.1; } else { s.0 = SortCol::Subject; s.1 = false; }
        drop(s); r3();
    });

    let header_bar = HeaderBar::new();

    let btn_new_mail = Button::from_icon_name("document-new-symbolic");
    btn_new_mail.set_tooltip_text(Some("Neue Mail verfassen"));
    header_bar.pack_start(&btn_new_mail);

    let btn_reply = Button::from_icon_name("mail-reply-sender-symbolic");
    btn_reply.set_sensitive(false);
    btn_reply.set_tooltip_text(Some("Antworten (Wähle eine Mail aus)"));
    header_bar.pack_start(&btn_reply);

    let btn_archive = Button::from_icon_name("folder-symbolic");
    btn_archive.set_sensitive(false);
    btn_archive.set_tooltip_text(Some("Archivieren (a)"));
    header_bar.pack_start(&btn_archive);

    let spinner = Spinner::builder().spinning(false).visible(false).build();
    header_bar.pack_end(&spinner);

    let search_entry = SearchEntry::builder().visible(false).width_request(250).build();
    header_bar.pack_end(&search_entry);

    let btn_search = ToggleButton::builder().icon_name("system-search-symbolic").tooltip_text("Suchen").build();
    header_bar.pack_end(&btn_search);

    let btn_addressbook = Button::from_icon_name("avatar-default-symbolic");
    btn_addressbook.set_tooltip_text(Some("Adressbuch"));
    header_bar.pack_end(&btn_addressbook);

    let search_entry_clone = search_entry.clone();
    btn_search.connect_toggled(move |btn| {
        search_entry_clone.set_visible(btn.is_active());
        if btn.is_active() {
            search_entry_clone.grab_focus();
        } else {
            search_entry_clone.set_text("");
        }
    });

    // NEU: ESC direkt im Suchfeld fangen
    let btn_search_stop = btn_search.clone();
    let mail_list_focus = mail_list.clone();
    search_entry.connect_stop_search(move |_| {
        btn_search_stop.set_active(false);
        mail_list_focus.grab_focus();
    });

    let q_clone = current_search_query.clone();
    let render_s_clone = do_sort_and_render.clone();
    search_entry.connect_search_changed(move |entry| {
        *q_clone.borrow_mut() = entry.text().to_string();
        render_s_clone();
    });

    let app_clone_ab = app.clone();
    btn_addressbook.connect_clicked(move |_| {
        addressbook::open_addressbook_window(&app_clone_ab);
    });

    // --- EVENT: Klick auf einen Ordner ---
    let folders_clone = folders.clone();
    let entries_clone = current_mail_entries.clone();
    let render_clone = do_sort_and_render.clone();
    let text_buffer_clone = text_buffer.clone();
    let btn_reply_clone1 = btn_reply.clone();
    let btn_archive_clone1 = btn_archive.clone();
    let spinner_clone = spinner.clone();
    let btn_search_reset = btn_search.clone();
    let search_entry_reset = search_entry.clone();

    folder_list.connect_row_activated(move |_, row| {
        let idx = row.index() as usize;
        if let Some(folder_name) = folders_clone.get(idx).cloned() {
            text_buffer_clone.set_text("");
            entries_clone.borrow_mut().clear();
            btn_reply_clone1.set_sensitive(false);
            btn_archive_clone1.set_sensitive(false);

            btn_search_reset.set_active(false);
            search_entry_reset.set_text("");

            render_clone();
            spinner_clone.set_spinning(true);
            spinner_clone.set_visible(true);

            let (sender, receiver) = std::sync::mpsc::channel();

            std::thread::spawn(move || {
                let maildir_path = if folder_name == "INBOX" && dirs::home_dir().unwrap().join(".Mail/cur").exists() {
                    dirs::home_dir().unwrap().join(".Mail")
                } else {
                    dirs::home_dir().unwrap().join(".Mail").join(folder_name)
                };

                let md = maildir::Maildir::from(maildir_path);
                let mut new_entries = Vec::new();
                let mut db_contacts = std::collections::HashMap::new();

                for entry in md.list_new().chain(md.list_cur()) {
                    if let Ok(mail) = entry {
                        let path = mail.path().to_path_buf();

                        let path_str = path.to_string_lossy();
                        let is_read = path_str.contains(":2,") && path_str.contains('S');

                        if let Ok(data) = std::fs::read(&path) {
                            if let Ok(parsed) = mailparse::parse_mail(&data) {
                                let headers = parsed.get_headers();
                                let subject = headers.get_first_value("Subject").unwrap_or_else(|| "Kein Betreff".to_string());
                                let from = headers.get_first_value("From").unwrap_or_else(|| "Unbekannt".to_string());
                                let return_path = headers.get_first_value("Return-Path").unwrap_or_else(|| "".to_string());
                                let date_str = headers.get_first_value("Date").unwrap_or_default();
                                let timestamp = mailparse::dateparse(&date_str).unwrap_or(0);

                                let date_short = gtk4::glib::DateTime::from_unix_local(timestamp)
                                    .map(|dt| dt.format("%d.%m.%y %H:%M").unwrap_or(date_str.clone().into()).to_string())
                                    .unwrap_or_else(|_| date_str.clone());

                                let (name, email) = db::parse_from(&from);
                                if !email.is_empty() {
                                    let entry = db_contacts.entry(email).or_insert_with(|| name.clone());
                                    if entry.is_empty() && !name.is_empty() {
                                        *entry = name;
                                    }
                                }

                                new_entries.push(MailEntry {
                                    path, timestamp, date_short, date_full: date_str, from, return_path, subject, is_read
                                });
                            }
                        }
                    }
                }

                if let Err(e) = db::bulk_upsert(&db_contacts) {
                    eprintln!("Fehler beim Speichern der Kontakte: {}", e);
                }

                let _ = sender.send(new_entries);
            });

            let entries_recv = entries_clone.clone();
            let render_recv = render_clone.clone();
            let spinner_recv = spinner_clone.clone();

            gtk4::glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                match receiver.try_recv() {
                    Ok(new_entries) => {
                        *entries_recv.borrow_mut() = new_entries;
                        render_recv();
                        spinner_recv.set_spinning(false);
                        spinner_recv.set_visible(false);
                        gtk4::glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        spinner_recv.set_spinning(false);
                        spinner_recv.set_visible(false);
                        gtk4::glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        gtk4::glib::ControlFlow::Continue
                    }
                }
            });
        }
    });

    let entries_clone2 = displayed_mail_entries.clone();
    let current_entries_for_read = current_mail_entries.clone();
    let text_buffer_clone2 = text_buffer.clone();
    let lbl_subj_clone = lbl_viewer_subj.clone();
    let lbl_date_clone = lbl_viewer_date.clone();
    let lbl_return_clone = lbl_viewer_return.clone();
    let btn_reply_clone2 = btn_reply.clone();
    let btn_archive_clone2 = btn_archive.clone();
    let selected_mail_clone = selected_mail.clone();

    // --- EVENT: Zeile ausgewählt ---
    mail_list.connect_row_selected(move |_, row_opt| {
        if let Some(row) = row_opt {
            let idx = row.index() as usize;

            let mut file_path_to_read = None;

            if let Some(entry) = entries_clone2.borrow_mut().get_mut(idx) {
                lbl_subj_clone.set_label(&format!("<b><span size='large'>{}</span></b>", gtk4::glib::markup_escape_text(&entry.subject)));
                lbl_date_clone.set_label(&entry.date_full);
                lbl_return_clone.set_label(&format!("Return-Path: {}", entry.return_path));

                *selected_mail_clone.borrow_mut() = Some(entry.clone());

                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
                let age_secs = now - entry.timestamp;
                let required_age = 24 * 60 * 60;

                if age_secs >= required_age {
                    btn_reply_clone2.set_sensitive(true);
                    btn_reply_clone2.set_tooltip_text(Some("Antworten"));
                } else {
                    btn_reply_clone2.set_sensitive(false);
                    let hours_left = 24 - (age_secs / 3600);
                    btn_reply_clone2.set_tooltip_text(Some(&format!("Antworten (erst in {}h möglich)", hours_left)));
                }

                btn_archive_clone2.set_sensitive(true);

                if !entry.is_read {
                    entry.is_read = true;
                    let old_path = entry.path.clone();

                    let file_name = old_path.file_name().unwrap().to_string_lossy().to_string();
                    let mut new_file_name = file_name.clone();

                    if !new_file_name.contains(":2,") {
                        new_file_name.push_str(":2,S");
                    } else if !new_file_name.contains('S') {
                        new_file_name.push('S');
                    }

                    let parent = old_path.parent().unwrap();
                    let new_path = if parent.ends_with("new") {
                        parent.parent().unwrap().join("cur").join(&new_file_name)
                    } else {
                        parent.join(&new_file_name)
                    };

                    if old_path != new_path {
                        if let Err(e) = std::fs::rename(&old_path, &new_path) {
                            eprintln!("Konnte Mail nicht als gelesen markieren: {}", e);
                        } else {
                            entry.path = new_path.clone();

                            if let Some(global_entry) = current_entries_for_read.borrow_mut().iter_mut().find(|e| e.path == old_path) {
                                global_entry.is_read = true;
                                global_entry.path = new_path;
                            }
                        }
                    }

                    if let Some(child) = row.child() {
                        if let Some(hbox) = child.downcast_ref::<gtk4::Box>() {
                            let mut curr = hbox.first_child();
                            while let Some(w) = curr {
                                w.remove_css_class("unread");
                                curr = w.next_sibling();
                            }
                        }
                    }
                }

                file_path_to_read = Some(entry.path.clone());
            }

            if let Some(path) = file_path_to_read {
                if let Ok(data) = std::fs::read(&path) {
                    if let Ok(parsed) = mailparse::parse_mail(&data) {
                        let body = extract_best_body(&parsed);
                        text_buffer_clone2.set_text(&body);
                    }
                }
            }
        }
    });

    let app_clone1 = app.clone();
    btn_new_mail.connect_clicked(move |_| {
        composer::open_composer_window(&app_clone1, None, None, None);
    });

    let app_clone2 = app.clone();
    let selected_mail_for_reply = selected_mail.clone();
    let text_buffer_for_reply = text_buffer.clone();

    btn_reply.connect_clicked(move |_| {
        if let Some(ref mail) = *selected_mail_for_reply.borrow() {
            let to = &mail.return_path;
            let mut subj = mail.subject.clone();
            if !subj.to_lowercase().starts_with("re:") {
                subj = format!("Re: {}", subj);
            }

            let (mut start, mut end) = text_buffer_for_reply.bounds();
            if let Some((s, e)) = text_buffer_for_reply.selection_bounds() {
                start = s;
                end = e;
            }

            let raw_text = text_buffer_for_reply.text(&start, &end, false);

            let mut quote = format!("--- Am {} schrieb {} :\n", mail.date_full, mail.from);
            for line in raw_text.lines() {
                quote.push_str("> ");
                quote.push_str(line);
                quote.push('\n');
            }
            quote.push('\n');

            composer::open_composer_window(&app_clone2, Some(to), Some(&subj), Some(&quote));
        }
    });

    // --- NEU: Archivieren-Logik (Panic-Fix) ---
    let do_archive = {
        let selected = selected_mail.clone();
        let all_entries = current_mail_entries.clone();
        let render = do_sort_and_render.clone();
        let text_buf = text_buffer.clone();
        let list_box = mail_list.clone();
        let btn_archive_state = btn_archive.clone();
        let btn_reply_state = btn_reply.clone();

        Rc::new(move || {
            let entry_opt = selected.borrow().clone(); // Clone entkoppelt Borrow

            if let Some(entry) = entry_opt {
                let archive_dir = dirs::home_dir().unwrap().join(".Mail").join("Archive").join("cur");

                if let Some(fname) = entry.path.file_name() {
                    let new_path = archive_dir.join(fname);

                    if entry.path != new_path {
                        if std::fs::rename(&entry.path, &new_path).is_ok() {
                            let current_idx = list_box.selected_row().map(|r| r.index()).unwrap_or(-1);

                            all_entries.borrow_mut().retain(|e| e.path != entry.path);
                            render();

                            if current_idx >= 0 {
                                if let Some(next_row) = list_box.row_at_index(current_idx) {
                                    list_box.select_row(Some(&next_row));
                                    next_row.grab_focus();
                                } else if let Some(prev_row) = list_box.row_at_index(current_idx - 1) {
                                    list_box.select_row(Some(&prev_row));
                                    prev_row.grab_focus();
                                } else {
                                    text_buf.set_text("");
                                    *selected.borrow_mut() = None;
                                    btn_archive_state.set_sensitive(false);
                                    btn_reply_state.set_sensitive(false);
                                }
                            }
                        }
                    }
                }
            }
        })
    };

    let archive_click_clone = do_archive.clone();
    btn_archive.connect_clicked(move |_| {
        archive_click_clone();
    });

    let right_pane = Paned::builder()
        .orientation(Orientation::Vertical)
        .start_child(&mail_list_vbox)
        .end_child(&viewer_vbox)
        .build();
    right_pane.set_position(250);

    let main_pane = Paned::builder()
        .orientation(Orientation::Horizontal)
        .start_child(&folder_scroll)
        .end_child(&right_pane)
        .build();
    main_pane.set_position(200);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("nox")
        .default_width(1200)
        .default_height(800)
        .child(&main_pane)
        .build();

    let key_controller = gtk4::EventControllerKey::new();
    let list_nav = mail_list.clone();
    let archive_shortcut_clone = do_archive.clone();
    let btn_search_shortcut = btn_search.clone(); // NEU für '/' und ESC Shortcut

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
            gdk::Key::a => {
                archive_shortcut_clone();
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::slash => {
                // NEU: '/' öffnet die Suche
                btn_search_shortcut.set_active(true);
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::Escape => {
                // NEU: ESC schließt die Suche (auch wenn Fokus woanders liegt)
                if btn_search_shortcut.is_active() {
                    btn_search_shortcut.set_active(false);
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

    window.set_titlebar(Some(&header_bar));
    window.present();
}

fn get_maildir_folders() -> Vec<String> {
    let mut folders = Vec::new();
    if let Some(mut path) = dirs::home_dir() {
        path.push(".Mail");
        
        let archive_path = path.join("Archive");
        if !archive_path.exists() {
            let _ = fs::create_dir_all(archive_path.join("cur"));
            let _ = fs::create_dir_all(archive_path.join("new"));
            let _ = fs::create_dir_all(archive_path.join("tmp"));
        }

        // NEU: Outbox Ordner garantieren
        let outbox_path = path.join("Outbox");
        if !outbox_path.exists() {
            let _ = fs::create_dir_all(outbox_path.join("cur"));
            let _ = fs::create_dir_all(outbox_path.join("new"));
            let _ = fs::create_dir_all(outbox_path.join("tmp"));
        }

        if path.join("cur").exists() {
            folders.push("INBOX".to_string());
        } else if let Ok(entries) = fs::read_dir(&path) {
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
    if folders.is_empty() {
        folders.push("INBOX".to_string());
    } else {
        folders.sort();
    }
    folders
}

fn extract_best_body(parsed: &mailparse::ParsedMail) -> String {
    let content_type = parsed.get_headers().get_first_value("Content-Type").unwrap_or_default().to_lowercase();
    if parsed.subparts.is_empty() {
        let body = parsed.get_body().unwrap_or_default();
        if content_type.contains("text/html") { return strip_html_tags(&body); }
        return body;
    }
    for part in &parsed.subparts {
        let ct = part.get_headers().get_first_value("Content-Type").unwrap_or_default().to_lowercase();
        if ct.contains("text/plain") { return part.get_body().unwrap_or_default(); }
    }
    for part in &parsed.subparts {
        let ct = part.get_headers().get_first_value("Content-Type").unwrap_or_default().to_lowercase();
        if ct.contains("text/html") { return strip_html_tags(&part.get_body().unwrap_or_default()); }
    }
    for part in &parsed.subparts {
        let text = extract_best_body(part);
        if !text.is_empty() { return text; }
    }
    "Kein anzeigbarer Text gefunden.".to_string()
}

fn strip_html_tags(html: &str) -> String {
    let mut html_clean = html.to_string();

    for tag in ["head", "style", "script"] {
        let open_tag = format!("<{}", tag);
        let close_tag = format!("</{}>", tag);
        while let Some(start) = html_clean.to_lowercase().find(&open_tag) {
            if let Some(end_offset) = html_clean[start..].to_lowercase().find(&close_tag) {
                html_clean.replace_range(start..start + end_offset + close_tag.len(), "");
            } else {
                html_clean.replace_range(start.., "");
            }
        }
    }

    while let Some(start) = html_clean.find("<!--") {
        if let Some(end_offset) = html_clean[start..].find("-->") {
            html_clean.replace_range(start..start + end_offset + 3, "");
        } else {
            html_clean.replace_range(start.., "");
        }
    }

    let mut result = String::with_capacity(html_clean.len());
    let mut in_tag = false;
    let html_replaced = html_clean
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<p>", "\n\n")
        .replace("</p>", "\n");

    for c in html_replaced.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    result.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&").replace("&quot;", "\"").trim().to_string()
}
