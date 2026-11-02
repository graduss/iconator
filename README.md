# Mainmatter Rust Assessment

> Please take no more than ~1 hour to complete this assignment. We’ll discuss your solutions in a call in a few days. Feel free to ask questions if the assignment is unclear at any point.

## Context

This repository contains a simple file-icon lookup library as may be used by IDEs for their project tree sidebar. `get_icon_for_file` and `get_icon_for_folder` accept a filepath and return the numeric ID of a corresponding icon if one is found. Each icon ID corresponds to an svg icon in `./icons`.

## Assignment

1. Implement a web server than allows users to query for file/folder icons over the network. 
2. Write a short (<= 1 page) design/implementation note describing:
  - Design choices and tradeoffs you made (high-level architecture, web server choice, libraries you picked etc.)
  - How you would productionize this service (deployment, observability, etc.)
  
Keep your audience in mind. We’re not interested in a perfect solution but one where we see your thought-process and working style in action.
  
You may add these notes at the end of this file if you wish.

---

# Solution

## Running it

```sh
cargo run                       # http://localhost:3000
cargo test

docker compose up -d --build    # same server in a container
docker compose down
```

```sh
curl "localhost:3000/api/v1/lookup?path=src/main.rs"
curl "localhost:3000/api/v1/lookup?path=.github&kind=folder"
```

There is one endpoint: `GET /api/v1/lookup?path=<path>&kind=file|folder`. It returns the icon's SVG, and `kind` defaults to `file`. There is also `GET /health`, which just answers `ok`.

| Status | When |
|--------|------|
| `200`  | Icon found |
| `400`  | Missing `path`, unknown `kind`, or a path that doesn't end in a name (`""`, `/`, `..`) |
| `404`  | No icon for this path |
| `500`  | The SVG file can't be read (e.g. wrong `ICONS_DIR`) |

| Env var      | Default                 | Description |
|--------------|-------------------------|-------------|
| `ICONS_DIR`  | `<crate dir>/icons`     | Directory with the SVG icons (in compose: the path inside the container, default `/usr/share/iconator/icons`) |
| `RUST_LOG`   | `info,tower_http=debug` | Log filter |
| `ICONS_PATH` | `./icons`               | Compose only: host directory mounted into the container |

The port is hardcoded to 3000. In Docker you can map it to any other port, e.g. `"8080:3000"` in `compose.yml`. On Ctrl+C or SIGTERM the server lets in-flight requests finish and exits.

## What I did and why

The server is a separate binary in the same crate: `src/main.rs` starts it, and the routes are in `src/app.rs`.

First I fixed the library itself. `get_icon_for_file` and `get_icon_for_folder` called `unwrap()` on `file_name()` and `extension()`, so a request like `path=LICENSE` or `path=..` ended in a panic. The service takes paths from the network, so that couldn't stay. I could have wrapped the calls in `catch_unwind`, but the library lives in this same repo, and replacing two `unwrap()`s with `?` is simpler. There's a test for it.

The web framework is axum on tokio. It's the most common choice, and it covers almost everything needed here. `Query` answers `400` on its own if `path` is missing or `kind` has a typo. `TraceLayer` from `tower-http` gives request logs in one line. The router can be tested with `oneshot` without opening a port.

The server returns the SVG directly, not the icon number. An IDE sidebar needs the image in the end, so it doesn't have to make a second request. If a client only needs the number (say, it already has the icons cached), that's a one-line change.

Icons are read from disk on every request, from the directory in `ICONS_DIR`, while the lookup tables are compiled into the binary. The binary stays small, and the icons don't go into the Docker image at all: compose mounts the directory. There are two downsides. The directory has to match the table the binary was built with, and nothing checks that. And every request is a file read. The OS cache makes it cheap, but keeping the icons in memory would be faster.

Errors are a small enum, and the status code for each one is chosen in one place. The server logs the cause of a `500`; the client only gets `Internal error`.

What's missing because it didn't fit in the hour: request timeouts, metrics and a proxy in front of the server.

I left the library's limitations alone. Matching is case-sensitive, so `main.RS` isn't found. Extension keys with a dot (`azure-pipelines.yml`, `.kusto`) never match, because `Path::extension()` returns only the last part of the name. And `/health` only checks that the process is alive; it doesn't look at whether `ICONS_DIR` has any icons.

## How I'd take it to production

- CI: `fmt`, `clippy`, tests, build the image and push it to a registry. The service has no state, so it scales by the number of replicas behind a load balancer or a Kubernetes Ingress, which also handles TLS.
- In production I would put the icons into the image after all, or release them as a separate versioned artifact. Right now the icon set and the table in the binary can drift apart, and you find out from a `500`.
- I'd keep `/health` for liveness and add an `ICONS_DIR` check to readiness, so a replica without icons gets no traffic.
- Icons only change with a release. With `Cache-Control: immutable` and an `ETag`, browsers and a CDN would take almost all the traffic.
- JSON logs for Loki or ELK. Prometheus metrics: requests per second, latency, status codes, `500`s separately. OpenTelemetry traces only if the service ends up in the middle of a long call chain.
- Timeouts and a request size limit via `tower-http`, rate limiting at the edge.

## AI

The tests (`unusual_paths_do_not_panic` in `src/lib.rs` and the HTTP tests in `src/app.rs`), the `Dockerfile` and `compose.yml` were written with the help of AI (Claude Code). I read them and checked them by hand: ran the tests, built the image, started the container with compose, and looked at the responses, the logs and how it shuts down.
