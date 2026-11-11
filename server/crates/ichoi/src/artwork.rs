//! Artwork storage and bounded image decoding. Manual choices take precedence over scans.
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};

use diesel::prelude::*;
use diesel::sql_types::{Binary, Nullable, Text};
use image::{ImageFormat, ImageReader, Limits};
use libichoi::csil::types::{ArtworkKind, CoverArt, ServiceError};

use crate::db::store;

pub const MAX_UPLOAD: usize = 8 * 1024 * 1024;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const QUEUE_CAPACITY: usize = 32;
type Job = Box<dyn FnOnce() + Send>;

/// One worker per application. The cache lives only on this thread.
pub struct ArtworkWorker {
    cache_bytes: usize,
    sender: OnceLock<Result<mpsc::SyncSender<Job>, String>>,
}

impl ArtworkWorker {
    pub fn new(cache_bytes: usize) -> Self {
        Self {
            cache_bytes,
            sender: OnceLock::new(),
        }
    }

    fn sender(&self) -> Result<&mpsc::SyncSender<Job>, ServiceError> {
        self.sender
            .get_or_init(|| {
                let (sender, receiver) = mpsc::sync_channel::<Job>(QUEUE_CAPACITY);
                let cache_bytes = self.cache_bytes;
                std::thread::Builder::new()
                    .name("ichoi-artwork".into())
                    .spawn(move || {
                        THUMBNAILS.with(|cache| {
                            *cache.borrow_mut() = Some(ThumbnailCache::new(cache_bytes))
                        });
                        while let Ok(job) = receiver.recv() {
                            job();
                        }
                    })
                    .map_err(|error| error.to_string())?;
                Ok(sender)
            })
            .as_ref()
            .map_err(|_| worker_error(503, "Artwork worker is unavailable. Try again."))
    }

    pub fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<T, ServiceError> + Send + 'static,
    ) -> Result<T, ServiceError> {
        self.run_with_timeout(REQUEST_TIMEOUT, work)
    }

    fn run_with_timeout<T: Send + 'static>(
        &self,
        timeout: Duration,
        work: impl FnOnce() -> Result<T, ServiceError> + Send + 'static,
    ) -> Result<T, ServiceError> {
        let (reply, result) = mpsc::sync_channel(1);
        let deadline = Instant::now() + timeout;
        self.sender()?
            .try_send(Box::new(move || {
                // Do not perform work for a request that expired while queued.
                if Instant::now() >= deadline {
                    return;
                }
                let value = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                    .unwrap_or_else(|_| {
                        Err(worker_error(500, "Artwork processing failed. Try again."))
                    });
                let _ = reply.send(value);
            }))
            .map_err(|_| worker_error(503, "Artwork queue is busy. Try again."))?;
        result
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| worker_error(504, "Artwork request timed out. Try again."))?
    }
}

fn worker_error(code: i64, message: &str) -> ServiceError {
    ServiceError {
        code,
        message: message.into(),
    }
}

struct ThumbnailCache {
    entries: HashMap<String, (CoverArt, u64, usize)>,
    recency: BTreeMap<u64, String>,
    clock: u64,
    bytes: usize,
    limit: usize,
}
impl ThumbnailCache {
    fn new(limit: usize) -> Self {
        Self {
            entries: HashMap::new(),
            recency: BTreeMap::new(),
            clock: 0,
            bytes: 0,
            limit,
        }
    }
    fn get(&mut self, key: &str) -> Option<CoverArt> {
        let (art, stamp, _) = self.entries.get_mut(key)?;
        self.recency.remove(stamp);
        self.clock += 1;
        *stamp = self.clock;
        self.recency.insert(*stamp, key.into());
        Some(art.clone())
    }
    fn insert(&mut self, key: String, art: &CoverArt) {
        // Charge image bytes, MIME text, and both retained copies of the key.
        let cost = art
            .data
            .len()
            .saturating_add(art.content_type.len())
            .saturating_add(key.len().saturating_mul(2));
        if self.limit == 0 || cost > self.limit {
            return;
        }
        if let Some((_, stamp, cost)) = self.entries.remove(&key) {
            self.recency.remove(&stamp);
            self.bytes -= cost;
        }
        while self.bytes > self.limit - cost {
            let Some((_, oldest)) = self.recency.pop_first() else {
                break;
            };
            if let Some((_, _, cost)) = self.entries.remove(&oldest) {
                self.bytes -= cost;
            }
        }
        self.clock += 1;
        self.bytes += cost;
        self.recency.insert(self.clock, key.clone());
        self.entries.insert(key, (art.clone(), self.clock, cost));
    }
}
thread_local! {
    // Scans and uploads outside the worker do not create additional caches.
    static THUMBNAILS: RefCell<Option<ThumbnailCache>> = const { RefCell::new(None) };
}
pub(crate) fn cached_thumbnail(key: &str) -> Option<CoverArt> {
    THUMBNAILS.with(|cache| cache.borrow_mut().as_mut()?.get(key))
}
pub(crate) fn cache_thumbnail(key: String, art: &CoverArt) {
    THUMBNAILS.with(|cache| {
        if let Some(cache) = cache.borrow_mut().as_mut() {
            cache.insert(key, art);
        }
    });
}

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
    let size = max_size.clamp(32, 1600) as u32;
    let key = format!("image:{size}:{:x}", Sha256::digest(bytes));
    if let Some(art) = cached_thumbnail(&key) {
        return Ok(art);
    }
    let decoded = decode(bytes)?;
    let thumbnail = decoded.thumbnail(size, size).to_rgb8();
    let mut data = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut data, 88).encode_image(&thumbnail)?;
    let art = CoverArt {
        content_type: "image/jpeg".into(),
        data,
    };
    cache_thumbnail(key, &art);
    Ok(art)
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

#[cfg(test)]
mod worker_tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn art(size: usize) -> CoverArt {
        CoverArt {
            content_type: "image/jpeg".into(),
            data: vec![1; size],
        }
    }

    #[test]
    fn cache_evicts_by_bytes_and_retains_recent_entries() {
        let mut cache = ThumbnailCache::new(224);
        cache.insert("a".into(), &art(100));
        cache.insert("b".into(), &art(100));
        assert_eq!(cache.bytes, 224);
        assert!(cache.get("a").is_some());
        cache.insert("c".into(), &art(100));
        assert!(cache.get("b").is_none());
        assert!(cache.get("a").is_some());
        cache.insert("a".into(), &art(10));
        assert_eq!(cache.bytes, 134);
        cache.insert("large".into(), &art(225));
        assert_eq!(cache.bytes, 134);
        assert!(cache.get("large").is_none());
        let mut disabled = ThumbnailCache::new(0);
        disabled.insert("a".into(), &art(1));
        assert!(disabled.get("a").is_none());
    }

    #[test]
    fn byte_budget_replaces_the_old_512_entry_limit() {
        let mut cache = ThumbnailCache::new(1_000_000);
        for i in 0..600 {
            cache.insert(i.to_string(), &art(100));
        }
        assert_eq!(cache.entries.len(), 600);
        assert!(cache.bytes <= cache.limit);
    }

    #[test]
    fn requests_share_a_dedicated_thread_and_cache() {
        let worker = ArtworkWorker::new(1000);
        let thread = worker
            .run(|| {
                cache_thumbnail("shared".into(), &art(100));
                Ok(std::thread::current().id())
            })
            .unwrap();
        assert_ne!(thread, std::thread::current().id());
        worker
            .run(move || {
                assert_eq!(thread, std::thread::current().id());
                assert!(cached_thumbnail("shared").is_some());
                Ok(())
            })
            .unwrap();
        // Other request and scan threads have no extra artwork cache.
        assert!(cached_thumbnail("shared").is_none());
        let other = ArtworkWorker::new(1000);
        assert!(other
            .run(|| Ok(cached_thumbnail("shared").is_none()))
            .unwrap());
    }

    #[test]
    fn full_queue_fails_promptly_and_expired_work_is_skipped() {
        let worker = ArtworkWorker::new(1000);
        let (release, gate) = mpsc::channel();
        let (started, ready) = mpsc::channel();
        worker
            .sender()
            .unwrap()
            .send(Box::new(move || {
                started.send(()).unwrap();
                let _ = gate.recv_timeout(Duration::from_secs(2));
            }))
            .unwrap();
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        let ran = Arc::new(AtomicBool::new(false));
        let check = ran.clone();
        let error = worker
            .run_with_timeout(Duration::from_millis(20), move || {
                check.store(true, Ordering::SeqCst);
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.code, 504);
        for _ in 1..QUEUE_CAPACITY {
            worker.sender().unwrap().try_send(Box::new(|| {})).unwrap();
        }
        assert_eq!(worker.run(|| Ok(())).unwrap_err().code, 503);
        release.send(()).unwrap();
        // A sentinel confirms that all older jobs were processed or skipped.
        let (done, completed) = mpsc::channel();
        worker
            .sender()
            .unwrap()
            .send(Box::new(move || {
                let _ = done.send(());
            }))
            .unwrap();
        completed.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(!ran.load(Ordering::SeqCst));
    }
}
