// Canonical CBOR (RFC 8949 §4.2.1 "core deterministic") — hand-rolled to be
// *byte-exact* with csilgen-transport's codec and the Rust `codec.gen.rs`.
//
// Why not a general CBOR library? The CSIL wire contract pins two things a
// general lib does not give out of the box:
//   1. Map keys laid down in canonical order — sorted by their *encoded* bytes
//      (which yields shorter-key-first, then bytewise). The Rust generator emits
//      keys already in this order; the TS transport sorts at encode time. We must
//      match, or a byte-comparison conformance test (or a strict peer) would fail.
//   2. A tag-24 ("encoded CBOR data item") wrapper around the opaque event
//      payload in the CSIL-Events envelope.
//
// This module is small, dependency-free, and verified against the published
// conformance vectors (see cbor.conformance.ts notes / README).

/** A CBOR tag: a tag number applied to a nested value. Used for tag-0 timestamps
 * and the tag-24 encoded-CBOR envelope payload. */
export class CborTag {
  constructor(
    readonly tag: number,
    readonly value: CborValue,
  ) {}
}

/** The closed set of JS values our codec round-trips. Plain objects become CBOR
 * maps (text-keyed); `undefined` properties are omitted. */
export type CborValue =
  | number
  | bigint
  | boolean
  | null
  | undefined
  | string
  | Uint8Array
  | Date
  | CborValue[]
  | CborTag
  | { [key: string]: CborValue };

// ---------------------------------------------------------------------------
// Encoder
// ---------------------------------------------------------------------------

class Writer {
  private chunks: number[] = [];

  bytes(): Uint8Array {
    return Uint8Array.from(this.chunks);
  }

  pushByte(b: number): void {
    this.chunks.push(b & 0xff);
  }

  pushBytes(b: Uint8Array): void {
    for (let i = 0; i < b.length; i++) this.chunks.push(b[i]!);
  }

  /** Emit a major-type head with its argument in the shortest form (canonical). */
  head(major: number, n: number): void {
    const mt = major << 5;
    if (n < 24) {
      this.pushByte(mt | n);
    } else if (n < 0x100) {
      this.pushByte(mt | 24);
      this.pushByte(n);
    } else if (n < 0x10000) {
      this.pushByte(mt | 25);
      this.pushByte(n >> 8);
      this.pushByte(n);
    } else if (n < 0x100000000) {
      this.pushByte(mt | 26);
      this.pushByte(n >>> 24);
      this.pushByte(n >>> 16);
      this.pushByte(n >>> 8);
      this.pushByte(n);
    } else {
      // 64-bit argument via BigInt to stay exact past 2^53.
      this.pushByte(mt | 27);
      const big = BigInt(n);
      for (let shift = 56n; shift >= 0n; shift -= 8n) {
        this.pushByte(Number((big >> shift) & 0xffn));
      }
    }
  }

  headBig(major: number, n: bigint): void {
    if (n < 24n) {
      this.pushByte((major << 5) | Number(n));
    } else if (n < 0x100000000n) {
      this.head(major, Number(n));
    } else {
      this.pushByte((major << 5) | 27);
      for (let shift = 56n; shift >= 0n; shift -= 8n) {
        this.pushByte(Number((n >> shift) & 0xffn));
      }
    }
  }
}

const utf8 = new TextEncoder();

function encodeInto(w: Writer, v: CborValue): void {
  if (v === undefined || v === null) {
    w.pushByte(0xf6); // null
    return;
  }
  switch (typeof v) {
    case "boolean":
      w.pushByte(v ? 0xf5 : 0xf4);
      return;
    case "number":
      encodeNumber(w, v);
      return;
    case "bigint":
      if (v >= 0n) w.headBig(0, v);
      else w.headBig(1, -1n - v);
      return;
    case "string": {
      const b = utf8.encode(v);
      w.head(3, b.length);
      w.pushBytes(b);
      return;
    }
  }
  if (v instanceof Uint8Array) {
    w.head(2, v.length);
    w.pushBytes(v);
    return;
  }
  if (v instanceof Date) {
    // Tag 0: RFC3339 text in UTC, matching the Rust `csil_enc_timestamp`.
    encodeInto(w, new CborTag(0, v.toISOString()));
    return;
  }
  if (v instanceof CborTag) {
    w.head(6, v.tag);
    encodeInto(w, v.value);
    return;
  }
  if (Array.isArray(v)) {
    w.head(4, v.length);
    for (const item of v) encodeInto(w, item);
    return;
  }
  // Plain object -> canonical text-keyed map. Omit undefined-valued keys so
  // optional CSIL fields are absent (not null) on the wire.
  encodeMap(w, v as { [k: string]: CborValue });
}

function encodeNumber(w: Writer, n: number): void {
  if (Number.isInteger(n)) {
    if (n >= 0) w.head(0, n);
    else w.head(1, -1 - n);
    return;
  }
  // Non-integer -> IEEE-754 float64 (major 7, additional info 27), matching the
  // Rust codec which always widens floats to f64 on the wire.
  w.pushByte(0xfb);
  const buf = new ArrayBuffer(8);
  new DataView(buf).setFloat64(0, n, false);
  w.pushBytes(new Uint8Array(buf));
}

function encodeMap(w: Writer, obj: { [k: string]: CborValue }): void {
  // Collect present entries, encode each key, then sort by encoded-key bytes
  // (RFC 8949 §4.2.1). Sorting the *encoded* key bytes lexicographically yields
  // shorter-key-first then bytewise — identical to the reference ordering.
  const entries: { keyBytes: Uint8Array; value: CborValue }[] = [];
  for (const key of Object.keys(obj)) {
    const value = obj[key];
    if (value === undefined) continue;
    const kw = new Writer();
    const kb = utf8.encode(key);
    kw.head(3, kb.length);
    kw.pushBytes(kb);
    entries.push({ keyBytes: kw.bytes(), value });
  }
  entries.sort((a, b) => compareBytes(a.keyBytes, b.keyBytes));
  w.head(5, entries.length);
  for (const e of entries) {
    w.pushBytes(e.keyBytes);
    encodeInto(w, e.value);
  }
}

function compareBytes(a: Uint8Array, b: Uint8Array): number {
  const len = Math.min(a.length, b.length);
  for (let i = 0; i < len; i++) {
    if (a[i] !== b[i]) return a[i]! - b[i]!;
  }
  return a.length - b.length;
}

/** Encode a JS value to canonical CBOR bytes. */
export function encode(v: CborValue): Uint8Array {
  const w = new Writer();
  encodeInto(w, v);
  return w.bytes();
}

// ---------------------------------------------------------------------------
// Decoder
// ---------------------------------------------------------------------------

/** What the decoder yields. Tag-0 timestamps become `Date`; every other tag is
 * surfaced as a `CborTag` (the envelope reader unwraps tag-24 itself). Maps
 * become plain objects keyed by their text keys. */
export type DecodedValue =
  | number
  | bigint
  | boolean
  | null
  | string
  | Uint8Array
  | Date
  | DecodedValue[]
  | CborTag
  | { [key: string]: DecodedValue };

class Reader {
  pos = 0;
  constructor(readonly buf: Uint8Array) {}

  u8(): number {
    if (this.pos >= this.buf.length) throw new CborError("unexpected end of input");
    return this.buf[this.pos++]!;
  }

  take(n: number): Uint8Array {
    if (this.pos + n > this.buf.length) throw new CborError("truncated item");
    const out = this.buf.subarray(this.pos, this.pos + n);
    this.pos += n;
    return out;
  }
}

export class CborError extends Error {}

function readArg(r: Reader, low: number): number {
  if (low < 24) return low;
  switch (low) {
    case 24:
      return r.u8();
    case 25: {
      const b = r.take(2);
      return (b[0]! << 8) | b[1]!;
    }
    case 26: {
      const b = r.take(4);
      return (b[0]! * 0x1000000) + ((b[1]! << 16) | (b[2]! << 8) | b[3]!);
    }
    case 27: {
      const b = r.take(8);
      let v = 0n;
      for (let i = 0; i < 8; i++) v = (v << 8n) | BigInt(b[i]!);
      // Keep small enough values as `number` for ergonomic downstream use.
      return v <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(v) : (v as unknown as number);
    }
    default:
      throw new CborError(`reserved additional info ${low}`);
  }
}

function decodeValue(r: Reader): DecodedValue {
  const ib = r.u8();
  const major = ib >> 5;
  const low = ib & 0x1f;

  if (major === 7) {
    switch (low) {
      case 20:
        return false;
      case 21:
        return true;
      case 22:
      case 23:
        return null;
      case 25: {
        // half float — rarely used; decode for completeness.
        const b = r.take(2);
        return halfToFloat((b[0]! << 8) | b[1]!);
      }
      case 26: {
        const b = r.take(4);
        return new DataView(b.buffer, b.byteOffset, 4).getFloat32(0, false);
      }
      case 27: {
        const b = r.take(8);
        return new DataView(b.buffer, b.byteOffset, 8).getFloat64(0, false);
      }
      default:
        throw new CborError(`unsupported simple value ${low}`);
    }
  }

  const arg = readArg(r, low);
  switch (major) {
    case 0:
      return arg;
    case 1:
      return typeof arg === "bigint" ? -1n - arg : -1 - (arg as number);
    case 2:
      return new Uint8Array(r.take(arg as number)); // copy out of the shared buffer
    case 3:
      return new TextDecoder().decode(r.take(arg as number));
    case 4: {
      const n = arg as number;
      const arr: DecodedValue[] = [];
      for (let i = 0; i < n; i++) arr.push(decodeValue(r));
      return arr;
    }
    case 5: {
      const n = arg as number;
      const obj: { [k: string]: DecodedValue } = {};
      for (let i = 0; i < n; i++) {
        const k = decodeValue(r);
        const val = decodeValue(r);
        if (typeof k !== "string") throw new CborError("non-text map key");
        obj[k] = val;
      }
      return obj;
    }
    case 6: {
      const inner = decodeValue(r);
      if (arg === 0 && typeof inner === "string") {
        const d = new Date(inner);
        if (Number.isNaN(d.getTime())) throw new CborError(`invalid tag-0 timestamp: ${inner}`);
        return d;
      }
      return new CborTag(arg as number, inner as CborValue);
    }
    default:
      throw new CborError(`unexpected major type ${major}`);
  }
}

function halfToFloat(h: number): number {
  const sign = (h & 0x8000) >> 15;
  const exp = (h & 0x7c00) >> 10;
  const frac = h & 0x03ff;
  let value: number;
  if (exp === 0) value = frac * 2 ** -24;
  else if (exp === 0x1f) value = frac ? NaN : Infinity;
  else value = (1 + frac / 1024) * 2 ** (exp - 15);
  return sign ? -value : value;
}

/** Decode exactly one CBOR item, rejecting trailing bytes. */
export function decode(bytes: Uint8Array): DecodedValue {
  const r = new Reader(bytes);
  const v = decodeValue(r);
  if (r.pos !== bytes.length) {
    throw new CborError(`${bytes.length - r.pos} trailing bytes`);
  }
  return v;
}
