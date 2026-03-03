#!/bin/bash

OUTBOX_NEW="$HOME/.Mail/Outbox/new"
OUTBOX_CUR="$HOME/.Mail/Outbox/cur"

# Ensure the directories exist
mkdir -p "$OUTBOX_NEW" "$OUTBOX_CUR"

# Search for all files in Outbox/new
for mailfile in "$OUTBOX_NEW"/*; do
    # Skip if the directory is empty (Globbing-Fix)
    [ -e "$mailfile" ] || continue

    echo "Sending $mailfile ..."

    # msmtp reads the file (-t reads recipient from the header)
    if msmtp -t < "$mailfile"; then
        echo "Successfully sent. Moving to cur."
        mv "$mailfile" "$OUTBOX_CUR/"
    else
        echo "Error sending $mailfile."
    fi
done
