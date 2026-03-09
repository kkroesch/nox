use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Button, HeaderBar, Label, ListBox, Orientation, Paned,
    ScrolledWindow, SearchEntry, SelectionMode, Spinner, TextView, ToggleButton,
};
use mailparse::MailHeaderMap;
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

mod addressbook;
mod composer;
mod db;
mod help;
mod status; // NEU

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
    list_unsubscribe: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum SortCol {
    Date,
    Sender,
    Subject,
}

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

fn move_mail_file(old_path: &PathBuf, target_folder: &str) -> Option<PathBuf> {
    let file_name = old_path.file_name()?;
    let mail_dir = dirs::home_dir()?.join(".Mail");

    let subfolder = if old_path.to_string_lossy().contains("/new/") {
        "new"
    } else {
        "cur"
    };
    let target_dir = mail_dir.join(target_folder).join(subfolder);

    if !target_dir.exists() {
        let _ = fs::create_dir_all(&target_dir);
    }

    let new_path = target_dir.join(file_name);
    if fs::rename(old_path, &new_path).is_ok() {
        Some(new_path)
    } else {
        None
    }
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

    let mail_list_vbox = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .build();
    let header_box = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .margin_start(10)
        .margin_end(10)
        .margin_top(5)
        .margin_bottom(5)
        .build();

    let btn_sort_date = Button::builder()
        .label("Datum")
        .width_request(130)
        .css_classes(["flat"])
        .build();
    let btn_sort_sender = Button::builder()
        .label("Absender")
        .width_request(200)
        .css_classes(["flat"])
        .build();
    let btn_sort_subject = Button::builder()
        .label("Betreff")
        .hexpand(true)
        .halign(gtk4::Align::Start)
        .css_classes(["flat"])
        .build();

    header_box.append(&btn_sort_date);
    header_box.append(&btn_sort_sender);
    header_box.append(&btn_sort_subject);

    let mail_list = ListBox::builder()
        .selection_mode(SelectionMode::Multiple)
        .build();

    // NEU: Klick-Verhalten fixen (Nur bei STRG/SHIFT mehrfach markieren)
    let click_gesture = gtk4::GestureClick::new();
    click_gesture.set_button(gdk::BUTTON_PRIMARY);
    let list_for_click = mail_list.clone();
    click_gesture.connect_pressed(move |gesture, n_press, _x, _y| {
        if n_press == 1 {
            let state = gesture.current_event_state();
            if !state.contains(gdk::ModifierType::CONTROL_MASK)
                && !state.contains(gdk::ModifierType::SHIFT_MASK)
            {
                // Bei einem normalen Klick löschen wir vorher alle Markierungen.
                // Das Markieren der exakt angeklickten Zeile macht GTK direkt im Anschluss selbst.
                list_for_click.unselect_all();
            }
        }
    });
    // Wichtig: Wir müssen den Klick abfangen, BEVOR die ListBox ihn verarbeitet
    click_gesture.set_propagation_phase(gtk4::PropagationPhase::Capture);
    mail_list.add_controller(click_gesture);

    let mail_scroll = ScrolledWindow::builder()
        .child(&mail_list)
        .vexpand(true)
        .build();

    mail_list_vbox.append(&header_box);
    mail_list_vbox.append(&gtk4::Separator::new(Orientation::Horizontal));
    mail_list_vbox.append(&mail_scroll);

    let viewer_vbox = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .build();
    let viewer_header_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .margin_start(15)
        .margin_end(15)
        .margin_top(15)
        .margin_bottom(15)
        .build();

    let lbl_viewer_subj = Label::builder()
        .halign(gtk4::Align::Start)
        .use_markup(true)
        .selectable(true)
        .build();
    let lbl_viewer_date = Label::builder()
        .halign(gtk4::Align::Start)
        .selectable(true)
        .build();
    let lbl_viewer_return = Label::builder()
        .halign(gtk4::Align::Start)
        .selectable(true)
        .css_classes(["dim-label"])
        .build();

    viewer_header_box.append(&lbl_viewer_subj);
    viewer_header_box.append(&lbl_viewer_date);
    viewer_header_box.append(&lbl_viewer_return);

    let btn_unsubscribe = Button::builder()
        .label("Abmelden (Unsubscribe)")
        .visible(false)
        .margin_top(5)
        .halign(gtk4::Align::Start)
        .css_classes(["suggested-action"])
        .build();
    viewer_header_box.append(&btn_unsubscribe);

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

    let viewer_scroll = ScrolledWindow::builder()
        .child(&mail_viewer)
        .vexpand(true)
        .build();

    viewer_vbox.append(&viewer_header_box);
    viewer_vbox.append(&gtk4::Separator::new(Orientation::Horizontal));
    viewer_vbox.append(&viewer_scroll);

    let current_mail_entries = Rc::new(RefCell::new(Vec::<MailEntry>::new()));
    let displayed_mail_entries = Rc::new(RefCell::new(Vec::<MailEntry>::new()));
    let sort_state = Rc::new(RefCell::new((SortCol::Date, true)));
    let selected_mail = Rc::new(RefCell::new(None::<MailEntry>));
    let current_search_query = Rc::new(RefCell::new(String::new()));

    let (status_box, status_label) = status::build_status_bar();
    let status_label_rc = Rc::new(status_label);

    let do_sort_and_render = {
        let list_box = mail_list.clone();
        let all_entries = current_mail_entries.clone();
        let disp_entries = displayed_mail_entries.clone();
        let state = sort_state.clone();
        let search_query = current_search_query.clone();
        let status_lbl = status_label_rc.clone();

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

            while let Some(child) = list_box.first_child() {
                list_box.remove(&child);
            }

            for entry in display_list.iter() {
                let hbox = gtk4::Box::builder()
                    .orientation(Orientation::Horizontal)
                    .spacing(10)
                    .margin_start(10)
                    .margin_end(10)
                    .margin_top(5)
                    .margin_bottom(5)
                    .build();

                let (name, email) = crate::db::parse_from(&entry.from);
                let display_from = if name.is_empty() { email } else { name };

                let lbl_date = Label::builder()
                    .label(&entry.date_short)
                    .xalign(0.0)
                    .width_request(130)
                    .build();
                let lbl_from = Label::builder()
                    .label(&display_from)
                    .xalign(0.0)
                    .width_request(200)
                    .max_width_chars(25)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .build();
                let lbl_subj = Label::builder()
                    .label(&entry.subject)
                    .xalign(0.0)
                    .hexpand(true)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .build();

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

            let all_ref = all_entries.borrow();
            let total = all_ref.len();
            let unread = all_ref.iter().filter(|e| !e.is_read).count();
            status_lbl.set_label(&format!("{} Mails, {} ungelesen", total, unread));
        })
    };

    let r1 = do_sort_and_render.clone();
    let st1 = sort_state.clone();
    btn_sort_date.connect_clicked(move |_| {
        let mut s = st1.borrow_mut();
        if s.0 == SortCol::Date {
            s.1 = !s.1;
        } else {
            s.0 = SortCol::Date;
            s.1 = true;
        }
        drop(s);
        r1();
    });

    let r2 = do_sort_and_render.clone();
    let st2 = sort_state.clone();
    btn_sort_sender.connect_clicked(move |_| {
        let mut s = st2.borrow_mut();
        if s.0 == SortCol::Sender {
            s.1 = !s.1;
        } else {
            s.0 = SortCol::Sender;
            s.1 = false;
        }
        drop(s);
        r2();
    });

    let r3 = do_sort_and_render.clone();
    let st3 = sort_state.clone();
    btn_sort_subject.connect_clicked(move |_| {
        let mut s = st3.borrow_mut();
        if s.0 == SortCol::Subject {
            s.1 = !s.1;
        } else {
            s.0 = SortCol::Subject;
            s.1 = false;
        }
        drop(s);
        r3();
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

    let search_entry = SearchEntry::builder()
        .visible(false)
        .width_request(250)
        .build();
    header_bar.pack_end(&search_entry);

    let btn_search = ToggleButton::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text("Suchen")
        .build();
    header_bar.pack_end(&btn_search);

    let btn_help = Button::from_icon_name("help-about-symbolic"); // NEU
    btn_help.set_tooltip_text(Some("Hilfe & Shortcuts (?)")); // NEU
    header_bar.pack_end(&btn_help); // NEU

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

    // NEU: Hilfe Klick Event
    let app_clone_help = app.clone();
    btn_help.connect_clicked(move |_| {
        help::show_help_window(&app_clone_help);
    });

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
                let maildir_path = if folder_name == "INBOX"
                    && dirs::home_dir().unwrap().join(".Mail/cur").exists()
                {
                    dirs::home_dir().unwrap().join(".Mail")
                } else {
                    dirs::home_dir().unwrap().join(".Mail").join(&folder_name)
                };

                let md = maildir::Maildir::from(maildir_path);
                let mut new_entries = Vec::new();
                let mut db_contacts: std::collections::HashMap<String, (String, Option<String>)> =
                    std::collections::HashMap::new();

                let mut verified_senders = std::collections::HashSet::new();
                if let Ok(conn) = rusqlite::Connection::open(db::db_path()) {
                    if let Ok(mut stmt) =
                        conn.prepare("SELECT email FROM contacts WHERE is_verified = 1")
                    {
                        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                            for email in rows.flatten() {
                                verified_senders.insert(email);
                            }
                        }
                    }
                }

                for entry in md.list_new().chain(md.list_cur()) {
                    if let Ok(mail) = entry {
                        let path = mail.path().to_path_buf();

                        let path_str = path.to_string_lossy();
                        let is_read = path_str.contains(":2,") && path_str.contains('S');

                        if let Ok(data) = std::fs::read(&path) {
                            if let Ok(parsed) = mailparse::parse_mail(&data) {
                                let headers = parsed.get_headers();
                                let subject = headers
                                    .get_first_value("Subject")
                                    .unwrap_or_else(|| "Kein Betreff".to_string());
                                let from = headers
                                    .get_first_value("From")
                                    .unwrap_or_else(|| "Unbekannt".to_string());
                                let return_path_raw = headers
                                    .get_first_value("Return-Path")
                                    .unwrap_or_else(|| "".to_string());
                                let date_str = headers.get_first_value("Date").unwrap_or_default();
                                let timestamp = mailparse::dateparse(&date_str).unwrap_or(0);

                                let mut pub_key = None;
                                if let Some(ac_val) = headers.get_first_value("Autocrypt") {
                                    if let Some(idx) = ac_val.find("keydata=") {
                                        pub_key = Some(ac_val[idx + 8..].trim().to_string());
                                    }
                                }

                                let mut list_unsubscribe = None;
                                if let Some(lu_val) = headers.get_first_value("List-Unsubscribe") {
                                    if let Some(start) = lu_val.find('<') {
                                        if let Some(end) = lu_val[start..].find('>') {
                                            list_unsubscribe =
                                                Some(lu_val[start + 1..start + end].to_string());
                                        }
                                    }
                                }

                                let date_short = gtk4::glib::DateTime::from_unix_local(timestamp)
                                    .map(|dt| {
                                        dt.format("%d.%m.%y %H:%M")
                                            .unwrap_or(date_str.clone().into())
                                            .to_string()
                                    })
                                    .unwrap_or_else(|_| date_str.clone());

                                let (name, email) = db::parse_from(&from);

                                let mut return_path_clean =
                                    return_path_raw.replace(['<', '>'], "").trim().to_string();
                                if return_path_clean.is_empty() {
                                    return_path_clean = email.clone();
                                }

                                if folder_name == "INBOX"
                                    && !verified_senders.contains(&return_path_clean)
                                {
                                    if move_mail_file(&path, "Quarantäne").is_some() {
                                        continue;
                                    }
                                }

                                if !email.is_empty() {
                                    let entry = db_contacts
                                        .entry(email)
                                        .or_insert_with(|| (name.clone(), pub_key.clone()));
                                    if entry.0.is_empty() && !name.is_empty() {
                                        entry.0 = name;
                                    }
                                    if entry.1.is_none() && pub_key.is_some() {
                                        entry.1 = pub_key;
                                    }
                                }

                                new_entries.push(MailEntry {
                                    path,
                                    timestamp,
                                    date_short,
                                    date_full: date_str,
                                    from,
                                    return_path: return_path_clean,
                                    subject,
                                    is_read,
                                    list_unsubscribe,
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
                    Err(std::sync::mpsc::TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
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
    let btn_unsubscribe_clone = btn_unsubscribe.clone();
    let status_lbl_read = status_label_rc.clone();

    // ÄNDERUNG: Auf Änderung der Mehrfachauswahl reagieren statt nur auf Einzel-Klick
    mail_list.connect_selected_rows_changed(move |list| {
        let selected_rows = list.selected_rows();

        if selected_rows.len() == 1 {
            let idx = selected_rows[0].index() as usize;
            let mut file_path_to_read = None;

            if let Some(entry) = entries_clone2.borrow_mut().get_mut(idx) {
                lbl_subj_clone.set_label(&format!(
                    "<b><span size='large'>{}</span></b>",
                    gtk4::glib::markup_escape_text(&entry.subject)
                ));
                lbl_date_clone.set_label(&entry.date_full);
                lbl_return_clone.set_label(&format!("Return-Path: {}", entry.return_path));

                if entry.list_unsubscribe.is_some() {
                    btn_unsubscribe_clone.set_visible(true);
                } else {
                    btn_unsubscribe_clone.set_visible(false);
                }

                *selected_mail_clone.borrow_mut() = Some(entry.clone());

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                let age_secs = now - entry.timestamp;
                let required_age = 24 * 60 * 60;

                if age_secs >= required_age {
                    btn_reply_clone2.set_sensitive(true);
                    btn_reply_clone2.set_tooltip_text(Some("Antworten"));
                } else {
                    btn_reply_clone2.set_sensitive(false);
                    let hours_left = 24 - (age_secs / 3600);
                    btn_reply_clone2.set_tooltip_text(Some(&format!(
                        "Antworten (erst in {}h möglich)",
                        hours_left
                    )));
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

                            if let Some(global_entry) = current_entries_for_read
                                .borrow_mut()
                                .iter_mut()
                                .find(|e| e.path == old_path)
                            {
                                global_entry.is_read = true;
                                global_entry.path = new_path;
                            }

                            let all_ref = current_entries_for_read.borrow();
                            let total = all_ref.len();
                            let unread = all_ref.iter().filter(|e| !e.is_read).count();
                            status_lbl_read
                                .set_label(&format!("{} Mails, {} ungelesen", total, unread));
                        }
                    }

                    if let Some(child) = selected_rows[0].child() {
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
        } else if selected_rows.len() > 1 {
            // Zeige Hinweis bei Mehrfachauswahl an
            lbl_subj_clone.set_label(&format!(
                "<b><span size='large'>{} Mails ausgewählt</span></b>",
                selected_rows.len()
            ));
            lbl_date_clone.set_label("");
            lbl_return_clone.set_label("");
            text_buffer_clone2.set_text(
                "Massenaktion (Archivieren, Löschen, Verschieben, Verifizieren) ist möglich.",
            );
            btn_unsubscribe_clone.set_visible(false);
            btn_reply_clone2.set_sensitive(false);
            btn_archive_clone2.set_sensitive(true);
            *selected_mail_clone.borrow_mut() = None;
        } else {
            // Nichts ausgewählt
            lbl_subj_clone.set_label("");
            lbl_date_clone.set_label("");
            lbl_return_clone.set_label("");
            text_buffer_clone2.set_text("");
            btn_unsubscribe_clone.set_visible(false);
            btn_reply_clone2.set_sensitive(false);
            btn_archive_clone2.set_sensitive(false);
            *selected_mail_clone.borrow_mut() = None;
        }
    });

    let app_clone1 = app.clone();
    btn_new_mail.connect_clicked(move |_| {
        composer::open_composer_window(&app_clone1, None, None, None);
    });

    let selected_mail_for_unsub = selected_mail.clone();
    let app_clone_unsub = app.clone();
    btn_unsubscribe.connect_clicked(move |_| {
        if let Some(ref mail) = *selected_mail_for_unsub.borrow() {
            if let Some(ref link) = mail.list_unsubscribe {
                if link.starts_with("mailto:") {
                    let to_clean = link
                        .trim_start_matches("mailto:")
                        .split('?')
                        .next()
                        .unwrap_or("")
                        .to_string();
                    composer::open_composer_window(
                        &app_clone_unsub,
                        Some(&to_clean),
                        Some("Unsubscribe"),
                        None,
                    );
                } else if link.starts_with("http") {
                    if let Err(e) = gtk4::gio::AppInfo::launch_default_for_uri(
                        link,
                        None::<&gtk4::gio::AppLaunchContext>,
                    ) {
                        eprintln!("Fehler beim Öffnen des Links: {}", e);
                    }
                }
            }
        }
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

    // ÄNDERUNG: Schleifen über alle gewählten Reihen für die Aktionen
    let do_archive = {
        let disp_entries = displayed_mail_entries.clone(); // NEU: Zugriff auf Anzeige-Liste nötig
        let all_entries = current_mail_entries.clone();
        let render = do_sort_and_render.clone();
        let text_buf = text_buffer.clone();
        let list_box = mail_list.clone();
        let btn_archive_state = btn_archive.clone();
        let btn_reply_state = btn_reply.clone();
        let selected_mail_state = selected_mail.clone();

        Rc::new(move || {
            let rows = list_box.selected_rows();
            if rows.is_empty() {
                return;
            }

            let mut paths_to_remove = Vec::new();
            let disp = disp_entries.borrow();

            for row in &rows {
                let idx = row.index() as usize;
                if let Some(entry) = disp.get(idx) {
                    if let Some(_new_path) = move_mail_file(&entry.path, "Archive") {
                        paths_to_remove.push(entry.path.clone());
                    }
                }
            }

            if !paths_to_remove.is_empty() {
                all_entries
                    .borrow_mut()
                    .retain(|e| !paths_to_remove.contains(&e.path));
                render();
                text_buf.set_text("");
                *selected_mail_state.borrow_mut() = None;
                btn_archive_state.set_sensitive(false);
                btn_reply_state.set_sensitive(false);
            }
        })
    };

    let archive_click_clone = do_archive.clone();
    btn_archive.connect_clicked(move |_| {
        archive_click_clone();
    });

    let do_trash = {
        let disp_entries = displayed_mail_entries.clone();
        let all_entries = current_mail_entries.clone();
        let render = do_sort_and_render.clone();
        let text_buf = text_buffer.clone();
        let list_box = mail_list.clone();
        let btn_archive_state = btn_archive.clone();
        let btn_reply_state = btn_reply.clone();
        let selected_mail_state = selected_mail.clone();

        Rc::new(move || {
            let rows = list_box.selected_rows();
            if rows.is_empty() {
                return;
            }

            let mut paths_to_remove = Vec::new();
            let disp = disp_entries.borrow();

            for row in &rows {
                let idx = row.index() as usize;
                if let Some(entry) = disp.get(idx) {
                    if let Some(_new_path) = move_mail_file(&entry.path, "TRASH") {
                        paths_to_remove.push(entry.path.clone());
                    }
                }
            }

            if !paths_to_remove.is_empty() {
                all_entries
                    .borrow_mut()
                    .retain(|e| !paths_to_remove.contains(&e.path));
                render();
                text_buf.set_text("");
                *selected_mail_state.borrow_mut() = None;
                btn_archive_state.set_sensitive(false);
                btn_reply_state.set_sensitive(false);
            }
        })
    };

    let do_toggle_verify = {
        let disp_entries = displayed_mail_entries.clone();
        let all_entries = current_mail_entries.clone();
        let render = do_sort_and_render.clone();
        let text_buf = text_buffer.clone();
        let list_box = mail_list.clone();
        let btn_archive_state = btn_archive.clone();
        let btn_reply_state = btn_reply.clone();
        let selected_mail_state = selected_mail.clone();

        Rc::new(move || {
            let rows = list_box.selected_rows();
            if rows.is_empty() {
                return;
            }

            let mut paths_to_remove = Vec::new();
            let disp = disp_entries.borrow();

            for row in &rows {
                let idx = row.index() as usize;
                if let Some(entry) = disp.get(idx) {
                    if let Ok(new_status) = db::toggle_verify_contact(&entry.return_path) {
                        let target_folder = if new_status { "INBOX" } else { "Quarantäne" };
                        if let Some(_new_path) = move_mail_file(&entry.path, target_folder) {
                            paths_to_remove.push(entry.path.clone());
                        }
                    }
                }
            }

            if !paths_to_remove.is_empty() {
                all_entries
                    .borrow_mut()
                    .retain(|e| !paths_to_remove.contains(&e.path));
                render();
                text_buf.set_text("");
                *selected_mail_state.borrow_mut() = None;
                btn_archive_state.set_sensitive(false);
                btn_reply_state.set_sensitive(false);
            }
        })
    };

    let do_move_interactive = {
        let disp_entries = displayed_mail_entries.clone();
        let all_entries = current_mail_entries.clone();
        let render = do_sort_and_render.clone();
        let text_buf = text_buffer.clone();
        let list_box = mail_list.clone();
        let btn_archive_state = btn_archive.clone();
        let btn_reply_state = btn_reply.clone();
        let selected_mail_state = selected_mail.clone();

        Rc::new(move || {
            let rows = list_box.selected_rows();
            if rows.is_empty() {
                return;
            }
            let first_row = &rows[0]; // Das Popover wird am ersten selektierten Element verankert

            let popover = gtk4::Popover::builder()
                .position(gtk4::PositionType::Bottom)
                .build();
            popover.set_parent(first_row);

            let folder_list = ListBox::builder()
                .selection_mode(SelectionMode::Single)
                .build();

            let folders = get_maildir_folders();
            for folder in &folders {
                let lbl = Label::builder()
                    .label(folder)
                    .margin_top(5)
                    .margin_bottom(5)
                    .margin_start(10)
                    .margin_end(10)
                    .halign(gtk4::Align::Start)
                    .build();
                folder_list.append(&lbl);
            }

            let scroll = ScrolledWindow::builder()
                .child(&folder_list)
                .max_content_height(300)
                .propagate_natural_height(true)
                .hscrollbar_policy(gtk4::PolicyType::Never)
                .build();

            popover.set_child(Some(&scroll));

            let popover_rc = Rc::new(popover);
            let p_clone1 = popover_rc.clone();
            let list_box_focus = list_box.clone();

            popover_rc.connect_closed(move |p| {
                p.unparent();
                list_box_focus.grab_focus();
            });

            let key_ctrl = gtk4::EventControllerKey::new();
            let fl_clone = folder_list.clone();
            key_ctrl.connect_key_pressed(move |_, keyval, _, _| {
                let idx = fl_clone.selected_row().map(|r| r.index()).unwrap_or(-1);
                match keyval {
                    gdk::Key::j => {
                        if let Some(r) = fl_clone.row_at_index(idx + 1) {
                            fl_clone.select_row(Some(&r));
                            r.grab_focus();
                        }
                        gtk4::glib::Propagation::Stop
                    }
                    gdk::Key::k => {
                        if idx > 0 {
                            if let Some(r) = fl_clone.row_at_index(idx - 1) {
                                fl_clone.select_row(Some(&r));
                                r.grab_focus();
                            }
                        }
                        gtk4::glib::Propagation::Stop
                    }
                    _ => gtk4::glib::Propagation::Proceed,
                }
            });
            folder_list.add_controller(key_ctrl);

            // Sammle die Einträge für das spätere Verschieben
            let mut entries_to_move = Vec::new();
            let disp = disp_entries.borrow();
            for r in &rows {
                if let Some(entry) = disp.get(r.index() as usize) {
                    entries_to_move.push(entry.clone());
                }
            }

            let all_entries_c = all_entries.clone();
            let render_c = render.clone();
            let text_buf_c = text_buf.clone();
            let btn_arc_c = btn_archive_state.clone();
            let btn_rep_c = btn_reply_state.clone();
            let sel_mail_c = selected_mail_state.clone();
            let list_box_c = list_box.clone();

            folder_list.connect_row_activated(move |_, f_row| {
                if let Some(child) = f_row.child() {
                    if let Ok(lbl) = child.downcast::<Label>() {
                        let target_folder = lbl.label().to_string();
                        let mut paths_to_remove = Vec::new();

                        for entry in &entries_to_move {
                            if let Some(_) = move_mail_file(&entry.path, &target_folder) {
                                paths_to_remove.push(entry.path.clone());
                            }
                        }

                        if !paths_to_remove.is_empty() {
                            all_entries_c
                                .borrow_mut()
                                .retain(|e| !paths_to_remove.contains(&e.path));
                            render_c();
                            text_buf_c.set_text("");
                            *sel_mail_c.borrow_mut() = None;
                            btn_arc_c.set_sensitive(false);
                            btn_rep_c.set_sensitive(false);
                            list_box_c.grab_focus();
                        }
                    }
                }
                p_clone1.popdown();
            });

            popover_rc.popup();
            if let Some(first_f_row) = folder_list.row_at_index(0) {
                folder_list.select_row(Some(&first_f_row));
                first_f_row.grab_focus();
            }
        })
    };

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
    main_pane.set_vexpand(true);

    let root_vbox = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .build();
    root_vbox.append(&main_pane);
    root_vbox.append(&gtk4::Separator::new(Orientation::Horizontal));
    root_vbox.append(&status_box);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("nox")
        .default_width(1200)
        .default_height(800)
        .child(&root_vbox)
        .build();

    let key_controller = gtk4::EventControllerKey::new();
    let list_nav = mail_list.clone();
    let archive_shortcut_clone = do_archive.clone();
    let verify_shortcut_clone = do_toggle_verify.clone();
    let move_interactive_shortcut_clone = do_move_interactive.clone();
    let trash_shortcut_clone = do_trash.clone();
    let btn_search_shortcut = btn_search.clone();
    let app_clone_help_key = app.clone();

    // ÄNDERUNG: Ctrl+A / Shift usw. dürfen nicht von uns verschluckt werden
    key_controller.connect_key_pressed(move |_, keyval, _, state| {
        // Ignoriere unsere Einzel-Buchstaben-Shortcuts, falls Ctrl, Alt oder Super gedrückt ist (z.B. für Ctrl+A)
        let mods = state.intersection(
            gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK
                | gdk::ModifierType::META_MASK,
        );
        if !mods.is_empty() {
            return gtk4::glib::Propagation::Proceed;
        }

        let selected = list_nav.selected_rows();
        let current_idx = if !selected.is_empty() {
            selected.last().unwrap().index()
        } else {
            -1
        };

        match keyval {
            gdk::Key::j => {
                if let Some(row) = list_nav.row_at_index(current_idx + 1) {
                    list_nav.unselect_all(); // NEU: Auswahl leeren
                    list_nav.select_row(Some(&row));
                    row.grab_focus();
                }
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::k => {
                if current_idx > 0 {
                    if let Some(row) = list_nav.row_at_index(current_idx - 1) {
                        list_nav.unselect_all(); // NEU: Auswahl leeren
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
            gdk::Key::d => {
                // NEU: Shortcut d für Trash
                trash_shortcut_clone();
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::v => {
                verify_shortcut_clone();
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::m => {
                // NEU: Shortcut m
                move_interactive_shortcut_clone();
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::slash => {
                btn_search_shortcut.set_active(true);
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::question => {
                // NEU: Shortcut ? für Hilfe
                help::show_help_window(&app_clone_help_key);
                gtk4::glib::Propagation::Stop
            }
            gdk::Key::Escape => {
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

        let outbox_path = path.join("Outbox");
        if !outbox_path.exists() {
            let _ = fs::create_dir_all(outbox_path.join("cur"));
            let _ = fs::create_dir_all(outbox_path.join("new"));
            let _ = fs::create_dir_all(outbox_path.join("tmp"));
        }

        let quarantine_path = path.join("Quarantäne");
        if !quarantine_path.exists() {
            let _ = fs::create_dir_all(quarantine_path.join("cur"));
            let _ = fs::create_dir_all(quarantine_path.join("new"));
            let _ = fs::create_dir_all(quarantine_path.join("tmp"));
        }

        // NEU: TRASH Ordner garantieren
        let trash_path = path.join("TRASH");
        if !trash_path.exists() {
            let _ = fs::create_dir_all(trash_path.join("cur"));
            let _ = fs::create_dir_all(trash_path.join("new"));
            let _ = fs::create_dir_all(trash_path.join("tmp"));
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
    let content_type = parsed
        .get_headers()
        .get_first_value("Content-Type")
        .unwrap_or_default()
        .to_lowercase();
    if parsed.subparts.is_empty() {
        let body = parsed.get_body().unwrap_or_default();
        if content_type.contains("text/html") {
            return strip_html_tags(&body);
        }
        return body;
    }
    for part in &parsed.subparts {
        let ct = part
            .get_headers()
            .get_first_value("Content-Type")
            .unwrap_or_default()
            .to_lowercase();
        if ct.contains("text/plain") {
            return part.get_body().unwrap_or_default();
        }
    }
    for part in &parsed.subparts {
        let ct = part
            .get_headers()
            .get_first_value("Content-Type")
            .unwrap_or_default()
            .to_lowercase();
        if ct.contains("text/html") {
            return strip_html_tags(&part.get_body().unwrap_or_default());
        }
    }
    for part in &parsed.subparts {
        let text = extract_best_body(part);
        if !text.is_empty() {
            return text;
        }
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

    result
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .trim()
        .to_string()
}
