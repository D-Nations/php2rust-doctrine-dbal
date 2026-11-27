# Doctrine DBAL

Private workspace crate: a port of `doctrine/dbal` 4.5.0 (`c435cd7e`), as
installed by symfony/demo. MIT; Doctrine source and tests copyright the
Doctrine Project. Unofficial, not affiliated with or endorsed by Doctrine.

```rust
use std::collections::BTreeMap;

use php2rust_doctrine_dbal::sql::Parser;
use php2rust_doctrine_dbal::{ArrayParameterType, ExpandArrayParameters, WrapperParameterType};
use php_value::{Key, PhpArray, Value};

let mut params = PhpArray::new();
params.insert("ids", Value::Array(PhpArray::list([Value::Int(1), Value::Int(2)])));
let mut types = BTreeMap::new();
types.insert(Key::from("ids"), WrapperParameterType::Array(ArrayParameterType::Integer));

let mut visitor = ExpandArrayParameters::new(&params, &types);
Parser::new(false)
    .parse(b"SELECT * FROM t WHERE id IN (:ids)", &mut visitor)
    .unwrap();
assert_eq!(visitor.get_sql().as_bytes(), b"SELECT * FROM t WHERE id IN (?, ?)");
assert_eq!(visitor.get_parameters(), &[Value::Int(1), Value::Int(2)]);
```

| Contract | Source | Evidence |
|---|---|---|
| Statement parameters found by `SQL\Parser`: named (`:name`), positional (`?`, not `??`), skipped inside ANSI or MySQL string literals, backtick and bracket identifiers (not `ARRAY[`), `::` casts and comments | `SQL/Parser.php`, `SQL/Parser/Visitor.php` | ParserTest (4); oracle vectors (`tests/parser_vectors.rs`) |
| List parameters expanded (`IN (?)` to `IN (?, ?, ?)`, empty list to `NULL`), named parameters made positional, types moved to their new positions; `MissingNamedParameter` / `MissingPositionalParameter` messages | `ExpandArrayParameters.php`, `ArrayParameters/Exception/*.php` | ExpandArrayParametersTest (3); oracle vectors |
| `ParameterType` and `ArrayParameterType` cases and `toElementParameterType()` | `ParameterType.php`, `ArrayParameterType.php` | ExpandArrayParametersTest |
| `QueryBuilder` SQL for SELECT (DISTINCT, FROM with aliases, inner/left/right/full joins in upstream's nesting order, WHERE, GROUP BY, HAVING, ORDER BY, LIMIT/OFFSET, FOR UPDATE [SKIP LOCKED], WITH), UNION [ALL], INSERT, UPDATE and DELETE; `where`/`andWhere`/`orWhere` and the HAVING equivalents' nesting; resets; the SQL cache, kept by `setValue()`, `setParameter()` and a failed `addUnion()` as in PHP | `Query/QueryBuilder.php`, `Query/*.php`, `SQL/Builder/{Select,DefaultSelect,Union,DefaultUnion,With}SQLBuilder.php` | QueryBuilderTest (73 of 84); oracle call sequences on two platforms (`tests/query_builder_vectors.rs`) |
| Parameters: `setParameter(s)`, `getParameter(Type)(s)` with PHP's key normalization and order, `createNamedParameter()` (`:dcValueN`, custom placeholder) and `createPositionalParameter()` sharing one counter; `clone` gives object parameters a new identity | `Query/QueryBuilder.php` | QueryBuilderTest; oracle call sequences |
| Messages of `QueryException`, `NonUniqueAlias`, `UnknownAlias`, `NotSupported` (`FOR UPDATE`, `SKIP LOCKED`) and the negative-offset `InvalidArgumentException` | `Query/QueryException.php`, `Query/Exception/*.php`, `Platforms/Exception/NotSupported.php`, `Exception/InvalidArgumentException.php` | QueryBuilderTest; oracle call sequences |
| `ExpressionBuilder` comparisons, `IS [NOT] NULL`, `[NOT] LIKE ... ESCAPE`, `[NOT] IN (...)`, `and()`/`or()`; `literal()` delegates to the connection's `quote()` | `Query/Expression/ExpressionBuilder.php` | ExpressionBuilderTest (19) |
| `CompositeExpression` `and()`/`or()`, immutable `with()`, `count()`, string form (one part bare, else parenthesized and joined) | `Query/Expression/CompositeExpression.php` | CompositeExpressionTest (3) |
| `AbstractPlatform` defaults: `createSelectSQLBuilder()` (`FOR UPDATE`, `SKIP LOCKED`), `createUnionSQLBuilder()`, `createWithSQLBuilder()`, `modifyLimitQuery()`/`doModifyLimitQuery()`, `getUnionSelectPartSQL()` (`(sub)`), `getUnionAllSQL()`, `getUnionDistinctSQL()` | `Platforms/AbstractPlatform.php` | oracle call sequences ("default" platform: the methods as PostgreSQLPlatform keeps them) |
| `Type` trait and the process-wide registry: `getType()`, `hasType()`, `addType()` (class or instance), `overrideType()`, `lookupName()`, `getTypesMap()`, identity by instance; `TypeRegistry` and its `TypeNotFound`, `TypesAlreadyExists`, `TypeAlreadyRegistered`, `TypeNotRegistered`, `UnknownColumnType`, `TypeArgumentCountError` messages | `Types/Type.php`, `Types/TypeRegistry.php`, `Types/Types.php`, `Types/Exception/*.php` | TypeTest (4), TypeRegistryTest (13), TypeAlreadyRegisteredTest (1), TypeNotRegisteredTest (1) |
| The 31 built-in types' PHP and database conversions, binding types, SQL declarations (through the platform) and `ConversionException` messages: strings, GUID, integers, floats, decimal, `BcMath\Number`, boolean, binary, blob, JSON and JSON object, simple array, enum, date, time, date-time (mutable, immutable, tz, UTC, variable) and date interval | `Types/*Type.php`, `Types/ConversionException.php`, `Types/Exception/*.php` | 164 tests of the 27 type test files (`tests/types_*_test.rs`; the inherited `BaseDateTypeTestCase` tests repeated in each date class), ConversionExceptionTest (4), SerializationFailedTest (1) |
| The `AbstractPlatform` methods the types call (declaration snippets, boolean conversion, date formats) and the column exceptions they throw (`ColumnLengthRequired`, `ColumnPrecisionRequired`, `ColumnScaleRequired`, `ColumnValuesRequired`, `InvalidColumnDeclaration`) | `Platforms/AbstractPlatform.php`, `Exception/*.php` | The type tests above, through a stub platform as upstream mocks it |
| `DateTime::createFromFormat()`, `new DateTime($text)` and `new DateInterval($duration)` as the date types use them (in php-value) | php-value | 12,036 + 89 + 1,602 recorded answers (`tests/date_vectors.rs`); the texts refused are counted there |
| `Connection`: lazy connect, `executeQuery()`, `executeStatement()`, `prepare()`, the `fetch*()` and `iterate*()` helpers, `insert()`/`update()`/`delete()`, list parameter expansion, type conversion of bound values, `quote()`/`quoteIdentifier()`, `lastInsertId()`, auto-commit, nested transactions with savepoints, rollback-only, `transactional()` with upstream's rollback rules, isolation levels, `close()`, and the conversion of driver exceptions (with the query) | `Connection.php`, `ConnectionException.php`, `TransactionIsolationLevel.php` | ConnectionTest (30 of 33, over a recording test driver); functional ConnectionTest (17 of 21), TransactionTest (1 of 6), WriteTest (19 of 20), NamedParametersTest, DataAccessTest (36), BooleanBindingTest, AutoIncrementColumnTest on `pdo_sqlite` |
| `Result` and `Statement`: `fetchNumeric/Associative/One()`, `fetchAll*()`, `fetchAllKeyValue()` (`NoKeyValue`), `fetchAllAssociativeIndexed()`, `fetchFirstColumn()`, the `iterate*()` forms, `rowCount()`, `columnCount()`, `getColumnName()`, `bindValue()` with wrapper types, `executeQuery()`/`executeStatement()` | `Result.php`, `Statement.php`, `Driver/FetchUtils.php` | Functional ResultTest, StatementTest (11 of 12), Connection/FetchTest (16), Connection/FetchEmptyTest (6) |
| Exceptions: the `SQLite` exception converter (`UniqueConstraintViolation`, `NotNullConstraintViolation`, `TableNotFound`, `TableExists`, `InvalidFieldName`, `NonUniqueFieldName`, `SyntaxError`, `ReadOnly`, `LockWaitTimeout`, `ConnectionException`, ...), `DriverException`'s message (`An exception occurred while executing a query: ...`), the `Exception/*` messages | `Driver/API/SQLite/ExceptionConverter.php`, `Exception/*.php` | Functional ExceptionTest (9 of 12); the recorded `pdo_sqlite` errors |
| The `pdo_sqlite` driver on sqlx: value storage classes, PDO's binding conversions, `SQLSTATE` messages, `rowCount()`, `lastInsertId()` (`NoIdentityValue`), `quote()`, `getServerVersion()`, `path`/`memory` parameters | `Driver/PDO/SQLite/Driver.php`, `Driver/PDO/*.php`, `Driver/AbstractSQLiteDriver.php`, `Driver/*.php` interfaces | `tests/pdo_sqlite_vectors.rs` (recorded `pdo_sqlite` answers) |
| `DriverManager::getConnection()` (driver map, `driverClass`, `wrapperClass` checks, URL parameters through `DsnParser`), `getAvailableDrivers()`; `DsnParser::parse()`; the `Configuration` parts the connection reads (auto-commit, type comments) | `DriverManager.php`, `Tools/DsnParser.php`, `Configuration.php` | DriverManagerTest (7 of 8), DsnParserTest (2), ConfigurationTest (2) |
| The driver middlewares `AbstractDriverMiddleware`, `AbstractConnectionMiddleware`, `AbstractStatementMiddleware` and `AbstractResultMiddleware`, delegating each method to the wrapped driver, connection, statement or result (a `LogicException` from `getColumnName()` of a result without it); the driver connection's `getNativeConnection()` (the `pdo_sqlite` connection gives its sqlx connection) | `Driver/Middleware/*.php`, `Driver/Connection.php` | AbstractConnectionMiddlewareTest (5), AbstractDriverMiddlewareTest (1), AbstractResultMiddlewareTest (1), AbstractStatementMiddlewareTest (1; `tests/driver_middleware_*_test.rs`) |
| `QueryBuilder` execution: `executeQuery()` (with the profile of `enableResultCache()`, removed by `disableResultCache()`), `executeStatement()` and the `fetch*()` methods over a `Connection` | `Query/QueryBuilder.php` | QueryBuilderTest execution tests (10; `tests/query_builder_execution_test.rs`) |
| On `pdo_sqlite`: `QueryBuilder` UNION queries (`UNION`/`UNION ALL` of SQL strings and of sub-builders, with `ORDER BY`, limit and offset, named parameters of the outer builder), common table expressions (with column lists, named, positional and list parameters, a UNION inside a CTE and a UNION main query), and the `NotSupported` of `SKIP LOCKED` | `Query/QueryBuilder.php`, `SQL/Builder/{DefaultSelectSQLBuilder,DefaultUnionSQLBuilder,WithSQLBuilder}.php` | Functional Query/QueryBuilderTest (19 of 22; the other 3 upstream does not run on SQLite; `tests/functional_query_query_builder_test.rs`) |
| The result cache: `Connection::executeCacheQuery()` and `executeQuery()` with a `QueryCacheProfile` (`execute_query_with_profile()`), the profile's or the `Configuration`'s pool, the rows kept as `ArrayResult`s under the real key, an older cached format re-executed, `NoResultDriverConfigured`; `QueryCacheProfile` (immutable setters, `NoCacheKey`, `generateCacheKeys()` with PHP's `serialize()`, `sha1()` and the SHA-256 of the connection parameters without the password); `ArrayResult` (fetches, `InvalidColumnIndex`, `free()`, `serialize()`/`unserialize()` in the 4.2 and 4.1 formats) | `Cache/*.php`, `Cache/Exception/*.php`, `Connection.php`, `Configuration.php` | ArrayResultTest (14), QueryCacheProfileTest (6) and the keys PHP 8.3 computes for them, Connection/CachedQueryTest (3), ConnectionTest's cache test |
| A PSR-6 pool for the result cache (and the ORM's caches): `CacheItemPool` (`Psr\Cache\CacheItemPoolInterface`, as far as DBAL and the ORM use it), `CacheItem`, `validateKey()`'s messages, and an in-memory `ArrayAdapter` as Symfony's (copies of the values, expiry by `microtime()`, `clear($prefix)`). It lives here until a psr-cache crate exists, and may move there | none (`cache/pool.rs`) | the tests above |
| `SQLitePlatform`: expressions (trim, locate, substring, date arithmetic and difference, mod, concat, bit operations), `escapeStringForLike()`, quoting, `TRUNCATE`, limit/offset, column declarations and defaults, `CREATE TABLE` (primary keys, autoincrement, indexes, unique constraints, inline foreign keys, comments, `WITHOUT ROWID`), `ALTER TABLE` (`ADD COLUMN`, `RENAME`, and the temporary-table rebuild), indexes, foreign key SQL, transaction isolation, the reserved keywords (`SQLiteKeywords`, `KeywordList`), the `NotSupported` and `NoColumnsSpecifiedForTable` messages | `Platforms/SQLitePlatform.php`, `Platforms/AbstractPlatform.php`, `Platforms/Keywords/{KeywordList,SQLiteKeywords}.php`, `Platforms/{DateIntervalUnit,TrimMode}.php`, `Platforms/Exception/*.php`, `TransactionIsolationLevel.php`, `LockMode.php` | SQLitePlatformTest with the inherited AbstractPlatformTestCase tests (92 of 96; the other 4 upstream does not run on SQLite); 1,500 expression, 600 `CREATE TABLE` and 400 `ALTER TABLE` recorded outputs (`sqlite_platform_*_vectors` in `tests/platforms_sqlite_platform_test.rs`) |
| Schema value objects: names and their parser, `AbstractAsset` (and its named subclasses), `Identifier`, `Column`, `ColumnDiff`, `Index`, `IndexedColumn`, `UniqueConstraint`, `ForeignKeyConstraint` (match type, referential actions, deferrability), `PrimaryKeyConstraint`, `Table`, `TableDiff`, `SchemaConfig`, their editors, the named object sets and the Schema exceptions | `Schema/*.php` (not `Comparator.php`, `ComparatorConfig.php` or the schema managers), `Schema/Name/**`, `Schema/Collections/**`, `Schema/Exception/*.php` | 297 tests of the Schema test classes (`tests/schema_*_test.rs`): TableTest 83 of 84, TableEditorTest 38, IndexTest 28, ForeignKeyConstraintEditorTest 18, ForeignKeyConstraintTest 16, ColumnTest 15, AbstractAssetTest 4 (name parsing deprecations on the Oracle, PostgreSQL and MySQL platforms), the Collections, Name, editor and constraint tests |
| `Comparator::compareSchemas()` (created and dropped namespaces, tables and sequences by their shortest names, altered tables and sequences, auto-increment sequences left out), `diffSequence()`, `compareTables()` (added, dropped, changed and renamed columns, explicit renames, added, dropped, modified and renamed indexes with the ambiguity rules, foreign keys compared by properties, the modified-indexes deprecation), `ComparatorConfig`, and the SQLite comparator that ignores an explicit `BINARY` collation | `Schema/Comparator.php`, `Schema/ComparatorConfig.php`, `Platforms/SQLite/Comparator.php` | SQLite ComparatorTest with the inherited AbstractComparatorTestCase tests (42 of 43; the other upstream does not run on SQLite; `tests/platforms_sqlite_comparator_test.rs`) |
| `MySQLPlatform`, `MySQL80Platform`, `MySQL84Platform` and `AbstractMySQLPlatform`: backtick quoting, expressions (`LOCATE`, `CONCAT()`, date arithmetic and difference, `RLIKE`), limit/offset (`LIMIT 18446744073709551615` without a limit), the integer, float, decimal (`UNSIGNED`), text and blob tiers, `ENUM()`, `JSON`, `CHAR(36)` GUIDs, column charset and collation, `CREATE TABLE` with inline indexes and the table options (`ENGINE`, charset, collation, `AUTO_INCREMENT`, `COMMENT`, row format, partitions), `ALTER TABLE` (`CHANGE`, `ADD`/`DROP PRIMARY KEY`, foreign keys dropped and re-created around renamed indexes), `RENAME INDEX`, `DROP INDEX ... ON`, `SET SESSION TRANSACTION ISOLATION LEVEL`, no `TEXT`/`BLOB` defaults, the `MySQL`, `MySQL80` and `MySQL84` keywords; the `MySQL` comparator (charset and collation normalized against the table's) with its `DefaultTableOptions` and metadata provider interfaces; the caching charset and collation metadata providers (one lookup per name, `null` answers cached too) | `Platforms/{AbstractMySQLPlatform,MySQLPlatform,MySQL80Platform,MySQL84Platform}.php`, `Platforms/Keywords/{MySQL,MySQL80,MySQL84}Keywords.php`, `Platforms/MySQL/{Comparator,DefaultTableOptions,CharsetMetadataProvider,CollationMetadataProvider}.php`, `Platforms/MySQL/CharsetMetadataProvider/CachingCharsetMetadataProvider.php`, `Platforms/MySQL/CollationMetadataProvider/CachingCollationMetadataProvider.php` | MySQLPlatformTest with the inherited AbstractMySQLPlatformTestCase and AbstractPlatformTestCase tests (95 of 97; the other 2 upstream does not run on MySQL), MySQL84PlatformTest (96 of 98, likewise), MySQL ComparatorTest with the inherited AbstractComparatorTestCase tests (42; `tests/platforms_mysql*_test.rs`), Schema/Platforms/MySQLSchemaTest (2), MySQLInheritCharsetTest (1 of 2; `tests/schema_platforms_mysql_schema_test.rs`, `tests/schema_mysql_inherit_charset_test.rs`), CachingCollationMetadataProviderTest (1) |
| `MariaDBPlatform`, `MariaDB1052Platform`, `MariaDB1060Platform`, `MariaDB1010Platform`, `MariaDB110700Platform`, `MariaDB120300Platform`: the `AbstractMySQLPlatform` SQL; no collation or charset on a `JSON` column; foreign keys dropped and re-created around a renamed index, which is dropped and created (`RENAME INDEX` from 10.5); no SKIP LOCKED before 10.6; the `MariaDB`, `MariaDB117` and `MariaDB123` keywords | `Platforms/{MariaDB,MariaDB1052,MariaDB1060,MariaDB1010,MariaDB110700,MariaDB120300}Platform.php`, `Platforms/Keywords/{MariaDB,MariaDB117,MariaDB123}Keywords.php` | MariaDBPlatformTest with the inherited AbstractMySQLPlatformTestCase and AbstractPlatformTestCase tests (92 of 95; the other 3 upstream does not run on MariaDB), MariaDB1052PlatformTest (92 of 95, likewise), MariaDB110700PlatformTest (93 of 96), MariaDB120300PlatformTest (94 of 97), MariaDBJsonComparatorTest (1, 6 rows; `tests/platforms_mariadb*_test.rs`, `tests/platforms_mysql_mariadb_json_comparator_test.rs`) |
| `SQLServerPlatform`: `[...]` quoting, expressions (`CHARINDEX`, `DATEADD`/`DATEDIFF`, `SUBSTRING`, `+` concatenation, `LEN`), `TOP`/`OFFSET ... FETCH` limit queries with the `ORDER BY (SELECT 0)` fallback, `NVARCHAR`/`VARBINARY`, `UNIQUEIDENTIFIER`, `DATETIME2`/`DATETIMEOFFSET`, booleans as `BIT` integers, `CREATE TABLE` with named default constraints and column comments as extended properties (`sp_addextendedproperty`, `sp_updateextendedproperty`, `sp_dropextendedproperty`), `ALTER TABLE` (`ALTER COLUMN`, default constraints dropped and re-added, `sp_rename` for columns, indexes and tables), filtered unique indexes, the lock hints and `WITH (UPDLOCK, ROWLOCK[, READPAST])` select builder, `N'...'` literals, savepoints, the `SQLServer` keywords; the SQL Server comparator that drops a column collation equal to the database's | `Platforms/SQLServerPlatform.php`, `Platforms/Keywords/SQLServerKeywords.php`, `Platforms/SQLServer/Comparator.php`, `Platforms/SQLServer/SQL/Builder/SQLServerSelectSQLBuilder.php` | SQLServerPlatformTest with the inherited AbstractPlatformTestCase tests (122 of 123; the other upstream does not run on SQL Server), SQL Server ComparatorTest with the inherited AbstractComparatorTestCase tests (42; `tests/platforms_sqlserver_*_test.rs`) |
| `DB2Platform`: `LOCATE`, `SUBSTR`, `LENGTH(..., CODEUNITS32)`, `BITAND`/`BITOR`, date arithmetic (`+ n DAY`, weeks and quarters multiplied), `DAYS() - DAYS()`, `GENERATED BY DEFAULT AS IDENTITY`, `BLOB(1M)`/`CLOB(1M)`, `... FOR BIT DATA` binaries, `TIMESTAMP(0)` with the deprecated `version` option, `CREATE TABLE` with separate index statements (no inline index declarations), `ALTER TABLE` (`ADD COLUMN ... WITH DEFAULT`, `ALTER COLUMN ... SET DATA TYPE`/`SET NOT NULL`/`SET DEFAULT`/`DROP DEFAULT`, `RENAME COLUMN`, the `REORG TABLE` call), `RENAME TABLE`, `RENAME INDEX`, `TRUNCATE ... IMMEDIATE`, `DECLARE GLOBAL TEMPORARY TABLE` and `SESSION.` names, `OFFSET ... ROWS FETCH NEXT ... ROWS ONLY`, `WITH RR USE AND KEEP UPDATE LOCKS`, no savepoints, the `DB2` keywords | `Platforms/DB2Platform.php`, `Platforms/Keywords/DB2Keywords.php` | DB2PlatformTest with the inherited AbstractPlatformTestCase tests (82 of 83; the other upstream does not run on DB2; `tests/platforms_db2_platform_test.rs`) |
| `PostgreSQLPlatform`: `SIMILAR TO`, `||` concatenation, `SUBSTRING(... FROM ... FOR ...)`, boolean literals and conversions (`setUseBooleanTrueFalseStrings()`, the `UnexpectedValueException` for an unknown literal), identity columns (`GENERATED BY DEFAULT AS IDENTITY`), `BYTEA`, `UUID`, `JSON`/`JSONB`, `CREATE [UNLOGGED] TABLE` with separate index, unique constraint, foreign key and comment statements, `ALTER TABLE` (`TYPE`, collations, defaults, `NOT NULL`, identity, comments, `DROP CONSTRAINT`), `ALTER INDEX ... RENAME TO`, `DROP INDEX` of a primary key, sequences (`CACHE`, `DROP SEQUENCE ... CASCADE`, `NEXTVAL()`), `getListSequencesSQL()`, `CREATE SCHEMA`, the `PostgreSQL` keywords | `Platforms/PostgreSQLPlatform.php`, `Platforms/Keywords/PostgreSQLKeywords.php` | PostgreSQLPlatformTest with the inherited AbstractPlatformTestCase tests (109 of 110; the other upstream does not run on PostgreSQL; `tests/platforms_postgresql_platform_test.rs`) |
| `OraclePlatform`: `SUBSTR`, `INSTR`, date arithmetic (`ADD_MONTHS`, day fractions), `BITAND()`, `NUMBER(...)`, `VARCHAR2`/`RAW` with a required length, `CLOB`/`BLOB`, `CREATE TABLE` with the autoincrement block, sequence and trigger, `getDropAutoincrementSql()`, `ALTER TABLE` (`ADD (...)`, `MODIFY (...)`, `DROP (...)`), `ALTER INDEX ... RENAME TO`, sequences (`NOCACHE`/`CACHE`), the referential actions (`RESTRICT` and `NO ACTION` left out), `CREATE USER`/`DROP USER ... CASCADE`, `OFFSET ... ROWS FETCH NEXT ... ROWS ONLY`, `TRUNCATE TABLE`, the `Oracle` keywords | `Platforms/OraclePlatform.php`, `Platforms/Keywords/OracleKeywords.php` | OraclePlatformTest with the inherited AbstractPlatformTestCase tests (88 of 89; the other upstream does not run on Oracle; `tests/platforms_oracle_platform_test.rs`) |
| `Schema` (tables and sequences by case-insensitive, namespace-resolved name, implicit and deprecated explicit namespaces, the deprecated `createTable()`/`renameTable()`/`dropTable()`/`createSequence()`/`dropSequence()`, the qualified/unqualified name deprecations, deep `clone`), `SchemaEditor` (default namespace, add/modify/rename/drop of tables and sequences, uniform qualification), `Sequence` (`isAutoIncrementsFor()`), `View`, their editors, `SchemaDiff`, and the `InvalidSchemaModification`, `ImproperlyQualifiedName`, `InvalidSequenceDefinition`, `InvalidViewDefinition`, `NamespaceAlreadyExists`, `Sequence`/`Table` `AlreadyExists`/`DoesNotExist` messages | `Schema/{Schema,SchemaEditor,SchemaDiff,Sequence,SequenceEditor,View,ViewEditor}.php`, `Schema/Exception/*.php` | SchemaTest (28), SchemaEditorTest (28), SequenceTest (5), SequenceEditorTest (5), ViewTest (3), ViewEditorTest (5) |
| Schema managers on `SQLite`: `Connection::createSchemaManager()` through `DefaultSchemaManagerFactory`, `AbstractSchemaManager` (the deprecated `list*()`/`introspectTable()` and the `introspect*()` methods, `tablesExist()`, the schema assets filter, `introspectSchema()`, `createSchemaConfig()`, the `create*()`/`drop*()`/`alter*()`/`renameTable()` methods, `migrateSchema()`), `SQLiteSchemaManager` (PHP's SQL and regular expressions for collations, comments, `WITHOUT ROWID`, foreign key details and the shorthand foreign key form), `SQLiteMetadataProvider`, `IntrospectingSchemaProvider` with its processors, the `Metadata` rows, `Schema::toSql()`/`toDropSql()` with the `Create`/`DropSchemaObjectsSQLBuilder`, `getAlterSchemaSQL()`, the `UnsupportedName`/`UnsupportedSchema` messages | `Schema/{AbstractSchemaManager,SQLiteSchemaManager,SchemaManagerFactory,DefaultSchemaManagerFactory}.php`, `Platforms/SQLite/SQLiteMetadataProvider*.php`, `Schema/Introspection/**`, `Schema/Metadata/**`, `SQL/Builder/{Create,Drop}SchemaObjectsSQLBuilder.php`, `Schema/Exception/Unsupported{Name,Schema}.php` | Functional SQLiteSchemaManagerTest with the inherited SchemaManagerFunctionalTestCase tests (54 of 65; the other 11 upstream does not run on SQLite; `tests/functional_schema_sqlite_schema_manager_test.rs`), functional SchemaManagerTest (15 of 22; the other 7 upstream skips on SQLite), IntrospectingSchemaProviderTest (2), the two metadata row tests; functional AlterTableTest (3 of 8), ColumnCommentTest (3), ColumnRenameTest (2), CustomIntrospectionTest (1), DefaultValueTest (2) and ForeignKeyConstraintTest (4 of 5, without the `RESTRICT` cases), the rest upstream does not run on SQLite (`tests/functional_schema_*_test.rs`) |
| On `pdo_sqlite`: the comparator against introspected tables (default values, float defaults converging after `alterTable()`, explicit column renames, collation changes), foreign key violations raised as `ForeignKeyConstraintViolationException` (insert, update, delete, truncate, in transactions and savepoints), and string and binary columns round-tripped through their types | `Schema/Comparator.php`, `Platforms/SQLite/Comparator.php`, `Schema/SQLiteSchemaManager.php`, `Driver/API/SQLite/ExceptionConverter.php`, `Types/{String,Binary}Type.php` | Functional Schema/ComparatorTest (3), Schema/SQLite/ComparatorTest (1), ForeignKeyExceptionTest (4), ForeignKeyConstraintViolationsTest (3 of 8; the other 5 upstream does not run on SQLite), Platform/ColumnTest/SQLite (8). The `EnableForeignKeys` middleware of upstream's test setup is not ported: the tests run its `PRAGMA foreign_keys=ON` |
| On `pdo_sqlite`: `modifyLimitQuery()` results, `escapeStringForLike()` in a `LIKE ... ESCAPE`, temporary tables inside transactions, binary parameters and `ArrayParameterType::BINARY` lists, the round trip of values through the mapping types (boolean, integer, bigint, float, string, text, json, json_object, jsonb_object, date and time, decimal, number, guid, ascii_string, binary, enum), unique constraint violations as `UniqueConstraintViolationException`, result metadata (`InvalidColumnIndex`, also after `free()`), the introspection of unsigned integers | `Platforms/SQLitePlatform.php`, `Driver/PDO/Result.php`, `Types/*.php`, `Driver/API/SQLite/ExceptionConverter.php`, `Schema/SQLiteSchemaManager.php` | Functional ModifyLimitQueryTest (9), LikeWildcardsEscapingTest (1), TemporaryTableTest (2), BinaryDataAccessTest (12), TypeConversionTest (8 of 9; not the named time zones), ResultMetadataTest (4 of 5), UniqueConstraintViolationsTest (3 of 8), BlobTest (2 of 8; not the streams), Types/{AsciiString,Binary,Decimal,Guid,Json,JsonObject,Jsonb,JsonbObject}Test, Types/BigIntTypeTest (1 of 2), Types/NumberTest (2), Types/EnumTypeTest (2 of 4), Ticket/DBAL752Test; the rest upstream does not run on SQLite |

Vectors: `scripts/record.sh dbal` runs `oracle/recorders/doctrine_dbal.php`
and writes `tests/data/php.json`: the upstream data providers, the
tokens that the real parser gives for generated statements, and
`QueryBuilder` call sequences with each call's result, the final SQL or
exception, and the parameters, and PHP's answers for generated date
formats, date texts and interval durations, and `SQLitePlatform`'s
outputs on generated expressions, tables and table diffs (`sqlite_platform`).
Some date rows depend on the clock (`now`, an empty text), so a new
recording changes them.

`ConnectionStandIn` is the part of `Connection.php` the query builder
uses (`getDatabasePlatform()`, `quote()`, `createExpressionBuilder()`,
`createQueryBuilder()`); `Connection` implements it, and a test can too.

Connection vectors: `scripts/record.sh dbal-connection` runs
`oracle/recorders/doctrine_dbal_connection.php` and writes
`tests/data/connection.json`: the providers of DsnParserTest,
DriverManagerTest, NamedParametersTest and DataAccessTest; the tables of
the functional tests as `SQLitePlatform` creates them; the SQL that
`SQLitePlatform` builds for those tests; and `pdo_sqlite`'s answers for
bindings, errors, `lastInsertId()` and connect parameters
(`tests/pdo_sqlite_vectors.rs`). `src/platforms/sqlite_platform_stand_in.rs`
holds the place of `SQLitePlatform.php` until the platform slice is
merged.

## Differences

PHP's parser is one PCRE pattern. PCRE stops at its limits where these
matchers, which do not backtrack, go on; PHP then throws
`SQL\Parser\Exception\RegularExpressionError`, which is not ported. Found
with the oracle (PHP 8.4.27, default `pcre.backtrack_limit` and JIT):

- An unterminated `/*` comment followed by about 20 or more bytes without
  `*`: "Backtrack limit exhausted" (the comment pattern backtracks
  exponentially). The Rust parser reads `/` as a single byte and goes on,
  as PHP does for shorter tails; the database then refuses the SQL.
- A MySQL-mode string literal of about 6,200 bytes or more, a bracket
  identifier of about 25,000 bytes, or an unterminated comment with many
  `*`: "JIT stack limit exhausted". The Rust parser accepts them.

`tests/parser_vectors.rs` checks that each statement PHP refuses is one of
these cases. The owner decided not to copy these limits (PLAN.md, decision 21).

- A list parameter given as an object is refused with the `TypeError`
  message that `count()` gives for a non-countable value. For a
  `\Countable` object PHP's message names `appendTypedParameter()` and a
  file path instead.

### Query

- QueryBuilderTest `testExecuteQueryWithResultCaching` mocks
  `Connection::executeQuery()` to check that it gets the profile; here
  the builder runs on a connection whose configuration has a result
  cache, and the second run reads the cached rows.
- QueryBuilderTest `testExecuteStatement` stays pending: it
  passes named parameters to `?` placeholders, which only a mocked
  `Connection::executeStatement()` accepts (a real one throws
  `MissingPositionalParameter`), and `Connection` cannot be mocked here.
- `(string) $qb` is `get_sql()`, which returns the exceptions that PHP's
  `__toString()` throws; there is no `Display`.
- A `QueryBuilder` given to `with()`, `union()` or `addUnion()` is moved
  into the query (and shared by its clones, as PHP shares the object). PHP
  keeps a reference, so changing the sub-builder afterwards changes the
  outer query; in Rust it cannot be changed once given.
- `clone` gives `stdClass`, `ArrayObject` and recorded-class (generic)
  parameters a new identity, as `__clone()` does. Other objects
  (closures, enum cases, custom `PhpObject`s) are shared: they have no
  general copy here, and PHP refuses to clone an enum case (`Error`),
  which `Clone` cannot report.
- An unknown join alias that is a canonical decimal integer (`'9'`)
  becomes an int array key in PHP, and `UnknownAlias::new()` refuses it
  with a `TypeError` naming the file and line. The port throws the
  `UnknownAlias` that PHP means; `tests/query_builder_vectors.rs` checks
  that this is the only difference in the recorded sequences.
- SQL fragments are Rust strings (UTF-8); PHP accepts any bytes.
  Parameter values and keys keep PHP's types (`Value`, `Key`).
- PHP's variadic arguments are a first argument and a list (`where($p,
  [])`, `addSelect('a', &[])`); PHP's default arguments are explicit:
  `DEFAULT_PARAMETER_TYPE` (`ParameterType::STRING`),
  `ConflictResolutionMode::default()` (`ORDINARY`), `UnionType::default()`
  (`DISTINCT`), `distinct(true)`.
- `modifyLimitQuery()` is final in PHP; a Rust trait method cannot be
  final, so platforms must override `do_modify_limit_query()` only.
- PHP's `From` class is `FromTable`, so that it does not shadow Rust's
  `From` trait.

### Types

- No resources: a stream given to `BinaryType`, `BlobType`, `JsonType` or
  `JsonObjectType` cannot be written; `BlobType` refuses a string
  (`ConversionError::Unsupported`) where PHP opens a `php://temp` stream.
  The upstream tests that read a stream (BinaryTest, JsonTest,
  JsonObjectTest `testJsonResourceConvertsToPHPValue`, a `tmpfile()` row of
  ConversionExceptionTest) stay pending.
- No recursive values: `json_encode()`'s recursion error cannot happen, so
  JsonTest and JsonObjectTest `testSerializationFailure` stay pending.
- No IANA time zones: DateTest `testDateRestsSummerTimeAffection`
  (Europe/Berlin) stays pending; the UTC date-time tests that use
  America/New_York use the fixed offset -05:00, which gives the same
  answers for the January dates they convert.
- Date and interval texts outside what php-value reads (relative words,
  `R` recurrences, `/` periods, texts of only blanks) are refused
  (`ConversionError::Unsupported`) instead of answered.
- `TypeError` messages leave out PHP's `called in <file> on line <n>`
  suffix.
- `getEnumDeclarationSQL()` refuses a `length` that is not an int and values
  that are not UTF-8 (`Exception` class `Unsupported`), where PHP casts;
  `getDecimalTypeDeclarationSQL()` likewise refuses an array or object
  precision or scale.
- `TypeAlreadyRegistered` and `TypeNotRegistered` name the instance by a
  hash of its address, not by PHP's `spl_object_hash()`.
- `BigIntType` with an out-of-range float throws the `AssertionError`
  PHP throws with assertions on (as under PHPUnit); with assertions off
  PHP returns the value.
- `BcMath\Number` is not in the oracle image; its parsing and comparison
  were checked on PHP 8.4.27 with the bcmath extension and are pinned in
  php-value's unit tests.

### Connection

- The API is async (PLAN.md, decision 22): the methods that may reach the
  database are `async fn`; a driver result is read at execute time, so
  `Result`'s `fetch*()` and `iterate*()` are synchronous. One connection
  serves one task at a time, as one PDO handle does.
- `pdo_sqlite` reads only the first statement given to `prepare()` or
  `query()`; sqlx runs them all. The recorded vectors skip these inputs.
- A string parameter that is not UTF-8 is refused (sqlx binds text as
  Rust strings); PDO binds the bytes as TEXT. Bind it as `BINARY` or
  `LARGE_OBJECT`.
- `driverOptions` (PDO attributes) are ignored; `persistent` is not
  supported (upstream skips its test on `SQLite`).
- A column name that is not UTF-8 is read lossily (sqlx gives column
  names as Rust strings).
- `getColumnName()` past the last column: PDO answers `100 another row
  available` when nothing was fetched yet and rows remain; that rule is
  kept, but after every row was fetched the port throws
  `InvalidColumnIndex` where PDO's message can depend on the statement's
  state.
- A `PDOException` is not a separate level of the `previous` chain: the
  driver's exception is the `PDO\Exception` itself.
- `wrapperClass` must be `Doctrine\DBAL\Connection`: Rust has no
  `Connection` subclasses, so any other class is refused with
  `InvalidWrapperClass` (DriverManagerTest `testCustomWrapper` and
  ConnectionTest `testItFailsDuringCommitBeforeTouchingDb` stay pending).
  A custom `driverClass` is registered with `DriverManager::get_connection_with()`
  (a `DriverClass`), and a DSN's driver class must be declared to
  `DsnParser`.
- The drivers of the driver map other than `pdo_sqlite` build a
  connection, but `connect()` refuses with `Unsupported` until they are
  ported.
- Parameter types are a `BTreeMap` (integer keys first), so
  `is_string(key($types))` is true only when every key is a string; PHP
  looks at the first key in insertion order. Mixed keys do not occur in
  DBAL's own calls.
- An `ArrayParameterType` that reaches `getBindingInfo()` (a value that
  was not expanded) throws the `TypeError` of PHP's parameter check, with
  its message but without the `called in <file> on line <n>` suffix.
- `ConnectionStandIn` is synchronous: through it, `quote()` needs a
  connection that is already open (otherwise `Unsupported`), and the
  platform is known without the database only for a driver that does not
  read the server version (the `SQLite` driver does not).
- `executeQuery()`'s optional `$qcp` is a separate method,
  `execute_query_with_profile()`. ConnectionTest's cache test mocks
  `QueryCacheProfile` (a value here): it gives the profile the key
  `cacheKey` and checks that the result is saved under the real key
  made from the parameters without the password.
- Not ported: `Configuration::setSchemaManagerFactory()`
  (`createSchemaManager()` uses the default factory; ConnectionTest's two
  factory tests stay pending, the other also needs the `sqlite3` driver),
  the wrapper's `getNativeConnection()`, `Configuration::setMiddlewares()`,
  the deprecation notices. A `BlobType` stream cannot be read
  (StatementTest `testFetchLongBlob` pending).
- Pending because upstream skips or needs other platforms on `SQLite`:
  functional ConnectionTest's three savepoints-not-supported tests and
  `testPersistentConnection`, TransactionTest's five connection-loss
  tests, WriteTest `testLastInsertIdNotSupported`, ExceptionTest's
  invalid user, password and host tests.

### MySQL, MariaDB, PostgreSQL, Oracle, SQL Server and DB2 platforms

- A method that calls `parent::` calls the default body, which lives in
  `abstract_platform::parent` as a free function the trait default also
  calls. The `AbstractMySQLPlatform` methods are written once and
  expanded into each `MySQL` platform's impl.
- `convertBooleans()`, `convertBooleansToDatabaseValue()` and
  `createWithSQLBuilder()` return a `Result`: `PostgreSQLPlatform` throws
  `UnexpectedValueException` for an unknown boolean literal and
  `MySQLPlatform` throws `NotSupported` for `WITH`.
- The `MySQL` `getAlterTableSQL()` works on a copy of the `TableDiff`
  (PHP changes the caller's diff while it drops and re-creates foreign
  keys around renamed indexes).
- `MariaDBPlatform`'s foreign keys of renamed indexes are the old
  table's less the dropped ones by their key (`array_diff_key()`): the
  comparator's diffs keep the keys it gave them, and a diff built by hand
  has a list's keys. A foreign key is left out when it equals a modified
  one (PHP compares object identity). `instanceof JsonType` is the
  `JsonType` and `JsonbType` classes.
- `SQLServerPlatform::doModifyLimitQuery()` matches its two patterns
  (`/^SELECT\s+DISTINCT/im`, `/[\s]+order\s+by\s/im`) by hand, with
  PCRE's `\s` and multiline `^`.
- Not ported: `SQLServerPlatform::createMetadataProvider()` and
  `createSchemaManager()` (they need a SQL Server connection), and
  `DB2Platform`'s (they need a Db2 connection).
- Not ported: the `MySQL` schema manager and metadata providers that
  query a connection (`Connection*`/`Caching*MetadataProvider`), the
  schema manager helpers of the platforms (`getColumnTypeSQLSnippet()`,
  `fetchTableOptionsByTable()`), and the drivers.
  MySQLInheritCharsetTest `testInheritTableOptionsFromDatabase` stays
  pending, and Platform/ColumnTest on MySQL, PostgreSQL, Oracle, SQL
  Server and Db2 stays pending (it needs a live server).

### SQLite platform and Schema

- The `AbstractPlatform` methods that PHP declares abstract and that this
  slice adds (`getLocateExpression()`, `getDateDiffExpression()`,
  `getDateArithmeticIntervalExpression()`, `getCurrentDatabaseExpression()`,
  `getAlterTableSQL()`, `getListViewsSQL()`,
  `getSetTransactionIsolationSQL()`, `getCommonIntegerTypeDeclarationSQL()`)
  have default bodies that throw `NotSupported` naming the method, and
  `initializeDoctrineTypeMappings()` and `createReservedKeywordsList()`
  default to empty. A platform that does not override them compiles; PHP
  would refuse the class.
- A platform's state (`$doctrineTypeMapping`, `$_keywords`) is a
  `PlatformState` the platform hands out (`platform_state()`). A platform
  that hands out none (the test stubs) has no type mapping, and
  `getReservedKeywordsList()` builds a new list on each call.
- `Deprecation::triggerIfCalledFromOutside()` is raised by the public
  methods only; the crate's own code calls internal variants, which gives
  PHP's result for a caller outside DBAL. The two outside-caller checks of
  overridable methods are not reproduced: `getReservedKeywordsList()`
  (6607) and `supportsColumnLengthIndexes()` (6886) raise nothing.
- The editors (`TableEditor`, `ColumnEditor`, `IndexEditor`,
  `ForeignKeyConstraintEditor`, `UniqueConstraintEditor`,
  `PrimaryKeyConstraintEditor`) are by-value builders. A call PHP throws
  from at once (an invalid name, an existing or missing object) keeps the
  first error, and `create()` returns it; the tests check the exception
  on `create()`.
- `SchemaEditor`, `SequenceEditor` and `ViewEditor` are by-value
  builders too; after a `SchemaEditor` call fails, the later calls change
  nothing (in PHP the exception ends the chain). `modifyTable()` takes a
  function from the table's editor to the changed editor.
- `Schema` owns its tables and sequences: its constructor takes them by
  value (PHP calls `setSchemaConfig()` on the caller's handles, so a
  caller's table changes too), the getters borrow them and
  `createTable()`/`createSequence()` return the stored object mutably.
  `renameTable()` changes the stored table's `_name`, `_namespace` and
  `_quoted` (not its object name), as PHP does. SchemaTest and
  SchemaEditorTest `assertSame()` on tables compare the tables a round trip
  through `edit()` gives; `assertNotSame()` after `clone` compares
  addresses.
- PHP compares objects by identity where this port compares values:
  `TableDiff::unsetAddedIndex()` and `unsetDroppedIndex()` remove every
  equal index, and TableTest
  `testAddingFulfillingExplicitIndexOverridingImplicitForeignKeyConstraintIndexWithSameName`
  stays pending, as its last assertion tells two equal indexes apart by
  identity.
- SQL, identifiers and names are UTF-8 `String`s; a non-UTF-8 PHP string
  given as an option or default is refused (`Unsupported`). A non-string
  given to a `string` parameter (a `bool` column default passed to
  `quoteStringLiteral()`) is the `TypeError` PHP raises under
  `strict_types`, without PHP's `called in` suffix.
- `$options` of `_getCreateTableSQL()` is the `CreateTableOptions` struct
  (the table options plus `primary`, `primary_index`, `indexes`,
  `uniqueConstraints` and `foreignKeys`).
- Not ported: the `SchemaDiff`, `Sequence` and `View` methods of the
  platforms that `SQLite` does not support (`NotSupported`), and
  `SchemaEditor::modifyTableByUnquotedName()` (testMigrateSchema edits the
  table and puts it back).
- `AbstractSchemaManager` is a trait (its protected methods are public
  trait methods) and is not object-safe: a connection hands out the
  concrete `SQLiteSchemaManager`, whose own methods are
  `createComparator()` (`create_comparator_with()` for a config) and
  `migrateSchema()`. The current schema name is not cached (`SQLite` has
  none). The schema assets filter is a `Configuration` closure set
  before the connection is made. A PHP regular expression that fails to
  match (PHP's warning and `false`) is an exception. Unit
  SQLiteSchemaManagerTest `testListTableForeignKeysDefaultDatabasePassing`
  stays pending (it stubs `Connection` and intercepts a protected method).
- AbstractOptionallyNamedObjectTest `testSetAccessToUninitializedName`
  stays pending: it builds a subclass whose constructor skips
  `parent::__construct()`, which `AbstractAsset::new()` cannot do.
- `TransactionIsolationLevel` and `LockMode` are at the crate root, as in
  PHP's namespace.
