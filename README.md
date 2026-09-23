# search-api

Product search service for the **Marketly** e-commerce platform.

Exposes a single read-only `GET /search?q=` endpoint backed by an
in-memory inverted index. The index is seeded at startup from a small
static catalog (in production it would be hydrated from the catalog
service, but it is kept local here so the service is self-contained).

## Stack

- **Rust 1.78** + **axum 0.7** + tokio
- In-memory `HashMap`-backed inverted index

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/search?q=...` | Ranked product search |
| GET | `/health` | Liveness probe |
| GET | `/ready` | Readiness probe |

## Local development

```bash
cargo run --release
# then:
curl 'http://localhost:8080/search?q=mechanical+keyboard'
```

## Tests

```bash
cargo test --all --verbose
cargo clippy --all-targets -- -D warnings
```

## Configuration

| Env var | Default | Description |
|---------|---------|-------------|
| `PORT` | `8080` | Listen port |
| `RUST_LOG` | `info` | Tracing filter |
