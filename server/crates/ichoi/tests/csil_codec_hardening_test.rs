//! Regression tests for untrusted CBOR input at the generated codec boundary.

use libichoi::csil::codec::{decode_search_request, encode_search_request};
use libichoi::csil::types::SearchRequest;

#[test]
fn search_request_round_trips() {
    let input = SearchRequest {
        query: "Black Parade".to_string(),
        library: None,
        limit: Some(50),
    };
    let decoded = decode_search_request(&encode_search_request(&input)).expect("decode request");
    assert_eq!(decoded.query, input.query);
    assert_eq!(decoded.limit, input.limit);
}

#[test]
fn decoder_rejects_a_hostile_declared_collection_length() {
    let payloads = [
        [0x9b, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00],
        [0xbb, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00],
        [0x5b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        [0x7b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
    ];
    for payload in payloads {
        assert!(decode_search_request(&payload).is_err());
    }
}

#[test]
fn decoder_rejects_excessive_nesting() {
    let mut payload = vec![0x81; 200];
    payload.push(0xf6);
    assert!(decode_search_request(&payload).is_err());
}

#[test]
fn decoder_rejects_invalid_utf8_and_truncated_values() {
    assert!(decode_search_request(&[0x61, 0xff]).is_err());
    assert!(decode_search_request(&[0x78, 0x05, b'a']).is_err());
}
