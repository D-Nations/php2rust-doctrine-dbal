# Instructions for agents

This repository ports doctrine/dbal to Rust, behaviour for behaviour.

## Per-file loop

1. Port one PHP file at a time. Read the whole file and its tests first.
   Each Rust module's first doc lines name the PHP file it ports
   (`//! Upstream: src/X.php`); `scripts/sources.sh` builds
   `parity/sources.tsv` from those lines. Then run
   `python scripts/classes.py sync`.
2. Port the upstream tests in the same change, with the same names and
   assertions, and mark them `ported` in `parity/doctrine/dbal.tsv`.
   Record vectors from real PHP for what tests leave open (a recorder in
   `oracle/recorders/`, a target in `scripts/record.sh`).
3. Update the crate README's contract table. Nothing outside it is claimed.

## Rules

- No PHP source code (php-src) in this repository. doctrine/dbal's source
  and tests (MIT) are the specification. PHP's own behaviour belongs in
  php2rust-compatibility-tools (php2rust-value), learned from the oracle.
- Copy behaviour, not style. Use upstream's message texts; never reword
  one. If PHP's behaviour cannot be kept, stop and ask the owner.
- SQL through sqlx with bound parameters only. No `unsafe`. No
  `unwrap`/`expect` on anything from a database, a file or the network.
- Keep `publish = false` until the owner decides to publish.
- Run `python scripts/dev_check.py full` before a push. There is no hosted
  CI; do not add GitHub workflows. Tests must also build on Windows.
- Small commits named after the PHP file (`feat(Connection.php): ...`).
- Keep the MIT licence and the Doctrine Project's copyright line. No
  wording that suggests Doctrine endorses this work.
