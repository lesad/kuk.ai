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

Early scaffold. Implementation in progress — see
[`plans/p-pairs-new-sorted-orbit.md`](https://github.com/lesad/peep-rs/blob/main/plans/p-pairs-new-sorted-orbit.md)
for the design.

## Usage (planned)

```sh
peep design.png impl.png
# → score: 0.9958 (99.58% similar)
# → diff:  diff.png
```

Flags:

- `--output <path>` — where to write the diff PNG (default: `diff.png`)
- `--threshold <f64>` — minimum acceptable similarity, range `[0, 1]` (default: `0.99`; `1.0` = identical)
- `--fail` — exit 1 when `score < threshold` (for CI)
- `--json` — emit machine-readable result on stdout
- `--no-diff` — skip writing the diff image

Errors exit with code `2`. `--fail` exits with `1` on threshold breach.

## License

MIT
