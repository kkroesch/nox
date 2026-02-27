# noxmail

A minimalist, fast, and keyboard-driven email client written in Rust and GTK4.

Designed for local Maildir setups and Linux power users.

## Features

- **Maildir First:** Reads directly from `~/.Mail/`. No built-in IMAP/POP3 sync – use `mbsync` or `offlineimap`.
    
- **Vim-like Keybindings:** Navigate and manage emails without touching the mouse (`j`, `k`, `a`, `v`).
    
- **Quarantine System:** Emails from unknown senders are automatically moved to a `Quarantine` folder.
    
- **Contact Management:** SQLite-based address book (`~/.noxmail.db`) with automatic contact harvesting.
    
- **Plain Text Focus:** Strips HTML to read emails safely and quickly.
    
- **Outbox Spooling:** Writes outgoing mails to `~/.Mail/Outbox/new/` for external MTAs (like `msmtp`) to process.
    

## Prerequisites

- Rust / Cargo
    
- Development libraries: GTK4 and SQLite (`gtk4-devel sqlite-devel` on Fedora, `gtk4 sqlite` on macOS via Homebrew)
    
- CLI tools for mail sync and routing: `isync` (provides `mbsync`) and `msmtp`
    
- A local Maildir setup at `~/.Mail/`
    

## Build & Run

I prefer [`just`](https://github.com/casey/just "null") over `make`. If you have `just` installed, you can easily build and run the app.

```
# Clone the repository
git clone [https://github.com/yourusername/noxmail.git](https://github.com/yourusername/noxmail.git)
cd noxmail

# Build and run using cargo...
cargo run --release

# ...or if you created a Justfile
just run
```

## Keybindings

### Main Window

|   |   |
|---|---|
|**Key**|**Action**|
|`j` / `k`|Move selection down / up|
|`a`|Archive selected email (moves to `~/.Mail/Archive/`)|
|`v`|Toggle Verification (moves mail between `INBOX` and `Quarantine`, updates DB)|
|`/`|Focus search bar|
|`Esc`|Clear search and return focus to mail list|

### Address Book

|   |   |
|---|---|
|**Key**|**Action**|
|`j` / `k`|Move selection down / up|
|`Enter`|Compose new email to selected contact|
|`v`|Toggle verified status of the contact|
|`r`|Rename contact (inline edit)|
|`d`|Hide/Delete contact|
|`/`|Focus search bar|
|`Esc`|Close search or close address book window|

## Directory Structure

`noxmail` expects and automatically manages the following structure:

- `~/.Mail/INBOX/` (or root `cur`/`new`)
    
- `~/.Mail/Archive/`
    
- `~/.Mail/Quarantäne/`
    
- `~/.Mail/Outbox/`
    
- `~/.noxmail.db` (SQLite database for contacts)
    

## Sending Emails

`noxmail` does not send emails directly. The composer creates standard RFC 2822 formatted text files in `~/.Mail/Outbox/new/`. You need to set up a background worker or cronjob using `msmtp`, `sendmail`, or a similar tool to watch this folder and dispatch the files.

## Author

Read more on my blog at [kroesch.ch](https://kroesch.ch/).
