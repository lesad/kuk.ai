# Security Policy

## Reporting a vulnerability

If you find a security issue in kuk or the bundled `kuk-compare` skill,
**please do not open a public issue**. Instead:

1. Use GitHub's private vulnerability reporting:
   <https://github.com/lesad/kuk.ai/security/advisories/new>
2. Or email the maintainer directly at the address listed in
   `Cargo.toml`'s `authors` field.

You should receive an acknowledgement within 7 days. If the issue is
confirmed, the maintainer will work with you on a fix and coordinate
disclosure.

## Scope

In scope:

- The `kuk` CLI binary (`src/`)
- The `figma-fetch.sh` helper script
- The `kuk-compare` skill's documented invariants

Out of scope:

- Vulnerabilities in upstream dependencies — report them upstream. Run
  `cargo audit` to check the current dependency tree.
- Misuse of `FIGMA_TOKEN` in your own shell environment (rotate the token
  via <https://www.figma.com/settings> if you suspect leakage).
- Issues that require physical access to the user's machine.

## Supported versions

Only the latest minor release line receives security fixes during the WIP
/ pre-1.0 phase. Pin a tag if you need stability.
