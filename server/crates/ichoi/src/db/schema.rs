//! Diesel table definitions, kept in sync by hand with `migrations/`.

diesel::table! {
    accounts (id) {
        id -> Text,
        handle -> Text,
        display_name -> Nullable<Text>,
        role -> Text,
        created_at -> Text,
    }
}

diesel::table! {
    sessions (token_sha256) {
        token_sha256 -> Text,
        account_id -> Text,
        created_at -> Text,
        expires_at -> Text,
    }
}

diesel::table! {
    settings (key) {
        key -> Text,
        value -> Text,
    }
}

diesel::table! {
    trusted_domains (domain) {
        domain -> Text,
    }
}

diesel::table! {
    linkkeys_local_rp_identities (fingerprint) {
        fingerprint -> Text,
        name -> Text,
        identity_bundle -> Binary,
        active -> Integer,
        created_at -> Text,
        expires_at -> Text,
    }
}

diesel::table! {
    linkkeys_trusted_identities (domain, handle) {
        domain -> Text,
        handle -> Text,
        source -> Text,
        created_at -> Text,
    }
}

diesel::table! {
    linkkeys_login_attempts (attempt_sha256) {
        attempt_sha256 -> Text,
        pending_login -> Text,
        expected_handle -> Nullable<Text>,
        created_at -> Text,
        expires_at -> Text,
    }
}

diesel::table! {
    linkkeys_login_exchanges (code_sha256) {
        code_sha256 -> Text,
        account_id -> Text,
        created_at -> Text,
        expires_at -> Text,
    }
}

diesel::table! {
    libraries (id) {
        id -> Text,
        kind -> Text,
        path -> Text,
    }
}

diesel::table! {
    artists (id) {
        id -> Text,
        name -> Text,
    }
}

diesel::table! {
    albums (id) {
        id -> Text,
        title -> Text,
        artist_id -> Nullable<Text>,
        year -> Nullable<Integer>,
        has_cover_art -> Integer,
        cover_art_path -> Nullable<Text>,
        art_checked -> Integer,
    }
}

diesel::table! {
    tracks (id) {
        id -> Text,
        library_id -> Text,
        root_relative_path -> Text,
        title -> Text,
        artist_id -> Nullable<Text>,
        album_id -> Nullable<Text>,
        track_no -> Nullable<Integer>,
        disc_no -> Nullable<Integer>,
        duration_ms -> BigInt,
        codec -> Text,
        bitrate_kbps -> Nullable<Integer>,
        sample_rate -> Integer,
        channels -> Integer,
        bit_depth -> Nullable<Integer>,
        size_bytes -> BigInt,
        mtime -> Text,
        content_hash -> Nullable<Text>,
        trim_start_samples -> BigInt,
        trim_end_samples -> BigInt,
    }
}

diesel::table! {
    playlists (id) {
        id -> Text,
        name -> Text,
        owner -> Nullable<Text>,
        root_relative_path -> Text,
        visibility -> Text,
    }
}

diesel::table! {
    nodes (id) {
        id -> Text,
        kind -> Text,
        hostname -> Text,
        friendly_name -> Text,
        token_sha256 -> Nullable<Text>,
        platform -> Text,
        arch -> Text,
        audio_outputs -> Text,
        last_seen -> Nullable<Text>,
    }
}

diesel::table! {
    output_devices (id) {
        id -> Text,
        node_id -> Text,
        os_device_id -> Text,
        friendly_name -> Text,
        is_default -> Integer,
    }
}

diesel::table! {
    players (id) {
        id -> Text,
        kind -> Text,
        output_device_id -> Nullable<Text>,
        owner_account_id -> Nullable<Text>,
        name -> Text,
        name_suffix -> Nullable<Text>,
    }
}

diesel::table! {
    player_queue_items (id) {
        id -> Integer,
        player_id -> Text,
        track_id -> Text,
        position -> Integer,
    }
}

diesel::table! {
    player_state (player_id) {
        player_id -> Text,
        status -> Text,
        current_index -> Nullable<Integer>,
        position_ms -> Nullable<BigInt>,
        volume -> Integer,
    }
}

diesel::table! {
    listens (id) {
        id -> Integer,
        account_id -> Text,
        track_id -> Text,
        played_at -> Text,
    }
}

diesel::table! {
    stars (account_id, item_id, item_type) {
        account_id -> Text,
        item_id -> Text,
        item_type -> Text,
    }
}

diesel::table! {
    audiobook_progress (account_id, track_id) {
        account_id -> Text,
        track_id -> Text,
        position_ms -> BigInt,
        completed -> Integer,
        updated_at -> Text,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    accounts,
    sessions,
    settings,
    trusted_domains,
    linkkeys_local_rp_identities,
    linkkeys_trusted_identities,
    linkkeys_login_attempts,
    linkkeys_login_exchanges,
    libraries,
    artists,
    albums,
    tracks,
    playlists,
    nodes,
    output_devices,
    players,
    player_queue_items,
    player_state,
    listens,
    stars,
    audiobook_progress,
);
