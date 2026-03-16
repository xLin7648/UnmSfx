# UnmSfx

UnmSfx is a low-latency sound effect playback stack composed of a Rust native
core and a Unity UPM package.

## Repository Layout

- `unm-sfx/`: Rust source code, public C header, and build notes for the
  native runtime.
- `com.xlin.unmsfx/`: Unity package that wraps the native runtime and ships the
  prebuilt binaries used by Unity projects.
- `LICENSE`: Apache License 2.0 for this repository's original source code.
- `THIRD-PARTY-NOTICES.txt`: Third-party dependency notices for the native
  runtime and packaged binaries.

## Build The Native Runtime

1. Open a terminal in `unm-sfx/`.
2. Build the native library for the target platform with Cargo, for example:
   `cargo build --release`
3. For Apple targets, follow the notes in `unm-sfx/docs/apple-build.md`.
4. Copy the generated outputs into `com.xlin.unmsfx/Runtime/bin/` using the
   platform naming expected by the Unity package.

## Use The Unity Package

1. Add `com.xlin.unmsfx/` to a Unity project's `Packages/` directory or consume
   it as a local UPM package.
2. Ensure the required native binaries are present under
   `com.xlin.unmsfx/Runtime/bin/`.
3. Use `UnmSfxManager.Instance` to initialize the runtime, load sounds, and
   play them from Unity code.

## Licensing

Unless otherwise stated in `THIRD-PARTY-NOTICES.txt`, the original source code
in this repository is licensed under Apache License 2.0.
