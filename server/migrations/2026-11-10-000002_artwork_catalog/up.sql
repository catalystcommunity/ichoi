-- Paths and image facts only. Manual images remain in artwork.
CREATE TABLE artwork_catalog (
    album_id TEXT PRIMARY KEY NOT NULL,
    candidates TEXT NOT NULL,
    selected_id TEXT
);
