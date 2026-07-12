-- Negative cache for the cover-art fill-in: once an album has been looked up (found or
-- not), mark it so startup doesn't re-query MusicBrainz for it every time.
ALTER TABLE albums ADD COLUMN art_checked INTEGER NOT NULL DEFAULT 0;
