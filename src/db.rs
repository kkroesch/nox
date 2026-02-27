use rusqlite::{Connection, Result};
use std::collections::HashMap;
use std::path::PathBuf;

pub fn db_path() -> PathBuf {
    dirs::home_dir().unwrap().join(".noxmail.db")
}

pub fn init_db() -> Result<()> {
    let conn = Connection::open(db_path())?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS contacts (
            id INTEGER PRIMARY KEY,
            name TEXT,
            email TEXT UNIQUE NOT NULL,
            is_verified BOOLEAN DEFAULT 0,
            pub_key TEXT
        )",
        [],
    )?;

    // Automatische Migration: Fügt die Spalte hinzu, ignoriert den Fehler, falls sie schon existiert
    let _ = conn.execute(
        "ALTER TABLE contacts ADD COLUMN is_hidden BOOLEAN DEFAULT 0",
        [],
    );

    Ok(())
}

pub fn bulk_upsert(contacts: &HashMap<String, String>) -> Result<()> {
    let mut conn = Connection::open(db_path())?;
    let tx = conn.transaction()?;

    {
        let mut stmt = tx.prepare(
            "INSERT INTO contacts (name, email) VALUES (?1, ?2)
             ON CONFLICT(email) DO UPDATE SET name=excluded.name WHERE contacts.name = '' OR contacts.name IS NULL"
        )?;

        for (email, name) in contacts {
            stmt.execute([name, email])?;
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn get_all_contacts() -> Result<Vec<(String, String, bool, bool)>> {
    let conn = Connection::open(db_path())?;
    // Nur Kontakte laden, die nicht versteckt sind
    let mut stmt = conn.prepare("SELECT name, email, is_verified, pub_key FROM contacts WHERE is_hidden = 0 OR is_hidden IS NULL ORDER BY name COLLATE NOCASE ASC, email ASC")?;
    let contact_iter = stmt.query_map([], |row| {
        let name: Option<String> = row.get(0)?;
        let email: String = row.get(1)?;
        let is_verified: bool = row.get(2)?;
        let pub_key: Option<String> = row.get(3)?;

        // Prüfen, ob ein Key existiert und nicht leer ist
        let has_pub_key = pub_key.is_some() && !pub_key.as_ref().unwrap().is_empty();

        Ok((name.unwrap_or_default(), email, is_verified, has_pub_key))
    })?;

    let mut contacts = Vec::new();
    for contact in contact_iter {
        contacts.push(contact?);
    }
    Ok(contacts)
}

// Ersetzt delete_contact
pub fn hide_contact(email: &str) -> Result<()> {
    let conn = Connection::open(db_path())?;
    conn.execute(
        "UPDATE contacts SET is_hidden = 1 WHERE email = ?1",
        [email],
    )?;
    Ok(())
}

pub fn verify_contact(email: &str) -> Result<()> {
    let conn = Connection::open(db_path())?;
    conn.execute(
        "UPDATE contacts SET is_verified = 1 WHERE email = ?1",
        [email],
    )?;
    Ok(())
}

// NEU: Namen in der DB aktualisieren
pub fn update_contact_name(email: &str, new_name: &str) -> Result<()> {
    let conn = Connection::open(db_path())?;
    conn.execute(
        "UPDATE contacts SET name = ?1 WHERE email = ?2",
        [new_name, email],
    )?;
    Ok(())
}

pub fn parse_from(from: &str) -> (String, String) {
    if let Some(start) = from.find('<') {
        if let Some(end) = from.find('>') {
            let name = from[..start].replace('"', "").trim().to_string();
            let email = from[start + 1..end].trim().to_string();
            return (name, email);
        }
    }
    ("".to_string(), from.trim().to_string())
}
