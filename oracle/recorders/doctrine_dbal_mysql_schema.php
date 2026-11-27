<?php
// doctrine/dbal MySQL schema manager vectors (scripts/record.sh
// dbal-mysql-schema), as typed JSON (lib/typed.php). Output:
// crates/php2rust-doctrine-dbal/tests/data/mysql_schema.json.
//
// No MySQL server: a scripted driver answers each query with the first
// response whose needle the SQL contains (an empty result otherwise),
// as information_schema would. Each case:
//
//   name       the case
//   platform   the platform class (short name)
//   call       introspectTable (MySQLMetadataProvider through
//              IntrospectingSchemaProvider), listTables (the deprecated
//              select*()/_getPortable*() path), listTableColumns, or
//              compare (createComparator()->compareTables() of the
//              recorder's two tables, then getAlterTableSQL())
//   responses  [[needle, column names, rows], ...] (typed values)
//   queries    the queries the call ran: [[sql, params], ...] (typed)
//   outcome    ["ok", typed] or ["throws", class, message]: for the
//              introspecting calls the platform's getCreateTableSQL() of
//              each table (listTableColumns: each column's toArray()
//              without "type", with the type's class).
//
// Run with: docker run --rm -v $PWD:/work:ro -w /work php:8.4.27-cli-bookworm \
//   php oracle/recorders/doctrine_dbal_mysql_schema.php

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
    use Doctrine\DBAL\Schema\MySQLSchemaManager;
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
            return '8.4.0';
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

    // information_schema rows, MySQL style (MariaDB quotes the defaults).
    $database = ['DATABASE()', ['DATABASE()'], [['db']]];

    $columnsNew = static fn (array $defaults, string $jsonType): array => [
        'c.COLLATION_NAME',
        ['TABLE_NAME', 'COLUMN_NAME', 'DATA_TYPE', 'COLUMN_TYPE', 'CHARACTER_MAXIMUM_LENGTH', 'CHARACTER_OCTET_LENGTH', 'NUMERIC_PRECISION', 'NUMERIC_SCALE', 'IS_NULLABLE', 'COLUMN_DEFAULT', 'EXTRA', 'COLUMN_COMMENT', 'CHARACTER_SET_NAME', 'COLLATION_NAME'],
        [
            ['t', 'id', 'int', 'int unsigned', null, null, 10, 0, 'NO', null, 'auto_increment', '', null, null],
            ['t', 'name', 'varchar', 'varchar(255)', 255, 1020, null, null, 'NO', $defaults['name'], '', 'the name', 'utf8mb4', 'utf8mb4_general_ci'],
            ['t', 'price', 'decimal', 'decimal(10,2)', null, null, 10, 2, 'YES', $defaults['price'], '', '', null, null],
            ['t', 'status', 'enum', "enum('a','b''c')", 3, 12, null, null, 'NO', $defaults['status'], '', '', 'utf8mb4', 'utf8mb4_bin'],
            ['t', 'created', 'datetime', 'datetime', null, null, null, null, 'NO', $defaults['created'], '', '', null, null],
            ['t', 'body', 'text', 'text', 65535, 65535, null, null, 'YES', null, '', '', 'latin1', 'latin1_swedish_ci'],
            ['t', 'data', 'blob', 'blob', 65535, 65535, null, null, 'YES', null, '', '', null, null],
            ['t', 'code', 'char', 'char(3)', 3, 12, null, null, 'NO', $defaults['code'], '', '', 'utf8mb4', 'utf8mb4_general_ci'],
            ['t', 'bin', 'varbinary', 'varbinary(16)', 16, 16, null, null, 'YES', null, '', '', null, null],
            ['t', 'doc', $jsonType, $jsonType, null, null, null, null, 'YES', null, '', '', null, null],
            ['t', 'other_id', 'bigint', 'bigint', null, null, 19, 0, 'YES', null, '', '', null, null],
        ],
    ];

    $indexesNew = ['information_schema.STATISTICS', ['TABLE_NAME', 'INDEX_NAME', 'INDEX_TYPE', 'NON_UNIQUE', 'COLUMN_NAME', 'SUB_PART'], [
        ['t', 'ft_body', 'FULLTEXT', 1, 'body', null],
        ['t', 'idx_name', 'BTREE', 1, 'name', 10],
        ['t', 'idx_name', 'BTREE', 1, 'price', null],
        ['t', 'uniq_status', 'BTREE', 0, 'status', null],
    ]];
    $primary = ["'PRIMARY KEY'", ['TABLE_NAME', 'CONSTRAINT_NAME', 'COLUMN_NAME'], [['t', 'PRIMARY', 'id']]];
    $unique = ["'UNIQUE'", ['TABLE_NAME', 'CONSTRAINT_NAME', 'COLUMN_NAME'], [['t', 'uniq_status', 'status']]];
    $foreignNew = ['REFERENTIAL_CONSTRAINTS', ['TABLE_NAME', 'CONSTRAINT_NAME', 'REFERENCED_TABLE_NAME', 'UPDATE_RULE', 'DELETE_RULE', 'COLUMN_NAME', 'REFERENCED_COLUMN_NAME'], [
        ['t', 'fk_other', 'other', 'RESTRICT', 'CASCADE', 'other_id', 'id'],
    ]];
    $options = ['COLLATION_CHARACTER_SET_APPLICABILITY', ['TABLE_NAME', 'ENGINE', 'AUTO_INCREMENT', 'TABLE_COMMENT', 'CREATE_OPTIONS', 'TABLE_COLLATION', 'CHARACTER_SET_NAME'], [
        ['t', 'InnoDB', 5, 'a table', 'row_format=DYNAMIC stats_persistent', 'utf8mb4_general_ci', 'utf8mb4'],
    ]];

    $columnsOld = static fn (array $defaults, string $jsonType): array => [
        'AS field',
        ['TABLE_NAME', 'field', 'type', 'COLUMN_TYPE', 'CHARACTER_MAXIMUM_LENGTH', 'CHARACTER_OCTET_LENGTH', 'NUMERIC_PRECISION', 'NUMERIC_SCALE', 'null', 'key', 'default', 'EXTRA', 'comment', 'characterset', 'collation'],
        [
            ['t', 'id', 'int', 'int unsigned', null, null, 10, 0, 'NO', 'PRI', null, 'auto_increment', '', null, null],
            ['t', 'name', 'varchar', 'varchar(255)', 255, 1020, null, null, 'NO', 'MUL', $defaults['name'], '', 'the name', 'utf8mb4', 'utf8mb4_general_ci'],
            ['t', 'price', 'decimal', 'decimal(10,2)', null, null, 10, 2, 'YES', '', $defaults['price'], '', '', null, null],
            ['t', 'status', 'enum', "enum('a','b''c')", 3, 12, null, null, 'NO', 'UNI', $defaults['status'], '', '', 'utf8mb4', 'utf8mb4_bin'],
            ['t', 'created', 'datetime', 'datetime', null, null, null, null, 'NO', '', $defaults['created'], '', '', null, null],
            ['t', 'body', 'text', 'text', 65535, 65535, null, null, 'YES', 'MUL', null, '', '', 'latin1', 'latin1_swedish_ci'],
            ['t', 'data', 'blob', 'blob', 65535, 65535, null, null, 'YES', '', null, '', '', null, null],
            ['t', 'code', 'char', 'char(3)', 3, 12, null, null, 'NO', '', $defaults['code'], '', '', 'utf8mb4', 'utf8mb4_general_ci'],
            ['t', 'bin', 'varbinary', 'varbinary(16)', 16, 16, null, null, 'YES', '', null, '', '', null, null],
            ['t', 'doc', $jsonType, $jsonType, null, null, null, null, 'YES', '', null, '', '', null, null],
            ['t', 'other_id', 'bigint', 'bigint', null, null, 19, 0, 'YES', 'MUL', null, '', '', null, null],
        ],
    ];
    $indexesOld = ['Key_name', ['TABLE_NAME', 'Non_Unique', 'Key_name', 'Column_Name', 'Sub_Part', 'Index_Type'], [
        ['t', 1, 'ft_body', 'body', null, 'FULLTEXT'],
        ['t', 1, 'idx_name', 'name', 10, 'BTREE'],
        ['t', 1, 'idx_name', 'price', null, 'BTREE'],
        ['t', 0, 'PRIMARY', 'id', null, 'BTREE'],
        ['t', 0, 'uniq_status', 'status', null, 'BTREE'],
    ]];
    $foreignOld = ['referential_constraints', ['TABLE_NAME', 'CONSTRAINT_NAME', 'COLUMN_NAME', 'REFERENCED_TABLE_NAME', 'REFERENCED_COLUMN_NAME', 'ORDINAL_POSITION', 'UPDATE_RULE', 'DELETE_RULE'], [
        ['t', 'fk_other', 'other_id', 'other', 'id', 1, 'RESTRICT', 'CASCADE'],
    ]];
    $tableNames = ["TABLE_TYPE = 'BASE TABLE'\nORDER BY TABLE_NAME", ['TABLE_NAME'], [['t']]];

    $mysqlDefaults = ['name' => 'x', 'price' => '1.50', 'status' => 'a', 'created' => 'CURRENT_TIMESTAMP', 'code' => "it's"];
    $mariadbDefaults = ['name' => "'x'", 'price' => '1.50', 'status' => "'a'", 'created' => 'current_timestamp()', 'code' => "'it''s\\n'"];

    $introspection = static fn (array $defaults, string $jsonType): array => [
        $database, $columnsNew($defaults, $jsonType), $indexesNew, $primary, $unique, $foreignNew, $options,
    ];
    $legacy = static fn (array $defaults, string $jsonType): array => [
        $database, $columnsOld($defaults, $jsonType), $indexesOld, $foreignOld, $options, $tableNames,
    ];
    $compare = [
        ['@@character_set_database', ['@@character_set_database', '@@collation_database'], [['utf8mb4', 'utf8mb4_general_ci']]],
        ['CHARACTER_SETS', ['DEFAULT_COLLATE_NAME'], [['utf8mb4_general_ci']]],
        ['COLLATIONS', ['CHARACTER_SET_NAME'], [['latin1']]],
    ];

    $cases = [
        ['mysql introspectTable', 'MySQLPlatform', 'introspectTable', $introspection($mysqlDefaults, 'json')],
        ['mysql80 introspectTable', 'MySQL80Platform', 'introspectTable', $introspection($mysqlDefaults, 'json')],
        ['mariadb introspectTable', 'MariaDBPlatform', 'introspectTable', $introspection($mariadbDefaults, 'json')],
        ['mariadb1010 introspectTable', 'MariaDB1010Platform', 'introspectTable', $introspection($mariadbDefaults, 'json')],
        ['mysql listTables', 'MySQLPlatform', 'listTables', $legacy($mysqlDefaults, 'json')],
        ['mariadb listTables', 'MariaDBPlatform', 'listTables', $legacy($mariadbDefaults, 'json')],
        ['mariadb120300 listTables', 'MariaDB120300Platform', 'listTables', $legacy($mariadbDefaults, 'json')],
        ['mysql listTableColumns', 'MySQLPlatform', 'listTableColumns', $legacy($mysqlDefaults, 'json')],
        ['mariadb listTableColumns', 'MariaDBPlatform', 'listTableColumns', $legacy($mariadbDefaults, 'json')],
        ['mysql no database', 'MySQLPlatform', 'introspectTable', [['DATABASE()', ['DATABASE()'], [[null]]]]],
        ['mysql compare', 'MySQLPlatform', 'compare', $compare],
        ['mariadb compare', 'MariaDBPlatform', 'compare', $compare],
    ];

    // compare: the column's charset and collation inherited from the
    // table and the database, and a changed collation.
    function compare_tables(): array
    {
        $old = Table::editor()
            ->setUnquotedName('t')
            ->setColumns(
                Column::editor()->setUnquotedName('a')->setTypeName(Types::STRING)->setLength(10)->setCharset('utf8mb4')->create(),
                Column::editor()->setUnquotedName('b')->setTypeName(Types::STRING)->setLength(10)->setCollation('latin1_swedish_ci')->create(),
                Column::editor()->setUnquotedName('c')->setTypeName(Types::STRING)->setLength(10)->create(),
            )
            ->create();
        $new = Table::editor()
            ->setUnquotedName('t')
            ->setColumns(
                Column::editor()->setUnquotedName('a')->setTypeName(Types::STRING)->setLength(10)->setCollation('utf8mb4_general_ci')->create(),
                Column::editor()->setUnquotedName('b')->setTypeName(Types::STRING)->setLength(10)->setCharset('latin1')->create(),
                Column::editor()->setUnquotedName('c')->setTypeName(Types::STRING)->setLength(10)->setCollation('utf8mb4_bin')->create(),
            )
            ->create();

        return [$old, $new];
    }

    $out = [];
    foreach ($cases as [$name, $platformName, $call, $responses]) {
        $script     = new Script($responses);
        $platform   = platform($platformName);
        $connection = new Connection([], new ScriptDriver($script, $platform), new Configuration());
        $manager    = new MySQLSchemaManager($connection, $platform);

        $outcome = typed_call(static function () use ($call, $manager, $platform): mixed {
            switch ($call) {
                case 'introspectTable':
                    return $platform->getCreateTableSQL($manager->introspectTableByUnquotedName('t'));

                case 'listTables':
                    return array_map(static fn (Table $t) => $platform->getCreateTableSQL($t), $manager->listTables());

                case 'listTableColumns':
                    $columns = [];
                    foreach ($manager->listTableColumns('t') as $key => $column) {
                        $array = $column->toArray();
                        $array['type'] = $array['type']::class;
                        if ($array['default'] instanceof \Doctrine\DBAL\Schema\DefaultExpression) {
                            $array['default'] = ['expression', $array['default']::class];
                        }

                        $columns[$key] = $array;
                    }

                    return $columns;

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
