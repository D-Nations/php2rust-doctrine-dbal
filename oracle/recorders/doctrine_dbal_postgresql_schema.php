<?php
// doctrine/dbal PostgreSQL schema manager vectors (scripts/record.sh
// dbal-pgsql-schema), as typed JSON (lib/typed.php). Output:
// crates/php2rust-doctrine-dbal/tests/data/postgresql_schema.json.
//
// No PostgreSQL server: a scripted driver answers each query with the
// first response whose needle the SQL contains (an empty result
// otherwise), as the system catalogs would, with the values pdo_pgsql
// gives (booleans as bool, the rest as strings). Each case:
//
//   name       the case
//   platform   the platform class (short name)
//   call       the PostgreSQLSchemaManager method: introspectTable
//              (PostgreSQLMetadataProvider through
//              IntrospectingSchemaProvider), listTables (the deprecated
//              select*()/_getPortable*() path), listTableColumns,
//              listTableIndexes, listTableForeignKeys, listTableNames,
//              listSchemaNames, listViews, listSequences, listDatabases,
//              introspectViews, introspectSequences,
//              introspectDatabaseNames, introspectSchemaNames,
//              introspectTableNames, or compare
//              (createComparator()->compareTables() of the recorder's two
//              tables, then getAlterTableSQL())
//   responses  [[needle, column names, rows], ...] (typed values)
//   queries    the queries the call ran: [[sql, params], ...] (typed)
//   outcome    ["ok", typed] or ["throws", class, message]; see export().
//
// Run with: docker run --rm -v $PWD:/work:ro -w /work php:8.4.27-cli-bookworm \
//   php oracle/recorders/doctrine_dbal_postgresql_schema.php

namespace Doctrine\Deprecations {
    if (!class_exists(Deprecation::class)) {
        final class Deprecation
        {
            public static function trigger(string $package, string $link, string $message, mixed ...$args): void
            {
            }

            public static function triggerIfCalledFromOutside(string $package, string $link, string $message, mixed ...$args): void
            {
            }
        }
    }
}

namespace {
    ini_set('display_errors', 'stderr');
    require __DIR__ . '/lib/typed.php';

    $dbal = '/work/upstream/doctrine/dbal';
    spl_autoload_register(static function (string $name) use ($dbal): void {
        foreach (['Doctrine\\DBAL\\' => '/src/'] as $prefix => $dir) {
            if (str_starts_with($name, $prefix)) {
                $file = $dbal . $dir . str_replace('\\', '/', substr($name, strlen($prefix))) . '.php';
                if (is_file($file)) {
                    require_once $file;
                }

                return;
            }
        }
    });

    use Doctrine\DBAL\Cache\ArrayResult;
    use Doctrine\DBAL\Configuration;
    use Doctrine\DBAL\Connection;
    use Doctrine\DBAL\Driver;
    use Doctrine\DBAL\Driver\API\ExceptionConverter;
    use Doctrine\DBAL\Driver\Exception as DriverException;
    use Doctrine\DBAL\Exception\DriverException as DBALDriverException;
    use Doctrine\DBAL\ParameterType;
    use Doctrine\DBAL\Platforms\AbstractPlatform;
    use Doctrine\DBAL\Query;
    use Doctrine\DBAL\Schema\Column;
    use Doctrine\DBAL\Schema\PostgreSQLSchemaManager;
    use Doctrine\DBAL\Schema\Table;
    use Doctrine\DBAL\ServerVersionProvider;
    use Doctrine\DBAL\Types\Types;

    final class Script
    {
        /** @var list<array{string, list<mixed>}> */
        public array $queries = [];

        /** @param list<array{string, list<string>, list<list<mixed>>}> $responses */
        public function __construct(public array $responses)
        {
        }

        public function answer(string $sql, array $params): ArrayResult
        {
            $this->queries[] = [$sql, $params];
            foreach ($this->responses as [$needle, $columns, $rows]) {
                if (str_contains($sql, $needle)) {
                    return new ArrayResult($columns, $rows);
                }
            }

            return new ArrayResult(['none'], []);
        }
    }

    final class ScriptStatement implements Driver\Statement
    {
        private array $params = [];

        public function __construct(private Script $script, private string $sql)
        {
        }

        public function bindValue(int|string $param, mixed $value, ParameterType $type): void
        {
            $this->params[$param] = $value;
        }

        public function execute(): Driver\Result
        {
            ksort($this->params);

            return $this->script->answer($this->sql, array_values($this->params));
        }
    }

    final class ScriptConnection implements Driver\Connection
    {
        public function __construct(private Script $script)
        {
        }

        public function prepare(string $sql): Driver\Statement
        {
            return new ScriptStatement($this->script, $sql);
        }

        public function query(string $sql): Driver\Result
        {
            return $this->script->answer($sql, []);
        }

        public function quote(string $value): string
        {
            return "'" . str_replace("'", "''", $value) . "'";
        }

        public function exec(string $sql): int
        {
            $this->script->queries[] = [$sql, []];

            return 0;
        }

        public function lastInsertId(): int
        {
            return 0;
        }

        public function beginTransaction(): void
        {
        }

        public function commit(): void
        {
        }

        public function rollBack(): void
        {
        }

        public function getServerVersion(): string
        {
            return '17.0';
        }

        public function getNativeConnection(): mixed
        {
            return null;
        }
    }

    final class ScriptConverter implements ExceptionConverter
    {
        public function convert(DriverException $exception, ?Query $query): DBALDriverException
        {
            return new DBALDriverException($exception, $query);
        }
    }

    final class ScriptDriver implements Driver
    {
        public function __construct(private Script $script, private AbstractPlatform $platform)
        {
        }

        public function connect(#[\SensitiveParameter] array $params): Driver\Connection
        {
            return new ScriptConnection($this->script);
        }

        public function getDatabasePlatform(ServerVersionProvider $versionProvider): AbstractPlatform
        {
            return $this->platform;
        }

        public function getExceptionConverter(): ExceptionConverter
        {
            return new ScriptConverter();
        }
    }

    function platform(string $name): AbstractPlatform
    {
        $class = 'Doctrine\\DBAL\\Platforms\\' . $name;

        return new $class();
    }

    $currentSchema = ['current_schema()', ['current_schema'], [['public']]];
    $currentDatabase = ['SELECT CURRENT_DATABASE()', ['current_database'], [['db']]];

    // The column rows: [nspname, relname, attname, typname, complete type,
    // domain base typname, domain complete type, attnotnull, attidentity,
    // default, description, collation].
    $columnRows = [
        ['public', 't', 'id', 'int4', 'integer', null, null, true, 'd', null, null, null],
        ['public', 't', 'name', 'varchar', 'character varying(255)', null, null, true, '', "'x'::character varying", 'the name', 'en_US.utf8'],
        ['public', 't', 'price', 'numeric', 'numeric(10,2)', null, null, false, '', '1.50', null, null],
        ['public', 't', 'code', 'bpchar', 'character(3)', null, null, true, '', "'it''s'::bpchar", null, 'C'],
        ['public', 't', 'active', 'bool', 'boolean', null, null, true, '', 'true', null, null],
        ['public', 't', 'doc', 'jsonb', 'jsonb', null, null, false, '', null, null, null],
        ['public', 't', 'doc2', 'json', 'json', null, null, false, '', 'NULL::json', null, null],
        ['public', 't', 'created', 'timestamp', 'timestamp(0) without time zone', null, null, true, '', 'CURRENT_TIMESTAMP', null, null],
        ['public', 't', 'email', 'email_domain', 'email_domain', 'varchar', 'character varying(100)', false, '', null, null, null],
        ['public', 't', 'data', 'bytea', 'bytea', null, null, false, '', null, null, null],
        ['public', 't', 'other_id', 'int8', 'bigint', null, null, false, '', null, null, null],
    ];
    // quote_ident() quotes a keyword.
    $otherColumnRows = [
        ['public', 't', '"order"', 'int4', 'integer', null, null, false, '', null, null, null],
        ['other_schema', 'u', 'id', 'int4', 'integer', null, null, true, '', null, null, null],
    ];

    $columnsNew = ['a.attnotnull,', ['nspname', 'relname', 'attname', 'typname', 'format_type', 'typname', 'format_type', 'attnotnull', 'attidentity', 'default', 'description', 'collation'], $columnRows];
    $indexesNew = ['i.indisprimary = false', ['nspname', 'relname', 'relname', 'indisunique', 'pg_get_expr', 'attname'], [
        ['public', 't', 'idx_name', false, null, 'name'],
        ['public', 't', 'idx_name', false, null, 'price'],
        ['public', 't', 'partial_idx', false, '(active = true)', 'created'],
        ['public', 't', 'uniq_code', true, null, 'code'],
    ]];
    $primary = ["ct.contype = 'p'", ['nspname', 'relname', 'conname', 'attname'], [['public', 't', 't_pkey', 'id']]];
    $unique = ["ct.contype = 'u'", ['nspname', 'relname', 'conname', 'attname'], [['public', 't', 'uniq_code', 'code']]];
    $foreignNew = ['r.confupdtype', ['nspname', 'relname', 'conname', 'nspname', 'relname', 'confupdtype', 'confdeltype', 'condeferrable', 'condeferred', 'attname', 'attname'], [
        ['public', 't', 'fk_def', 'public', 'other', 'r', 'n', true, true, 'name', 'name'],
        ['public', 't', 'fk_other', 'public', 'other', 'a', 'c', false, false, 'other_id', 'id'],
    ]];
    $optionsNew = ['THEN true ELSE false END,', ['nspname', 'relname', 'case', 'obj_description'], [['public', 't', false, 'a table']]];

    $legacyColumns = static fn (array $rows): array => ['AS isnotnull', ['schema_name', 'table_name', 'field', 'type', 'complete_type', 'domain_type', 'domain_complete_type', 'isnotnull', 'attidentity', 'default', 'comment', 'collation'], $rows];
    $indexesOld = ['i.indisprimary,', ['schema_name', 'table_name', 'relname', 'indisunique', 'indisprimary', 'indkey', 'indrelid', 'where', 'attname'], [
        ['public', 't', 't_pkey', true, true, '1', '16384', null, 'id'],
        ['public', 't', 'idx_name', false, false, '2 3', '16384', null, 'name'],
        ['public', 't', 'idx_name', false, false, '2 3', '16384', null, 'price'],
        ['public', 't', 'partial_idx', false, false, '8', '16384', '(active = true)', 'created'],
        ['public', 't', 'uniq_code', true, false, '4', '16384', null, 'code'],
        ['other_schema', 'u', 'u_pkey', true, true, '1', '16390', null, 'id'],
    ]];
    $foreignOld = ['pg_get_constraintdef', ['schema_name', 'table_name', 'conname', 'condef', 'condeferrable', 'condeferred'], [
        ['public', 't', 'fk_def', 'FOREIGN KEY (name) REFERENCES other(name) ON UPDATE RESTRICT ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED', true, true],
        ['public', 't', 'fk_other', 'FOREIGN KEY (other_id, "order") REFERENCES other_schema.u(id, "user") ON DELETE CASCADE', false, false],
    ]];
    $optionsOld = ['END as unlogged', ['schema_name', 'table_name', 'unlogged', 'comment'], [
        ['public', 't', false, 'a table'],
        ['other_schema', 'u', true, null],
    ]];
    $tableNamesOld = ['quote_ident(table_name) AS table_name', ['table_name', 'schema_name'], [['t', 'public'], ['u', 'other_schema']]];

    $introspection = [$currentSchema, $columnsNew, $indexesNew, $primary, $unique, $foreignNew, $optionsNew];
    $legacy = [
        $currentDatabase, $currentSchema, $legacyColumns(array_merge($columnRows, $otherColumnRows)), $indexesOld, $foreignOld, $optionsOld, $tableNamesOld,
    ];

    $cases = [
        ['pgsql introspectTable', 'PostgreSQLPlatform', 'introspectTable', $introspection],
        ['pgsql120 introspectTable', 'PostgreSQL120Platform', 'introspectTable', $introspection],
        ['pgsql listTables', 'PostgreSQLPlatform', 'listTables', $legacy],
        ['pgsql120 listTables', 'PostgreSQL120Platform', 'listTables', $legacy],
        ['pgsql listTableColumns', 'PostgreSQLPlatform', 'listTableColumns', $legacy],
        ['pgsql listTableColumns qualified', 'PostgreSQLPlatform', 'listTableColumnsQualified', $legacy],
        ['pgsql listTableIndexes', 'PostgreSQLPlatform', 'listTableIndexes', $legacy],
        ['pgsql listTableForeignKeys', 'PostgreSQLPlatform', 'listTableForeignKeys', $legacy],
        ['pgsql listTableNames', 'PostgreSQLPlatform', 'listTableNames', $legacy],
        ['pgsql listTableNames empty', 'PostgreSQLPlatform', 'listTableNames', [$currentDatabase, $currentSchema]],
        ['pgsql listSchemaNames', 'PostgreSQLPlatform', 'listSchemaNames', [['information_schema.schemata', ['schema_name'], [['other_schema'], ['public']]]]],
        ['pgsql listViews', 'PostgreSQLPlatform', 'listViews', [$currentDatabase, ['view_definition AS definition', ['viewname', 'schemaname', 'definition'], [['v', 'public', ' SELECT t.id FROM t;']]]]],
        ['pgsql listSequences', 'PostgreSQLPlatform', 'listSequences', [$currentDatabase, ['AS increment_by', ['relname', 'schemaname', 'min_value', 'increment_by'], [['seq', 'public', '1', '1'], ['seq2', 'other_schema', '10', '5']]]]],
        ['pgsql listDatabases', 'PostgreSQLPlatform', 'listDatabases', [['FROM pg_database', ['datname'], [['db'], ['postgres']]]]],
        ['pgsql introspectViews', 'PostgreSQLPlatform', 'introspectViews', [$currentSchema, ['FROM pg_views', ['schemaname', 'viewname', 'definition'], [['public', 'v', ' SELECT t.id FROM t;'], ['other_schema', 'w', ' SELECT 1;']]]]],
        ['pgsql introspectSequences', 'PostgreSQLPlatform', 'introspectSequences', [$currentSchema, ['information_schema.sequences', ['sequence_schema', 'sequence_name', 'increment', 'minimum_value'], [['other_schema', 'seq2', '5', '10'], ['public', 'seq', '1', '1']]]]],
        ['pgsql introspectDatabaseNames', 'PostgreSQLPlatform', 'introspectDatabaseNames', [$currentSchema, ['FROM pg_database', ['datname'], [['db'], ['postgres']]]]],
        ['pgsql introspectSchemaNames', 'PostgreSQLPlatform', 'introspectSchemaNames', [$currentSchema, ['FROM pg_namespace', ['nspname'], [['other_schema'], ['public']]]]],
        ['pgsql introspectTableNames', 'PostgreSQLPlatform', 'introspectTableNames', [$currentSchema, ['FROM pg_class c', ['nspname', 'relname'], [['other_schema', 'u'], ['public', 't']]]]],
        ['pgsql compare', 'PostgreSQLPlatform', 'compare', []],
    ];

    // compare: an explicit "default" collation is no change; another
    // collation is.
    function compare_tables(): array
    {
        $old = Table::editor()
            ->setUnquotedName('t')
            ->setColumns(
                Column::editor()->setUnquotedName('a')->setTypeName(Types::STRING)->setLength(10)->create(),
                Column::editor()->setUnquotedName('b')->setTypeName(Types::STRING)->setLength(10)->create(),
            )
            ->create();
        $new = Table::editor()
            ->setUnquotedName('t')
            ->setColumns(
                Column::editor()->setUnquotedName('a')->setTypeName(Types::STRING)->setLength(10)->setCollation('default')->create(),
                Column::editor()->setUnquotedName('b')->setTypeName(Types::STRING)->setLength(10)->setCollation('C')->create(),
            )
            ->create();

        return [$old, $new];
    }

    function column_array(Column $column): array
    {
        $array = $column->toArray();
        $array['type'] = $array['type']::class;
        if ($array['default'] instanceof \Doctrine\DBAL\Schema\DefaultExpression) {
            $array['default'] = ['expression', $array['default']::class];
        }

        return $array;
    }

    function index_array(\Doctrine\DBAL\Schema\Index $index): array
    {
        return [$index->getName(), $index->getColumns(), $index->isUnique(), $index->isPrimary(), $index->getFlags(), $index->getOptions()];
    }

    function foreign_key_array(\Doctrine\DBAL\Schema\ForeignKeyConstraint $fk): array
    {
        return [$fk->getName(), $fk->getLocalColumns(), $fk->getForeignTableName(), $fk->getForeignColumns(), $fk->getOptions()];
    }

    $out = [];
    foreach ($cases as [$name, $platformName, $call, $responses]) {
        $script     = new Script($responses);
        $platform   = platform($platformName);
        $connection = new Connection([], new ScriptDriver($script, $platform), new Configuration());
        $manager    = new PostgreSQLSchemaManager($connection, $platform);

        $outcome = typed_call(static function () use ($call, $manager, $platform): mixed {
            switch ($call) {
                case 'introspectTable':
                    return $platform->getCreateTableSQL($manager->introspectTableByUnquotedName('t'));

                case 'listTables':
                    return array_map(static fn (Table $t) => $platform->getCreateTableSQL($t), $manager->listTables());

                case 'listTableColumns':
                    return array_map('column_array', $manager->listTableColumns('t'));

                case 'listTableColumnsQualified':
                    return array_map('column_array', $manager->listTableColumns('other_schema.u'));

                case 'listTableIndexes':
                    return array_map('index_array', $manager->listTableIndexes('t'));

                case 'listTableForeignKeys':
                    return array_map('foreign_key_array', $manager->listTableForeignKeys('t'));

                case 'listTableNames':
                    return $manager->listTableNames();

                case 'listSchemaNames':
                    return $manager->listSchemaNames();

                case 'listViews':
                    return array_map(static fn ($v) => [$v->getName(), $v->getSql()], array_values($manager->listViews()));

                case 'listSequences':
                    return array_map(static fn ($s) => [$s->getName(), $s->getAllocationSize(), $s->getInitialValue()], $manager->listSequences());

                case 'listDatabases':
                    return $manager->listDatabases();

                case 'introspectViews':
                    return array_map(static fn ($v) => [$v->getName(), $v->getSql()], $manager->introspectViews());

                case 'introspectSequences':
                    return array_map(static fn ($s) => [$s->getName(), $s->getAllocationSize(), $s->getInitialValue()], $manager->introspectSequences());

                case 'introspectDatabaseNames':
                    return array_map(static fn ($n) => $n->toString(), $manager->introspectDatabaseNames());

                case 'introspectSchemaNames':
                    return array_map(static fn ($n) => $n->toString(), $manager->introspectSchemaNames());

                case 'introspectTableNames':
                    return array_map(static fn ($n) => $n->toString(), $manager->introspectTableNames());

                case 'compare':
                    [$old, $new] = compare_tables();

                    return $platform->getAlterTableSQL($manager->createComparator()->compareTables($old, $new));
            }

            throw new \LogicException($call);
        });

        $out[] = [
            'name' => $name,
            'platform' => $platformName,
            'call' => $call,
            'responses' => array_map(static fn (array $r) => [$r[0], $r[1], typed_value($r[2])], $responses),
            'queries' => array_map(static fn (array $q) => [$q[0], typed_value($q[1])], $script->queries),
            'outcome' => $outcome[0] === 'throws' ? $outcome : ['ok', $outcome],
        ];
    }

    echo json_encode(['cases' => $out], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";
}
