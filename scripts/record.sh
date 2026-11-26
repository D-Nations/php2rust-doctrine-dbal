#!/usr/bin/env bash
# Record test vectors from real PHP and doctrine/dbal (the oracle in
# oracle/, Docker) into crates/php2rust-doctrine-dbal/tests/data/.
#
#   scripts/record.sh dbal             tests/data/php.json (providers, SQL\Parser, ExpandArrayParameters, Query\QueryBuilder, date vectors)
#   scripts/record.sh dbal-connection  tests/data/connection.json (pdo_sqlite behaviour, SQLitePlatform SQL and DDL, DsnParser/DriverManager providers)
#   scripts/record.sh all              both
#
# Run scripts/fetch-upstream.sh first: the recorders load doctrine/dbal and
# its tests from upstream/doctrine/dbal.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
export MSYS_NO_PATHCONV=1

if [[ ! -d upstream/doctrine/dbal ]]; then
    echo "upstream/doctrine/dbal is not fetched: run scripts/fetch-upstream.sh" >&2
    exit 1
fi

oracle() { docker compose -f oracle/compose.yaml run --rm -T php "$@"; }

# record RECORDER OUT: run RECORDER, check that it wrote JSON, move it to OUT.
record() {
    local recorder=$1 out=$2 tmp
    mkdir -p oracle/out "$(dirname "$out")"
    tmp=$(mktemp -p oracle/out)
    oracle "oracle/recorders/$recorder" > "$tmp"
    python -c 'import json,sys; json.load(open(sys.argv[1], encoding="utf-8"))' "$tmp"
    mv "$tmp" "$out"
    echo "written to $out"
}

data=crates/php2rust-doctrine-dbal/tests/data

case "${1:-}" in
    dbal) record doctrine_dbal.php "$data/php.json" ;;
    dbal-connection) record doctrine_dbal_connection.php "$data/connection.json" ;;
    all) record doctrine_dbal.php "$data/php.json"; record doctrine_dbal_connection.php "$data/connection.json" ;;
    *) echo "usage: scripts/record.sh dbal|dbal-connection|all" >&2; exit 2 ;;
esac
