# Audio Book Server
Audio Book Server

Runs with [wasmtime](https://wasmtime.dev/) using `wasmtime serve`. The `--dir` flag sandboxes the component so it can **only** read from the `files/` directory — nothing else on the filesystem is accessible.

Because the output is a `.wasm` file, it is architecture-independent. Build once on any platform (macOS, Linux, etc.) and run the same binary everywhere wasmtime is installed.

## Requirements

| Tool | Version | Purpose |
|---|---|---|
| Rust (nightly) | ≥ 1.82 | Compiler with `wasm32-wasip2` target |
| [wasm-tools](https://github.com/bytecodealliance/wasm-tools) | 1.200+ | Inspect/validate `.wasm` components |
| [wasmtime](https://wasmtime.dev/) | 14+ | Run the component |

### Install Rust target

```bash
rustup target add wasm32-wasip2
```

### Install wasmtime

```bash
curl https://wasmtime.dev/install.sh -sSf | bash
```

## Build

```bash
cargo build --release
```

Output: `target/wasm32-wasip2/release/audio_book_server.wasm`

## Run

```bash
wasmtime serve \
  -S cli \
  --env MEDIA_BASE_URL=https://media.example.com \
  --env PODCAST_TITLE="My Audio Book" \
  --env PODCAST_LINK=https://example.com \
  --env PODCAST_DESCRIPTION="Episode feed" \
  --env RSS_SELF_URL=https://feed.example.com/files/A \
  --dir ./files::/files \
  target/wasm32-wasip2/release/audio_book_server.wasm
```

**`--dir ./files::/files`** maps the host's `./files/` to `/files` inside the WASM sandbox. The component **cannot** access any other path.

`-S cli` is required so the component can read environment variables.

By default `wasmtime serve` listens on `0.0.0.0:8080`. To change the address:

```bash
wasmtime serve --addr 127.0.0.1:3000 --dir ./files::/files \
  audio-book-server/target/wasm32-wasip2/release/audio-book-server.wasm
```


## API

### `GET /files/A`

Returns an RSS file in the style of a podcast with every audio file in `/files/A/` sorted such that the first file in the list is the 'oldest' date in the RSS feed.  This allows podcast players to get all the audio files in order to play.

Each RSS item includes `pubDate`. The start date is deterministically derived from the directory name and forced into the past; each subsequent lexically sorted file gets `+1 minute`.

If your audio files are served by Apache or Nginx, build enclosure URLs using a media base URL prefix:

`{MEDIA_BASE_URL}/{dir}/{file}`

Example:

- `MEDIA_BASE_URL=https://media.example.com`
- `dir=A`
- `file=chapter-01.mp3`
- enclosure URL: `https://media.example.com/A/chapter-01.mp3`

The crate now includes `build_media_url(base_url, directory, file_name)` in `src/lib.rs` to normalize slashes and URL-encode path segments for this format.

## Configuration

Required environment variables:

- `MEDIA_BASE_URL`
- `PODCAST_TITLE`
- `PODCAST_LINK`
- `PODCAST_DESCRIPTION`
- `RSS_SELF_URL`

If any required value is missing or empty, the server returns HTTP 500 with a configuration error.

```bash
curl http://localhost:8080/files/A
```

### `GET /`

Returns available routes.  Returns an HTML page with a list of all the subdirectories available in `/files`.
