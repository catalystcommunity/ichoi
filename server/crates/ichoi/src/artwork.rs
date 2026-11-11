//! Artwork storage and bounded image decoding. Manual choices take precedence over scans.
use std::io::{Cursor, Read};
use std::path::Path;

use diesel::prelude::*;
use diesel::sql_types::{Binary, Nullable, Text};
use image::{ImageFormat, ImageReader, Limits};
use libichoi::csil::types::{ArtworkKind, CoverArt};

use crate::db::store;

pub const MAX_UPLOAD: usize = 8 * 1024 * 1024;

pub fn kind_name(kind: &ArtworkKind) -> &'static str {
    match kind {
        ArtworkKind::Album => "album",
        ArtworkKind::Artist => "artist",
    }
}

#[derive(QueryableByName)]
struct Stored {
    #[diesel(sql_type = Text)]
    content_type: String,
    #[diesel(sql_type = Binary)]
    data: Vec<u8>,
}

pub fn stored(
    conn: &mut SqliteConnection,
    kind: &str,
    id: &str,
    source: &str,
) -> QueryResult<Option<CoverArt>> {
    diesel::sql_query(
        "SELECT content_type, data FROM artwork WHERE kind = ? AND target_id = ? AND source = ?",
    )
    .bind::<Text, _>(kind)
    .bind::<Text, _>(id)
    .bind::<Text, _>(source)
    .get_result::<Stored>(conn)
    .optional()
    .map(|row| {
        row.map(|row| CoverArt {
            content_type: row.content_type,
            data: row.data,
        })
    })
}

pub fn has_stored(conn: &mut SqliteConnection, kind: &str, id: &str) -> QueryResult<bool> {
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }
    diesel::sql_query("SELECT count(*) AS count FROM artwork WHERE kind = ? AND target_id = ?")
        .bind::<Text, _>(kind)
        .bind::<Text, _>(id)
        .get_result::<Count>(conn)
        .map(|row| row.count > 0)
}

pub fn save(
    conn: &mut SqliteConnection,
    kind: &str,
    id: &str,
    source: &str,
    art: &CoverArt,
    account: Option<&str>,
) -> QueryResult<()> {
    diesel::sql_query("INSERT INTO artwork (kind, target_id, source, content_type, data, updated_by) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(kind, target_id, source) DO UPDATE SET content_type=excluded.content_type, data=excluded.data, updated_by=excluded.updated_by")
        .bind::<Text, _>(kind).bind::<Text, _>(id).bind::<Text, _>(source)
        .bind::<Text, _>(&art.content_type).bind::<Binary, _>(&art.data).bind::<Nullable<Text>, _>(account).execute(conn)?;
    Ok(())
}

pub fn reset(conn: &mut SqliteConnection, kind: &str, id: &str) -> QueryResult<()> {
    diesel::sql_query("DELETE FROM artwork WHERE kind = ? AND target_id = ? AND source = 'manual'")
        .bind::<Text, _>(kind)
        .bind::<Text, _>(id)
        .execute(conn)?;
    Ok(())
}

/// Decode before storage or display. Strip metadata and limit output dimensions and size.
pub fn normalize(bytes: &[u8], max_size: u64) -> anyhow::Result<CoverArt> {
    anyhow::ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_UPLOAD,
        "Choose an image no larger than 8 MB."
    );
    let decoded = decode(bytes)?;
    let size = max_size.clamp(32, 1600) as u32;
    let thumbnail = decoded.thumbnail(size, size).to_rgb8();
    let mut data = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut data, 88).encode_image(&thumbnail)?;
    Ok(CoverArt {
        content_type: "image/jpeg".into(),
        data,
    })
}

pub(crate) fn decode(bytes: &[u8]) -> anyhow::Result<image::DynamicImage> {
    anyhow::ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_UPLOAD,
        "image exceeds 8 MB"
    );
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    anyhow::ensure!(
        matches!(
            reader.format(),
            Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::Gif | ImageFormat::WebP)
        ),
        "Choose a JPEG, PNG, GIF, or WebP image."
    );
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?)
}

pub(crate) fn read_bytes(path: &Path) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_UPLOAD as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= MAX_UPLOAD).then_some(bytes)
}

pub fn read_image(path: &Path, size: u64) -> Option<CoverArt> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_UPLOAD as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    normalize(&bytes, size).ok()
}

pub fn exists(conn: &mut SqliteConnection, kind: &ArtworkKind, id: &str) -> QueryResult<bool> {
    match kind {
        ArtworkKind::Album => store::get_album(conn, id).map(|a| a.is_some()),
        ArtworkKind::Artist => store::get_artist(conn, id).map(|a| a.is_some()),
    }
}

pub fn resolve(
    conn: &mut SqliteConnection,
    kind: &ArtworkKind,
    id: &str,
    size: u64,
) -> anyhow::Result<Option<CoverArt>> {
    if let Some(art) = stored(conn, kind_name(kind), id, "manual")? {
        return Ok(Some(normalize(&art.data, size)?));
    }
    match kind {
        ArtworkKind::Album => album_cover(conn, id, size),
        ArtworkKind::Artist => {
            // An album cover represents this artist until a user chooses an artist image.
            for library in store::list_libraries(conn)? {
                let mut albums = store::albums_for_artist(conn, &library.id, id)?;
                albums.sort_by(|a, b| {
                    b.has_cover_art
                        .cmp(&a.has_cover_art)
                        .then(a.title.cmp(&b.title))
                        .then(a.id.cmp(&b.id))
                });
                for album in albums {
                    if let Some(art) = resolve(conn, &ArtworkKind::Album, &album.id, size)? {
                        return Ok(Some(art));
                    }
                }
            }
            Ok(None)
        }
    }
}

fn album_cover(
    conn: &mut SqliteConnection,
    id: &str,
    size: u64,
) -> anyhow::Result<Option<CoverArt>> {
    if store::get_album(conn, id)?.is_none() {
        return Ok(None);
    }
    if let Some(art) = crate::artwork_discovery::selected(conn, id, size)? {
        return Ok(Some(art));
    }
    if let Some(art) = stored(conn, "album", id, "automatic")? {
        return Ok(Some(normalize(&art.data, size)?));
    }
    Ok(None)
}

/// Read the current account role on every write, including on existing connections.
pub fn can_edit(conn: &mut SqliteConnection, id: &str) -> QueryResult<bool> {
    let Some(account) = store::get_account(conn, id)? else {
        return Ok(false);
    };
    if account.role == "admin" {
        return Ok(true);
    }
    if account.role != "member" {
        return Ok(false);
    }
    #[derive(QueryableByName)]
    struct Editor {
        #[diesel(sql_type = Text)]
        account_id: String,
    }
    Ok(
        diesel::sql_query("SELECT account_id FROM library_editors WHERE account_id = ?")
            .bind::<Text, _>(id)
            .get_result::<Editor>(conn)
            .optional()?
            .is_some_and(|row| row.account_id == id),
    )
}

pub fn set_editor(conn: &mut SqliteConnection, id: &str, enabled: bool) -> QueryResult<()> {
    let sql = if enabled {
        "INSERT OR IGNORE INTO library_editors (account_id) VALUES (?)"
    } else {
        "DELETE FROM library_editors WHERE account_id = ?"
    };
    diesel::sql_query(sql).bind::<Text, _>(id).execute(conn)?;
    Ok(())
}
