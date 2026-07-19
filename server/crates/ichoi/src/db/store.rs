//! Query functions over the pool's connection. Handlers call these; nothing here owns a
//! connection or a transaction.

use diesel::prelude::*;

use super::models::*;
use super::schema::*;

// ---------------------------------------------------------------------------- accounts

pub fn count_accounts(conn: &mut SqliteConnection) -> QueryResult<i64> {
    accounts::table.count().get_result(conn)
}

pub fn get_account(conn: &mut SqliteConnection, id: &str) -> QueryResult<Option<Account>> {
    accounts::table
        .find(id)
        .select(Account::as_select())
        .first(conn)
        .optional()
}

pub fn upsert_account(conn: &mut SqliteConnection, row: &Account) -> QueryResult<()> {
    diesel::insert_into(accounts::table)
        .values(row)
        .on_conflict(accounts::id)
        .do_update()
        .set(row)
        .execute(conn)?;
    Ok(())
}

/// Refresh LinkKeys-owned profile fields without changing local authorization.
pub fn upsert_linkkeys_account(
    conn: &mut SqliteConnection,
    id: &str,
    handle: &str,
    display_name: Option<&str>,
) -> QueryResult<Account> {
    let now = chrono::Utc::now().to_rfc3339();
    diesel::insert_into(accounts::table)
        .values((
            accounts::id.eq(id),
            accounts::handle.eq(handle),
            accounts::display_name.eq(display_name),
            accounts::role.eq("member"),
            accounts::created_at.eq(&now),
        ))
        .on_conflict(accounts::id)
        .do_update()
        .set((
            accounts::handle.eq(handle),
            accounts::display_name.eq(display_name),
        ))
        .execute(conn)?;
    accounts::table
        .find(id)
        .select(Account::as_select())
        .first(conn)
}

pub fn set_role(conn: &mut SqliteConnection, id: &str, role: &str) -> QueryResult<()> {
    diesel::update(accounts::table.find(id))
        .set(accounts::role.eq(role))
        .execute(conn)?;
    Ok(())
}

pub fn list_accounts(
    conn: &mut SqliteConnection,
    offset: i64,
    limit: i64,
) -> QueryResult<Vec<Account>> {
    accounts::table
        .order(accounts::handle.asc())
        .offset(offset)
        .limit(limit)
        .select(Account::as_select())
        .load(conn)
}

// ---------------------------------------------------------------------------- sessions

pub fn create_session(
    conn: &mut SqliteConnection,
    token_sha256: &str,
    account_id: &str,
    expires_at: &str,
) -> QueryResult<()> {
    let row = Session {
        token_sha256: token_sha256.to_string(),
        account_id: account_id.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        expires_at: expires_at.to_string(),
    };
    diesel::insert_into(sessions::table)
        .values(&row)
        .execute(conn)?;
    Ok(())
}

pub fn account_for_token(
    conn: &mut SqliteConnection,
    token_sha256: &str,
) -> QueryResult<Option<Account>> {
    sessions::table
        .inner_join(accounts::table.on(accounts::id.eq(sessions::account_id)))
        .filter(sessions::token_sha256.eq(token_sha256))
        .select(Account::as_select())
        .first(conn)
        .optional()
}

pub fn delete_session(conn: &mut SqliteConnection, token_sha256: &str) -> QueryResult<()> {
    diesel::delete(sessions::table.find(token_sha256)).execute(conn)?;
    Ok(())
}

// ---------------------------------------------------------------------------- settings

pub fn get_setting(conn: &mut SqliteConnection, key: &str) -> QueryResult<Option<String>> {
    settings::table
        .find(key)
        .select(settings::value)
        .first(conn)
        .optional()
}

pub fn set_setting(conn: &mut SqliteConnection, key: &str, value: &str) -> QueryResult<()> {
    let row = Setting {
        key: key.to_string(),
        value: value.to_string(),
    };
    diesel::insert_into(settings::table)
        .values(&row)
        .on_conflict(settings::key)
        .do_update()
        .set(&row)
        .execute(conn)?;
    Ok(())
}

pub fn all_settings(conn: &mut SqliteConnection) -> QueryResult<Vec<Setting>> {
    settings::table.select(Setting::as_select()).load(conn)
}

// ----------------------------------------------------------- LinkKeys local RP

pub fn active_local_rp_identity(
    conn: &mut SqliteConnection,
) -> QueryResult<Option<LinkkeysLocalRpIdentity>> {
    linkkeys_local_rp_identities::table
        .filter(linkkeys_local_rp_identities::active.eq(1))
        .select(LinkkeysLocalRpIdentity::as_select())
        .first(conn)
        .optional()
}

pub fn insert_local_rp_identity(
    conn: &mut SqliteConnection,
    row: &LinkkeysLocalRpIdentity,
) -> QueryResult<()> {
    diesel::insert_into(linkkeys_local_rp_identities::table)
        .values(row)
        .execute(conn)?;
    Ok(())
}

pub fn add_linkkeys_trust(
    conn: &mut SqliteConnection,
    domain: &str,
    handle: Option<&str>,
    source: &str,
) -> QueryResult<()> {
    let row = LinkkeysTrustedIdentity {
        domain: domain.to_string(),
        handle: handle.unwrap_or_default().to_string(),
        source: source.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    diesel::insert_into(linkkeys_trusted_identities::table)
        .values(&row)
        .on_conflict((
            linkkeys_trusted_identities::domain,
            linkkeys_trusted_identities::handle,
        ))
        .do_nothing()
        .execute(conn)?;
    Ok(())
}

pub fn linkkeys_identity_is_trusted(
    conn: &mut SqliteConnection,
    domain: &str,
    handle: Option<&str>,
) -> QueryResult<bool> {
    let legacy_domain_count: i64 = trusted_domains::table
        .filter(trusted_domains::domain.eq(domain))
        .count()
        .get_result(conn)?;
    if legacy_domain_count > 0 {
        return Ok(true);
    }
    let handle = handle.unwrap_or_default();
    let count: i64 = linkkeys_trusted_identities::table
        .filter(linkkeys_trusted_identities::domain.eq(domain))
        .filter(
            linkkeys_trusted_identities::handle
                .eq("")
                .or(linkkeys_trusted_identities::handle.eq(handle)),
        )
        .count()
        .get_result(conn)?;
    Ok(count > 0)
}

pub fn create_linkkeys_attempt(
    conn: &mut SqliteConnection,
    row: &LinkkeysLoginAttempt,
) -> QueryResult<()> {
    diesel::insert_into(linkkeys_login_attempts::table)
        .values(row)
        .execute(conn)?;
    Ok(())
}

pub fn purge_expired_linkkeys_state(conn: &mut SqliteConnection, now: &str) -> QueryResult<()> {
    diesel::delete(
        linkkeys_login_attempts::table.filter(linkkeys_login_attempts::expires_at.le(now)),
    )
    .execute(conn)?;
    diesel::delete(
        linkkeys_login_exchanges::table.filter(linkkeys_login_exchanges::expires_at.le(now)),
    )
    .execute(conn)?;
    Ok(())
}

pub fn consume_linkkeys_attempt(
    conn: &mut SqliteConnection,
    attempt_sha256: &str,
) -> QueryResult<Option<LinkkeysLoginAttempt>> {
    conn.transaction(|conn| {
        let row = linkkeys_login_attempts::table
            .find(attempt_sha256)
            .select(LinkkeysLoginAttempt::as_select())
            .first(conn)
            .optional()?;
        diesel::delete(linkkeys_login_attempts::table.find(attempt_sha256)).execute(conn)?;
        Ok(row)
    })
}

pub fn create_linkkeys_exchange(
    conn: &mut SqliteConnection,
    row: &LinkkeysLoginExchange,
) -> QueryResult<()> {
    diesel::insert_into(linkkeys_login_exchanges::table)
        .values(row)
        .execute(conn)?;
    Ok(())
}

pub fn consume_linkkeys_exchange(
    conn: &mut SqliteConnection,
    code_sha256: &str,
) -> QueryResult<Option<LinkkeysLoginExchange>> {
    conn.transaction(|conn| {
        let row = linkkeys_login_exchanges::table
            .find(code_sha256)
            .select(LinkkeysLoginExchange::as_select())
            .first(conn)
            .optional()?;
        diesel::delete(linkkeys_login_exchanges::table.find(code_sha256)).execute(conn)?;
        Ok(row)
    })
}

// ------------------------------------------------------------------- trusted domains

pub fn add_trusted_domain(conn: &mut SqliteConnection, domain: &str) -> QueryResult<()> {
    diesel::insert_or_ignore_into(trusted_domains::table)
        .values(trusted_domains::domain.eq(domain))
        .execute(conn)?;
    Ok(())
}

pub fn list_trusted_domains(conn: &mut SqliteConnection) -> QueryResult<Vec<String>> {
    trusted_domains::table
        .order(trusted_domains::domain.asc())
        .select(trusted_domains::domain)
        .load(conn)
}

// --------------------------------------------------------------------------- libraries

pub fn upsert_library(conn: &mut SqliteConnection, row: &Library) -> QueryResult<()> {
    diesel::insert_into(libraries::table)
        .values(row)
        .on_conflict(libraries::id)
        .do_update()
        .set(row)
        .execute(conn)?;
    Ok(())
}

pub fn list_libraries(conn: &mut SqliteConnection) -> QueryResult<Vec<Library>> {
    libraries::table.select(Library::as_select()).load(conn)
}

pub fn get_library(conn: &mut SqliteConnection, id: &str) -> QueryResult<Option<Library>> {
    libraries::table
        .find(id)
        .select(Library::as_select())
        .first(conn)
        .optional()
}

// ----------------------------------------------------------------- artists / albums

pub fn upsert_artist(conn: &mut SqliteConnection, row: &Artist) -> QueryResult<()> {
    diesel::insert_into(artists::table)
        .values(row)
        .on_conflict(artists::id)
        .do_update()
        .set(row)
        .execute(conn)?;
    Ok(())
}

pub fn upsert_album(conn: &mut SqliteConnection, row: &Album) -> QueryResult<()> {
    diesel::insert_into(albums::table)
        .values(row)
        .on_conflict(albums::id)
        .do_update()
        .set(row)
        .execute(conn)?;
    Ok(())
}

pub fn list_albums(
    conn: &mut SqliteConnection,
    library_id: &str,
    offset: i64,
    limit: i64,
) -> QueryResult<Vec<Album>> {
    let album_ids = tracks::table
        .filter(tracks::library_id.eq(library_id))
        .filter(tracks::album_id.is_not_null())
        .select(tracks::album_id.assume_not_null())
        .distinct();
    albums::table
        .filter(albums::id.eq_any(album_ids))
        .order(albums::title.asc())
        .offset(offset)
        .limit(limit)
        .select(Album::as_select())
        .load(conn)
}

pub fn count_albums(conn: &mut SqliteConnection, library_id: &str) -> QueryResult<i64> {
    let ids: Vec<String> = tracks::table
        .filter(tracks::library_id.eq(library_id))
        .filter(tracks::album_id.is_not_null())
        .select(tracks::album_id.assume_not_null())
        .distinct()
        .load(conn)?;
    Ok(ids.len() as i64)
}

pub fn albums_without_cover(conn: &mut SqliteConnection, limit: i64) -> QueryResult<Vec<Album>> {
    albums::table
        .filter(albums::has_cover_art.eq(0))
        .filter(albums::art_checked.eq(0))
        .order(albums::title.asc())
        .limit(limit)
        .select(Album::as_select())
        .load(conn)
}

pub fn set_album_cover(conn: &mut SqliteConnection, id: &str, path: &str) -> QueryResult<()> {
    diesel::update(albums::table.find(id))
        .set((
            albums::cover_art_path.eq(path),
            albums::has_cover_art.eq(1),
            albums::art_checked.eq(1),
        ))
        .execute(conn)?;
    Ok(())
}

/// Mark an album as art-checked (found nothing / not applicable) so startup won't re-query it.
pub fn mark_art_checked(conn: &mut SqliteConnection, id: &str) -> QueryResult<()> {
    diesel::update(albums::table.find(id))
        .set(albums::art_checked.eq(1))
        .execute(conn)?;
    Ok(())
}

/// Reset the art-checked cache so a `fetch-art --retry` re-queries everything.
pub fn reset_art_checked(conn: &mut SqliteConnection) -> QueryResult<usize> {
    diesel::update(albums::table)
        .set(albums::art_checked.eq(0))
        .execute(conn)
}

/// `(album_id, track_count)` for every album — used to find under-populated albums.
pub fn album_track_counts(
    conn: &mut SqliteConnection,
    library_id: &str,
) -> QueryResult<Vec<(String, i64)>> {
    tracks::table
        .filter(tracks::library_id.eq(library_id))
        .filter(tracks::album_id.is_not_null())
        .group_by(tracks::album_id)
        .select((
            tracks::album_id.assume_not_null(),
            diesel::dsl::count_star(),
        ))
        .load(conn)
}

pub fn set_track_album(
    conn: &mut SqliteConnection,
    track_id: &str,
    album_id: &str,
) -> QueryResult<()> {
    diesel::update(tracks::table.find(track_id))
        .set(tracks::album_id.eq(album_id))
        .execute(conn)?;
    Ok(())
}

/// Delete albums that no longer have any tracks (after consolidation).
pub fn delete_empty_albums(conn: &mut SqliteConnection) -> QueryResult<usize> {
    let with_tracks = tracks::table
        .filter(tracks::album_id.is_not_null())
        .select(tracks::album_id.assume_not_null())
        .distinct();
    diesel::delete(albums::table.filter(diesel::dsl::not(albums::id.eq_any(with_tracks))))
        .execute(conn)
}

pub fn count_all_albums(conn: &mut SqliteConnection) -> QueryResult<i64> {
    albums::table.count().get_result(conn)
}

pub fn albums_missing_artist(conn: &mut SqliteConnection) -> QueryResult<Vec<Album>> {
    albums::table
        .filter(albums::artist_id.is_null())
        .select(Album::as_select())
        .load(conn)
}

pub fn set_album_artist(conn: &mut SqliteConnection, id: &str, artist_id: &str) -> QueryResult<()> {
    diesel::update(albums::table.find(id))
        .set(albums::artist_id.eq(artist_id))
        .execute(conn)?;
    Ok(())
}

pub fn get_album(conn: &mut SqliteConnection, id: &str) -> QueryResult<Option<Album>> {
    albums::table
        .find(id)
        .select(Album::as_select())
        .first(conn)
        .optional()
}

pub fn list_artists(
    conn: &mut SqliteConnection,
    library_id: &str,
    offset: i64,
    limit: i64,
) -> QueryResult<Vec<Artist>> {
    let artist_ids = tracks::table
        .filter(tracks::library_id.eq(library_id))
        .filter(tracks::artist_id.is_not_null())
        .select(tracks::artist_id.assume_not_null())
        .distinct();
    artists::table
        .filter(artists::id.eq_any(artist_ids))
        .order(artists::name.asc())
        .offset(offset)
        .limit(limit)
        .select(Artist::as_select())
        .load(conn)
}

pub fn count_artists(conn: &mut SqliteConnection, library_id: &str) -> QueryResult<i64> {
    let ids: Vec<String> = tracks::table
        .filter(tracks::library_id.eq(library_id))
        .filter(tracks::artist_id.is_not_null())
        .select(tracks::artist_id.assume_not_null())
        .distinct()
        .load(conn)?;
    Ok(ids.len() as i64)
}

pub fn get_artist(conn: &mut SqliteConnection, id: &str) -> QueryResult<Option<Artist>> {
    artists::table
        .find(id)
        .select(Artist::as_select())
        .first(conn)
        .optional()
}

pub fn albums_for_artist(
    conn: &mut SqliteConnection,
    library_id: &str,
    artist_id: &str,
) -> QueryResult<Vec<Album>> {
    let album_ids = tracks::table
        .filter(tracks::library_id.eq(library_id))
        .filter(tracks::artist_id.eq(artist_id))
        .filter(tracks::album_id.is_not_null())
        .select(tracks::album_id.assume_not_null())
        .distinct();
    albums::table
        .filter(albums::id.eq_any(album_ids))
        .order(albums::year.asc())
        .select(Album::as_select())
        .load(conn)
}

pub fn count_albums_for_artist(
    conn: &mut SqliteConnection,
    library_id: &str,
    artist_id: &str,
) -> QueryResult<i64> {
    let ids: Vec<String> = tracks::table
        .filter(tracks::library_id.eq(library_id))
        .filter(tracks::artist_id.eq(artist_id))
        .filter(tracks::album_id.is_not_null())
        .select(tracks::album_id.assume_not_null())
        .distinct()
        .load(conn)?;
    Ok(ids.len() as i64)
}

// ------------------------------------------------------------------------------ tracks

pub fn upsert_track(conn: &mut SqliteConnection, row: &Track) -> QueryResult<()> {
    diesel::insert_into(tracks::table)
        .values(row)
        .on_conflict(tracks::id)
        .do_update()
        .set(row)
        .execute(conn)?;
    Ok(())
}

pub fn get_track(conn: &mut SqliteConnection, id: &str) -> QueryResult<Option<Track>> {
    tracks::table
        .find(id)
        .select(Track::as_select())
        .first(conn)
        .optional()
}

pub fn track_by_root_path(conn: &mut SqliteConnection, path: &str) -> QueryResult<Option<Track>> {
    tracks::table
        .filter(tracks::root_relative_path.eq(path))
        .select(Track::as_select())
        .first(conn)
        .optional()
}

pub fn track_by_library_root_path(
    conn: &mut SqliteConnection,
    library_id: &str,
    path: &str,
) -> QueryResult<Option<Track>> {
    tracks::table
        .filter(tracks::library_id.eq(library_id))
        .filter(tracks::root_relative_path.eq(path))
        .select(Track::as_select())
        .first(conn)
        .optional()
}

pub fn tracks_for_library(
    conn: &mut SqliteConnection,
    library_id: &str,
) -> QueryResult<Vec<Track>> {
    tracks::table
        .filter(tracks::library_id.eq(library_id))
        .select(Track::as_select())
        .load(conn)
}

pub fn delete_track(conn: &mut SqliteConnection, track_id: &str) -> QueryResult<usize> {
    diesel::delete(tracks::table.find(track_id)).execute(conn)
}

pub fn tracks_for_album(conn: &mut SqliteConnection, album_id: &str) -> QueryResult<Vec<Track>> {
    tracks::table
        .filter(tracks::album_id.eq(album_id))
        .order((tracks::disc_no.asc(), tracks::track_no.asc()))
        .select(Track::as_select())
        .load(conn)
}

pub fn count_tracks_for_album(conn: &mut SqliteConnection, album_id: &str) -> QueryResult<i64> {
    tracks::table
        .filter(tracks::album_id.eq(album_id))
        .count()
        .get_result(conn)
}

pub fn count_tracks(conn: &mut SqliteConnection) -> QueryResult<i64> {
    tracks::table.count().get_result(conn)
}

pub fn search_tracks(
    conn: &mut SqliteConnection,
    library_id: &str,
    query: &str,
    limit: i64,
) -> QueryResult<Vec<Track>> {
    let pattern = format!("%{query}%");
    tracks::table
        .filter(tracks::library_id.eq(library_id))
        .filter(tracks::title.like(pattern))
        .order(tracks::title.asc())
        .limit(limit)
        .select(Track::as_select())
        .load(conn)
}

pub fn search_albums(
    conn: &mut SqliteConnection,
    library_id: &str,
    query: &str,
    limit: i64,
) -> QueryResult<Vec<Album>> {
    let pattern = format!("%{query}%");
    let album_ids = tracks::table
        .filter(tracks::library_id.eq(library_id))
        .filter(tracks::album_id.is_not_null())
        .select(tracks::album_id.assume_not_null())
        .distinct();
    albums::table
        .filter(albums::id.eq_any(album_ids))
        .filter(albums::title.like(pattern))
        .order(albums::title.asc())
        .limit(limit)
        .select(Album::as_select())
        .load(conn)
}

pub fn search_artists(
    conn: &mut SqliteConnection,
    library_id: &str,
    query: &str,
    limit: i64,
) -> QueryResult<Vec<Artist>> {
    let pattern = format!("%{query}%");
    let artist_ids = tracks::table
        .filter(tracks::library_id.eq(library_id))
        .filter(tracks::artist_id.is_not_null())
        .select(tracks::artist_id.assume_not_null())
        .distinct();
    artists::table
        .filter(artists::id.eq_any(artist_ids))
        .filter(artists::name.like(pattern))
        .order(artists::name.asc())
        .limit(limit)
        .select(Artist::as_select())
        .load(conn)
}

// ------------------------------------------------------------- audiobook progress

pub fn audiobook_progress_for_tracks(
    conn: &mut SqliteConnection,
    account_id: &str,
    track_ids: &[String],
) -> QueryResult<Vec<AudiobookProgress>> {
    audiobook_progress::table
        .filter(audiobook_progress::account_id.eq(account_id))
        .filter(audiobook_progress::track_id.eq_any(track_ids))
        .order(audiobook_progress::updated_at.desc())
        .select(AudiobookProgress::as_select())
        .load(conn)
}

pub fn upsert_audiobook_progress(
    conn: &mut SqliteConnection,
    row: &AudiobookProgress,
) -> QueryResult<()> {
    diesel::insert_into(audiobook_progress::table)
        .values(row)
        .on_conflict((audiobook_progress::account_id, audiobook_progress::track_id))
        .do_update()
        .set((
            audiobook_progress::position_ms.eq(row.position_ms),
            audiobook_progress::completed.eq(row.completed),
            audiobook_progress::updated_at.eq(&row.updated_at),
        ))
        .execute(conn)?;
    Ok(())
}

// --------------------------------------------------------------------------- playlists

pub fn list_playlists(conn: &mut SqliteConnection) -> QueryResult<Vec<Playlist>> {
    playlists::table
        .order(playlists::name.asc())
        .select(Playlist::as_select())
        .load(conn)
}

pub fn get_playlist(conn: &mut SqliteConnection, id: &str) -> QueryResult<Option<Playlist>> {
    playlists::table
        .find(id)
        .select(Playlist::as_select())
        .first(conn)
        .optional()
}

pub fn upsert_playlist(conn: &mut SqliteConnection, row: &Playlist) -> QueryResult<()> {
    diesel::insert_into(playlists::table)
        .values(row)
        .on_conflict(playlists::id)
        .do_update()
        .set((
            playlists::name.eq(&row.name),
            playlists::owner.eq(&row.owner),
            playlists::root_relative_path.eq(&row.root_relative_path),
            playlists::visibility.eq(&row.visibility),
        ))
        .execute(conn)?;
    Ok(())
}

// ------------------------------------------------------------------- nodes / devices

pub fn ensure_core_node(conn: &mut SqliteConnection, hostname: &str) -> QueryResult<()> {
    let exists: i64 = nodes::table
        .filter(nodes::kind.eq("core"))
        .count()
        .get_result(conn)?;
    if exists == 0 {
        let row = Node {
            id: format!("core:{hostname}"),
            kind: "core".to_string(),
            hostname: hostname.to_string(),
            friendly_name: hostname.to_string(),
            token_sha256: None,
            platform: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            audio_outputs: "none".to_string(),
            last_seen: Some(chrono::Utc::now().to_rfc3339()),
        };
        diesel::insert_into(nodes::table)
            .values(&row)
            .execute(conn)?;
    }
    Ok(())
}

pub fn sync_core_outputs(
    conn: &mut SqliteConnection,
    hostname: &str,
    outputs: &[crate::audio::AudioOutput],
) -> QueryResult<()> {
    let node_id = format!("core:{hostname}");
    diesel::update(nodes::table.find(&node_id))
        .set(nodes::audio_outputs.eq(if outputs.is_empty() { "none" } else { "some" }))
        .execute(conn)?;

    for out in outputs {
        let dev = OutputDevice {
            id: format!("{node_id}:{}", out.os_device_id),
            node_id: node_id.clone(),
            os_device_id: out.os_device_id.clone(),
            friendly_name: out.friendly_name.clone(),
            is_default: i32::from(out.is_default),
        };
        upsert_device(conn, &dev)?;
        create_player(
            conn,
            &Player {
                id: format!("player:{}", dev.id),
                kind: "shared".to_string(),
                output_device_id: Some(dev.id.clone()),
                owner_account_id: None,
                name: format!("{hostname} · {}", dev.friendly_name),
                name_suffix: None,
            },
        )?;
    }

    Ok(())
}

pub fn upsert_node(conn: &mut SqliteConnection, row: &Node) -> QueryResult<()> {
    diesel::insert_into(nodes::table)
        .values(row)
        .on_conflict(nodes::id)
        .do_update()
        .set(row)
        .execute(conn)?;
    Ok(())
}

pub fn list_nodes(conn: &mut SqliteConnection) -> QueryResult<Vec<Node>> {
    nodes::table
        .order(nodes::friendly_name.asc())
        .select(Node::as_select())
        .load(conn)
}

pub fn rename_node(
    conn: &mut SqliteConnection,
    id: &str,
    friendly_name: &str,
) -> QueryResult<usize> {
    diesel::update(nodes::table.find(id))
        .set(nodes::friendly_name.eq(friendly_name))
        .execute(conn)
}

pub fn devices_for_node(
    conn: &mut SqliteConnection,
    node_id: &str,
) -> QueryResult<Vec<OutputDevice>> {
    output_devices::table
        .filter(output_devices::node_id.eq(node_id))
        .select(OutputDevice::as_select())
        .load(conn)
}

pub fn upsert_device(conn: &mut SqliteConnection, row: &OutputDevice) -> QueryResult<()> {
    diesel::insert_into(output_devices::table)
        .values(row)
        .on_conflict(output_devices::id)
        .do_update()
        .set(row)
        .execute(conn)?;
    Ok(())
}

pub fn rename_device(
    conn: &mut SqliteConnection,
    id: &str,
    friendly_name: &str,
) -> QueryResult<usize> {
    diesel::update(output_devices::table.find(id))
        .set(output_devices::friendly_name.eq(friendly_name))
        .execute(conn)
}

// ----------------------------------------------------------------------------- players

pub fn list_players(conn: &mut SqliteConnection, kind: Option<&str>) -> QueryResult<Vec<Player>> {
    let mut q = players::table.into_boxed();
    if let Some(k) = kind {
        q = q.filter(players::kind.eq(k.to_string()));
    }
    q.order(players::name.asc())
        .select(Player::as_select())
        .load(conn)
}

pub fn get_player(conn: &mut SqliteConnection, id: &str) -> QueryResult<Option<Player>> {
    players::table
        .find(id)
        .select(Player::as_select())
        .first(conn)
        .optional()
}

pub fn create_player(conn: &mut SqliteConnection, row: &Player) -> QueryResult<()> {
    diesel::insert_into(players::table)
        .values(row)
        .on_conflict(players::id)
        .do_update()
        .set(row)
        .execute(conn)?;
    Ok(())
}

pub fn delete_player(conn: &mut SqliteConnection, id: &str) -> QueryResult<usize> {
    diesel::delete(players::table.find(id)).execute(conn)
}

pub fn player_name_taken(conn: &mut SqliteConnection, name: &str) -> QueryResult<bool> {
    let n: i64 = players::table
        .filter(players::name.eq(name))
        .count()
        .get_result(conn)?;
    Ok(n > 0)
}

pub fn get_state(
    conn: &mut SqliteConnection,
    player_id: &str,
) -> QueryResult<Option<PlayerStateRow>> {
    player_state::table
        .find(player_id)
        .select(PlayerStateRow::as_select())
        .first(conn)
        .optional()
}

pub fn upsert_state(conn: &mut SqliteConnection, row: &PlayerStateRow) -> QueryResult<()> {
    diesel::insert_into(player_state::table)
        .values(row)
        .on_conflict(player_state::player_id)
        .do_update()
        .set(row)
        .execute(conn)?;
    Ok(())
}

pub fn queue_items(conn: &mut SqliteConnection, player_id: &str) -> QueryResult<Vec<QueueItem>> {
    player_queue_items::table
        .filter(player_queue_items::player_id.eq(player_id))
        .order(player_queue_items::position.asc())
        .select(QueueItem::as_select())
        .load(conn)
}

/// Replace a player's whole queue with an ordered list of track ids.
pub fn set_queue(
    conn: &mut SqliteConnection,
    player_id: &str,
    track_ids: &[String],
) -> QueryResult<()> {
    conn.transaction(|conn| {
        diesel::delete(
            player_queue_items::table.filter(player_queue_items::player_id.eq(player_id)),
        )
        .execute(conn)?;
        for (i, track_id) in track_ids.iter().enumerate() {
            let row = NewQueueItem {
                player_id: player_id.to_string(),
                track_id: track_id.clone(),
                position: i as i32,
            };
            diesel::insert_into(player_queue_items::table)
                .values(&row)
                .execute(conn)?;
        }
        Ok(())
    })
}
