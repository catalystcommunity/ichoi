// Decoder shim for the Symphonia→WASM module `ichoi-decoder` (DESIGN §5.5). The
// browser decodes EVERY codec with this one WASM build — not WebCodecs — because
// only Symphonia gives us ALAC in a browser and dodges WebCodecs' Firefox/Linux
// AAC holes.
//
// The wasm module may not exist yet. We code against a stable interface and
// degrade gracefully: if the module can't be loaded, `loadDecoder` returns
// `undefined`, the UI still browses/queues/controls shared targets, and local
// playback logs a clear "decoder unavailable" message instead of throwing.

import type { Codec } from "../schema.ts";

/** Per-(re)open stream description, mirroring the `MediaHeader` fields the decoder
 * needs. `codecConfig` carries FLAC STREAMINFO / AAC AudioSpecificConfig / etc. */
export interface DecoderConfig {
  codec: Codec;
  sampleRate: number;
  channels: number;
  codecConfig?: Uint8Array;
}

/**
 * The interface the WASM module must expose. `decode` takes one or more codec
 * packets (as delivered in `MediaChunk.data`) and returns planar PCM: one
 * Float32Array per channel, in native sample rate. An empty array means "packet
 * consumed, no output yet" (decoder priming). `close` frees WASM state.
 */
export interface IchoiDecoder {
  /** Decode a single codec packet into planar Float32 PCM (one array per channel). */
  decode(packet: Uint8Array): Float32Array[];
  /** Flush any buffered output at end-of-stream. */
  flush(): Float32Array[];
  /** Reset decoder state after a seek. */
  reset(): void;
  /** Release WASM resources. */
  close(): void;
}

/** The module-level factory the WASM glue is expected to export. */
export interface IchoiDecoderModule {
  /** Construct a decoder for a given stream description. */
  createDecoder(config: DecoderConfig): IchoiDecoder;
  /** Codecs this build can decode (Opus/HE-AAC never appear — they arrive
   * pre-transcoded to AAC-LC from the server, §5.6). */
  supportedCodecs(): Codec[];
}

let cached: IchoiDecoderModule | null | undefined;

/**
 * Attempt to load `ichoi-decoder`. Resolution order:
 *   1. a global `window.ichoiDecoder` (e.g. injected by a native shell),
 *   2. a sibling ES module `./ichoi-decoder.js` (wasm-bindgen glue) served next
 *      to the theme.
 * Returns `undefined` (once, memoized) if neither is present.
 */
export async function loadDecoder(baseUrl = import.meta.env.BASE_URL): Promise<
  IchoiDecoderModule | undefined
> {
  if (cached !== undefined) return cached ?? undefined;

  // 1. Native/global injection.
  const injected = (globalThis as { ichoiDecoder?: IchoiDecoderModule }).ichoiDecoder;
  if (injected && typeof injected.createDecoder === "function") {
    cached = injected;
    return injected;
  }

  // 2. Sibling wasm-bindgen glue module. Guarded so a 404 degrades quietly.
  try {
    const url = new URL("ichoi-decoder.js", new URL(baseUrl, location.href)).href;
    const mod = (await import(/* @vite-ignore */ url)) as {
      default?: (input?: unknown) => Promise<unknown>;
      createDecoder?: IchoiDecoderModule["createDecoder"];
      supportedCodecs?: IchoiDecoderModule["supportedCodecs"];
    };
    // wasm-bindgen modules expose a default `init(wasmUrl)`; call it if present.
    if (typeof mod.default === "function") {
      const wasmUrl = new URL("ichoi-decoder_bg.wasm", new URL(baseUrl, location.href)).href;
      await mod.default(wasmUrl);
    }
    if (typeof mod.createDecoder === "function") {
      cached = {
        createDecoder: mod.createDecoder,
        supportedCodecs: mod.supportedCodecs ?? (() => []),
      };
      return cached;
    }
    throw new Error("module lacks createDecoder()");
  } catch (e) {
    console.warn(
      "[ichoi] decoder module 'ichoi-decoder' unavailable — local playback disabled. " +
        "Browse, queue, and jukebox control still work.",
      e,
    );
    cached = null;
    return undefined;
  }
}

/** Reset the memoized load (used by tests / after installing the wasm). */
export function resetDecoderCache(): void {
  cached = undefined;
}
