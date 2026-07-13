//! Handwritten codecs for channel union payloads used outside generated service dispatch.
//!
//! csilgen currently emits public byte codecs for record types, but not for union aliases used
//! directly as bidirectional channel payloads. Keep these wrappers out of generated files so a
//! fresh csilgen run is reproducible.

use crate::csil::codec::*;
use crate::csil::types::*;

fn union_encode(variant: u8, mut payload: Vec<u8>) -> Vec<u8> {
    debug_assert!(variant < 24);
    let mut out = Vec::with_capacity(payload.len() + 2);
    out.push(0x82);
    out.push(variant);
    out.append(&mut payload);
    out
}

fn union_split(data: &[u8]) -> Result<(u8, &[u8]), CsilCborError> {
    if data.len() < 2 {
        return Err(CsilCborError(
            "csil cbor: truncated union array".to_string(),
        ));
    }
    if data[0] != 0x82 {
        return Err(CsilCborError(
            "csil cbor: union expects a 2-element array".to_string(),
        ));
    }
    let variant = data[1];
    if variant >= 24 {
        return Err(CsilCborError(
            "csil cbor: union variant must be a small unsigned integer".to_string(),
        ));
    }
    Ok((variant, &data[2..]))
}

pub fn encode_media_control(v: &MediaControl) -> Vec<u8> {
    match v {
        MediaControl::Variant0(x) => union_encode(0, encode_media_open(x)),
        MediaControl::Variant1(x) => union_encode(1, encode_media_seek(x)),
        MediaControl::Variant2(x) => union_encode(2, encode_media_pause(x)),
        MediaControl::Variant3(x) => union_encode(3, encode_media_resume(x)),
        MediaControl::Variant4(x) => union_encode(4, encode_media_stop(x)),
    }
}

pub fn decode_media_control(data: &[u8]) -> Result<MediaControl, CsilCborError> {
    let (variant, payload) = union_split(data)?;
    match variant {
        0 => decode_media_open(payload).map(MediaControl::Variant0),
        1 => decode_media_seek(payload).map(MediaControl::Variant1),
        2 => decode_media_pause(payload).map(MediaControl::Variant2),
        3 => decode_media_resume(payload).map(MediaControl::Variant3),
        4 => decode_media_stop(payload).map(MediaControl::Variant4),
        other => Err(CsilCborError(format!(
            "csil cbor: unknown MediaControl variant {other}"
        ))),
    }
}

pub fn encode_media_event(v: &MediaEvent) -> Vec<u8> {
    match v {
        MediaEvent::Variant0(x) => union_encode(0, encode_media_header(x)),
        MediaEvent::Variant1(x) => union_encode(1, encode_media_chunk(x)),
        MediaEvent::Variant2(x) => union_encode(2, encode_media_end(x)),
        MediaEvent::Variant3(x) => union_encode(3, encode_media_fail(x)),
    }
}

pub fn decode_media_event(data: &[u8]) -> Result<MediaEvent, CsilCborError> {
    let (variant, payload) = union_split(data)?;
    match variant {
        0 => decode_media_header(payload).map(MediaEvent::Variant0),
        1 => decode_media_chunk(payload).map(MediaEvent::Variant1),
        2 => decode_media_end(payload).map(MediaEvent::Variant2),
        3 => decode_media_fail(payload).map(MediaEvent::Variant3),
        other => Err(CsilCborError(format!(
            "csil cbor: unknown MediaEvent variant {other}"
        ))),
    }
}

pub fn encode_node_directive(v: &NodeDirective) -> Vec<u8> {
    match v {
        NodeDirective::Variant0(x) => union_encode(0, encode_dir_load(x)),
        NodeDirective::Variant1(x) => union_encode(1, encode_dir_pause(x)),
        NodeDirective::Variant2(x) => union_encode(2, encode_dir_resume(x)),
        NodeDirective::Variant3(x) => union_encode(3, encode_dir_stop(x)),
        NodeDirective::Variant4(x) => union_encode(4, encode_dir_volume(x)),
    }
}

pub fn decode_node_directive(data: &[u8]) -> Result<NodeDirective, CsilCborError> {
    let (variant, payload) = union_split(data)?;
    match variant {
        0 => decode_dir_load(payload).map(NodeDirective::Variant0),
        1 => decode_dir_pause(payload).map(NodeDirective::Variant1),
        2 => decode_dir_resume(payload).map(NodeDirective::Variant2),
        3 => decode_dir_stop(payload).map(NodeDirective::Variant3),
        4 => decode_dir_volume(payload).map(NodeDirective::Variant4),
        other => Err(CsilCborError(format!(
            "csil cbor: unknown NodeDirective variant {other}"
        ))),
    }
}
