# Audio Book Server
Audio Book Server

Runs with [wasmtime](https://wasmtime.dev/) using `wasmtime serve`. The `--dir` flag sandboxes the component so it can **only** read from the `files/` directory — nothing else on the filesystem is accessible.

Because the output is a `.wasm` file, it is architecture-independent. Build once on any platform (macOS, Linux, etc.) and run the same binary everywhere wasmtime is installed.

## Requirements

| Tool | Version | Purpose |
|---|---|---|
| Rust (nightly) | ≥ 1.82 | Compiler with `wasm32-wasip2` target |
| [wkg](https://github.com/bytecodealliance/wasm-pkg-tools) | 0.15+ | Fetches WASI WIT interface definitions |
| [wasm-tools](https://github.com/bytecodealliance/wasm-tools) | 1.200+ | Inspect/validate `.wasm` components |
| [wasmtime](https://wasmtime.dev/) | 14+ | Run the component |

### Install Rust target

```bash
rustup target add wasm32-wasip2
```

### Install wkg

```bash
cargo install wkg
```

### Install wasmtime

```bash
curl https://wasmtime.dev/install.sh -sSf | bash
```

## Build

```bash
# 1. Fetch WASI WIT interface definitions (only needed once, or after wkg.lock changes)
wkg wit fetch --type wit

# 2. Compile
cargo build --release
```

Output: `target/wasm32-wasip2/release/audio-book-server.wasm`

> `wit/deps/` is populated by `wkg wit fetch` and is gitignored. `wkg.lock` is committed to pin
> the exact WASI interface versions used.

## Run

```bash
wasmtime serve \
  -S cli \
  --dir ./files::/files \
  dates-api/target/wasm32-wasip2/release/audio-book-server.wasm
```

**`--dir ./files::/files`** maps the host's `./files/` to `/files` inside the WASM sandbox. The component **cannot** access any other path.

By default `wasmtime serve` listens on `0.0.0.0:8080`. To change the address:

```bash
wasmtime serve --addr 127.0.0.1:3000 --dir ./files::/files \
  audio-book-server/target/wasm32-wasip2/release/audio-book-server.wasm
```


## API

### `GET /files/A`

Returns an RSS file in the style of a podcast with every audio file in `/files/A/` sorted such that the first file in the list is the 'oldest' date in the RSS feed.  This allows podcast players to get all the audio files in order to play.

```bash
curl http://localhost:8080/files/A
```

### `GET /`

Returns available routes.  Returns an HTML page with a list of all the subdirectories available in `/files`.
