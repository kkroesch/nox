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

// GEÄNDERT: Gibt jetzt auch is_verified zurück
pub fn get_all_contacts() -> Result<Vec<(String, String, bool)>> {
    let conn = Connection::open(db_path())?;
    let mut stmt = conn.prepare(
        "SELECT name, email, is_verified FROM contacts ORDER BY name COLLATE NOCASE ASC, email ASC",
    )?;
    let contact_iter = stmt.query_map([], |row| {
        let name: Option<String> = row.get(0)?;
        let email: String = row.get(1)?;
        let is_verified: bool = row.get(2)?;
        Ok((name.unwrap_or_default(), email, is_verified))
    })?;

    let mut contacts = Vec::new();
    for contact in contact_iter {
        contacts.push(contact?);
    }
    Ok(contacts)
}

// NEU: Kontakt löschen
pub fn delete_contact(email: &str) -> Result<()> {
    let conn = Connection::open(db_path())?;
    conn.execute("DELETE FROM contacts WHERE email = ?1", [email])?;
    Ok(())
}

// NEU: Kontakt verifizieren
pub fn verify_contact(email: &str) -> Result<()> {
    let conn = Connection::open(db_path())?;
    conn.execute(
        "UPDATE contacts SET is_verified = 1 WHERE email = ?1",
        [email],
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
