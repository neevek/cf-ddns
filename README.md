# cf-ddns

Simple DDNS client for Cloudflare. This is designed primarily for intranet use where the machine's
outbound interface IP should be published to a DNS record (no public IP lookup).

## Features

- Updates a single DNS record on a fixed interval.
- Reads a TOML config file.
- Picks a local interface IP (A or AAAA). Optional `interface_name` override.
- Uses Cloudflare API via `lmrc-cloudflare`.

## Usage

Default config path (`./config.toml`):

```
cf-ddns
```

Custom config path:

```
cf-ddns --cfg /path/to/config.toml
```

## Config

See `config.example.toml` for a template. Copy it to `./config.toml` and edit values.

Key fields:

- `api_token`: Cloudflare API token with DNS edit permissions.
- `zone`: Cloudflare zone name (example: `example.com`).
- `record_name`: DNS record name (example: `home.example.com`). Defaults to `zone`.
- `record_type`: `A` or `AAAA`.
- `interval_seconds`: Update interval in seconds.
- `interface_name`: Optional NIC name (example: `en0`, `eth0`).
- `proxied`: Optional Cloudflare proxy toggle.
- `ttl`: Optional TTL (1 means "auto" in Cloudflare).

## Notes

- This is meant for intranet scenarios. It does not query a public IP endpoint.
- If auto-detection picks the wrong interface, set `interface_name` explicitly.
- Logging uses `tracing`. Set `RUST_LOG=info` or higher for more detail.

## Interface selection

When `interface_name` is not set, the client chooses a local IP with these rules:

- Skips loopback, virtual, and link-local/multicast/unspecified addresses.
- Prefers "physical-looking" interface names per OS:
  - macOS: `en*`
  - Linux: `en*`, `eth*`, `wl*`, `wlan*`, `wlp*`, `eno*`, `ens*`, `enp*`, `em*`, `p*`
  - Windows: names containing `Ethernet` or `Wi-Fi`
- Falls back to any non-virtual, non-loopback interface if no preferred name matches.

## Example

```
cp config.example.toml ./config.toml
cf-ddns --cfg ./config.toml
```

## Troubleshooting

- `failed to load config at ./config.toml`: Ensure the file exists and is valid TOML.
- `no matching IP found on interface X`: The interface name is wrong or has no IP of the requested type.
- `no suitable interface found for A/AAAA`: Set `interface_name` explicitly, or check that your NIC has an IP of that family.
- `failed to resolve zone_id`: The `zone` name is wrong or the API token lacks permission.

## License

MIT. See `LICENSE`.
