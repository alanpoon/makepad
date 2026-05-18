spec: task
name: "Minimal MP3 Player Example"
tags: [example, audio, mp3]
---

## Intent

Add a new makepad example crate `makepad-example-media-player` at
`examples/media_player/` that loads a single MP3 file bundled into the binary,
decodes it to PCM with `symphonia`, and plays it through makepad's existing
`cx.audio_output` API. The UI exposes one button that toggles play/pause.
The example exists to demonstrate the smallest viable wiring from a bundled
compressed audio asset to makepad's low-level audio output callback, on
parity with the `counter` and `teamtalk` examples.

## Decisions

- Crate path: `examples/media_player/` with layout `Cargo.toml`, `src/main.rs`, `src/decoder.rs`, `src/player.rs`, `resources/sample.mp3`.
- Crate name: `makepad-example-media-player`, version `1.0.0`, edition `2021`.
- UI dependency: `makepad-widgets = { path = "../../widgets", version = "2.0.0" }`, matching sibling examples.
- MP3 decoder: `symphonia = { version = "0.5", default-features = false, features = ["mp3"] }`. No other audio crates (no `cpal`, no `rodio`, no `minimp3`).
- Asset bundling: `include_bytes!("../resources/sample.mp3")` — the file is baked into the binary; the example performs no filesystem reads at runtime.
- Sample asset: a short (under 5 seconds) public-domain sine-wave tone generated and committed at `examples/media_player/resources/sample.mp3`.
- Decode timing: full decode-to-`Vec<f32>` happens once at `App::live_register` / startup; the audio callback never decodes.
- PCM layout: stereo interleaved `f32`. Mono sources are duplicated to both channels at decode time.
- Sample-rate conversion: linear interpolation inside the `cx.audio_output` callback using `info.sample_rate` as the destination rate. No pre-resample, no caching of resampled buffers.
- Shared state: `Arc<Mutex<PlayerState>>` shared between UI thread and audio callback, where `PlayerState { cursor_frames: f64, playing: bool }`. The mutex is acceptable for this demo (single button, near-zero contention).
- Audio device: `cx.use_audio_outputs(&devices.default_output())` on `AudioDevices` event, exactly the teamtalk pattern.
- End-of-source behavior: when the cursor reaches the last source frame, `playing` is set to `false` and `cursor_frames` is reset to `0.0` in the same callback tick.
- Button label: `Play` when `playing == false`, `Pause` when `playing == true`. The label updates via redraw on click.
- Workspace registration: `examples/media_player` is added to the `members` list in the root `Cargo.toml`.

## Boundaries

### Allowed Changes
- examples/media_player/**
- Cargo.toml
- specs/media-player.spec.md

### Forbidden
- Do not modify any file under `platform/`, `audio_graph/`, `widgets/`, `draw/`, `libs/`, or other example crates.
- Do not add new platform-level audio APIs or extend `media_api.rs`.
- Do not add audio backends other than `symphonia` (no `cpal`, no `rodio`, no `minimp3`, no `puremp3`).
- Do not perform any filesystem or network I/O at runtime; the MP3 must be embedded.
- Do not introduce `.unwrap()` or `.expect()` on the decode path — decode errors must return a `Result`.
- Do not commit MP3 files of unclear copyright; only a self-generated tone or explicitly public-domain audio.

## Completion Criteria

Scenario: Decoder produces stereo interleaved f32 PCM for the bundled sample
  Test:
    Package: makepad-example-media-player
    Filter: test_decode_returns_stereo_interleaved_f32_for_sample
  Given the bundled `resources/sample.mp3` bytes
  When `decoder::decode_mp3(bytes)` is called
  Then the result is `Ok(DecodedPcm)`
  And the channel count is "2"
  And the sample-rate value is greater than "0"
  And the interleaved-samples length is divisible by "2"

Scenario: Paused player does not advance the cursor
  Test:
    Package: makepad-example-media-player
    Filter: test_paused_player_does_not_advance_cursor
  Given a `PlayerState` with `playing = false` and `cursor_frames = 100.0`
  And a decoded source of "44100" Hz with "10000" interleaved samples
  When the audio callback fills "512" output frames at "48000" Hz output rate
  Then the cursor_frames value equals "100.0"
  And the output buffer contains "0.0" in every sample slot

Scenario: Decoder returns Err for truncated input instead of panicking
  Test:
    Package: makepad-example-media-player
    Filter: test_decode_returns_error_for_truncated_input
  Given a byte slice containing the first "16" bytes of a valid MP3
  When `decoder::decode_mp3(bytes)` is called
  Then the result is `Err(DecodeError)`
  And the call returns without panicking

Scenario: Playback stops and cursor resets at end of source
  Test:
    Package: makepad-example-media-player
    Filter: test_player_stops_and_resets_at_end_of_source
  Given a `PlayerState` with `playing = true` and `cursor_frames` set "5" frames before the end of a source of "100" frames
  And source sample-rate equals output sample-rate
  When the audio callback requests "32" output frames
  Then the playing flag becomes "false"
  And the cursor_frames value equals "0.0"
  And the output buffer contains audio samples in the first "5" frames and silence in the remaining "27" frames

Scenario: Example crate builds against the makepad workspace
  Test:
    Package: makepad-example-media-player
    Filter: test_example_crate_smoke_links_app
  Given the `makepad-example-media-player` crate is a workspace member
  And the crate declares a dependency on `makepad-widgets` at path `../../widgets`
  When the test references the crate's `App` type and constructs a default `PlayerState`
  Then the test compiles and links against `makepad-widgets` without error
  And the resulting `PlayerState.playing` value equals the default-stopped value

## Out of Scope

- Seek bar, volume slider, transport controls beyond a single play/pause button.
- Native file-open dialog or any user-selectable file.
- Streaming or chunked decoding; the entire file is decoded at startup.
- Playlist support or more than one bundled track.
- Other audio formats (WAV, FLAC, OGG, AAC).
- Web/Android-specific verification beyond what `cargo check --workspace` covers on the host.
- Any extension to `platform/src/media_api.rs` or new platform-level MP3 APIs.
