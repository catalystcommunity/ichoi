//! Generated self-contained canonical-CBOR codec from CSIL specification.
//!
//! CSIL is the CBOR Service Interface Language; this codec owns the payload
//! wire (a CBOR map keyed by the verbatim CSIL field name in canonical RFC
//! 8949 order) so the generated types need no serde derive. One
//! `encode_`/`decode_` pair is emitted per record type.
#![allow(dead_code, clippy::vec_init_then_push)]

use super::types::*;

/// A decode failure: the CBOR was malformed or did not match the expected shape.
#[derive(Debug, Clone, PartialEq)]
pub struct CsilCborError(pub String);

impl std::fmt::Display for CsilCborError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CsilCborError {}

/// A minimal canonical-CBOR value tree: a closed set of variants the generated codec
/// builds and walks. A map is an ordered list of pairs, so the encoder controls the
/// wire order of a record's keys explicitly (laid down in canonical order).
#[derive(Debug, Clone, PartialEq)]
pub enum CsilCborValue {
    Uint(u64),
    Int(i64),
    Bool(bool),
    Float(f64),
    Null,
    Text(String),
    Bytes(Vec<u8>),
    Array(Vec<CsilCborValue>),
    Map(Vec<(CsilCborValue, CsilCborValue)>),
    Tag(u64, Box<CsilCborValue>),
}

fn cbor_int(x: i64) -> CsilCborValue {
    CsilCborValue::Int(x)
}
fn cbor_uint(x: u64) -> CsilCborValue {
    CsilCborValue::Uint(x)
}
fn cbor_float(x: f64) -> CsilCborValue {
    CsilCborValue::Float(x)
}
fn cbor_bool(x: bool) -> CsilCborValue {
    CsilCborValue::Bool(x)
}
fn cbor_text(x: &str) -> CsilCborValue {
    CsilCborValue::Text(x.to_string())
}
fn cbor_bytes(x: &[u8]) -> CsilCborValue {
    CsilCborValue::Bytes(x.to_vec())
}

/// Serialize a value tree to canonical CBOR bytes.
fn cbor_encode(v: &CsilCborValue) -> Vec<u8> {
    let mut out = Vec::new();
    cbor_enc(v, &mut out);
    out
}

fn cbor_head(major: u8, n: u64, out: &mut Vec<u8>) {
    let mt = major << 5;
    if n < 24 {
        out.push(mt | n as u8);
    } else if n < 0x100 {
        out.push(mt | 24);
        out.push(n as u8);
    } else if n < 0x10000 {
        out.push(mt | 25);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n < 0x1_0000_0000 {
        out.push(mt | 26);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.push(mt | 27);
        out.extend_from_slice(&n.to_be_bytes());
    }
}

fn cbor_enc(v: &CsilCborValue, out: &mut Vec<u8>) {
    match v {
        CsilCborValue::Uint(x) => cbor_head(0, *x, out),
        // A non-negative `Int` rides major type 0 so it is byte-identical to a `Uint`
        // of the same magnitude; only a genuinely negative value uses major type 1.
        CsilCborValue::Int(x) => {
            if *x >= 0 {
                cbor_head(0, *x as u64, out);
            } else {
                cbor_head(1, (-(*x + 1)) as u64, out);
            }
        }
        CsilCborValue::Bool(x) => out.push(if *x { 0xf5 } else { 0xf4 }),
        CsilCborValue::Null => out.push(0xf6),
        CsilCborValue::Float(x) => {
            out.push(0xfb);
            out.extend_from_slice(&x.to_bits().to_be_bytes());
        }
        CsilCborValue::Text(s) => {
            let bytes = s.as_bytes();
            cbor_head(3, bytes.len() as u64, out);
            out.extend_from_slice(bytes);
        }
        CsilCborValue::Bytes(b) => {
            cbor_head(2, b.len() as u64, out);
            out.extend_from_slice(b);
        }
        CsilCborValue::Array(items) => {
            cbor_head(4, items.len() as u64, out);
            for item in items {
                cbor_enc(item, out);
            }
        }
        CsilCborValue::Map(entries) => {
            cbor_head(5, entries.len() as u64, out);
            for (k, val) in entries {
                cbor_enc(k, out);
                cbor_enc(val, out);
            }
        }
        CsilCborValue::Tag(num, inner) => {
            cbor_head(6, *num, out);
            cbor_enc(inner, out);
        }
    }
}

/// Parse a full CBOR item and reject trailing bytes, so a payload that is not
/// exactly one value is an error rather than a silently-truncated read.
fn cbor_decode(b: &[u8]) -> Result<CsilCborValue, CsilCborError> {
    let mut pos = 0usize;
    let v = cbor_dec(b, &mut pos)?;
    if pos != b.len() {
        return Err(CsilCborError(format!(
            "csil cbor: {} trailing bytes",
            b.len() - pos
        )));
    }
    Ok(v)
}

fn cbor_read_arg(b: &[u8], pos: &mut usize, low: u8) -> Result<u64, CsilCborError> {
    if low < 24 {
        *pos += 1;
        return Ok(low as u64);
    }
    let width = match low {
        24 => 1usize,
        25 => 2,
        26 => 4,
        27 => 8,
        _ => {
            return Err(CsilCborError(format!(
                "csil cbor: reserved additional info {low}"
            )))
        }
    };
    if *pos + 1 + width > b.len() {
        return Err(CsilCborError("csil cbor: truncated argument".to_string()));
    }
    let mut v = 0u64;
    for &byte in &b[*pos + 1..*pos + 1 + width] {
        v = (v << 8) | byte as u64;
    }
    *pos += 1 + width;
    Ok(v)
}

fn cbor_dec(b: &[u8], pos: &mut usize) -> Result<CsilCborValue, CsilCborError> {
    if *pos >= b.len() {
        return Err(CsilCborError(
            "csil cbor: unexpected end of input".to_string(),
        ));
    }
    let ib = b[*pos];
    let major = ib >> 5;
    let low = ib & 0x1f;
    if major == 7 {
        return match low {
            20 => {
                *pos += 1;
                Ok(CsilCborValue::Bool(false))
            }
            21 => {
                *pos += 1;
                Ok(CsilCborValue::Bool(true))
            }
            22 | 23 => {
                *pos += 1;
                Ok(CsilCborValue::Null)
            }
            26 => {
                let bits = cbor_read_arg(b, pos, low)?;
                Ok(CsilCborValue::Float(f32::from_bits(bits as u32) as f64))
            }
            27 => {
                let bits = cbor_read_arg(b, pos, low)?;
                Ok(CsilCborValue::Float(f64::from_bits(bits)))
            }
            _ => Err(CsilCborError(format!(
                "csil cbor: unsupported simple value {low}"
            ))),
        };
    }
    let arg = cbor_read_arg(b, pos, low)?;
    match major {
        0 => Ok(CsilCborValue::Uint(arg)),
        1 => {
            if arg > i64::MAX as u64 {
                return Err(CsilCborError(
                    "csil cbor: negative integer out of range".to_string(),
                ));
            }
            Ok(CsilCborValue::Int(-1 - arg as i64))
        }
        2 => {
            let n = arg as usize;
            if *pos + n > b.len() {
                return Err(CsilCborError(
                    "csil cbor: truncated byte string".to_string(),
                ));
            }
            let slice = b[*pos..*pos + n].to_vec();
            *pos += n;
            Ok(CsilCborValue::Bytes(slice))
        }
        3 => {
            let n = arg as usize;
            if *pos + n > b.len() {
                return Err(CsilCborError(
                    "csil cbor: truncated text string".to_string(),
                ));
            }
            let s = std::str::from_utf8(&b[*pos..*pos + n])
                .map_err(|e| CsilCborError(format!("csil cbor: invalid utf-8: {e}")))?
                .to_string();
            *pos += n;
            Ok(CsilCborValue::Text(s))
        }
        4 => {
            let n = arg as usize;
            let mut items = Vec::with_capacity(n);
            for _ in 0..n {
                items.push(cbor_dec(b, pos)?);
            }
            Ok(CsilCborValue::Array(items))
        }
        5 => {
            let n = arg as usize;
            let mut entries = Vec::with_capacity(n);
            for _ in 0..n {
                let k = cbor_dec(b, pos)?;
                let val = cbor_dec(b, pos)?;
                entries.push((k, val));
            }
            Ok(CsilCborValue::Map(entries))
        }
        6 => {
            let inner = cbor_dec(b, pos)?;
            Ok(CsilCborValue::Tag(arg, Box::new(inner)))
        }
        _ => Err(CsilCborError(format!(
            "csil cbor: unexpected major type {major}"
        ))),
    }
}

/// Map a typed slice to a CBOR array via the per-element encoder.
fn cbor_enc_array<E>(xs: &[E], f: impl Fn(&E) -> CsilCborValue) -> CsilCborValue {
    CsilCborValue::Array(xs.iter().map(f).collect())
}

/// Map a typed map to a CBOR map. Rust `HashMap` iteration is unordered, so the inner
/// map's entry order is not canonicalized; the record's own keys (laid down at
/// generation time) are what the cross-language wire contract pins.
fn cbor_enc_map<K, V>(
    m: &std::collections::HashMap<K, V>,
    kf: impl Fn(&K) -> CsilCborValue,
    vf: impl Fn(&V) -> CsilCborValue,
) -> CsilCborValue {
    CsilCborValue::Map(m.iter().map(|(k, v)| (kf(k), vf(v))).collect())
}

fn cbor_dec_array<E>(
    v: &CsilCborValue,
    f: impl Fn(&CsilCborValue) -> Result<E, CsilCborError>,
) -> Result<Vec<E>, CsilCborError> {
    cbor_as_array(v)?.iter().map(f).collect()
}

fn cbor_dec_map<K: std::cmp::Eq + std::hash::Hash, V>(
    v: &CsilCborValue,
    kf: impl Fn(&CsilCborValue) -> Result<K, CsilCborError>,
    vf: impl Fn(&CsilCborValue) -> Result<V, CsilCborError>,
) -> Result<std::collections::HashMap<K, V>, CsilCborError> {
    let entries = cbor_as_map(v)?;
    let mut out = std::collections::HashMap::with_capacity(entries.len());
    for (k, val) in entries {
        out.insert(kf(k)?, vf(val)?);
    }
    Ok(out)
}

fn cbor_map_get<'a>(v: &'a CsilCborValue, key: &str) -> Option<&'a CsilCborValue> {
    if let CsilCborValue::Map(entries) = v {
        for (k, val) in entries {
            if matches!(k, CsilCborValue::Text(name) if name == key) {
                return Some(val);
            }
        }
    }
    None
}

fn cbor_expect_value(v: &CsilCborValue, expected: &CsilCborValue) -> Result<(), CsilCborError> {
    if v == expected {
        Ok(())
    } else {
        Err(CsilCborError(format!(
            "csil cbor: expected literal {expected:?}, got {v:?}"
        )))
    }
}

fn cbor_require<'a>(v: &'a CsilCborValue, key: &str) -> Result<&'a CsilCborValue, CsilCborError> {
    cbor_map_get(v, key).ok_or_else(|| CsilCborError(format!("csil cbor: missing field {key:?}")))
}

fn cbor_as_i64(v: &CsilCborValue) -> Result<i64, CsilCborError> {
    match v {
        CsilCborValue::Uint(x) => i64::try_from(*x)
            .map_err(|_| CsilCborError("csil cbor: integer overflows i64".to_string())),
        CsilCborValue::Int(x) => Ok(*x),
        _ => Err(CsilCborError("csil cbor: expected integer".to_string())),
    }
}

fn cbor_as_u64(v: &CsilCborValue) -> Result<u64, CsilCborError> {
    match v {
        CsilCborValue::Uint(x) => Ok(*x),
        CsilCborValue::Int(x) if *x >= 0 => Ok(*x as u64),
        CsilCborValue::Int(_) => Err(CsilCborError(
            "csil cbor: negative integer where unsigned expected".to_string(),
        )),
        _ => Err(CsilCborError(
            "csil cbor: expected unsigned integer".to_string(),
        )),
    }
}

fn cbor_as_f64(v: &CsilCborValue) -> Result<f64, CsilCborError> {
    match v {
        CsilCborValue::Float(x) => Ok(*x),
        CsilCborValue::Uint(x) => Ok(*x as f64),
        CsilCborValue::Int(x) => Ok(*x as f64),
        _ => Err(CsilCborError("csil cbor: expected float".to_string())),
    }
}

fn cbor_as_bool(v: &CsilCborValue) -> Result<bool, CsilCborError> {
    match v {
        CsilCborValue::Bool(b) => Ok(*b),
        _ => Err(CsilCborError("csil cbor: expected bool".to_string())),
    }
}

fn cbor_as_text(v: &CsilCborValue) -> Result<String, CsilCborError> {
    match v {
        CsilCborValue::Text(s) => Ok(s.clone()),
        _ => Err(CsilCborError("csil cbor: expected text".to_string())),
    }
}

fn cbor_as_bytes(v: &CsilCborValue) -> Result<Vec<u8>, CsilCborError> {
    match v {
        CsilCborValue::Bytes(b) => Ok(b.clone()),
        _ => Err(CsilCborError("csil cbor: expected byte string".to_string())),
    }
}

fn cbor_as_array(v: &CsilCborValue) -> Result<&[CsilCborValue], CsilCborError> {
    match v {
        CsilCborValue::Array(a) => Ok(a),
        _ => Err(CsilCborError("csil cbor: expected array".to_string())),
    }
}

fn cbor_as_map(v: &CsilCborValue) -> Result<&[(CsilCborValue, CsilCborValue)], CsilCborError> {
    match v {
        CsilCborValue::Map(m) => Ok(m),
        _ => Err(CsilCborError("csil cbor: expected map".to_string())),
    }
}

/// Encode a UTC instant as CBOR tag 0 RFC3339 text in UTC, per the wire contract;
/// sub-second precision is preserved when present and the `Z` offset is forced.
fn csil_enc_timestamp(t: &chrono::DateTime<chrono::Utc>) -> CsilCborValue {
    let text = t.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
    CsilCborValue::Tag(0, Box::new(CsilCborValue::Text(text)))
}

/// Decode a CBOR tag 0 RFC3339 timestamp back to a UTC instant.
fn csil_as_timestamp(v: &CsilCborValue) -> Result<chrono::DateTime<chrono::Utc>, CsilCborError> {
    let CsilCborValue::Tag(0, inner) = v else {
        return Err(CsilCborError(
            "csil cbor: expected CBOR tag 0 timestamp".to_string(),
        ));
    };
    let CsilCborValue::Text(s) = inner.as_ref() else {
        return Err(CsilCborError(
            "csil cbor: timestamp content must be text".to_string(),
        ));
    };
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| CsilCborError(format!("csil cbor: invalid timestamp: {e}")))
}

/// Build the canonical CBOR value tree for a StreamPref.
fn csil_enc_stream_pref(csil_v: &StreamPref) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(3);
    if let Some(csil_inner) = &csil_v.prefer_original {
        csil_entries.push((cbor_text("prefer_original"), cbor_bool(*csil_inner)));
    }
    if let Some(csil_inner) = &csil_v.transcode_codec {
        csil_entries.push((
            cbor_text("transcode_codec"),
            csil_enc_transcode_codec(csil_inner),
        ));
    }
    if let Some(csil_inner) = &csil_v.max_bitrate_kbps {
        csil_entries.push((cbor_text("max_bitrate_kbps"), cbor_uint(*csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a StreamPref from a decoded CBOR value tree.
fn csil_dec_stream_pref(csil_root: &CsilCborValue) -> Result<StreamPref, CsilCborError> {
    let max_bitrate_kbps = match cbor_map_get(csil_root, "max_bitrate_kbps") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let prefer_original = match cbor_map_get(csil_root, "prefer_original") {
        Some(csil_field) => {
            let csil_decode = cbor_as_bool;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let transcode_codec = match cbor_map_get(csil_root, "transcode_codec") {
        Some(csil_field) => {
            let csil_decode = csil_dec_transcode_codec;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(StreamPref {
        max_bitrate_kbps,
        prefer_original,
        transcode_codec,
    })
}

/// Encode a StreamPref to canonical CSIL CBOR bytes.
pub fn encode_stream_pref(csil_v: &StreamPref) -> Vec<u8> {
    cbor_encode(&csil_enc_stream_pref(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a StreamPref.
pub fn decode_stream_pref(csil_data: &[u8]) -> Result<StreamPref, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_stream_pref(&csil_root)
}

/// Build the canonical CBOR value tree for a Page.
fn csil_enc_page(csil_v: &Page) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    if let Some(csil_inner) = &csil_v.limit {
        csil_entries.push((cbor_text("limit"), cbor_uint(*csil_inner)));
    }
    if let Some(csil_inner) = &csil_v.offset {
        csil_entries.push((cbor_text("offset"), cbor_uint(*csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a Page from a decoded CBOR value tree.
fn csil_dec_page(csil_root: &CsilCborValue) -> Result<Page, CsilCborError> {
    let offset = match cbor_map_get(csil_root, "offset") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let limit = match cbor_map_get(csil_root, "limit") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(Page { offset, limit })
}

/// Encode a Page to canonical CSIL CBOR bytes.
pub fn encode_page(csil_v: &Page) -> Vec<u8> {
    cbor_encode(&csil_enc_page(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a Page.
pub fn decode_page(csil_data: &[u8]) -> Result<Page, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_page(&csil_root)
}

/// Build the canonical CBOR value tree for a Ok.
fn csil_enc_ok(csil_v: &Ok) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("ok"), cbor_bool(csil_v.ok)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a Ok from a decoded CBOR value tree.
fn csil_dec_ok(csil_root: &CsilCborValue) -> Result<Ok, CsilCborError> {
    let ok = {
        let csil_field = cbor_require(csil_root, "ok")?;
        let csil_decode = cbor_as_bool;
        csil_decode(csil_field)?
    };
    Ok(Ok { ok })
}

/// Encode a Ok to canonical CSIL CBOR bytes.
pub fn encode_ok(csil_v: &Ok) -> Vec<u8> {
    cbor_encode(&csil_enc_ok(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a Ok.
pub fn decode_ok(csil_data: &[u8]) -> Result<Ok, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_ok(&csil_root)
}

/// Build the canonical CBOR value tree for a ServiceError.
fn csil_enc_service_error(csil_v: &ServiceError) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("code"), cbor_int(csil_v.code)));
    csil_entries.push((cbor_text("message"), cbor_text(&csil_v.message)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a ServiceError from a decoded CBOR value tree.
fn csil_dec_service_error(csil_root: &CsilCborValue) -> Result<ServiceError, CsilCborError> {
    let code = {
        let csil_field = cbor_require(csil_root, "code")?;
        let csil_decode = cbor_as_i64;
        csil_decode(csil_field)?
    };
    let message = {
        let csil_field = cbor_require(csil_root, "message")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(ServiceError { code, message })
}

/// Encode a ServiceError to canonical CSIL CBOR bytes.
pub fn encode_service_error(csil_v: &ServiceError) -> Vec<u8> {
    cbor_encode(&csil_enc_service_error(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a ServiceError.
pub fn decode_service_error(csil_data: &[u8]) -> Result<ServiceError, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_service_error(&csil_root)
}

/// Build the canonical CBOR value tree for a AuthRequest.
fn csil_enc_auth_request(csil_v: &AuthRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    if let Some(csil_inner) = &csil_v.bootstrap_token {
        csil_entries.push((cbor_text("bootstrap_token"), cbor_text(csil_inner)));
    }
    if let Some(csil_inner) = &csil_v.linkkeys_assertion {
        csil_entries.push((cbor_text("linkkeys_assertion"), cbor_bytes(csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a AuthRequest from a decoded CBOR value tree.
fn csil_dec_auth_request(csil_root: &CsilCborValue) -> Result<AuthRequest, CsilCborError> {
    let linkkeys_assertion = match cbor_map_get(csil_root, "linkkeys_assertion") {
        Some(csil_field) => {
            let csil_decode = cbor_as_bytes;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let bootstrap_token = match cbor_map_get(csil_root, "bootstrap_token") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(AuthRequest {
        linkkeys_assertion,
        bootstrap_token,
    })
}

/// Encode a AuthRequest to canonical CSIL CBOR bytes.
pub fn encode_auth_request(csil_v: &AuthRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_auth_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a AuthRequest.
pub fn decode_auth_request(csil_data: &[u8]) -> Result<AuthRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_auth_request(&csil_root)
}

/// Build the canonical CBOR value tree for a SessionInfo.
fn csil_enc_session_info(csil_v: &SessionInfo) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(5);
    csil_entries.push((cbor_text("role"), csil_enc_role(&csil_v.role)));
    if let Some(csil_inner) = &csil_v.token {
        csil_entries.push((cbor_text("token"), cbor_text(csil_inner)));
    }
    csil_entries.push((cbor_text("handle"), cbor_text(&csil_v.handle)));
    csil_entries.push((cbor_text("account_id"), cbor_text(&csil_v.account_id)));
    if let Some(csil_inner) = &csil_v.display_name {
        csil_entries.push((cbor_text("display_name"), cbor_text(csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a SessionInfo from a decoded CBOR value tree.
fn csil_dec_session_info(csil_root: &CsilCborValue) -> Result<SessionInfo, CsilCborError> {
    let account_id = {
        let csil_field = cbor_require(csil_root, "account_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let handle = {
        let csil_field = cbor_require(csil_root, "handle")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let display_name = match cbor_map_get(csil_root, "display_name") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let role = {
        let csil_field = cbor_require(csil_root, "role")?;
        let csil_decode = csil_dec_role;
        csil_decode(csil_field)?
    };
    let token = match cbor_map_get(csil_root, "token") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(SessionInfo {
        account_id,
        handle,
        display_name,
        role,
        token,
    })
}

/// Encode a SessionInfo to canonical CSIL CBOR bytes.
pub fn encode_session_info(csil_v: &SessionInfo) -> Vec<u8> {
    cbor_encode(&csil_enc_session_info(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a SessionInfo.
pub fn decode_session_info(csil_data: &[u8]) -> Result<SessionInfo, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_session_info(&csil_root)
}

/// Build the canonical CBOR value tree for a Track.
fn csil_enc_track(csil_v: &Track) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(14);
    csil_entries.push((cbor_text("id"), cbor_text(&csil_v.id)));
    csil_entries.push((cbor_text("codec"), csil_enc_codec(&csil_v.codec)));
    csil_entries.push((cbor_text("title"), cbor_text(&csil_v.title)));
    if let Some(csil_inner) = &csil_v.disc_no {
        csil_entries.push((cbor_text("disc_no"), cbor_uint(*csil_inner)));
    }
    if let Some(csil_inner) = &csil_v.album_id {
        csil_entries.push((cbor_text("album_id"), cbor_text(csil_inner)));
    }
    csil_entries.push((cbor_text("channels"), cbor_uint(csil_v.channels)));
    if let Some(csil_inner) = &csil_v.track_no {
        csil_entries.push((cbor_text("track_no"), cbor_uint(*csil_inner)));
    }
    if let Some(csil_inner) = &csil_v.artist_id {
        csil_entries.push((cbor_text("artist_id"), cbor_text(csil_inner)));
    }
    if let Some(csil_inner) = &csil_v.bit_depth {
        csil_entries.push((cbor_text("bit_depth"), cbor_uint(*csil_inner)));
    }
    csil_entries.push((cbor_text("duration_ms"), cbor_uint(csil_v.duration_ms)));
    csil_entries.push((cbor_text("sample_rate"), cbor_uint(csil_v.sample_rate)));
    if let Some(csil_inner) = &csil_v.bitrate_kbps {
        csil_entries.push((cbor_text("bitrate_kbps"), cbor_uint(*csil_inner)));
    }
    if let Some(csil_inner) = &csil_v.content_hash {
        csil_entries.push((cbor_text("content_hash"), cbor_text(csil_inner)));
    }
    csil_entries.push((
        cbor_text("root_relative_path"),
        cbor_text(&csil_v.root_relative_path),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a Track from a decoded CBOR value tree.
fn csil_dec_track(csil_root: &CsilCborValue) -> Result<Track, CsilCborError> {
    let id = {
        let csil_field = cbor_require(csil_root, "id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let title = {
        let csil_field = cbor_require(csil_root, "title")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let artist_id = match cbor_map_get(csil_root, "artist_id") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let album_id = match cbor_map_get(csil_root, "album_id") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let track_no = match cbor_map_get(csil_root, "track_no") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let disc_no = match cbor_map_get(csil_root, "disc_no") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let duration_ms = {
        let csil_field = cbor_require(csil_root, "duration_ms")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    let codec = {
        let csil_field = cbor_require(csil_root, "codec")?;
        let csil_decode = csil_dec_codec;
        csil_decode(csil_field)?
    };
    let bitrate_kbps = match cbor_map_get(csil_root, "bitrate_kbps") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let sample_rate = {
        let csil_field = cbor_require(csil_root, "sample_rate")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    let channels = {
        let csil_field = cbor_require(csil_root, "channels")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    let bit_depth = match cbor_map_get(csil_root, "bit_depth") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let root_relative_path = {
        let csil_field = cbor_require(csil_root, "root_relative_path")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let content_hash = match cbor_map_get(csil_root, "content_hash") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(Track {
        id,
        title,
        artist_id,
        album_id,
        track_no,
        disc_no,
        duration_ms,
        codec,
        bitrate_kbps,
        sample_rate,
        channels,
        bit_depth,
        root_relative_path,
        content_hash,
    })
}

/// Encode a Track to canonical CSIL CBOR bytes.
pub fn encode_track(csil_v: &Track) -> Vec<u8> {
    cbor_encode(&csil_enc_track(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a Track.
pub fn decode_track(csil_data: &[u8]) -> Result<Track, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_track(&csil_root)
}

/// Build the canonical CBOR value tree for a Album.
fn csil_enc_album(csil_v: &Album) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(6);
    csil_entries.push((cbor_text("id"), cbor_text(&csil_v.id)));
    if let Some(csil_inner) = &csil_v.year {
        csil_entries.push((cbor_text("year"), cbor_uint(*csil_inner)));
    }
    csil_entries.push((cbor_text("title"), cbor_text(&csil_v.title)));
    if let Some(csil_inner) = &csil_v.artist_id {
        csil_entries.push((cbor_text("artist_id"), cbor_text(csil_inner)));
    }
    csil_entries.push((cbor_text("track_count"), cbor_uint(csil_v.track_count)));
    csil_entries.push((cbor_text("has_cover_art"), cbor_bool(csil_v.has_cover_art)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a Album from a decoded CBOR value tree.
fn csil_dec_album(csil_root: &CsilCborValue) -> Result<Album, CsilCborError> {
    let id = {
        let csil_field = cbor_require(csil_root, "id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let title = {
        let csil_field = cbor_require(csil_root, "title")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let artist_id = match cbor_map_get(csil_root, "artist_id") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let year = match cbor_map_get(csil_root, "year") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let has_cover_art = {
        let csil_field = cbor_require(csil_root, "has_cover_art")?;
        let csil_decode = cbor_as_bool;
        csil_decode(csil_field)?
    };
    let track_count = {
        let csil_field = cbor_require(csil_root, "track_count")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    Ok(Album {
        id,
        title,
        artist_id,
        year,
        has_cover_art,
        track_count,
    })
}

/// Encode a Album to canonical CSIL CBOR bytes.
pub fn encode_album(csil_v: &Album) -> Vec<u8> {
    cbor_encode(&csil_enc_album(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a Album.
pub fn decode_album(csil_data: &[u8]) -> Result<Album, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_album(&csil_root)
}

/// Build the canonical CBOR value tree for a Artist.
fn csil_enc_artist(csil_v: &Artist) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(3);
    csil_entries.push((cbor_text("id"), cbor_text(&csil_v.id)));
    csil_entries.push((cbor_text("name"), cbor_text(&csil_v.name)));
    csil_entries.push((cbor_text("album_count"), cbor_uint(csil_v.album_count)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a Artist from a decoded CBOR value tree.
fn csil_dec_artist(csil_root: &CsilCborValue) -> Result<Artist, CsilCborError> {
    let id = {
        let csil_field = cbor_require(csil_root, "id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let name = {
        let csil_field = cbor_require(csil_root, "name")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let album_count = {
        let csil_field = cbor_require(csil_root, "album_count")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    Ok(Artist {
        id,
        name,
        album_count,
    })
}

/// Encode a Artist to canonical CSIL CBOR bytes.
pub fn encode_artist(csil_v: &Artist) -> Vec<u8> {
    cbor_encode(&csil_enc_artist(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a Artist.
pub fn decode_artist(csil_data: &[u8]) -> Result<Artist, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_artist(&csil_root)
}

/// Build the canonical CBOR value tree for a Playlist.
fn csil_enc_playlist(csil_v: &Playlist) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(5);
    csil_entries.push((cbor_text("id"), cbor_text(&csil_v.id)));
    csil_entries.push((cbor_text("name"), cbor_text(&csil_v.name)));
    if let Some(csil_inner) = &csil_v.owner {
        csil_entries.push((cbor_text("owner"), cbor_text(csil_inner)));
    }
    csil_entries.push((cbor_text("entry_count"), cbor_uint(csil_v.entry_count)));
    csil_entries.push((
        cbor_text("root_relative_path"),
        cbor_text(&csil_v.root_relative_path),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a Playlist from a decoded CBOR value tree.
fn csil_dec_playlist(csil_root: &CsilCborValue) -> Result<Playlist, CsilCborError> {
    let id = {
        let csil_field = cbor_require(csil_root, "id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let name = {
        let csil_field = cbor_require(csil_root, "name")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let owner = match cbor_map_get(csil_root, "owner") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let entry_count = {
        let csil_field = cbor_require(csil_root, "entry_count")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    let root_relative_path = {
        let csil_field = cbor_require(csil_root, "root_relative_path")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(Playlist {
        id,
        name,
        owner,
        entry_count,
        root_relative_path,
    })
}

/// Encode a Playlist to canonical CSIL CBOR bytes.
pub fn encode_playlist(csil_v: &Playlist) -> Vec<u8> {
    cbor_encode(&csil_enc_playlist(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a Playlist.
pub fn decode_playlist(csil_data: &[u8]) -> Result<Playlist, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_playlist(&csil_root)
}

/// Build the canonical CBOR value tree for a BrowseRequest.
fn csil_enc_browse_request(csil_v: &BrowseRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(3);
    if let Some(csil_inner) = &csil_v.limit {
        csil_entries.push((cbor_text("limit"), cbor_uint(*csil_inner)));
    }
    if let Some(csil_inner) = &csil_v.offset {
        csil_entries.push((cbor_text("offset"), cbor_uint(*csil_inner)));
    }
    if let Some(csil_inner) = &csil_v.library {
        csil_entries.push((cbor_text("library"), csil_enc_library(csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a BrowseRequest from a decoded CBOR value tree.
fn csil_dec_browse_request(csil_root: &CsilCborValue) -> Result<BrowseRequest, CsilCborError> {
    let library = match cbor_map_get(csil_root, "library") {
        Some(csil_field) => {
            let csil_decode = csil_dec_library;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let offset = match cbor_map_get(csil_root, "offset") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let limit = match cbor_map_get(csil_root, "limit") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(BrowseRequest {
        library,
        offset,
        limit,
    })
}

/// Encode a BrowseRequest to canonical CSIL CBOR bytes.
pub fn encode_browse_request(csil_v: &BrowseRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_browse_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a BrowseRequest.
pub fn decode_browse_request(csil_data: &[u8]) -> Result<BrowseRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_browse_request(&csil_root)
}

/// Build the canonical CBOR value tree for a AlbumsResponse.
fn csil_enc_albums_response(csil_v: &AlbumsResponse) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("total"), cbor_uint(csil_v.total)));
    csil_entries.push((
        cbor_text("albums"),
        cbor_enc_array(&csil_v.albums, csil_enc_album),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a AlbumsResponse from a decoded CBOR value tree.
fn csil_dec_albums_response(csil_root: &CsilCborValue) -> Result<AlbumsResponse, CsilCborError> {
    let albums = {
        let csil_field = cbor_require(csil_root, "albums")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_album);
        csil_decode(csil_field)?
    };
    let total = {
        let csil_field = cbor_require(csil_root, "total")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    Ok(AlbumsResponse { albums, total })
}

/// Encode a AlbumsResponse to canonical CSIL CBOR bytes.
pub fn encode_albums_response(csil_v: &AlbumsResponse) -> Vec<u8> {
    cbor_encode(&csil_enc_albums_response(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a AlbumsResponse.
pub fn decode_albums_response(csil_data: &[u8]) -> Result<AlbumsResponse, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_albums_response(&csil_root)
}

/// Build the canonical CBOR value tree for a ArtistsResponse.
fn csil_enc_artists_response(csil_v: &ArtistsResponse) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("total"), cbor_uint(csil_v.total)));
    csil_entries.push((
        cbor_text("artists"),
        cbor_enc_array(&csil_v.artists, csil_enc_artist),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a ArtistsResponse from a decoded CBOR value tree.
fn csil_dec_artists_response(csil_root: &CsilCborValue) -> Result<ArtistsResponse, CsilCborError> {
    let artists = {
        let csil_field = cbor_require(csil_root, "artists")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_artist);
        csil_decode(csil_field)?
    };
    let total = {
        let csil_field = cbor_require(csil_root, "total")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    Ok(ArtistsResponse { artists, total })
}

/// Encode a ArtistsResponse to canonical CSIL CBOR bytes.
pub fn encode_artists_response(csil_v: &ArtistsResponse) -> Vec<u8> {
    cbor_encode(&csil_enc_artists_response(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a ArtistsResponse.
pub fn decode_artists_response(csil_data: &[u8]) -> Result<ArtistsResponse, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_artists_response(&csil_root)
}

/// Build the canonical CBOR value tree for a AlbumRequest.
fn csil_enc_album_request(csil_v: &AlbumRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("album_id"), cbor_text(&csil_v.album_id)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a AlbumRequest from a decoded CBOR value tree.
fn csil_dec_album_request(csil_root: &CsilCborValue) -> Result<AlbumRequest, CsilCborError> {
    let album_id = {
        let csil_field = cbor_require(csil_root, "album_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(AlbumRequest { album_id })
}

/// Encode a AlbumRequest to canonical CSIL CBOR bytes.
pub fn encode_album_request(csil_v: &AlbumRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_album_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a AlbumRequest.
pub fn decode_album_request(csil_data: &[u8]) -> Result<AlbumRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_album_request(&csil_root)
}

/// Build the canonical CBOR value tree for a AlbumDetail.
fn csil_enc_album_detail(csil_v: &AlbumDetail) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("album"), csil_enc_album(&csil_v.album)));
    csil_entries.push((
        cbor_text("tracks"),
        cbor_enc_array(&csil_v.tracks, csil_enc_track),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a AlbumDetail from a decoded CBOR value tree.
fn csil_dec_album_detail(csil_root: &CsilCborValue) -> Result<AlbumDetail, CsilCborError> {
    let album = {
        let csil_field = cbor_require(csil_root, "album")?;
        let csil_decode = csil_dec_album;
        csil_decode(csil_field)?
    };
    let tracks = {
        let csil_field = cbor_require(csil_root, "tracks")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_track);
        csil_decode(csil_field)?
    };
    Ok(AlbumDetail { album, tracks })
}

/// Encode a AlbumDetail to canonical CSIL CBOR bytes.
pub fn encode_album_detail(csil_v: &AlbumDetail) -> Vec<u8> {
    cbor_encode(&csil_enc_album_detail(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a AlbumDetail.
pub fn decode_album_detail(csil_data: &[u8]) -> Result<AlbumDetail, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_album_detail(&csil_root)
}

/// Build the canonical CBOR value tree for a ArtistRequest.
fn csil_enc_artist_request(csil_v: &ArtistRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("artist_id"), cbor_text(&csil_v.artist_id)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a ArtistRequest from a decoded CBOR value tree.
fn csil_dec_artist_request(csil_root: &CsilCborValue) -> Result<ArtistRequest, CsilCborError> {
    let artist_id = {
        let csil_field = cbor_require(csil_root, "artist_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(ArtistRequest { artist_id })
}

/// Encode a ArtistRequest to canonical CSIL CBOR bytes.
pub fn encode_artist_request(csil_v: &ArtistRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_artist_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a ArtistRequest.
pub fn decode_artist_request(csil_data: &[u8]) -> Result<ArtistRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_artist_request(&csil_root)
}

/// Build the canonical CBOR value tree for a ArtistDetail.
fn csil_enc_artist_detail(csil_v: &ArtistDetail) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((
        cbor_text("albums"),
        cbor_enc_array(&csil_v.albums, csil_enc_album),
    ));
    csil_entries.push((cbor_text("artist"), csil_enc_artist(&csil_v.artist)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a ArtistDetail from a decoded CBOR value tree.
fn csil_dec_artist_detail(csil_root: &CsilCborValue) -> Result<ArtistDetail, CsilCborError> {
    let artist = {
        let csil_field = cbor_require(csil_root, "artist")?;
        let csil_decode = csil_dec_artist;
        csil_decode(csil_field)?
    };
    let albums = {
        let csil_field = cbor_require(csil_root, "albums")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_album);
        csil_decode(csil_field)?
    };
    Ok(ArtistDetail { artist, albums })
}

/// Encode a ArtistDetail to canonical CSIL CBOR bytes.
pub fn encode_artist_detail(csil_v: &ArtistDetail) -> Vec<u8> {
    cbor_encode(&csil_enc_artist_detail(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a ArtistDetail.
pub fn decode_artist_detail(csil_data: &[u8]) -> Result<ArtistDetail, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_artist_detail(&csil_root)
}

/// Build the canonical CBOR value tree for a SearchRequest.
fn csil_enc_search_request(csil_v: &SearchRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    if let Some(csil_inner) = &csil_v.limit {
        csil_entries.push((cbor_text("limit"), cbor_uint(*csil_inner)));
    }
    csil_entries.push((cbor_text("query"), cbor_text(&csil_v.query)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a SearchRequest from a decoded CBOR value tree.
fn csil_dec_search_request(csil_root: &CsilCborValue) -> Result<SearchRequest, CsilCborError> {
    let query = {
        let csil_field = cbor_require(csil_root, "query")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let limit = match cbor_map_get(csil_root, "limit") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(SearchRequest { query, limit })
}

/// Encode a SearchRequest to canonical CSIL CBOR bytes.
pub fn encode_search_request(csil_v: &SearchRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_search_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a SearchRequest.
pub fn decode_search_request(csil_data: &[u8]) -> Result<SearchRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_search_request(&csil_root)
}

/// Build the canonical CBOR value tree for a SearchResponse.
fn csil_enc_search_response(csil_v: &SearchResponse) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(3);
    csil_entries.push((
        cbor_text("albums"),
        cbor_enc_array(&csil_v.albums, csil_enc_album),
    ));
    csil_entries.push((
        cbor_text("tracks"),
        cbor_enc_array(&csil_v.tracks, csil_enc_track),
    ));
    csil_entries.push((
        cbor_text("artists"),
        cbor_enc_array(&csil_v.artists, csil_enc_artist),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a SearchResponse from a decoded CBOR value tree.
fn csil_dec_search_response(csil_root: &CsilCborValue) -> Result<SearchResponse, CsilCborError> {
    let artists = {
        let csil_field = cbor_require(csil_root, "artists")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_artist);
        csil_decode(csil_field)?
    };
    let albums = {
        let csil_field = cbor_require(csil_root, "albums")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_album);
        csil_decode(csil_field)?
    };
    let tracks = {
        let csil_field = cbor_require(csil_root, "tracks")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_track);
        csil_decode(csil_field)?
    };
    Ok(SearchResponse {
        artists,
        albums,
        tracks,
    })
}

/// Encode a SearchResponse to canonical CSIL CBOR bytes.
pub fn encode_search_response(csil_v: &SearchResponse) -> Vec<u8> {
    cbor_encode(&csil_enc_search_response(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a SearchResponse.
pub fn decode_search_response(csil_data: &[u8]) -> Result<SearchResponse, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_search_response(&csil_root)
}

/// Build the canonical CBOR value tree for a PlaylistsResponse.
fn csil_enc_playlists_response(csil_v: &PlaylistsResponse) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((
        cbor_text("playlists"),
        cbor_enc_array(&csil_v.playlists, csil_enc_playlist),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a PlaylistsResponse from a decoded CBOR value tree.
fn csil_dec_playlists_response(
    csil_root: &CsilCborValue,
) -> Result<PlaylistsResponse, CsilCborError> {
    let playlists = {
        let csil_field = cbor_require(csil_root, "playlists")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_playlist);
        csil_decode(csil_field)?
    };
    Ok(PlaylistsResponse { playlists })
}

/// Encode a PlaylistsResponse to canonical CSIL CBOR bytes.
pub fn encode_playlists_response(csil_v: &PlaylistsResponse) -> Vec<u8> {
    cbor_encode(&csil_enc_playlists_response(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a PlaylistsResponse.
pub fn decode_playlists_response(csil_data: &[u8]) -> Result<PlaylistsResponse, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_playlists_response(&csil_root)
}

/// Build the canonical CBOR value tree for a PlaylistRequest.
fn csil_enc_playlist_request(csil_v: &PlaylistRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("playlist_id"), cbor_text(&csil_v.playlist_id)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a PlaylistRequest from a decoded CBOR value tree.
fn csil_dec_playlist_request(csil_root: &CsilCborValue) -> Result<PlaylistRequest, CsilCborError> {
    let playlist_id = {
        let csil_field = cbor_require(csil_root, "playlist_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(PlaylistRequest { playlist_id })
}

/// Encode a PlaylistRequest to canonical CSIL CBOR bytes.
pub fn encode_playlist_request(csil_v: &PlaylistRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_playlist_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a PlaylistRequest.
pub fn decode_playlist_request(csil_data: &[u8]) -> Result<PlaylistRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_playlist_request(&csil_root)
}

/// Build the canonical CBOR value tree for a PlaylistDetail.
fn csil_enc_playlist_detail(csil_v: &PlaylistDetail) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((
        cbor_text("tracks"),
        cbor_enc_array(&csil_v.tracks, csil_enc_track),
    ));
    csil_entries.push((cbor_text("playlist"), csil_enc_playlist(&csil_v.playlist)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a PlaylistDetail from a decoded CBOR value tree.
fn csil_dec_playlist_detail(csil_root: &CsilCborValue) -> Result<PlaylistDetail, CsilCborError> {
    let playlist = {
        let csil_field = cbor_require(csil_root, "playlist")?;
        let csil_decode = csil_dec_playlist;
        csil_decode(csil_field)?
    };
    let tracks = {
        let csil_field = cbor_require(csil_root, "tracks")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_track);
        csil_decode(csil_field)?
    };
    Ok(PlaylistDetail { playlist, tracks })
}

/// Encode a PlaylistDetail to canonical CSIL CBOR bytes.
pub fn encode_playlist_detail(csil_v: &PlaylistDetail) -> Vec<u8> {
    cbor_encode(&csil_enc_playlist_detail(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a PlaylistDetail.
pub fn decode_playlist_detail(csil_data: &[u8]) -> Result<PlaylistDetail, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_playlist_detail(&csil_root)
}

/// Build the canonical CBOR value tree for a CoverArtRequest.
fn csil_enc_cover_art_request(csil_v: &CoverArtRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("album_id"), cbor_text(&csil_v.album_id)));
    if let Some(csil_inner) = &csil_v.max_size {
        csil_entries.push((cbor_text("max_size"), cbor_uint(*csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CoverArtRequest from a decoded CBOR value tree.
fn csil_dec_cover_art_request(csil_root: &CsilCborValue) -> Result<CoverArtRequest, CsilCborError> {
    let album_id = {
        let csil_field = cbor_require(csil_root, "album_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let max_size = match cbor_map_get(csil_root, "max_size") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(CoverArtRequest { album_id, max_size })
}

/// Encode a CoverArtRequest to canonical CSIL CBOR bytes.
pub fn encode_cover_art_request(csil_v: &CoverArtRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_cover_art_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CoverArtRequest.
pub fn decode_cover_art_request(csil_data: &[u8]) -> Result<CoverArtRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_cover_art_request(&csil_root)
}

/// Build the canonical CBOR value tree for a CoverArt.
fn csil_enc_cover_art(csil_v: &CoverArt) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("data"), cbor_bytes(&csil_v.data)));
    csil_entries.push((cbor_text("content_type"), cbor_text(&csil_v.content_type)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CoverArt from a decoded CBOR value tree.
fn csil_dec_cover_art(csil_root: &CsilCborValue) -> Result<CoverArt, CsilCborError> {
    let content_type = {
        let csil_field = cbor_require(csil_root, "content_type")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let data = {
        let csil_field = cbor_require(csil_root, "data")?;
        let csil_decode = cbor_as_bytes;
        csil_decode(csil_field)?
    };
    Ok(CoverArt { content_type, data })
}

/// Encode a CoverArt to canonical CSIL CBOR bytes.
pub fn encode_cover_art(csil_v: &CoverArt) -> Vec<u8> {
    cbor_encode(&csil_enc_cover_art(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CoverArt.
pub fn decode_cover_art(csil_data: &[u8]) -> Result<CoverArt, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_cover_art(&csil_root)
}

/// Build the canonical CBOR value tree for a Player.
fn csil_enc_player(csil_v: &Player) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(6);
    csil_entries.push((cbor_text("id"), cbor_text(&csil_v.id)));
    csil_entries.push((cbor_text("kind"), csil_enc_player_kind(&csil_v.kind)));
    csil_entries.push((cbor_text("name"), cbor_text(&csil_v.name)));
    if let Some(csil_inner) = &csil_v.owner {
        csil_entries.push((cbor_text("owner"), cbor_text(csil_inner)));
    }
    if let Some(csil_inner) = &csil_v.node_id {
        csil_entries.push((cbor_text("node_id"), cbor_text(csil_inner)));
    }
    if let Some(csil_inner) = &csil_v.device_id {
        csil_entries.push((cbor_text("device_id"), cbor_text(csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a Player from a decoded CBOR value tree.
fn csil_dec_player(csil_root: &CsilCborValue) -> Result<Player, CsilCborError> {
    let id = {
        let csil_field = cbor_require(csil_root, "id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let kind = {
        let csil_field = cbor_require(csil_root, "kind")?;
        let csil_decode = csil_dec_player_kind;
        csil_decode(csil_field)?
    };
    let name = {
        let csil_field = cbor_require(csil_root, "name")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let node_id = match cbor_map_get(csil_root, "node_id") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let device_id = match cbor_map_get(csil_root, "device_id") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let owner = match cbor_map_get(csil_root, "owner") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(Player {
        id,
        kind,
        name,
        node_id,
        device_id,
        owner,
    })
}

/// Encode a Player to canonical CSIL CBOR bytes.
pub fn encode_player(csil_v: &Player) -> Vec<u8> {
    cbor_encode(&csil_enc_player(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a Player.
pub fn decode_player(csil_data: &[u8]) -> Result<Player, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_player(&csil_root)
}

/// Build the canonical CBOR value tree for a QueueItem.
fn csil_enc_queue_item(csil_v: &QueueItem) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(4);
    if let Some(csil_inner) = &csil_v.title {
        csil_entries.push((cbor_text("title"), cbor_text(csil_inner)));
    }
    if let Some(csil_inner) = &csil_v.artist {
        csil_entries.push((cbor_text("artist"), cbor_text(csil_inner)));
    }
    csil_entries.push((cbor_text("track_id"), cbor_text(&csil_v.track_id)));
    if let Some(csil_inner) = &csil_v.duration_ms {
        csil_entries.push((cbor_text("duration_ms"), cbor_uint(*csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a QueueItem from a decoded CBOR value tree.
fn csil_dec_queue_item(csil_root: &CsilCborValue) -> Result<QueueItem, CsilCborError> {
    let track_id = {
        let csil_field = cbor_require(csil_root, "track_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let title = match cbor_map_get(csil_root, "title") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let artist = match cbor_map_get(csil_root, "artist") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let duration_ms = match cbor_map_get(csil_root, "duration_ms") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(QueueItem {
        track_id,
        title,
        artist,
        duration_ms,
    })
}

/// Encode a QueueItem to canonical CSIL CBOR bytes.
pub fn encode_queue_item(csil_v: &QueueItem) -> Vec<u8> {
    cbor_encode(&csil_enc_queue_item(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a QueueItem.
pub fn decode_queue_item(csil_data: &[u8]) -> Result<QueueItem, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_queue_item(&csil_root)
}

/// Build the canonical CBOR value tree for a PlayerState.
fn csil_enc_player_state(csil_v: &PlayerState) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(6);
    csil_entries.push((
        cbor_text("queue"),
        cbor_enc_array(&csil_v.queue, csil_enc_queue_item),
    ));
    csil_entries.push((cbor_text("status"), csil_enc_player_status(&csil_v.status)));
    csil_entries.push((cbor_text("volume"), cbor_uint(csil_v.volume)));
    csil_entries.push((cbor_text("player_id"), cbor_text(&csil_v.player_id)));
    if let Some(csil_inner) = &csil_v.position_ms {
        csil_entries.push((cbor_text("position_ms"), cbor_uint(*csil_inner)));
    }
    if let Some(csil_inner) = &csil_v.current_index {
        csil_entries.push((cbor_text("current_index"), cbor_uint(*csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a PlayerState from a decoded CBOR value tree.
fn csil_dec_player_state(csil_root: &CsilCborValue) -> Result<PlayerState, CsilCborError> {
    let player_id = {
        let csil_field = cbor_require(csil_root, "player_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let status = {
        let csil_field = cbor_require(csil_root, "status")?;
        let csil_decode = csil_dec_player_status;
        csil_decode(csil_field)?
    };
    let current_index = match cbor_map_get(csil_root, "current_index") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let position_ms = match cbor_map_get(csil_root, "position_ms") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let volume = {
        let csil_field = cbor_require(csil_root, "volume")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    let queue = {
        let csil_field = cbor_require(csil_root, "queue")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_queue_item);
        csil_decode(csil_field)?
    };
    Ok(PlayerState {
        player_id,
        status,
        current_index,
        position_ms,
        volume,
        queue,
    })
}

/// Encode a PlayerState to canonical CSIL CBOR bytes.
pub fn encode_player_state(csil_v: &PlayerState) -> Vec<u8> {
    cbor_encode(&csil_enc_player_state(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a PlayerState.
pub fn decode_player_state(csil_data: &[u8]) -> Result<PlayerState, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_player_state(&csil_root)
}

/// Build the canonical CBOR value tree for a ListPlayersRequest.
fn csil_enc_list_players_request(csil_v: &ListPlayersRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    if let Some(csil_inner) = &csil_v.kind {
        csil_entries.push((cbor_text("kind"), csil_enc_player_kind(csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a ListPlayersRequest from a decoded CBOR value tree.
fn csil_dec_list_players_request(
    csil_root: &CsilCborValue,
) -> Result<ListPlayersRequest, CsilCborError> {
    let kind = match cbor_map_get(csil_root, "kind") {
        Some(csil_field) => {
            let csil_decode = csil_dec_player_kind;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(ListPlayersRequest { kind })
}

/// Encode a ListPlayersRequest to canonical CSIL CBOR bytes.
pub fn encode_list_players_request(csil_v: &ListPlayersRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_list_players_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a ListPlayersRequest.
pub fn decode_list_players_request(csil_data: &[u8]) -> Result<ListPlayersRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_list_players_request(&csil_root)
}

/// Build the canonical CBOR value tree for a ListPlayersResponse.
fn csil_enc_list_players_response(csil_v: &ListPlayersResponse) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((
        cbor_text("players"),
        cbor_enc_array(&csil_v.players, csil_enc_player),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a ListPlayersResponse from a decoded CBOR value tree.
fn csil_dec_list_players_response(
    csil_root: &CsilCborValue,
) -> Result<ListPlayersResponse, CsilCborError> {
    let players = {
        let csil_field = cbor_require(csil_root, "players")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_player);
        csil_decode(csil_field)?
    };
    Ok(ListPlayersResponse { players })
}

/// Encode a ListPlayersResponse to canonical CSIL CBOR bytes.
pub fn encode_list_players_response(csil_v: &ListPlayersResponse) -> Vec<u8> {
    cbor_encode(&csil_enc_list_players_response(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a ListPlayersResponse.
pub fn decode_list_players_response(
    csil_data: &[u8],
) -> Result<ListPlayersResponse, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_list_players_response(&csil_root)
}

/// Build the canonical CBOR value tree for a SubscribeRequest.
fn csil_enc_subscribe_request(csil_v: &SubscribeRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("player_id"), cbor_text(&csil_v.player_id)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a SubscribeRequest from a decoded CBOR value tree.
fn csil_dec_subscribe_request(
    csil_root: &CsilCborValue,
) -> Result<SubscribeRequest, CsilCborError> {
    let player_id = {
        let csil_field = cbor_require(csil_root, "player_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(SubscribeRequest { player_id })
}

/// Encode a SubscribeRequest to canonical CSIL CBOR bytes.
pub fn encode_subscribe_request(csil_v: &SubscribeRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_subscribe_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a SubscribeRequest.
pub fn decode_subscribe_request(csil_data: &[u8]) -> Result<SubscribeRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_subscribe_request(&csil_root)
}

/// Build the canonical CBOR value tree for a CmdEnqueue.
fn csil_enc_cmd_enqueue(csil_v: &CmdEnqueue) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(3);
    csil_entries.push((cbor_text("op"), cbor_text("enqueue")));
    if let Some(csil_inner) = &csil_v.at_index {
        csil_entries.push((cbor_text("at_index"), cbor_uint(*csil_inner)));
    }
    csil_entries.push((
        cbor_text("track_ids"),
        cbor_enc_array(&csil_v.track_ids, |csil_elem| cbor_text(csil_elem)),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CmdEnqueue from a decoded CBOR value tree.
fn csil_dec_cmd_enqueue(csil_root: &CsilCborValue) -> Result<CmdEnqueue, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("enqueue"))?;
            Ok("enqueue".to_string())
        };
        csil_decode(csil_field)?
    };
    let track_ids = {
        let csil_field = cbor_require(csil_root, "track_ids")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, cbor_as_text);
        csil_decode(csil_field)?
    };
    let at_index = match cbor_map_get(csil_root, "at_index") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(CmdEnqueue {
        op,
        track_ids,
        at_index,
    })
}

/// Encode a CmdEnqueue to canonical CSIL CBOR bytes.
pub fn encode_cmd_enqueue(csil_v: &CmdEnqueue) -> Vec<u8> {
    cbor_encode(&csil_enc_cmd_enqueue(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CmdEnqueue.
pub fn decode_cmd_enqueue(csil_data: &[u8]) -> Result<CmdEnqueue, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_cmd_enqueue(&csil_root)
}

/// Build the canonical CBOR value tree for a CmdRemove.
fn csil_enc_cmd_remove(csil_v: &CmdRemove) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("op"), cbor_text("remove")));
    csil_entries.push((cbor_text("index"), cbor_uint(csil_v.index)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CmdRemove from a decoded CBOR value tree.
fn csil_dec_cmd_remove(csil_root: &CsilCborValue) -> Result<CmdRemove, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("remove"))?;
            Ok("remove".to_string())
        };
        csil_decode(csil_field)?
    };
    let index = {
        let csil_field = cbor_require(csil_root, "index")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    Ok(CmdRemove { op, index })
}

/// Encode a CmdRemove to canonical CSIL CBOR bytes.
pub fn encode_cmd_remove(csil_v: &CmdRemove) -> Vec<u8> {
    cbor_encode(&csil_enc_cmd_remove(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CmdRemove.
pub fn decode_cmd_remove(csil_data: &[u8]) -> Result<CmdRemove, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_cmd_remove(&csil_root)
}

/// Build the canonical CBOR value tree for a CmdReorder.
fn csil_enc_cmd_reorder(csil_v: &CmdReorder) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(3);
    csil_entries.push((cbor_text("op"), cbor_text("reorder")));
    csil_entries.push((cbor_text("to_index"), cbor_uint(csil_v.to_index)));
    csil_entries.push((cbor_text("from_index"), cbor_uint(csil_v.from_index)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CmdReorder from a decoded CBOR value tree.
fn csil_dec_cmd_reorder(csil_root: &CsilCborValue) -> Result<CmdReorder, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("reorder"))?;
            Ok("reorder".to_string())
        };
        csil_decode(csil_field)?
    };
    let from_index = {
        let csil_field = cbor_require(csil_root, "from_index")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    let to_index = {
        let csil_field = cbor_require(csil_root, "to_index")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    Ok(CmdReorder {
        op,
        from_index,
        to_index,
    })
}

/// Encode a CmdReorder to canonical CSIL CBOR bytes.
pub fn encode_cmd_reorder(csil_v: &CmdReorder) -> Vec<u8> {
    cbor_encode(&csil_enc_cmd_reorder(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CmdReorder.
pub fn decode_cmd_reorder(csil_data: &[u8]) -> Result<CmdReorder, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_cmd_reorder(&csil_root)
}

/// Build the canonical CBOR value tree for a CmdClear.
fn csil_enc_cmd_clear(_csil_v: &CmdClear) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("op"), cbor_text("clear")));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CmdClear from a decoded CBOR value tree.
fn csil_dec_cmd_clear(csil_root: &CsilCborValue) -> Result<CmdClear, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("clear"))?;
            Ok("clear".to_string())
        };
        csil_decode(csil_field)?
    };
    Ok(CmdClear { op })
}

/// Encode a CmdClear to canonical CSIL CBOR bytes.
pub fn encode_cmd_clear(csil_v: &CmdClear) -> Vec<u8> {
    cbor_encode(&csil_enc_cmd_clear(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CmdClear.
pub fn decode_cmd_clear(csil_data: &[u8]) -> Result<CmdClear, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_cmd_clear(&csil_root)
}

/// Build the canonical CBOR value tree for a CmdPlay.
fn csil_enc_cmd_play(csil_v: &CmdPlay) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("op"), cbor_text("play")));
    if let Some(csil_inner) = &csil_v.index {
        csil_entries.push((cbor_text("index"), cbor_uint(*csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CmdPlay from a decoded CBOR value tree.
fn csil_dec_cmd_play(csil_root: &CsilCborValue) -> Result<CmdPlay, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("play"))?;
            Ok("play".to_string())
        };
        csil_decode(csil_field)?
    };
    let index = match cbor_map_get(csil_root, "index") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(CmdPlay { op, index })
}

/// Encode a CmdPlay to canonical CSIL CBOR bytes.
pub fn encode_cmd_play(csil_v: &CmdPlay) -> Vec<u8> {
    cbor_encode(&csil_enc_cmd_play(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CmdPlay.
pub fn decode_cmd_play(csil_data: &[u8]) -> Result<CmdPlay, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_cmd_play(&csil_root)
}

/// Build the canonical CBOR value tree for a CmdPause.
fn csil_enc_cmd_pause(_csil_v: &CmdPause) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("op"), cbor_text("pause")));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CmdPause from a decoded CBOR value tree.
fn csil_dec_cmd_pause(csil_root: &CsilCborValue) -> Result<CmdPause, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("pause"))?;
            Ok("pause".to_string())
        };
        csil_decode(csil_field)?
    };
    Ok(CmdPause { op })
}

/// Encode a CmdPause to canonical CSIL CBOR bytes.
pub fn encode_cmd_pause(csil_v: &CmdPause) -> Vec<u8> {
    cbor_encode(&csil_enc_cmd_pause(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CmdPause.
pub fn decode_cmd_pause(csil_data: &[u8]) -> Result<CmdPause, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_cmd_pause(&csil_root)
}

/// Build the canonical CBOR value tree for a CmdNext.
fn csil_enc_cmd_next(_csil_v: &CmdNext) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("op"), cbor_text("next")));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CmdNext from a decoded CBOR value tree.
fn csil_dec_cmd_next(csil_root: &CsilCborValue) -> Result<CmdNext, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("next"))?;
            Ok("next".to_string())
        };
        csil_decode(csil_field)?
    };
    Ok(CmdNext { op })
}

/// Encode a CmdNext to canonical CSIL CBOR bytes.
pub fn encode_cmd_next(csil_v: &CmdNext) -> Vec<u8> {
    cbor_encode(&csil_enc_cmd_next(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CmdNext.
pub fn decode_cmd_next(csil_data: &[u8]) -> Result<CmdNext, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_cmd_next(&csil_root)
}

/// Build the canonical CBOR value tree for a CmdPrevious.
fn csil_enc_cmd_previous(_csil_v: &CmdPrevious) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("op"), cbor_text("previous")));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CmdPrevious from a decoded CBOR value tree.
fn csil_dec_cmd_previous(csil_root: &CsilCborValue) -> Result<CmdPrevious, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("previous"))?;
            Ok("previous".to_string())
        };
        csil_decode(csil_field)?
    };
    Ok(CmdPrevious { op })
}

/// Encode a CmdPrevious to canonical CSIL CBOR bytes.
pub fn encode_cmd_previous(csil_v: &CmdPrevious) -> Vec<u8> {
    cbor_encode(&csil_enc_cmd_previous(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CmdPrevious.
pub fn decode_cmd_previous(csil_data: &[u8]) -> Result<CmdPrevious, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_cmd_previous(&csil_root)
}

/// Build the canonical CBOR value tree for a CmdSeek.
fn csil_enc_cmd_seek(csil_v: &CmdSeek) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("op"), cbor_text("seek")));
    csil_entries.push((cbor_text("position_ms"), cbor_uint(csil_v.position_ms)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CmdSeek from a decoded CBOR value tree.
fn csil_dec_cmd_seek(csil_root: &CsilCborValue) -> Result<CmdSeek, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("seek"))?;
            Ok("seek".to_string())
        };
        csil_decode(csil_field)?
    };
    let position_ms = {
        let csil_field = cbor_require(csil_root, "position_ms")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    Ok(CmdSeek { op, position_ms })
}

/// Encode a CmdSeek to canonical CSIL CBOR bytes.
pub fn encode_cmd_seek(csil_v: &CmdSeek) -> Vec<u8> {
    cbor_encode(&csil_enc_cmd_seek(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CmdSeek.
pub fn decode_cmd_seek(csil_data: &[u8]) -> Result<CmdSeek, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_cmd_seek(&csil_root)
}

/// Build the canonical CBOR value tree for a CmdVolume.
fn csil_enc_cmd_volume(csil_v: &CmdVolume) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("op"), cbor_text("volume")));
    csil_entries.push((cbor_text("volume"), cbor_uint(csil_v.volume)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CmdVolume from a decoded CBOR value tree.
fn csil_dec_cmd_volume(csil_root: &CsilCborValue) -> Result<CmdVolume, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("volume"))?;
            Ok("volume".to_string())
        };
        csil_decode(csil_field)?
    };
    let volume = {
        let csil_field = cbor_require(csil_root, "volume")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    Ok(CmdVolume { op, volume })
}

/// Encode a CmdVolume to canonical CSIL CBOR bytes.
pub fn encode_cmd_volume(csil_v: &CmdVolume) -> Vec<u8> {
    cbor_encode(&csil_enc_cmd_volume(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CmdVolume.
pub fn decode_cmd_volume(csil_data: &[u8]) -> Result<CmdVolume, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_cmd_volume(&csil_root)
}

/// Build the canonical CBOR value tree for a CommandRequest.
fn csil_enc_command_request(csil_v: &CommandRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((
        cbor_text("command"),
        csil_enc_player_command(&csil_v.command),
    ));
    csil_entries.push((cbor_text("player_id"), cbor_text(&csil_v.player_id)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CommandRequest from a decoded CBOR value tree.
fn csil_dec_command_request(csil_root: &CsilCborValue) -> Result<CommandRequest, CsilCborError> {
    let player_id = {
        let csil_field = cbor_require(csil_root, "player_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let command = {
        let csil_field = cbor_require(csil_root, "command")?;
        let csil_decode = csil_dec_player_command;
        csil_decode(csil_field)?
    };
    Ok(CommandRequest { player_id, command })
}

/// Encode a CommandRequest to canonical CSIL CBOR bytes.
pub fn encode_command_request(csil_v: &CommandRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_command_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CommandRequest.
pub fn decode_command_request(csil_data: &[u8]) -> Result<CommandRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_command_request(&csil_root)
}

/// Build the canonical CBOR value tree for a EnableShareRequest.
fn csil_enc_enable_share_request(csil_v: &EnableShareRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    if let Some(csil_inner) = &csil_v.suffix {
        csil_entries.push((cbor_text("suffix"), cbor_text(csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a EnableShareRequest from a decoded CBOR value tree.
fn csil_dec_enable_share_request(
    csil_root: &CsilCborValue,
) -> Result<EnableShareRequest, CsilCborError> {
    let suffix = match cbor_map_get(csil_root, "suffix") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(EnableShareRequest { suffix })
}

/// Encode a EnableShareRequest to canonical CSIL CBOR bytes.
pub fn encode_enable_share_request(csil_v: &EnableShareRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_enable_share_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a EnableShareRequest.
pub fn decode_enable_share_request(csil_data: &[u8]) -> Result<EnableShareRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_enable_share_request(&csil_root)
}

/// Build the canonical CBOR value tree for a DisableShareRequest.
fn csil_enc_disable_share_request(csil_v: &DisableShareRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("player_id"), cbor_text(&csil_v.player_id)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a DisableShareRequest from a decoded CBOR value tree.
fn csil_dec_disable_share_request(
    csil_root: &CsilCborValue,
) -> Result<DisableShareRequest, CsilCborError> {
    let player_id = {
        let csil_field = cbor_require(csil_root, "player_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(DisableShareRequest { player_id })
}

/// Encode a DisableShareRequest to canonical CSIL CBOR bytes.
pub fn encode_disable_share_request(csil_v: &DisableShareRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_disable_share_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a DisableShareRequest.
pub fn decode_disable_share_request(
    csil_data: &[u8],
) -> Result<DisableShareRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_disable_share_request(&csil_root)
}

/// Build the canonical CBOR value tree for a ShareResult.
fn csil_enc_share_result(csil_v: &ShareResult) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("player"), csil_enc_player(&csil_v.player)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a ShareResult from a decoded CBOR value tree.
fn csil_dec_share_result(csil_root: &CsilCborValue) -> Result<ShareResult, CsilCborError> {
    let player = {
        let csil_field = cbor_require(csil_root, "player")?;
        let csil_decode = csil_dec_player;
        csil_decode(csil_field)?
    };
    Ok(ShareResult { player })
}

/// Encode a ShareResult to canonical CSIL CBOR bytes.
pub fn encode_share_result(csil_v: &ShareResult) -> Vec<u8> {
    cbor_encode(&csil_enc_share_result(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a ShareResult.
pub fn decode_share_result(csil_data: &[u8]) -> Result<ShareResult, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_share_result(&csil_root)
}

/// Build the canonical CBOR value tree for a MediaOpen.
fn csil_enc_media_open(csil_v: &MediaOpen) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(3);
    csil_entries.push((cbor_text("kind"), cbor_text("open")));
    csil_entries.push((cbor_text("pref"), csil_enc_stream_pref(&csil_v.pref)));
    csil_entries.push((cbor_text("track_id"), cbor_text(&csil_v.track_id)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a MediaOpen from a decoded CBOR value tree.
fn csil_dec_media_open(csil_root: &CsilCborValue) -> Result<MediaOpen, CsilCborError> {
    let kind = {
        let csil_field = cbor_require(csil_root, "kind")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("open"))?;
            Ok("open".to_string())
        };
        csil_decode(csil_field)?
    };
    let track_id = {
        let csil_field = cbor_require(csil_root, "track_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let pref = {
        let csil_field = cbor_require(csil_root, "pref")?;
        let csil_decode = csil_dec_stream_pref;
        csil_decode(csil_field)?
    };
    Ok(MediaOpen {
        kind,
        track_id,
        pref,
    })
}

/// Encode a MediaOpen to canonical CSIL CBOR bytes.
pub fn encode_media_open(csil_v: &MediaOpen) -> Vec<u8> {
    cbor_encode(&csil_enc_media_open(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a MediaOpen.
pub fn decode_media_open(csil_data: &[u8]) -> Result<MediaOpen, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_media_open(&csil_root)
}

/// Build the canonical CBOR value tree for a MediaSeek.
fn csil_enc_media_seek(csil_v: &MediaSeek) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("kind"), cbor_text("seek")));
    csil_entries.push((cbor_text("position_ms"), cbor_uint(csil_v.position_ms)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a MediaSeek from a decoded CBOR value tree.
fn csil_dec_media_seek(csil_root: &CsilCborValue) -> Result<MediaSeek, CsilCborError> {
    let kind = {
        let csil_field = cbor_require(csil_root, "kind")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("seek"))?;
            Ok("seek".to_string())
        };
        csil_decode(csil_field)?
    };
    let position_ms = {
        let csil_field = cbor_require(csil_root, "position_ms")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    Ok(MediaSeek { kind, position_ms })
}

/// Encode a MediaSeek to canonical CSIL CBOR bytes.
pub fn encode_media_seek(csil_v: &MediaSeek) -> Vec<u8> {
    cbor_encode(&csil_enc_media_seek(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a MediaSeek.
pub fn decode_media_seek(csil_data: &[u8]) -> Result<MediaSeek, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_media_seek(&csil_root)
}

/// Build the canonical CBOR value tree for a MediaPause.
fn csil_enc_media_pause(_csil_v: &MediaPause) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("kind"), cbor_text("pause")));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a MediaPause from a decoded CBOR value tree.
fn csil_dec_media_pause(csil_root: &CsilCborValue) -> Result<MediaPause, CsilCborError> {
    let kind = {
        let csil_field = cbor_require(csil_root, "kind")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("pause"))?;
            Ok("pause".to_string())
        };
        csil_decode(csil_field)?
    };
    Ok(MediaPause { kind })
}

/// Encode a MediaPause to canonical CSIL CBOR bytes.
pub fn encode_media_pause(csil_v: &MediaPause) -> Vec<u8> {
    cbor_encode(&csil_enc_media_pause(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a MediaPause.
pub fn decode_media_pause(csil_data: &[u8]) -> Result<MediaPause, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_media_pause(&csil_root)
}

/// Build the canonical CBOR value tree for a MediaResume.
fn csil_enc_media_resume(_csil_v: &MediaResume) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("kind"), cbor_text("resume")));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a MediaResume from a decoded CBOR value tree.
fn csil_dec_media_resume(csil_root: &CsilCborValue) -> Result<MediaResume, CsilCborError> {
    let kind = {
        let csil_field = cbor_require(csil_root, "kind")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("resume"))?;
            Ok("resume".to_string())
        };
        csil_decode(csil_field)?
    };
    Ok(MediaResume { kind })
}

/// Encode a MediaResume to canonical CSIL CBOR bytes.
pub fn encode_media_resume(csil_v: &MediaResume) -> Vec<u8> {
    cbor_encode(&csil_enc_media_resume(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a MediaResume.
pub fn decode_media_resume(csil_data: &[u8]) -> Result<MediaResume, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_media_resume(&csil_root)
}

/// Build the canonical CBOR value tree for a MediaStop.
fn csil_enc_media_stop(_csil_v: &MediaStop) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("kind"), cbor_text("stop")));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a MediaStop from a decoded CBOR value tree.
fn csil_dec_media_stop(csil_root: &CsilCborValue) -> Result<MediaStop, CsilCborError> {
    let kind = {
        let csil_field = cbor_require(csil_root, "kind")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("stop"))?;
            Ok("stop".to_string())
        };
        csil_decode(csil_field)?
    };
    Ok(MediaStop { kind })
}

/// Encode a MediaStop to canonical CSIL CBOR bytes.
pub fn encode_media_stop(csil_v: &MediaStop) -> Vec<u8> {
    cbor_encode(&csil_enc_media_stop(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a MediaStop.
pub fn decode_media_stop(csil_data: &[u8]) -> Result<MediaStop, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_media_stop(&csil_root)
}

/// Build the canonical CBOR value tree for a MediaHeader.
fn csil_enc_media_header(csil_v: &MediaHeader) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(9);
    csil_entries.push((cbor_text("kind"), cbor_text("header")));
    csil_entries.push((cbor_text("codec"), csil_enc_codec(&csil_v.codec)));
    csil_entries.push((cbor_text("channels"), cbor_uint(csil_v.channels)));
    csil_entries.push((cbor_text("transcoded"), cbor_bool(csil_v.transcoded)));
    if let Some(csil_inner) = &csil_v.duration_ms {
        csil_entries.push((cbor_text("duration_ms"), cbor_uint(*csil_inner)));
    }
    csil_entries.push((cbor_text("sample_rate"), cbor_uint(csil_v.sample_rate)));
    if let Some(csil_inner) = &csil_v.codec_config {
        csil_entries.push((cbor_text("codec_config"), cbor_bytes(csil_inner)));
    }
    csil_entries.push((
        cbor_text("trim_end_samples"),
        cbor_uint(csil_v.trim_end_samples),
    ));
    csil_entries.push((
        cbor_text("trim_start_samples"),
        cbor_uint(csil_v.trim_start_samples),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a MediaHeader from a decoded CBOR value tree.
fn csil_dec_media_header(csil_root: &CsilCborValue) -> Result<MediaHeader, CsilCborError> {
    let kind = {
        let csil_field = cbor_require(csil_root, "kind")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("header"))?;
            Ok("header".to_string())
        };
        csil_decode(csil_field)?
    };
    let codec = {
        let csil_field = cbor_require(csil_root, "codec")?;
        let csil_decode = csil_dec_codec;
        csil_decode(csil_field)?
    };
    let transcoded = {
        let csil_field = cbor_require(csil_root, "transcoded")?;
        let csil_decode = cbor_as_bool;
        csil_decode(csil_field)?
    };
    let sample_rate = {
        let csil_field = cbor_require(csil_root, "sample_rate")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    let channels = {
        let csil_field = cbor_require(csil_root, "channels")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    let duration_ms = match cbor_map_get(csil_root, "duration_ms") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let trim_start_samples = {
        let csil_field = cbor_require(csil_root, "trim_start_samples")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    let trim_end_samples = {
        let csil_field = cbor_require(csil_root, "trim_end_samples")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    let codec_config = match cbor_map_get(csil_root, "codec_config") {
        Some(csil_field) => {
            let csil_decode = cbor_as_bytes;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(MediaHeader {
        kind,
        codec,
        transcoded,
        sample_rate,
        channels,
        duration_ms,
        trim_start_samples,
        trim_end_samples,
        codec_config,
    })
}

/// Encode a MediaHeader to canonical CSIL CBOR bytes.
pub fn encode_media_header(csil_v: &MediaHeader) -> Vec<u8> {
    cbor_encode(&csil_enc_media_header(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a MediaHeader.
pub fn decode_media_header(csil_data: &[u8]) -> Result<MediaHeader, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_media_header(&csil_root)
}

/// Build the canonical CBOR value tree for a MediaChunk.
fn csil_enc_media_chunk(csil_v: &MediaChunk) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(4);
    csil_entries.push((cbor_text("seq"), cbor_uint(csil_v.seq)));
    csil_entries.push((cbor_text("data"), cbor_bytes(&csil_v.data)));
    csil_entries.push((cbor_text("kind"), cbor_text("chunk")));
    if let Some(csil_inner) = &csil_v.timestamp_ms {
        csil_entries.push((cbor_text("timestamp_ms"), cbor_uint(*csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a MediaChunk from a decoded CBOR value tree.
fn csil_dec_media_chunk(csil_root: &CsilCborValue) -> Result<MediaChunk, CsilCborError> {
    let kind = {
        let csil_field = cbor_require(csil_root, "kind")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("chunk"))?;
            Ok("chunk".to_string())
        };
        csil_decode(csil_field)?
    };
    let seq = {
        let csil_field = cbor_require(csil_root, "seq")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    let timestamp_ms = match cbor_map_get(csil_root, "timestamp_ms") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let data = {
        let csil_field = cbor_require(csil_root, "data")?;
        let csil_decode = cbor_as_bytes;
        csil_decode(csil_field)?
    };
    Ok(MediaChunk {
        kind,
        seq,
        timestamp_ms,
        data,
    })
}

/// Encode a MediaChunk to canonical CSIL CBOR bytes.
pub fn encode_media_chunk(csil_v: &MediaChunk) -> Vec<u8> {
    cbor_encode(&csil_enc_media_chunk(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a MediaChunk.
pub fn decode_media_chunk(csil_data: &[u8]) -> Result<MediaChunk, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_media_chunk(&csil_root)
}

/// Build the canonical CBOR value tree for a MediaEnd.
fn csil_enc_media_end(csil_v: &MediaEnd) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("kind"), cbor_text("end")));
    if let Some(csil_inner) = &csil_v.reason {
        csil_entries.push((cbor_text("reason"), csil_enc_media_end_reason(csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a MediaEnd from a decoded CBOR value tree.
fn csil_dec_media_end(csil_root: &CsilCborValue) -> Result<MediaEnd, CsilCborError> {
    let kind = {
        let csil_field = cbor_require(csil_root, "kind")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("end"))?;
            Ok("end".to_string())
        };
        csil_decode(csil_field)?
    };
    let reason = match cbor_map_get(csil_root, "reason") {
        Some(csil_field) => {
            let csil_decode = csil_dec_media_end_reason;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(MediaEnd { kind, reason })
}

/// Encode a MediaEnd to canonical CSIL CBOR bytes.
pub fn encode_media_end(csil_v: &MediaEnd) -> Vec<u8> {
    cbor_encode(&csil_enc_media_end(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a MediaEnd.
pub fn decode_media_end(csil_data: &[u8]) -> Result<MediaEnd, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_media_end(&csil_root)
}

/// Build the canonical CBOR value tree for a MediaFail.
fn csil_enc_media_fail(csil_v: &MediaFail) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("kind"), cbor_text("error")));
    csil_entries.push((cbor_text("error"), csil_enc_service_error(&csil_v.error)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a MediaFail from a decoded CBOR value tree.
fn csil_dec_media_fail(csil_root: &CsilCborValue) -> Result<MediaFail, CsilCborError> {
    let kind = {
        let csil_field = cbor_require(csil_root, "kind")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("error"))?;
            Ok("error".to_string())
        };
        csil_decode(csil_field)?
    };
    let error = {
        let csil_field = cbor_require(csil_root, "error")?;
        let csil_decode = csil_dec_service_error;
        csil_decode(csil_field)?
    };
    Ok(MediaFail { kind, error })
}

/// Encode a MediaFail to canonical CSIL CBOR bytes.
pub fn encode_media_fail(csil_v: &MediaFail) -> Vec<u8> {
    cbor_encode(&csil_enc_media_fail(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a MediaFail.
pub fn decode_media_fail(csil_data: &[u8]) -> Result<MediaFail, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_media_fail(&csil_root)
}

/// Build the canonical CBOR value tree for a AudioOutput.
fn csil_enc_audio_output(csil_v: &AudioOutput) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(5);
    csil_entries.push((cbor_text("channels"), cbor_uint(csil_v.channels)));
    csil_entries.push((cbor_text("is_default"), cbor_bool(csil_v.is_default)));
    csil_entries.push((cbor_text("os_device_id"), cbor_text(&csil_v.os_device_id)));
    csil_entries.push((
        cbor_text("sample_rates"),
        cbor_enc_array(&csil_v.sample_rates, |csil_elem| cbor_uint(*csil_elem)),
    ));
    if let Some(csil_inner) = &csil_v.friendly_name {
        csil_entries.push((cbor_text("friendly_name"), cbor_text(csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a AudioOutput from a decoded CBOR value tree.
fn csil_dec_audio_output(csil_root: &CsilCborValue) -> Result<AudioOutput, CsilCborError> {
    let os_device_id = {
        let csil_field = cbor_require(csil_root, "os_device_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let friendly_name = match cbor_map_get(csil_root, "friendly_name") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let channels = {
        let csil_field = cbor_require(csil_root, "channels")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    let sample_rates = {
        let csil_field = cbor_require(csil_root, "sample_rates")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, cbor_as_u64);
        csil_decode(csil_field)?
    };
    let is_default = {
        let csil_field = cbor_require(csil_root, "is_default")?;
        let csil_decode = cbor_as_bool;
        csil_decode(csil_field)?
    };
    Ok(AudioOutput {
        os_device_id,
        friendly_name,
        channels,
        sample_rates,
        is_default,
    })
}

/// Encode a AudioOutput to canonical CSIL CBOR bytes.
pub fn encode_audio_output(csil_v: &AudioOutput) -> Vec<u8> {
    cbor_encode(&csil_enc_audio_output(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a AudioOutput.
pub fn decode_audio_output(csil_data: &[u8]) -> Result<AudioOutput, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_audio_output(&csil_root)
}

/// Build the canonical CBOR value tree for a RegisterNodeRequest.
fn csil_enc_register_node_request(csil_v: &RegisterNodeRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(4);
    csil_entries.push((cbor_text("arch"), cbor_text(&csil_v.arch)));
    csil_entries.push((
        cbor_text("outputs"),
        cbor_enc_array(&csil_v.outputs, csil_enc_audio_output),
    ));
    csil_entries.push((cbor_text("hostname"), cbor_text(&csil_v.hostname)));
    csil_entries.push((cbor_text("platform"), cbor_text(&csil_v.platform)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a RegisterNodeRequest from a decoded CBOR value tree.
fn csil_dec_register_node_request(
    csil_root: &CsilCborValue,
) -> Result<RegisterNodeRequest, CsilCborError> {
    let hostname = {
        let csil_field = cbor_require(csil_root, "hostname")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let platform = {
        let csil_field = cbor_require(csil_root, "platform")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let arch = {
        let csil_field = cbor_require(csil_root, "arch")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let outputs = {
        let csil_field = cbor_require(csil_root, "outputs")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_audio_output);
        csil_decode(csil_field)?
    };
    Ok(RegisterNodeRequest {
        hostname,
        platform,
        arch,
        outputs,
    })
}

/// Encode a RegisterNodeRequest to canonical CSIL CBOR bytes.
pub fn encode_register_node_request(csil_v: &RegisterNodeRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_register_node_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a RegisterNodeRequest.
pub fn decode_register_node_request(
    csil_data: &[u8],
) -> Result<RegisterNodeRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_register_node_request(&csil_root)
}

/// Build the canonical CBOR value tree for a RegisterNodeResponse.
fn csil_enc_register_node_response(csil_v: &RegisterNodeResponse) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("node_id"), cbor_text(&csil_v.node_id)));
    csil_entries.push((
        cbor_text("players"),
        cbor_enc_array(&csil_v.players, csil_enc_player),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a RegisterNodeResponse from a decoded CBOR value tree.
fn csil_dec_register_node_response(
    csil_root: &CsilCborValue,
) -> Result<RegisterNodeResponse, CsilCborError> {
    let node_id = {
        let csil_field = cbor_require(csil_root, "node_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let players = {
        let csil_field = cbor_require(csil_root, "players")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_player);
        csil_decode(csil_field)?
    };
    Ok(RegisterNodeResponse { node_id, players })
}

/// Encode a RegisterNodeResponse to canonical CSIL CBOR bytes.
pub fn encode_register_node_response(csil_v: &RegisterNodeResponse) -> Vec<u8> {
    cbor_encode(&csil_enc_register_node_response(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a RegisterNodeResponse.
pub fn decode_register_node_response(
    csil_data: &[u8],
) -> Result<RegisterNodeResponse, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_register_node_response(&csil_root)
}

/// Build the canonical CBOR value tree for a DirLoad.
fn csil_enc_dir_load(csil_v: &DirLoad) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(5);
    csil_entries.push((cbor_text("op"), cbor_text("load")));
    csil_entries.push((cbor_text("pref"), csil_enc_stream_pref(&csil_v.pref)));
    csil_entries.push((cbor_text("track_id"), cbor_text(&csil_v.track_id)));
    csil_entries.push((cbor_text("player_id"), cbor_text(&csil_v.player_id)));
    if let Some(csil_inner) = &csil_v.position_ms {
        csil_entries.push((cbor_text("position_ms"), cbor_uint(*csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a DirLoad from a decoded CBOR value tree.
fn csil_dec_dir_load(csil_root: &CsilCborValue) -> Result<DirLoad, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("load"))?;
            Ok("load".to_string())
        };
        csil_decode(csil_field)?
    };
    let player_id = {
        let csil_field = cbor_require(csil_root, "player_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let track_id = {
        let csil_field = cbor_require(csil_root, "track_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let pref = {
        let csil_field = cbor_require(csil_root, "pref")?;
        let csil_decode = csil_dec_stream_pref;
        csil_decode(csil_field)?
    };
    let position_ms = match cbor_map_get(csil_root, "position_ms") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(DirLoad {
        op,
        player_id,
        track_id,
        pref,
        position_ms,
    })
}

/// Encode a DirLoad to canonical CSIL CBOR bytes.
pub fn encode_dir_load(csil_v: &DirLoad) -> Vec<u8> {
    cbor_encode(&csil_enc_dir_load(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a DirLoad.
pub fn decode_dir_load(csil_data: &[u8]) -> Result<DirLoad, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_dir_load(&csil_root)
}

/// Build the canonical CBOR value tree for a DirPause.
fn csil_enc_dir_pause(csil_v: &DirPause) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("op"), cbor_text("pause")));
    csil_entries.push((cbor_text("player_id"), cbor_text(&csil_v.player_id)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a DirPause from a decoded CBOR value tree.
fn csil_dec_dir_pause(csil_root: &CsilCborValue) -> Result<DirPause, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("pause"))?;
            Ok("pause".to_string())
        };
        csil_decode(csil_field)?
    };
    let player_id = {
        let csil_field = cbor_require(csil_root, "player_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(DirPause { op, player_id })
}

/// Encode a DirPause to canonical CSIL CBOR bytes.
pub fn encode_dir_pause(csil_v: &DirPause) -> Vec<u8> {
    cbor_encode(&csil_enc_dir_pause(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a DirPause.
pub fn decode_dir_pause(csil_data: &[u8]) -> Result<DirPause, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_dir_pause(&csil_root)
}

/// Build the canonical CBOR value tree for a DirResume.
fn csil_enc_dir_resume(csil_v: &DirResume) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("op"), cbor_text("resume")));
    csil_entries.push((cbor_text("player_id"), cbor_text(&csil_v.player_id)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a DirResume from a decoded CBOR value tree.
fn csil_dec_dir_resume(csil_root: &CsilCborValue) -> Result<DirResume, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("resume"))?;
            Ok("resume".to_string())
        };
        csil_decode(csil_field)?
    };
    let player_id = {
        let csil_field = cbor_require(csil_root, "player_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(DirResume { op, player_id })
}

/// Encode a DirResume to canonical CSIL CBOR bytes.
pub fn encode_dir_resume(csil_v: &DirResume) -> Vec<u8> {
    cbor_encode(&csil_enc_dir_resume(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a DirResume.
pub fn decode_dir_resume(csil_data: &[u8]) -> Result<DirResume, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_dir_resume(&csil_root)
}

/// Build the canonical CBOR value tree for a DirStop.
fn csil_enc_dir_stop(csil_v: &DirStop) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("op"), cbor_text("stop")));
    csil_entries.push((cbor_text("player_id"), cbor_text(&csil_v.player_id)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a DirStop from a decoded CBOR value tree.
fn csil_dec_dir_stop(csil_root: &CsilCborValue) -> Result<DirStop, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("stop"))?;
            Ok("stop".to_string())
        };
        csil_decode(csil_field)?
    };
    let player_id = {
        let csil_field = cbor_require(csil_root, "player_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(DirStop { op, player_id })
}

/// Encode a DirStop to canonical CSIL CBOR bytes.
pub fn encode_dir_stop(csil_v: &DirStop) -> Vec<u8> {
    cbor_encode(&csil_enc_dir_stop(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a DirStop.
pub fn decode_dir_stop(csil_data: &[u8]) -> Result<DirStop, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_dir_stop(&csil_root)
}

/// Build the canonical CBOR value tree for a DirVolume.
fn csil_enc_dir_volume(csil_v: &DirVolume) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(3);
    csil_entries.push((cbor_text("op"), cbor_text("volume")));
    csil_entries.push((cbor_text("volume"), cbor_uint(csil_v.volume)));
    csil_entries.push((cbor_text("player_id"), cbor_text(&csil_v.player_id)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a DirVolume from a decoded CBOR value tree.
fn csil_dec_dir_volume(csil_root: &CsilCborValue) -> Result<DirVolume, CsilCborError> {
    let op = {
        let csil_field = cbor_require(csil_root, "op")?;
        let csil_decode = |csil_v| {
            cbor_expect_value(csil_v, &cbor_text("volume"))?;
            Ok("volume".to_string())
        };
        csil_decode(csil_field)?
    };
    let player_id = {
        let csil_field = cbor_require(csil_root, "player_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let volume = {
        let csil_field = cbor_require(csil_root, "volume")?;
        let csil_decode = cbor_as_u64;
        csil_decode(csil_field)?
    };
    Ok(DirVolume {
        op,
        player_id,
        volume,
    })
}

/// Encode a DirVolume to canonical CSIL CBOR bytes.
pub fn encode_dir_volume(csil_v: &DirVolume) -> Vec<u8> {
    cbor_encode(&csil_enc_dir_volume(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a DirVolume.
pub fn decode_dir_volume(csil_data: &[u8]) -> Result<DirVolume, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_dir_volume(&csil_root)
}

/// Build the canonical CBOR value tree for a NodeReport.
fn csil_enc_node_report(csil_v: &NodeReport) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(3);
    csil_entries.push((cbor_text("status"), csil_enc_player_status(&csil_v.status)));
    csil_entries.push((cbor_text("player_id"), cbor_text(&csil_v.player_id)));
    if let Some(csil_inner) = &csil_v.position_ms {
        csil_entries.push((cbor_text("position_ms"), cbor_uint(*csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a NodeReport from a decoded CBOR value tree.
fn csil_dec_node_report(csil_root: &CsilCborValue) -> Result<NodeReport, CsilCborError> {
    let player_id = {
        let csil_field = cbor_require(csil_root, "player_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let status = {
        let csil_field = cbor_require(csil_root, "status")?;
        let csil_decode = csil_dec_player_status;
        csil_decode(csil_field)?
    };
    let position_ms = match cbor_map_get(csil_root, "position_ms") {
        Some(csil_field) => {
            let csil_decode = cbor_as_u64;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(NodeReport {
        player_id,
        status,
        position_ms,
    })
}

/// Encode a NodeReport to canonical CSIL CBOR bytes.
pub fn encode_node_report(csil_v: &NodeReport) -> Vec<u8> {
    cbor_encode(&csil_enc_node_report(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a NodeReport.
pub fn decode_node_report(csil_data: &[u8]) -> Result<NodeReport, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_node_report(&csil_root)
}

/// Build the canonical CBOR value tree for a Account.
fn csil_enc_account(csil_v: &Account) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(5);
    csil_entries.push((cbor_text("id"), cbor_text(&csil_v.id)));
    csil_entries.push((cbor_text("role"), csil_enc_role(&csil_v.role)));
    csil_entries.push((cbor_text("handle"), cbor_text(&csil_v.handle)));
    csil_entries.push((
        cbor_text("created_at"),
        csil_enc_timestamp(&csil_v.created_at),
    ));
    if let Some(csil_inner) = &csil_v.display_name {
        csil_entries.push((cbor_text("display_name"), cbor_text(csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a Account from a decoded CBOR value tree.
fn csil_dec_account(csil_root: &CsilCborValue) -> Result<Account, CsilCborError> {
    let id = {
        let csil_field = cbor_require(csil_root, "id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let handle = {
        let csil_field = cbor_require(csil_root, "handle")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let display_name = match cbor_map_get(csil_root, "display_name") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let role = {
        let csil_field = cbor_require(csil_root, "role")?;
        let csil_decode = csil_dec_role;
        csil_decode(csil_field)?
    };
    let created_at = {
        let csil_field = cbor_require(csil_root, "created_at")?;
        let csil_decode = csil_as_timestamp;
        csil_decode(csil_field)?
    };
    Ok(Account {
        id,
        handle,
        display_name,
        role,
        created_at,
    })
}

/// Encode a Account to canonical CSIL CBOR bytes.
pub fn encode_account(csil_v: &Account) -> Vec<u8> {
    cbor_encode(&csil_enc_account(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a Account.
pub fn decode_account(csil_data: &[u8]) -> Result<Account, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_account(&csil_root)
}

/// Build the canonical CBOR value tree for a ListAccountsResponse.
fn csil_enc_list_accounts_response(csil_v: &ListAccountsResponse) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((
        cbor_text("accounts"),
        cbor_enc_array(&csil_v.accounts, csil_enc_account),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a ListAccountsResponse from a decoded CBOR value tree.
fn csil_dec_list_accounts_response(
    csil_root: &CsilCborValue,
) -> Result<ListAccountsResponse, CsilCborError> {
    let accounts = {
        let csil_field = cbor_require(csil_root, "accounts")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_account);
        csil_decode(csil_field)?
    };
    Ok(ListAccountsResponse { accounts })
}

/// Encode a ListAccountsResponse to canonical CSIL CBOR bytes.
pub fn encode_list_accounts_response(csil_v: &ListAccountsResponse) -> Vec<u8> {
    cbor_encode(&csil_enc_list_accounts_response(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a ListAccountsResponse.
pub fn decode_list_accounts_response(
    csil_data: &[u8],
) -> Result<ListAccountsResponse, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_list_accounts_response(&csil_root)
}

/// Build the canonical CBOR value tree for a SetRoleRequest.
fn csil_enc_set_role_request(csil_v: &SetRoleRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("role"), csil_enc_role(&csil_v.role)));
    csil_entries.push((cbor_text("account_id"), cbor_text(&csil_v.account_id)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a SetRoleRequest from a decoded CBOR value tree.
fn csil_dec_set_role_request(csil_root: &CsilCborValue) -> Result<SetRoleRequest, CsilCborError> {
    let account_id = {
        let csil_field = cbor_require(csil_root, "account_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let role = {
        let csil_field = cbor_require(csil_root, "role")?;
        let csil_decode = csil_dec_role;
        csil_decode(csil_field)?
    };
    Ok(SetRoleRequest { account_id, role })
}

/// Encode a SetRoleRequest to canonical CSIL CBOR bytes.
pub fn encode_set_role_request(csil_v: &SetRoleRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_set_role_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a SetRoleRequest.
pub fn decode_set_role_request(csil_data: &[u8]) -> Result<SetRoleRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_set_role_request(&csil_root)
}

/// Build the canonical CBOR value tree for a TrustDomainRequest.
fn csil_enc_trust_domain_request(csil_v: &TrustDomainRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((cbor_text("domain"), cbor_text(&csil_v.domain)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a TrustDomainRequest from a decoded CBOR value tree.
fn csil_dec_trust_domain_request(
    csil_root: &CsilCborValue,
) -> Result<TrustDomainRequest, CsilCborError> {
    let domain = {
        let csil_field = cbor_require(csil_root, "domain")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(TrustDomainRequest { domain })
}

/// Encode a TrustDomainRequest to canonical CSIL CBOR bytes.
pub fn encode_trust_domain_request(csil_v: &TrustDomainRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_trust_domain_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a TrustDomainRequest.
pub fn decode_trust_domain_request(csil_data: &[u8]) -> Result<TrustDomainRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_trust_domain_request(&csil_root)
}

/// Build the canonical CBOR value tree for a TrustedDomains.
fn csil_enc_trusted_domains(csil_v: &TrustedDomains) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((
        cbor_text("domains"),
        cbor_enc_array(&csil_v.domains, |csil_elem| cbor_text(csil_elem)),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a TrustedDomains from a decoded CBOR value tree.
fn csil_dec_trusted_domains(csil_root: &CsilCborValue) -> Result<TrustedDomains, CsilCborError> {
    let domains = {
        let csil_field = cbor_require(csil_root, "domains")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, cbor_as_text);
        csil_decode(csil_field)?
    };
    Ok(TrustedDomains { domains })
}

/// Encode a TrustedDomains to canonical CSIL CBOR bytes.
pub fn encode_trusted_domains(csil_v: &TrustedDomains) -> Vec<u8> {
    cbor_encode(&csil_enc_trusted_domains(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a TrustedDomains.
pub fn decode_trusted_domains(csil_data: &[u8]) -> Result<TrustedDomains, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_trusted_domains(&csil_root)
}

/// Build the canonical CBOR value tree for a DeviceInfo.
fn csil_enc_device_info(csil_v: &DeviceInfo) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(4);
    csil_entries.push((cbor_text("id"), cbor_text(&csil_v.id)));
    csil_entries.push((cbor_text("is_default"), cbor_bool(csil_v.is_default)));
    csil_entries.push((cbor_text("os_device_id"), cbor_text(&csil_v.os_device_id)));
    csil_entries.push((cbor_text("friendly_name"), cbor_text(&csil_v.friendly_name)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a DeviceInfo from a decoded CBOR value tree.
fn csil_dec_device_info(csil_root: &CsilCborValue) -> Result<DeviceInfo, CsilCborError> {
    let id = {
        let csil_field = cbor_require(csil_root, "id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let os_device_id = {
        let csil_field = cbor_require(csil_root, "os_device_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let friendly_name = {
        let csil_field = cbor_require(csil_root, "friendly_name")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let is_default = {
        let csil_field = cbor_require(csil_root, "is_default")?;
        let csil_decode = cbor_as_bool;
        csil_decode(csil_field)?
    };
    Ok(DeviceInfo {
        id,
        os_device_id,
        friendly_name,
        is_default,
    })
}

/// Encode a DeviceInfo to canonical CSIL CBOR bytes.
pub fn encode_device_info(csil_v: &DeviceInfo) -> Vec<u8> {
    cbor_encode(&csil_enc_device_info(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a DeviceInfo.
pub fn decode_device_info(csil_data: &[u8]) -> Result<DeviceInfo, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_device_info(&csil_root)
}

/// Build the canonical CBOR value tree for a NodeInfo.
fn csil_enc_node_info(csil_v: &NodeInfo) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(9);
    csil_entries.push((cbor_text("id"), cbor_text(&csil_v.id)));
    csil_entries.push((cbor_text("arch"), cbor_text(&csil_v.arch)));
    csil_entries.push((cbor_text("kind"), csil_enc_node_kind(&csil_v.kind)));
    csil_entries.push((
        cbor_text("devices"),
        cbor_enc_array(&csil_v.devices, csil_enc_device_info),
    ));
    csil_entries.push((cbor_text("hostname"), cbor_text(&csil_v.hostname)));
    csil_entries.push((cbor_text("platform"), cbor_text(&csil_v.platform)));
    if let Some(csil_inner) = &csil_v.last_seen {
        csil_entries.push((cbor_text("last_seen"), csil_enc_timestamp(csil_inner)));
    }
    csil_entries.push((
        cbor_text("audio_outputs"),
        csil_enc_audio_outputs_state(&csil_v.audio_outputs),
    ));
    csil_entries.push((cbor_text("friendly_name"), cbor_text(&csil_v.friendly_name)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a NodeInfo from a decoded CBOR value tree.
fn csil_dec_node_info(csil_root: &CsilCborValue) -> Result<NodeInfo, CsilCborError> {
    let id = {
        let csil_field = cbor_require(csil_root, "id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let kind = {
        let csil_field = cbor_require(csil_root, "kind")?;
        let csil_decode = csil_dec_node_kind;
        csil_decode(csil_field)?
    };
    let hostname = {
        let csil_field = cbor_require(csil_root, "hostname")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let friendly_name = {
        let csil_field = cbor_require(csil_root, "friendly_name")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let platform = {
        let csil_field = cbor_require(csil_root, "platform")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let arch = {
        let csil_field = cbor_require(csil_root, "arch")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let last_seen = match cbor_map_get(csil_root, "last_seen") {
        Some(csil_field) => {
            let csil_decode = csil_as_timestamp;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let audio_outputs = {
        let csil_field = cbor_require(csil_root, "audio_outputs")?;
        let csil_decode = csil_dec_audio_outputs_state;
        csil_decode(csil_field)?
    };
    let devices = {
        let csil_field = cbor_require(csil_root, "devices")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_device_info);
        csil_decode(csil_field)?
    };
    Ok(NodeInfo {
        id,
        kind,
        hostname,
        friendly_name,
        platform,
        arch,
        last_seen,
        audio_outputs,
        devices,
    })
}

/// Encode a NodeInfo to canonical CSIL CBOR bytes.
pub fn encode_node_info(csil_v: &NodeInfo) -> Vec<u8> {
    cbor_encode(&csil_enc_node_info(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a NodeInfo.
pub fn decode_node_info(csil_data: &[u8]) -> Result<NodeInfo, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_node_info(&csil_root)
}

/// Build the canonical CBOR value tree for a ListNodesResponse.
fn csil_enc_list_nodes_response(csil_v: &ListNodesResponse) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((
        cbor_text("nodes"),
        cbor_enc_array(&csil_v.nodes, csil_enc_node_info),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a ListNodesResponse from a decoded CBOR value tree.
fn csil_dec_list_nodes_response(
    csil_root: &CsilCborValue,
) -> Result<ListNodesResponse, CsilCborError> {
    let nodes = {
        let csil_field = cbor_require(csil_root, "nodes")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, csil_dec_node_info);
        csil_decode(csil_field)?
    };
    Ok(ListNodesResponse { nodes })
}

/// Encode a ListNodesResponse to canonical CSIL CBOR bytes.
pub fn encode_list_nodes_response(csil_v: &ListNodesResponse) -> Vec<u8> {
    cbor_encode(&csil_enc_list_nodes_response(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a ListNodesResponse.
pub fn decode_list_nodes_response(csil_data: &[u8]) -> Result<ListNodesResponse, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_list_nodes_response(&csil_root)
}

/// Build the canonical CBOR value tree for a RenameNodeRequest.
fn csil_enc_rename_node_request(csil_v: &RenameNodeRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("node_id"), cbor_text(&csil_v.node_id)));
    csil_entries.push((cbor_text("friendly_name"), cbor_text(&csil_v.friendly_name)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a RenameNodeRequest from a decoded CBOR value tree.
fn csil_dec_rename_node_request(
    csil_root: &CsilCborValue,
) -> Result<RenameNodeRequest, CsilCborError> {
    let node_id = {
        let csil_field = cbor_require(csil_root, "node_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let friendly_name = {
        let csil_field = cbor_require(csil_root, "friendly_name")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(RenameNodeRequest {
        node_id,
        friendly_name,
    })
}

/// Encode a RenameNodeRequest to canonical CSIL CBOR bytes.
pub fn encode_rename_node_request(csil_v: &RenameNodeRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_rename_node_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a RenameNodeRequest.
pub fn decode_rename_node_request(csil_data: &[u8]) -> Result<RenameNodeRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_rename_node_request(&csil_root)
}

/// Build the canonical CBOR value tree for a RenameDeviceRequest.
fn csil_enc_rename_device_request(csil_v: &RenameDeviceRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("device_id"), cbor_text(&csil_v.device_id)));
    csil_entries.push((cbor_text("friendly_name"), cbor_text(&csil_v.friendly_name)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a RenameDeviceRequest from a decoded CBOR value tree.
fn csil_dec_rename_device_request(
    csil_root: &CsilCborValue,
) -> Result<RenameDeviceRequest, CsilCborError> {
    let device_id = {
        let csil_field = cbor_require(csil_root, "device_id")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let friendly_name = {
        let csil_field = cbor_require(csil_root, "friendly_name")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(RenameDeviceRequest {
        device_id,
        friendly_name,
    })
}

/// Encode a RenameDeviceRequest to canonical CSIL CBOR bytes.
pub fn encode_rename_device_request(csil_v: &RenameDeviceRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_rename_device_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a RenameDeviceRequest.
pub fn decode_rename_device_request(
    csil_data: &[u8],
) -> Result<RenameDeviceRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_rename_device_request(&csil_root)
}

/// Build the canonical CBOR value tree for a CreateNodeTokenRequest.
fn csil_enc_create_node_token_request(csil_v: &CreateNodeTokenRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    if let Some(csil_inner) = &csil_v.label {
        csil_entries.push((cbor_text("label"), cbor_text(csil_inner)));
    }
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a CreateNodeTokenRequest from a decoded CBOR value tree.
fn csil_dec_create_node_token_request(
    csil_root: &CsilCborValue,
) -> Result<CreateNodeTokenRequest, CsilCborError> {
    let label = match cbor_map_get(csil_root, "label") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    Ok(CreateNodeTokenRequest { label })
}

/// Encode a CreateNodeTokenRequest to canonical CSIL CBOR bytes.
pub fn encode_create_node_token_request(csil_v: &CreateNodeTokenRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_create_node_token_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a CreateNodeTokenRequest.
pub fn decode_create_node_token_request(
    csil_data: &[u8],
) -> Result<CreateNodeTokenRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_create_node_token_request(&csil_root)
}

/// Build the canonical CBOR value tree for a NodeTokenResult.
fn csil_enc_node_token_result(csil_v: &NodeTokenResult) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("token"), cbor_text(&csil_v.token)));
    csil_entries.push((
        cbor_text("fingerprints"),
        cbor_enc_array(&csil_v.fingerprints, |csil_elem| cbor_text(csil_elem)),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a NodeTokenResult from a decoded CBOR value tree.
fn csil_dec_node_token_result(csil_root: &CsilCborValue) -> Result<NodeTokenResult, CsilCborError> {
    let token = {
        let csil_field = cbor_require(csil_root, "token")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let fingerprints = {
        let csil_field = cbor_require(csil_root, "fingerprints")?;
        let csil_decode = |csil_v| cbor_dec_array(csil_v, cbor_as_text);
        csil_decode(csil_field)?
    };
    Ok(NodeTokenResult {
        token,
        fingerprints,
    })
}

/// Encode a NodeTokenResult to canonical CSIL CBOR bytes.
pub fn encode_node_token_result(csil_v: &NodeTokenResult) -> Vec<u8> {
    cbor_encode(&csil_enc_node_token_result(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a NodeTokenResult.
pub fn decode_node_token_result(csil_data: &[u8]) -> Result<NodeTokenResult, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_node_token_result(&csil_root)
}

/// Build the canonical CBOR value tree for a ImportTrackRequest.
fn csil_enc_import_track_request(csil_v: &ImportTrackRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(4);
    csil_entries.push((cbor_text("data"), cbor_bytes(&csil_v.data)));
    if let Some(csil_inner) = &csil_v.content_hash {
        csil_entries.push((cbor_text("content_hash"), cbor_text(csil_inner)));
    }
    csil_entries.push((cbor_text("content_type"), cbor_text(&csil_v.content_type)));
    csil_entries.push((
        cbor_text("root_relative_path"),
        cbor_text(&csil_v.root_relative_path),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a ImportTrackRequest from a decoded CBOR value tree.
fn csil_dec_import_track_request(
    csil_root: &CsilCborValue,
) -> Result<ImportTrackRequest, CsilCborError> {
    let root_relative_path = {
        let csil_field = cbor_require(csil_root, "root_relative_path")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let content_type = {
        let csil_field = cbor_require(csil_root, "content_type")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let content_hash = match cbor_map_get(csil_root, "content_hash") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let data = {
        let csil_field = cbor_require(csil_root, "data")?;
        let csil_decode = cbor_as_bytes;
        csil_decode(csil_field)?
    };
    Ok(ImportTrackRequest {
        root_relative_path,
        content_type,
        content_hash,
        data,
    })
}

/// Encode a ImportTrackRequest to canonical CSIL CBOR bytes.
pub fn encode_import_track_request(csil_v: &ImportTrackRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_import_track_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a ImportTrackRequest.
pub fn decode_import_track_request(csil_data: &[u8]) -> Result<ImportTrackRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_import_track_request(&csil_root)
}

/// Build the canonical CBOR value tree for a ImportResult.
fn csil_enc_import_result(csil_v: &ImportResult) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(3);
    csil_entries.push((cbor_text("imported"), cbor_bool(csil_v.imported)));
    if let Some(csil_inner) = &csil_v.track_id {
        csil_entries.push((cbor_text("track_id"), cbor_text(csil_inner)));
    }
    csil_entries.push((
        cbor_text("skipped_existing"),
        cbor_bool(csil_v.skipped_existing),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a ImportResult from a decoded CBOR value tree.
fn csil_dec_import_result(csil_root: &CsilCborValue) -> Result<ImportResult, CsilCborError> {
    let imported = {
        let csil_field = cbor_require(csil_root, "imported")?;
        let csil_decode = cbor_as_bool;
        csil_decode(csil_field)?
    };
    let track_id = match cbor_map_get(csil_root, "track_id") {
        Some(csil_field) => {
            let csil_decode = cbor_as_text;
            Some(csil_decode(csil_field)?)
        }
        None => None,
    };
    let skipped_existing = {
        let csil_field = cbor_require(csil_root, "skipped_existing")?;
        let csil_decode = cbor_as_bool;
        csil_decode(csil_field)?
    };
    Ok(ImportResult {
        imported,
        track_id,
        skipped_existing,
    })
}

/// Encode a ImportResult to canonical CSIL CBOR bytes.
pub fn encode_import_result(csil_v: &ImportResult) -> Vec<u8> {
    cbor_encode(&csil_enc_import_result(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a ImportResult.
pub fn decode_import_result(csil_data: &[u8]) -> Result<ImportResult, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_import_result(&csil_root)
}

/// Build the canonical CBOR value tree for a Settings.
fn csil_enc_settings(csil_v: &Settings) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(1);
    csil_entries.push((
        cbor_text("entries"),
        cbor_enc_map(
            &csil_v.entries,
            |csil_mk| cbor_text(csil_mk),
            |csil_mv| cbor_text(csil_mv),
        ),
    ));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a Settings from a decoded CBOR value tree.
fn csil_dec_settings(csil_root: &CsilCborValue) -> Result<Settings, CsilCborError> {
    let entries = {
        let csil_field = cbor_require(csil_root, "entries")?;
        let csil_decode = |csil_v| cbor_dec_map(csil_v, cbor_as_text, cbor_as_text);
        csil_decode(csil_field)?
    };
    Ok(Settings { entries })
}

/// Encode a Settings to canonical CSIL CBOR bytes.
pub fn encode_settings(csil_v: &Settings) -> Vec<u8> {
    cbor_encode(&csil_enc_settings(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a Settings.
pub fn decode_settings(csil_data: &[u8]) -> Result<Settings, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_settings(&csil_root)
}

/// Build the canonical CBOR value tree for a SetSettingRequest.
fn csil_enc_set_setting_request(csil_v: &SetSettingRequest) -> CsilCborValue {
    let mut csil_entries: Vec<(CsilCborValue, CsilCborValue)> = Vec::with_capacity(2);
    csil_entries.push((cbor_text("key"), cbor_text(&csil_v.key)));
    csil_entries.push((cbor_text("value"), cbor_text(&csil_v.value)));
    CsilCborValue::Map(csil_entries)
}

/// Reconstruct a SetSettingRequest from a decoded CBOR value tree.
fn csil_dec_set_setting_request(
    csil_root: &CsilCborValue,
) -> Result<SetSettingRequest, CsilCborError> {
    let key = {
        let csil_field = cbor_require(csil_root, "key")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    let value = {
        let csil_field = cbor_require(csil_root, "value")?;
        let csil_decode = cbor_as_text;
        csil_decode(csil_field)?
    };
    Ok(SetSettingRequest { key, value })
}

/// Encode a SetSettingRequest to canonical CSIL CBOR bytes.
pub fn encode_set_setting_request(csil_v: &SetSettingRequest) -> Vec<u8> {
    cbor_encode(&csil_enc_set_setting_request(csil_v))
}

/// Decode canonical CSIL CBOR bytes into a SetSettingRequest.
pub fn decode_set_setting_request(csil_data: &[u8]) -> Result<SetSettingRequest, CsilCborError> {
    let csil_root = cbor_decode(csil_data)?;
    csil_dec_set_setting_request(&csil_root)
}

/// Encode a Role enum as its bare literal value.
fn csil_enc_role(csil_v: &Role) -> CsilCborValue {
    match csil_v {
        Role::Admin => cbor_text("admin"),
        Role::Member => cbor_text("member"),
        Role::Guest => cbor_text("guest"),
    }
}

/// Decode a bare literal value into a Role enum.
fn csil_dec_role(csil_v: &CsilCborValue) -> Result<Role, CsilCborError> {
    let csil_val = cbor_as_text(csil_v)?;
    match csil_val.as_str() {
        "admin" => Ok(Role::Admin),
        "member" => Ok(Role::Member),
        "guest" => Ok(Role::Guest),
        csil_other => Err(CsilCborError(format!(
            "csil cbor: unknown Role value {csil_other:?}"
        ))),
    }
}

/// Encode a PlayerStatus enum as its bare literal value.
fn csil_enc_player_status(csil_v: &PlayerStatus) -> CsilCborValue {
    match csil_v {
        PlayerStatus::Stopped => cbor_text("stopped"),
        PlayerStatus::Playing => cbor_text("playing"),
        PlayerStatus::Paused => cbor_text("paused"),
    }
}

/// Decode a bare literal value into a PlayerStatus enum.
fn csil_dec_player_status(csil_v: &CsilCborValue) -> Result<PlayerStatus, CsilCborError> {
    let csil_val = cbor_as_text(csil_v)?;
    match csil_val.as_str() {
        "stopped" => Ok(PlayerStatus::Stopped),
        "playing" => Ok(PlayerStatus::Playing),
        "paused" => Ok(PlayerStatus::Paused),
        csil_other => Err(CsilCborError(format!(
            "csil cbor: unknown PlayerStatus value {csil_other:?}"
        ))),
    }
}

/// Encode a Codec enum as its bare literal value.
fn csil_enc_codec(csil_v: &Codec) -> CsilCborValue {
    match csil_v {
        Codec::Mp3 => cbor_text("mp3"),
        Codec::Aac => cbor_text("aac"),
        Codec::Vorbis => cbor_text("vorbis"),
        Codec::Flac => cbor_text("flac"),
        Codec::Alac => cbor_text("alac"),
        Codec::Opus => cbor_text("opus"),
        Codec::Wav => cbor_text("wav"),
        Codec::Wma => cbor_text("wma"),
    }
}

/// Decode a bare literal value into a Codec enum.
fn csil_dec_codec(csil_v: &CsilCborValue) -> Result<Codec, CsilCborError> {
    let csil_val = cbor_as_text(csil_v)?;
    match csil_val.as_str() {
        "mp3" => Ok(Codec::Mp3),
        "aac" => Ok(Codec::Aac),
        "vorbis" => Ok(Codec::Vorbis),
        "flac" => Ok(Codec::Flac),
        "alac" => Ok(Codec::Alac),
        "opus" => Ok(Codec::Opus),
        "wav" => Ok(Codec::Wav),
        "wma" => Ok(Codec::Wma),
        csil_other => Err(CsilCborError(format!(
            "csil cbor: unknown Codec value {csil_other:?}"
        ))),
    }
}

/// Encode a TranscodeCodec enum as its bare literal value.
fn csil_enc_transcode_codec(csil_v: &TranscodeCodec) -> CsilCborValue {
    match csil_v {
        TranscodeCodec::Aac => cbor_text("aac"),
        TranscodeCodec::Mp3 => cbor_text("mp3"),
    }
}

/// Decode a bare literal value into a TranscodeCodec enum.
fn csil_dec_transcode_codec(csil_v: &CsilCborValue) -> Result<TranscodeCodec, CsilCborError> {
    let csil_val = cbor_as_text(csil_v)?;
    match csil_val.as_str() {
        "aac" => Ok(TranscodeCodec::Aac),
        "mp3" => Ok(TranscodeCodec::Mp3),
        csil_other => Err(CsilCborError(format!(
            "csil cbor: unknown TranscodeCodec value {csil_other:?}"
        ))),
    }
}

/// Encode a Library enum as its bare literal value.
fn csil_enc_library(csil_v: &Library) -> CsilCborValue {
    match csil_v {
        Library::Music => cbor_text("music"),
        Library::Audiobook => cbor_text("audiobook"),
    }
}

/// Decode a bare literal value into a Library enum.
fn csil_dec_library(csil_v: &CsilCborValue) -> Result<Library, CsilCborError> {
    let csil_val = cbor_as_text(csil_v)?;
    match csil_val.as_str() {
        "music" => Ok(Library::Music),
        "audiobook" => Ok(Library::Audiobook),
        csil_other => Err(CsilCborError(format!(
            "csil cbor: unknown Library value {csil_other:?}"
        ))),
    }
}

/// Encode a PlayerKind enum as its bare literal value.
fn csil_enc_player_kind(csil_v: &PlayerKind) -> CsilCborValue {
    match csil_v {
        PlayerKind::Shared => cbor_text("shared"),
        PlayerKind::Private => cbor_text("private"),
    }
}

/// Decode a bare literal value into a PlayerKind enum.
fn csil_dec_player_kind(csil_v: &CsilCborValue) -> Result<PlayerKind, CsilCborError> {
    let csil_val = cbor_as_text(csil_v)?;
    match csil_val.as_str() {
        "shared" => Ok(PlayerKind::Shared),
        "private" => Ok(PlayerKind::Private),
        csil_other => Err(CsilCborError(format!(
            "csil cbor: unknown PlayerKind value {csil_other:?}"
        ))),
    }
}

/// Encode a PlayerCommand union as a tagged sum `[variant_index, value]`.
fn csil_enc_player_command(csil_v: &PlayerCommand) -> CsilCborValue {
    match csil_v {
        PlayerCommand::Variant0(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(0), csil_enc_cmd_enqueue(csil_x)])
        }
        PlayerCommand::Variant1(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(1), csil_enc_cmd_remove(csil_x)])
        }
        PlayerCommand::Variant2(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(2), csil_enc_cmd_reorder(csil_x)])
        }
        PlayerCommand::Variant3(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(3), csil_enc_cmd_clear(csil_x)])
        }
        PlayerCommand::Variant4(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(4), csil_enc_cmd_play(csil_x)])
        }
        PlayerCommand::Variant5(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(5), csil_enc_cmd_pause(csil_x)])
        }
        PlayerCommand::Variant6(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(6), csil_enc_cmd_next(csil_x)])
        }
        PlayerCommand::Variant7(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(7), csil_enc_cmd_previous(csil_x)])
        }
        PlayerCommand::Variant8(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(8), csil_enc_cmd_seek(csil_x)])
        }
        PlayerCommand::Variant9(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(9), csil_enc_cmd_volume(csil_x)])
        }
    }
}

/// Decode a tagged sum `[variant_index, value]` into a PlayerCommand union.
fn csil_dec_player_command(csil_v: &CsilCborValue) -> Result<PlayerCommand, CsilCborError> {
    let csil_arr = match csil_v {
        CsilCborValue::Array(csil_a) => csil_a,
        _ => {
            return Err(CsilCborError(
                "csil cbor: union expects a 2-element array".to_string(),
            ))
        }
    };
    if csil_arr.len() != 2 {
        return Err(CsilCborError(format!(
            "csil cbor: union array has {} elements, expected 2",
            csil_arr.len()
        )));
    }
    let csil_idx = cbor_as_u64(&csil_arr[0])?;
    match csil_idx {
        0 => {
            let csil_decode = csil_dec_cmd_enqueue;
            Ok(PlayerCommand::Variant0(csil_decode(&csil_arr[1])?))
        }
        1 => {
            let csil_decode = csil_dec_cmd_remove;
            Ok(PlayerCommand::Variant1(csil_decode(&csil_arr[1])?))
        }
        2 => {
            let csil_decode = csil_dec_cmd_reorder;
            Ok(PlayerCommand::Variant2(csil_decode(&csil_arr[1])?))
        }
        3 => {
            let csil_decode = csil_dec_cmd_clear;
            Ok(PlayerCommand::Variant3(csil_decode(&csil_arr[1])?))
        }
        4 => {
            let csil_decode = csil_dec_cmd_play;
            Ok(PlayerCommand::Variant4(csil_decode(&csil_arr[1])?))
        }
        5 => {
            let csil_decode = csil_dec_cmd_pause;
            Ok(PlayerCommand::Variant5(csil_decode(&csil_arr[1])?))
        }
        6 => {
            let csil_decode = csil_dec_cmd_next;
            Ok(PlayerCommand::Variant6(csil_decode(&csil_arr[1])?))
        }
        7 => {
            let csil_decode = csil_dec_cmd_previous;
            Ok(PlayerCommand::Variant7(csil_decode(&csil_arr[1])?))
        }
        8 => {
            let csil_decode = csil_dec_cmd_seek;
            Ok(PlayerCommand::Variant8(csil_decode(&csil_arr[1])?))
        }
        9 => {
            let csil_decode = csil_dec_cmd_volume;
            Ok(PlayerCommand::Variant9(csil_decode(&csil_arr[1])?))
        }
        csil_other => Err(CsilCborError(format!(
            "csil cbor: unknown PlayerCommand variant {csil_other}"
        ))),
    }
}

/// Encode a MediaControl union as a tagged sum `[variant_index, value]`.
fn csil_enc_media_control(csil_v: &MediaControl) -> CsilCborValue {
    match csil_v {
        MediaControl::Variant0(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(0), csil_enc_media_open(csil_x)])
        }
        MediaControl::Variant1(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(1), csil_enc_media_seek(csil_x)])
        }
        MediaControl::Variant2(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(2), csil_enc_media_pause(csil_x)])
        }
        MediaControl::Variant3(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(3), csil_enc_media_resume(csil_x)])
        }
        MediaControl::Variant4(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(4), csil_enc_media_stop(csil_x)])
        }
    }
}

/// Decode a tagged sum `[variant_index, value]` into a MediaControl union.
fn csil_dec_media_control(csil_v: &CsilCborValue) -> Result<MediaControl, CsilCborError> {
    let csil_arr = match csil_v {
        CsilCborValue::Array(csil_a) => csil_a,
        _ => {
            return Err(CsilCborError(
                "csil cbor: union expects a 2-element array".to_string(),
            ))
        }
    };
    if csil_arr.len() != 2 {
        return Err(CsilCborError(format!(
            "csil cbor: union array has {} elements, expected 2",
            csil_arr.len()
        )));
    }
    let csil_idx = cbor_as_u64(&csil_arr[0])?;
    match csil_idx {
        0 => {
            let csil_decode = csil_dec_media_open;
            Ok(MediaControl::Variant0(csil_decode(&csil_arr[1])?))
        }
        1 => {
            let csil_decode = csil_dec_media_seek;
            Ok(MediaControl::Variant1(csil_decode(&csil_arr[1])?))
        }
        2 => {
            let csil_decode = csil_dec_media_pause;
            Ok(MediaControl::Variant2(csil_decode(&csil_arr[1])?))
        }
        3 => {
            let csil_decode = csil_dec_media_resume;
            Ok(MediaControl::Variant3(csil_decode(&csil_arr[1])?))
        }
        4 => {
            let csil_decode = csil_dec_media_stop;
            Ok(MediaControl::Variant4(csil_decode(&csil_arr[1])?))
        }
        csil_other => Err(CsilCborError(format!(
            "csil cbor: unknown MediaControl variant {csil_other}"
        ))),
    }
}

/// Encode a MediaEndReason enum as its bare literal value.
fn csil_enc_media_end_reason(csil_v: &MediaEndReason) -> CsilCborValue {
    match csil_v {
        MediaEndReason::Eos => cbor_text("eos"),
        MediaEndReason::Stopped => cbor_text("stopped"),
    }
}

/// Decode a bare literal value into a MediaEndReason enum.
fn csil_dec_media_end_reason(csil_v: &CsilCborValue) -> Result<MediaEndReason, CsilCborError> {
    let csil_val = cbor_as_text(csil_v)?;
    match csil_val.as_str() {
        "eos" => Ok(MediaEndReason::Eos),
        "stopped" => Ok(MediaEndReason::Stopped),
        csil_other => Err(CsilCborError(format!(
            "csil cbor: unknown MediaEndReason value {csil_other:?}"
        ))),
    }
}

/// Encode a MediaEvent union as a tagged sum `[variant_index, value]`.
fn csil_enc_media_event(csil_v: &MediaEvent) -> CsilCborValue {
    match csil_v {
        MediaEvent::Variant0(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(0), csil_enc_media_header(csil_x)])
        }
        MediaEvent::Variant1(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(1), csil_enc_media_chunk(csil_x)])
        }
        MediaEvent::Variant2(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(2), csil_enc_media_end(csil_x)])
        }
        MediaEvent::Variant3(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(3), csil_enc_media_fail(csil_x)])
        }
    }
}

/// Decode a tagged sum `[variant_index, value]` into a MediaEvent union.
fn csil_dec_media_event(csil_v: &CsilCborValue) -> Result<MediaEvent, CsilCborError> {
    let csil_arr = match csil_v {
        CsilCborValue::Array(csil_a) => csil_a,
        _ => {
            return Err(CsilCborError(
                "csil cbor: union expects a 2-element array".to_string(),
            ))
        }
    };
    if csil_arr.len() != 2 {
        return Err(CsilCborError(format!(
            "csil cbor: union array has {} elements, expected 2",
            csil_arr.len()
        )));
    }
    let csil_idx = cbor_as_u64(&csil_arr[0])?;
    match csil_idx {
        0 => {
            let csil_decode = csil_dec_media_header;
            Ok(MediaEvent::Variant0(csil_decode(&csil_arr[1])?))
        }
        1 => {
            let csil_decode = csil_dec_media_chunk;
            Ok(MediaEvent::Variant1(csil_decode(&csil_arr[1])?))
        }
        2 => {
            let csil_decode = csil_dec_media_end;
            Ok(MediaEvent::Variant2(csil_decode(&csil_arr[1])?))
        }
        3 => {
            let csil_decode = csil_dec_media_fail;
            Ok(MediaEvent::Variant3(csil_decode(&csil_arr[1])?))
        }
        csil_other => Err(CsilCborError(format!(
            "csil cbor: unknown MediaEvent variant {csil_other}"
        ))),
    }
}

/// Encode a NodeDirective union as a tagged sum `[variant_index, value]`.
fn csil_enc_node_directive(csil_v: &NodeDirective) -> CsilCborValue {
    match csil_v {
        NodeDirective::Variant0(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(0), csil_enc_dir_load(csil_x)])
        }
        NodeDirective::Variant1(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(1), csil_enc_dir_pause(csil_x)])
        }
        NodeDirective::Variant2(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(2), csil_enc_dir_resume(csil_x)])
        }
        NodeDirective::Variant3(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(3), csil_enc_dir_stop(csil_x)])
        }
        NodeDirective::Variant4(csil_x) => {
            CsilCborValue::Array(vec![CsilCborValue::Uint(4), csil_enc_dir_volume(csil_x)])
        }
    }
}

/// Decode a tagged sum `[variant_index, value]` into a NodeDirective union.
fn csil_dec_node_directive(csil_v: &CsilCborValue) -> Result<NodeDirective, CsilCborError> {
    let csil_arr = match csil_v {
        CsilCborValue::Array(csil_a) => csil_a,
        _ => {
            return Err(CsilCborError(
                "csil cbor: union expects a 2-element array".to_string(),
            ))
        }
    };
    if csil_arr.len() != 2 {
        return Err(CsilCborError(format!(
            "csil cbor: union array has {} elements, expected 2",
            csil_arr.len()
        )));
    }
    let csil_idx = cbor_as_u64(&csil_arr[0])?;
    match csil_idx {
        0 => {
            let csil_decode = csil_dec_dir_load;
            Ok(NodeDirective::Variant0(csil_decode(&csil_arr[1])?))
        }
        1 => {
            let csil_decode = csil_dec_dir_pause;
            Ok(NodeDirective::Variant1(csil_decode(&csil_arr[1])?))
        }
        2 => {
            let csil_decode = csil_dec_dir_resume;
            Ok(NodeDirective::Variant2(csil_decode(&csil_arr[1])?))
        }
        3 => {
            let csil_decode = csil_dec_dir_stop;
            Ok(NodeDirective::Variant3(csil_decode(&csil_arr[1])?))
        }
        4 => {
            let csil_decode = csil_dec_dir_volume;
            Ok(NodeDirective::Variant4(csil_decode(&csil_arr[1])?))
        }
        csil_other => Err(CsilCborError(format!(
            "csil cbor: unknown NodeDirective variant {csil_other}"
        ))),
    }
}

/// Encode a NodeKind enum as its bare literal value.
fn csil_enc_node_kind(csil_v: &NodeKind) -> CsilCborValue {
    match csil_v {
        NodeKind::Core => cbor_text("core"),
        NodeKind::Satellite => cbor_text("satellite"),
        NodeKind::Client => cbor_text("client"),
    }
}

/// Decode a bare literal value into a NodeKind enum.
fn csil_dec_node_kind(csil_v: &CsilCborValue) -> Result<NodeKind, CsilCborError> {
    let csil_val = cbor_as_text(csil_v)?;
    match csil_val.as_str() {
        "core" => Ok(NodeKind::Core),
        "satellite" => Ok(NodeKind::Satellite),
        "client" => Ok(NodeKind::Client),
        csil_other => Err(CsilCborError(format!(
            "csil cbor: unknown NodeKind value {csil_other:?}"
        ))),
    }
}

/// Encode a AudioOutputsState enum as its bare literal value.
fn csil_enc_audio_outputs_state(csil_v: &AudioOutputsState) -> CsilCborValue {
    match csil_v {
        AudioOutputsState::None => cbor_text("none"),
        AudioOutputsState::Some => cbor_text("some"),
    }
}

/// Decode a bare literal value into a AudioOutputsState enum.
fn csil_dec_audio_outputs_state(
    csil_v: &CsilCborValue,
) -> Result<AudioOutputsState, CsilCborError> {
    let csil_val = cbor_as_text(csil_v)?;
    match csil_val.as_str() {
        "none" => Ok(AudioOutputsState::None),
        "some" => Ok(AudioOutputsState::Some),
        csil_other => Err(CsilCborError(format!(
            "csil cbor: unknown AudioOutputsState value {csil_other:?}"
        ))),
    }
}
