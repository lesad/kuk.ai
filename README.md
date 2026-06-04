# peep-rs

```
                                _ __ ___
   _ __   ___  ___ _ __     _ _| '__/ __|
  | '_ \ / _ \/ _ \ '_ \   | '_| |  \__ \
  | |_) |  __/  __/ |_) | _| | | |  ___) |
  | .__/ \___|\___| .__/(_)_| |_|  |____/
  |_|             |_|
```

CLI that compares two webpage screenshots — design vs implementation — and produces
a similarity score plus a red-overlay diff PNG highlighting the deltas.

Built around the [`image-compare`](https://docs.rs/image-compare) crate using its
hybrid algorithm (MSSIM on luma + RMS on chroma + alpha), tuned for screenshots
with anti-aliased text.

## Status

v0.1.0 — initial release. CLI fully working; deferred for later: TOML config,
multiple algorithms, side-by-side output, anti-aliasing toggle.

## Usage

```sh
peep design.png impl.png
# → score: 0.9958 (99.58% similar)
# → diff:  diff.png
```

Flags:

- `--output <path>` — where to write the diff PNG (default: `diff.png`)
- `--threshold <f64>` — minimum acceptable similarity, range `[0, 1]` (default: `0.99`; `1.0` = identical)
- `--gain <f32>` — visibility gain on the per-pixel diff before clamp (default: `4.0`; higher = exaggerate small differences)
- `--fail` — exit 1 when `score < threshold` (for CI)
- `--json` — emit machine-readable result on stdout
- `--no-diff` — skip writing the diff image

Errors exit with code `2`. `--fail` exits with `1` on threshold breach.

## License

MIT
