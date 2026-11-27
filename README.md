# php2rust-doctrine-dbal

An unofficial Rust port of [Doctrine DBAL](https://github.com/doctrine/dbal)
4.5.0, behaviour for behaviour: the same SQL, parameters, messages and
errors as the PHP library. The upstream PHPUnit tests are ported with the
same names, and vectors recorded from real PHP 8.4.27 and doctrine/dbal
pin what the tests leave open. Not affiliated with or endorsed by the
Doctrine Project.

Part of the php2rust project. The crate came from
[symfony-rs](https://github.com/php2rust/symfony-rs), with its git
history. PHP's own behaviour (values, arrays, dates) comes from
[php2rust/compatibility-tools](https://github.com/php2rust/compatibility-tools).

**Status.** Work in progress: see the crate's contract table in
[crates/php2rust-doctrine-dbal/README.md](crates/php2rust-doctrine-dbal/README.md)
and the test manifest `parity/doctrine/dbal.tsv` (`ported`, `n/a` or
`pending` for every upstream test). The SQLite driver runs on sqlx. Not
published to crates.io (`publish = false`); use a git dependency pinned to
a commit:

```toml
[dependencies]
php2rust-doctrine-dbal = { git = "https://github.com/php2rust/doctrine-dbal-rs", rev = "<commit>" }
```

To change php2rust-compatibility-tools and see the result here at once,
add an uncommitted `.cargo/config.toml`:

```toml
[patch."https://github.com/php2rust/compatibility-tools"]
php2rust-value = { path = "../php2rust-compatibility-tools/crates/php2rust-value" }
```

When the change is merged there, update `rev` in the root `Cargo.toml`.

## Checks

There is no hosted CI. Run the checks locally:

```sh
python scripts/dev_check.py full     # fmt, clippy -D warnings, tests, parity
```

or one at a time:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/sources.sh --check      # parity/sources.tsv from `//! Upstream:` lines
bash scripts/parity-check.sh         # parity/doctrine/dbal.tsv against the tests
python scripts/classes.py check      # parity/classes/ against parity/sources.tsv
```

## Upstream and vectors

`scripts/fetch-upstream.sh` checks out doctrine/dbal at the pinned commit
(`parity/packages.tsv`) into `upstream/` (ignored by Git, read only).
`scripts/record.sh dbal|dbal-connection` runs the recorders in
`oracle/recorders/` in the PHP 8.4.27 image (`oracle/`, Docker) and writes
`crates/php2rust-doctrine-dbal/tests/data/`.

## Licence

MIT, see [LICENSE](LICENSE). Doctrine DBAL's source and tests are
copyright the Doctrine Project (MIT); the port keeps their notice.
