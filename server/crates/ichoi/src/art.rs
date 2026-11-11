//! External cover-art lookup. Store results in the database so media folders can be read-only.
//! A manual choice always takes precedence over a downloaded image.

use std::io::Read;
use std::time::Duration;

use diesel::SqliteConnection;

use crate::artwork;
use crate::db::store;

#[derive(Debug, Default)]
pub struct ArtStats {
    pub fetched: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// Fetch cover art for up to `limit` albums that currently have none.
pub fn fetch_missing(conn: &mut SqliteConnection, limit: usize) -> anyhow::Result<ArtStats> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!(
            "Ichoi/{} (https://github.com/catalystcommunity/ichoi)",
            crate::version()
        ))
        .timeout(Duration::from_secs(25))
        .build()?;

    let albums = store::albums_without_cover(conn, limit as i64)?;
    log::info!("art: {} albums missing cover art", albums.len());
    let mut stats = ArtStats::default();

    for album in albums {
        if artwork::has_stored(conn, "album", &album.id)? {
            store::mark_art_checked(conn, &album.id)?;
            stats.skipped += 1;
            continue;
        }

        let artist = album
            .artist_id
            .as_ref()
            .and_then(|id| store::get_artist(conn, id).ok().flatten())
            .map(|a| a.name)
            .unwrap_or_default();

        match fetch_one(&client, &album.title, &artist, album.year) {
            Ok(Some(bytes)) => {
                let art = match artwork::normalize(&bytes, 1600) {
                    Ok(art) => art,
                    Err(error) => {
                        log::warn!("art: invalid image for {}: {error}", album.title);
                        store::mark_art_checked(conn, &album.id)?;
                        stats.failed += 1;
                        std::thread::sleep(Duration::from_millis(1100));
                        continue;
                    }
                };
                artwork::save(conn, "album", &album.id, "automatic", &art, None)?;
                store::mark_art_checked(conn, &album.id)?;
                stats.fetched += 1;
                log::info!("art: saved {} — {}", artist, album.title);
            }
            Ok(None) => {
                store::mark_art_checked(conn, &album.id)?;
                stats.failed += 1;
            }
            Err(e) => {
                log::warn!("art: {artist} — {}: {e}", album.title);
                store::mark_art_checked(conn, &album.id)?;
                stats.failed += 1;
            }
        }

        // MusicBrainz asks for no more than one request per second.
        std::thread::sleep(Duration::from_millis(1100));
    }
    Ok(stats)
}

fn fetch_one(
    client: &reqwest::blocking::Client,
    album: &str,
    artist: &str,
    year: Option<i32>,
) -> anyhow::Result<Option<Vec<u8>>> {
    if album.is_empty()
        || album.starts_with("Unknown")
        || artist.is_empty()
        || artist.starts_with("Unknown")
    {
        return Ok(None);
    }
    let mut query = format!("release:\"{}\"", album.replace('"', ""));
    if !artist.is_empty() && !artist.starts_with("Unknown") {
        query.push_str(&format!(" AND artist:\"{}\"", artist.replace('"', "")));
    }

    let resp: serde_json::Value = client
        .get("https://musicbrainz.org/ws/2/release/")
        .query(&[("query", query.as_str()), ("fmt", "json"), ("limit", "10")])
        .send()?
        .error_for_status()?
        .json()?;

    let Some(mbid) = verified_release(&resp, album, artist, year) else {
        return Ok(None);
    };

    let art = client
        .get(format!("https://coverartarchive.org/release/{mbid}/front"))
        .send()?;
    if art.status().is_success() {
        let mut bytes = Vec::new();
        art.take(artwork::MAX_UPLOAD as u64 + 1)
            .read_to_end(&mut bytes)?;
        Ok(Some(bytes))
    } else {
        Ok(None)
    }
}

fn verified_release<'a>(
    response: &'a serde_json::Value,
    album: &str,
    artist: &str,
    year: Option<i32>,
) -> Option<&'a str> {
    fn normalized(value: &str) -> String {
        value
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect()
    }
    let matches = response["releases"]
        .as_array()?
        .iter()
        .filter(|release| {
            let credit = release["artist-credit"]
                .as_array()
                .map(|credits| {
                    credits
                        .iter()
                        .filter_map(|credit| credit["artist"]["name"].as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            let score = release["score"]
                .as_u64()
                .or_else(|| release["score"].as_str()?.parse().ok())
                .unwrap_or(0);
            score >= 95
                && normalized(release["title"].as_str().unwrap_or("")) == normalized(album)
                && normalized(&credit) == normalized(artist)
                && year.is_none_or(|year| {
                    release["date"]
                        .as_str()
                        .and_then(|date| date.split('-').next())
                        == Some(year.to_string().as_str())
                })
        })
        .collect::<Vec<_>>();
    // Conflicting editions require a manual choice. Do not pick the first search result.
    if matches.len() == 1 {
        matches[0]["id"].as_str()
    } else {
        None
    }
}

#[cfg(test)]
mod match_tests {
    use super::verified_release;
    #[test]
    fn rejects_wrong_artists_years_and_ambiguous_editions() {
        let release = serde_json::json!({"id":"one","title":"The Book","score":100,"date":"2020-01-01","artist-credit":[{"artist":{"name":"The Reader"}}]});
        let response = serde_json::json!({"releases":[release.clone()]});
        assert_eq!(
            verified_release(&response, "The Book", "The Reader", Some(2020)),
            Some("one")
        );
        assert_eq!(
            verified_release(&response, "The Book", "Other Reader", Some(2020)),
            None
        );
        assert_eq!(
            verified_release(&response, "The Book", "The Reader", Some(2021)),
            None
        );
        assert_eq!(
            verified_release(
                &serde_json::json!({"releases":[release.clone(),release]}),
                "The Book",
                "The Reader",
                None
            ),
            None
        );
    }
}
