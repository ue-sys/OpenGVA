# Web viewer

Shows one web page in a bare window as the page of an HMI area.

    opengva-web-viewer NAME      # reads /etc/opengva/web.d/NAME.conf

Run in the VM (including a TAK Server with client certificate). Not run on real hardware.

## Config

One `name value` per line, `#` comments:

    url          https://server:8443/status/index.html
    window       server-status
    certificate  /etc/station/certificate.pem
    key          /etc/station/key.pem
    authority    /etc/station/authority.pem
    allow        https://maps:8443

| Option | Meaning |
|---|---|
| `url` | Start page (`http`/`https`); the view returns here if lost |
| `window` | Window name; `gva-fa.d` uses it to give an area this view |
| `certificate`, `key` | PEM identity shown to servers |
| `authority` | PEM of the one authority to trust (unset: system's). Never both |
| `allow` | Extra `https://name[:port]` to navigate to; repeatable. Default: the `url` host only |

Other destinations are blocked and logged. Certificate checks are never off.

Run it from a unit bound to the display: `ExecStart=/usr/libexec/opengva/on-display /usr/bin/opengva-web-viewer NAME`.

On load failure the window says why and retries (1, 2, 4, 8, then every 15 s). Logs `OPENGVA web NAME loaded uptime=N`.

No popups, downloads, printing, file choosers, permissions, cookies or storage kept. Typing uses the HMI on-screen keyboard (F5 if the area says `keyboard yes`).

## Build and test

    cd app/viewers/web && cargo test -p web-view

Needs OpenSSL-based GLib TLS (`os/hmi/Containerfile` builds it beside Rocky's GnuTLS one). Code: `crates/web-view` (config, retry), `viewer` (GTK 3 + WebKitGTK), `os/hmi/Containerfile` (built into the crew station's image).
