# VNC viewer

Shows a VNC server's screen in a bare window as the page of an HMI area.

    opengva-vnc-viewer NAME      # reads /etc/opengva/vnc.d/NAME.conf

Run in the VM with a mouse only. Not tried with real touch, tiled areas, host names or IPv6.

## Config

One `name value` per line, `#` comments:

    server     192.168.1.20:5901
    window     engine-room
    view-only  yes

| Option | Meaning |
|---|---|
| `server` | `host:port`, or `host` for 5900. IPv6 with port: `[brackets]` |
| `window` | Window name; `gva-fa.d` uses it to give an area this view |
| `view-only` | `yes`: look only. `no`/unset: touch is a mouse click |
| `password` | File with the server password on line 1; `hmi` user must read it |
| `scale` | `fit` (default) or `none` (1:1, centred) |

Run it from a unit bound to the display: `ExecStart=/usr/libexec/opengva/on-display /usr/bin/opengva-vnc-viewer NAME`.

If the server is gone the window says so and retries (1, 2, 4, 8, then every 10 s). Logs `OPENGVA vnc NAME connected|lost uptime=N`.

No keyboard, sound or clipboard; the link is unencrypted.

## Build and test

    make test                                                       # repo root
    cd app/viewers/vnc && cargo test -p vnc-view -- --include-ignored   # incl. QEMU VNC
    npm install && npm run dev                                      # page in a browser
    cargo run -p vnc-view --example bridge -- HOST:PORT

Browser page: `?url=ws://127.0.0.1:PORT/KEY` plus `&view-only=yes`, `&scale=none`, `&password=`.
Code: `crates/vnc-view` (config, bridge), `src` (noVNC page), `src-tauri` (window), `os/hmi/Containerfile` (built into the crew station's image). Licences: [`NOTICE`](NOTICE).
