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
`pending` for every upstream test). The SQLite driver runs on sqlx, and
so does the MySQL driver (`pdo_mysql`, behind the `pdo_mysql` cargo
feature, so that the default build has no MySQL client). Not
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
php2rust-value = { path = "../compatibility-tools/crates/php2rust-value" }
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
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
bash scripts/sources.sh --check      # parity/sources.tsv from `//! Upstream:` lines
bash scripts/parity-check.sh         # parity/doctrine/dbal.tsv against the tests
python scripts/classes.py check      # parity/classes/ against parity/sources.tsv
```

`full` also runs clippy and the tests with `--all-features` (the
`pdo_mysql` driver and its tests).

## Live MySQL tests

The tests that need a MySQL server (functional MySQLSchemaManagerTest,
Functional/Driver/PDO/MySQL, ...) read the server from
`DBAL_TEST_MYSQL_URL` and pass with a note on stderr when it is not set,
so their rows in `parity/doctrine/dbal.tsv` stay `pending` until they are
run on a machine with a server. Start MySQL 8.4 in Docker and wait until
it accepts TCP connections:

```sh
docker run -d --name dbal-mysql -p 3306:3306 \
  -e MYSQL_ROOT_PASSWORD=root -e MYSQL_DATABASE=doctrine_tests mysql:8.4
until docker exec dbal-mysql mysql -h127.0.0.1 -uroot -proot -e 'SELECT 1' >/dev/null 2>&1; do sleep 1; done
```

Then run the tests with the feature and the URL:

```sh
DBAL_TEST_MYSQL_URL=mysql://root:root@127.0.0.1:3306/doctrine_tests \
  cargo test -p php2rust-doctrine-dbal --features pdo_mysql
```

(PowerShell: `$env:DBAL_TEST_MYSQL_URL = "mysql://root:root@127.0.0.1:3306/doctrine_tests"`,
then the same `cargo test` line.) The database is created when it is
missing; the tests of one file run one after the other. Stop the server
with `docker rm -f dbal-mysql`.

## Live PostgreSQL tests

The tests that need a PostgreSQL server (Functional/Driver/PDO/PgSQL,
the ported PostgreSQLSchemaManagerTest tests and
`tests/pdo_pgsql_live_test.rs`) read the server from
`DBAL_TEST_PGSQL_URL` and pass with a note on stderr when it is not set,
so their rows in `parity/doctrine/dbal.tsv` stay `pending` until they are
run on a machine with a server. Start PostgreSQL 17 in Docker and wait
until it accepts connections:

```sh
docker run -d --name dbal-pgsql -p 5432:5432 \
  -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=doctrine_tests postgres:17
until docker exec dbal-pgsql pg_isready -h 127.0.0.1 -U postgres >/dev/null 2>&1; do sleep 1; done
```

Then run the tests with the feature and the URL:

```sh
DBAL_TEST_PGSQL_URL=postgres://postgres:postgres@127.0.0.1:5432/doctrine_tests \
  cargo test -p php2rust-doctrine-dbal --features pdo_pgsql
```

(PowerShell: `$env:DBAL_TEST_PGSQL_URL = "postgres://postgres:postgres@127.0.0.1:5432/doctrine_tests"`,
then the same `cargo test` line.) The driver speaks the protocol without
TLS, which the image accepts with password (SCRAM) authentication. The
database is created when it is missing; the tests of one file run one
after the other. Stop the server with `docker rm -f dbal-pgsql`.

## Upstream and vectors

`scripts/fetch-upstream.sh` checks out doctrine/dbal at the pinned commit
(`parity/packages.tsv`) into `upstream/` (ignored by Git, read only).
`scripts/record.sh dbal|dbal-connection|dbal-mysql-schema|dbal-pgsql-schema` runs the recorders in
`oracle/recorders/` in the PHP 8.4.27 image (`oracle/`, Docker) and writes
`crates/php2rust-doctrine-dbal/tests/data/`.

## Licence

MIT, see [LICENSE](LICENSE). Doctrine DBAL's source and tests are
copyright the Doctrine Project (MIT); the port keeps their notice.
