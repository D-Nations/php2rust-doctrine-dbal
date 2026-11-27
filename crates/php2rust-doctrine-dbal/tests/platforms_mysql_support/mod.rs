//! Port of tests/Platforms/AbstractMySQLPlatformTestCase.php and
//! tests/Platforms/MySQLPlatformTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, for `MySQLPlatformTest` and
//! the test that extends it (`MySQL84PlatformTest`), and for
//! `MariaDBPlatformTest` and its subclasses, which extend
//! `AbstractMySQLPlatformTestCase`.
//!
//! A concrete test case implements [`MySQLPlatformTestCase`] (its
//! `createPlatform()`); the expectations of both abstract classes are the
//! blanket [`AbstractPlatformTestCase`] impl, and each test of theirs is a
//! generic function here that a test file calls from a `#[test]` of the
//! same name.
//!
//! Upstream: doctrine/dbal tests/Platforms/AbstractMySQLPlatformTestCase.php tests/Platforms/MySQLPlatformTest.php
#![allow(
    dead_code,
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

use php_value::{PhpArray, Value, php_array};
use php2rust_doctrine_dbal::platforms::AbstractPlatform;
use php2rust_doctrine_dbal::platforms::mysql::{
    CharsetMetadataProvider, CollationMetadataProvider, Comparator, DefaultTableOptions,
};
use php2rust_doctrine_dbal::schema::{ComparatorConfig, Index, IndexType, Table};
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{Exception, TransactionIsolationLevel};

use crate::platforms_support::{
    AbstractPlatformTestCase, assert_invalid_column_declaration, column, pk, sql,
};

/// A test case of a `MySQL` platform: `MySQLPlatformTest` or a subclass.
pub trait MySQLPlatformTestCase {
    type Platform: AbstractPlatform;

    /// The recorded providers' class (`Platforms/MySQLPlatformTest`).
    const PROVIDERS: &'static str;

    /// `createPlatform()`.
    fn create_platform() -> Self::Platform;

    /// The expected SQL of the index renaming tests:
    /// `AbstractMySQLPlatformTestCase`'s unless the test class overrides
    /// them (`MySQLPlatformTest`, `MariaDB1052PlatformTest`:
    /// [`RENAME_INDEX_IN_PLACE`]).
    const RENAME_INDEX_SQL: &'static RenameIndexSql = &DROP_AND_CREATE_INDEX;
}

/// `getAlterTableRenameIndexSQL()`, `getQuotedAlterTableRenameIndexSQL()`,
/// `getAlterTableRenameIndexInSchemaSQL()`,
/// `getQuotedAlterTableRenameIndexInSchemaSQL()` and
/// `getGeneratesAlterTableRenameIndexUsedByForeignKeySQL()`.
pub struct RenameIndexSql {
    pub alter_table_rename_index: &'static [&'static str],
    pub quoted_alter_table_rename_index: &'static [&'static str],
    pub alter_table_rename_index_in_schema: &'static [&'static str],
    pub quoted_alter_table_rename_index_in_schema: &'static [&'static str],
    pub generates_alter_table_rename_index_used_by_foreign_key: &'static [&'static str],
}

/// `AbstractMySQLPlatformTestCase`'s: the index is dropped and created.
pub const DROP_AND_CREATE_INDEX: RenameIndexSql = RenameIndexSql {
    alter_table_rename_index: &[
        "DROP INDEX idx_foo ON mytable",
        "CREATE INDEX idx_bar ON mytable (id)",
    ],
    quoted_alter_table_rename_index: &[
        "DROP INDEX `create` ON `table`",
        "CREATE INDEX `select` ON `table` (id)",
        "DROP INDEX `foo` ON `table`",
        "CREATE INDEX `bar` ON `table` (id)",
    ],
    alter_table_rename_index_in_schema: &[
        "DROP INDEX idx_foo ON myschema.mytable",
        "CREATE INDEX idx_bar ON myschema.mytable (id)",
    ],
    quoted_alter_table_rename_index_in_schema: &[
        "DROP INDEX `create` ON `schema`.`table`",
        "CREATE INDEX `select` ON `schema`.`table` (id)",
        "DROP INDEX `foo` ON `schema`.`table`",
        "CREATE INDEX `bar` ON `schema`.`table` (id)",
    ],
    generates_alter_table_rename_index_used_by_foreign_key: &[
        "ALTER TABLE mytable DROP FOREIGN KEY fk_foo",
        "DROP INDEX idx_foo ON mytable",
        "CREATE INDEX idx_foo_renamed ON mytable (foo)",
        "ALTER TABLE mytable ADD CONSTRAINT fk_foo FOREIGN KEY (foo) REFERENCES foreign_table (id)",
    ],
};

/// `MySQLPlatformTest`'s and `MariaDB1052PlatformTest`'s overrides:
/// `RENAME INDEX`.
pub const RENAME_INDEX_IN_PLACE: RenameIndexSql = RenameIndexSql {
    alter_table_rename_index: &["ALTER TABLE mytable RENAME INDEX idx_foo TO idx_bar"],
    quoted_alter_table_rename_index: &[
        "ALTER TABLE `table` RENAME INDEX `create` TO `select`",
        "ALTER TABLE `table` RENAME INDEX `foo` TO `bar`",
    ],
    alter_table_rename_index_in_schema: &[
        "ALTER TABLE myschema.mytable RENAME INDEX idx_foo TO idx_bar",
    ],
    quoted_alter_table_rename_index_in_schema: &[
        "ALTER TABLE `schema`.`table` RENAME INDEX `create` TO `select`",
        "ALTER TABLE `schema`.`table` RENAME INDEX `foo` TO `bar`",
    ],
    generates_alter_table_rename_index_used_by_foreign_key: &[
        "ALTER TABLE mytable RENAME INDEX idx_foo TO idx_foo_renamed",
    ],
};

impl<C: MySQLPlatformTestCase> AbstractPlatformTestCase for C {
    type Platform = C::Platform;

    const PROVIDERS: &'static str = C::PROVIDERS;

    fn create_platform() -> Self::Platform {
        C::create_platform()
    }

    fn get_generate_table_sql() -> &'static str {
        "CREATE TABLE test (id INT AUTO_INCREMENT NOT NULL, test VARCHAR(255) DEFAULT NULL, \
         PRIMARY KEY (id))"
    }

    fn get_generate_table_with_multi_column_unique_index_sql() -> &'static [&'static str] {
        &[
            "CREATE TABLE test (foo VARCHAR(255) DEFAULT NULL, bar VARCHAR(255) DEFAULT NULL, \
           UNIQUE INDEX UNIQ_D87F7E0C8C73652176FF8CAA (foo, bar))",
        ]
    }

    fn get_generate_index_sql() -> &'static str {
        "CREATE INDEX my_idx ON mytable (user_name, last_login)"
    }

    fn get_generate_unique_index_sql() -> &'static str {
        "CREATE UNIQUE INDEX index_name ON test (test, test2)"
    }

    fn get_generate_foreign_key_sql() -> &'static str {
        "ALTER TABLE test ADD FOREIGN KEY (fk_name_id) REFERENCES other_table (id)"
    }

    fn get_quoted_column_in_primary_key_sql() -> &'static [&'static str] {
        &["CREATE TABLE `quoted` (`create` VARCHAR(255) NOT NULL, PRIMARY KEY (`create`))"]
    }

    fn get_quoted_column_in_index_sql() -> &'static [&'static str] {
        &["CREATE TABLE `quoted` (`create` VARCHAR(255) NOT NULL, \
           INDEX IDX_22660D028FD6E0FB (`create`))"]
    }

    fn get_quoted_name_in_index_sql() -> &'static [&'static str] {
        &["CREATE TABLE test (column1 VARCHAR(255) NOT NULL, INDEX `key` (column1))"]
    }

    fn get_quoted_column_in_foreign_key_sql() -> &'static [&'static str] {
        &[
            "CREATE TABLE `quoted` (`create` VARCHAR(255) NOT NULL, foo VARCHAR(255) NOT NULL, \
             `bar` VARCHAR(255) NOT NULL, INDEX IDX_22660D028FD6E0FB8C7365216D704F76 (`create`, foo, `bar`))",
            "ALTER TABLE `quoted` ADD CONSTRAINT FK_WITH_RESERVED_KEYWORD FOREIGN KEY (`create`, foo, `bar`) \
             REFERENCES `foreign` (`create`, bar, `foo-bar`)",
            "ALTER TABLE `quoted` ADD CONSTRAINT FK_WITH_NON_RESERVED_KEYWORD FOREIGN KEY (`create`, foo, `bar`) \
             REFERENCES foo (`create`, bar, `foo-bar`)",
            "ALTER TABLE `quoted` ADD CONSTRAINT FK_WITH_INTENDED_QUOTATION FOREIGN KEY (`create`, foo, `bar`) \
             REFERENCES `foo-bar` (`create`, bar, `foo-bar`)",
        ]
    }

    fn get_quotes_reserved_keyword_in_unique_constraint_declaration_sql() -> &'static str {
        "CONSTRAINT `select` UNIQUE (foo)"
    }

    fn get_quotes_reserved_keyword_in_truncate_table_sql() -> &'static str {
        "TRUNCATE `select`"
    }

    fn get_quotes_reserved_keyword_in_index_declaration_sql() -> &'static str {
        "INDEX `select` (foo)"
    }

    fn get_alter_table_rename_index_sql() -> &'static [&'static str] {
        C::RENAME_INDEX_SQL.alter_table_rename_index
    }

    fn get_quoted_alter_table_rename_index_sql() -> &'static [&'static str] {
        C::RENAME_INDEX_SQL.quoted_alter_table_rename_index
    }

    fn get_alter_table_rename_index_in_schema_sql() -> &'static [&'static str] {
        C::RENAME_INDEX_SQL.alter_table_rename_index_in_schema
    }

    fn get_quoted_alter_table_rename_index_in_schema_sql() -> &'static [&'static str] {
        C::RENAME_INDEX_SQL.quoted_alter_table_rename_index_in_schema
    }

    fn get_comment_on_column_sql() -> &'static [&'static str] {
        &[
            "COMMENT ON COLUMN foo.bar IS 'comment'",
            "COMMENT ON COLUMN `Foo`.`BAR` IS 'comment'",
            "COMMENT ON COLUMN `select`.`from` IS 'comment'",
        ]
    }

    fn get_alter_string_to_fixed_string_sql() -> &'static [&'static str] {
        &["ALTER TABLE mytable CHANGE name name CHAR(2) NOT NULL"]
    }

    fn get_generates_alter_table_rename_index_used_by_foreign_key_sql() -> &'static [&'static str] {
        C::RENAME_INDEX_SQL.generates_alter_table_rename_index_used_by_foreign_key
    }
}

fn platform<T: MySQLPlatformTestCase>() -> T::Platform {
    T::create_platform()
}

/// `self::createStub(CharsetMetadataProvider::class)`: `null`.
struct CharsetMetadataProviderStub;

impl CharsetMetadataProvider for CharsetMetadataProviderStub {
    fn get_default_charset_collation(&self, _charset: &str) -> Result<Option<String>, Exception> {
        Ok(None)
    }
}

/// `self::createStub(CollationMetadataProvider::class)`: `null`.
struct CollationMetadataProviderStub;

impl CollationMetadataProvider for CollationMetadataProviderStub {
    fn get_collation_charset(&self, _collation: &str) -> Result<Option<String>, Exception> {
        Ok(None)
    }
}

/// `createComparator()`.
fn create_comparator(platform: &dyn AbstractPlatform) -> Comparator<'_> {
    Comparator::new(
        platform,
        Box::new(CharsetMetadataProviderStub),
        Box::new(CollationMetadataProviderStub),
        DefaultTableOptions::new("utf8mb4", "utf8mb4_general_ci"),
        ComparatorConfig::default(),
    )
}

// AbstractMySQLPlatformTestCase's tests.

pub fn testModifyLimitQueryWithoutLimit<T: MySQLPlatformTestCase>() {
    let sql = platform::<T>()
        .modify_limit_query("SELECT n FROM Foo", None, 10)
        .unwrap();
    assert_eq!(
        sql,
        "SELECT n FROM Foo LIMIT 18446744073709551615 OFFSET 10"
    );
}

pub fn testGenerateMixedCaseTableCreate<T: MySQLPlatformTestCase>() {
    let table = Table::editor()
        .set_unquoted_name("Foo", None)
        .set_columns(vec![column("Bar", Types::INTEGER).create().unwrap()])
        .create()
        .unwrap();

    let sql = platform::<T>().get_create_table_sql(&table).unwrap();
    assert_eq!(sql[0], "CREATE TABLE Foo (Bar INT NOT NULL)");
}

pub fn testGeneratesSqlSnippets<T: MySQLPlatformTestCase>() {
    let platform = platform::<T>();
    assert_eq!(platform.get_regexp_expression().unwrap(), "RLIKE");
    assert_eq!(
        platform.get_concat_expression(&["column1", "column2", "column3"]),
        "CONCAT(column1, column2, column3)"
    );
}

pub fn testGeneratesTransactionsCommands<T: MySQLPlatformTestCase>() {
    let platform = platform::<T>();
    for (level, expected) in [
        (
            TransactionIsolationLevel::ReadUncommitted,
            "SET SESSION TRANSACTION ISOLATION LEVEL READ UNCOMMITTED",
        ),
        (
            TransactionIsolationLevel::ReadCommitted,
            "SET SESSION TRANSACTION ISOLATION LEVEL READ COMMITTED",
        ),
        (
            TransactionIsolationLevel::RepeatableRead,
            "SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ",
        ),
        (
            TransactionIsolationLevel::Serializable,
            "SET SESSION TRANSACTION ISOLATION LEVEL SERIALIZABLE",
        ),
    ] {
        assert_eq!(
            platform.get_set_transaction_isolation_sql(level).unwrap(),
            expected
        );
    }
}

pub fn testGeneratesDDLSnippets<T: MySQLPlatformTestCase>() {
    let platform = platform::<T>();
    assert_eq!(platform.get_list_databases_sql().unwrap(), "SHOW DATABASES");
    assert_eq!(
        platform.get_create_database_sql("foobar").unwrap(),
        "CREATE DATABASE foobar"
    );
    assert_eq!(
        platform.get_drop_database_sql("foobar").unwrap(),
        "DROP DATABASE foobar"
    );
    assert_eq!(platform.get_drop_table_sql("foobar"), "DROP TABLE foobar");
}

pub fn testGeneratesTypeDeclarationForIntegers<T: MySQLPlatformTestCase>() {
    let platform = platform::<T>();
    assert_eq!(
        platform
            .get_integer_type_declaration_sql(&PhpArray::new())
            .unwrap(),
        sql("INT")
    );
    assert_eq!(
        platform
            .get_integer_type_declaration_sql(&php_array!["autoincrement" => true])
            .unwrap(),
        sql("INT AUTO_INCREMENT")
    );
    assert_eq!(
        platform
            .get_integer_type_declaration_sql(
                &php_array!["autoincrement" => true, "primary" => true]
            )
            .unwrap(),
        sql("INT AUTO_INCREMENT")
    );
}

pub fn testSupportsIdentityColumns<T: MySQLPlatformTestCase>() {
    assert!(platform::<T>().supports_identity_columns());
}

pub fn testDoesSupportSavePoints<T: MySQLPlatformTestCase>() {
    assert!(platform::<T>().supports_savepoints());
}

pub fn testUniquePrimaryKey<T: MySQLPlatformTestCase>() {
    let platform = platform::<T>();
    let old_table = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![
            column("bar", Types::INTEGER).create().unwrap(),
            column("baz", Types::STRING)
                .set_length(Some(32))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    let key_table = old_table
        .edit()
        .unwrap()
        .add_primary_key_constraint(pk(&["bar"]).unwrap())
        .add_index(
            Index::editor()
                .set_type(IndexType::Unique)
                .set_unquoted_column_names(&["baz"]),
        )
        .create()
        .unwrap();

    let diff = create_comparator(&platform)
        .compare_tables(&old_table, &key_table)
        .unwrap();

    let sql = platform.get_alter_table_sql(&diff).unwrap();

    assert_eq!(
        sql,
        vec![
            "ALTER TABLE foo ADD PRIMARY KEY (bar)",
            "CREATE UNIQUE INDEX UNIQ_8C73652178240498 ON foo (baz)",
        ]
    );
}

pub fn testModifyLimitQuery<T: MySQLPlatformTestCase>() {
    let sql = platform::<T>()
        .modify_limit_query("SELECT * FROM user", Some(10), 0)
        .unwrap();
    assert_eq!(sql, "SELECT * FROM user LIMIT 10");
}

pub fn testModifyLimitQueryWithEmptyOffset<T: MySQLPlatformTestCase>() {
    let sql = platform::<T>()
        .modify_limit_query("SELECT * FROM user", Some(10), 0)
        .unwrap();
    assert_eq!(sql, "SELECT * FROM user LIMIT 10");
}

pub fn testGetDateTimeTypeDeclarationSql<T: MySQLPlatformTestCase>() {
    let platform = platform::<T>();
    assert_eq!(
        platform
            .get_date_time_type_declaration_sql(&php_array!["version" => false])
            .unwrap(),
        sql("DATETIME")
    );
    assert_eq!(
        platform
            .get_date_time_type_declaration_sql(&php_array!["version" => true])
            .unwrap(),
        sql("TIMESTAMP")
    );
    assert_eq!(
        platform
            .get_date_time_type_declaration_sql(&PhpArray::new())
            .unwrap(),
        sql("DATETIME")
    );
}

/// A table with a `MyISAM` `$type` index on one TEXT column.
fn myisam_table(name: &str, column_name: &str, index_name: &str, type_: IndexType) -> Table {
    Table::editor()
        .set_unquoted_name(name, None)
        .set_columns(vec![column(column_name, Types::TEXT).create().unwrap()])
        .set_indexes(vec![
            Index::editor()
                .set_unquoted_name(index_name)
                .set_type(type_)
                .set_unquoted_column_names(&[column_name])
                .create()
                .unwrap(),
        ])
        .set_options(php_array!["engine" => "MyISAM"])
        .create()
        .unwrap()
}

pub fn testCreateTableWithFulltextIndex<T: MySQLPlatformTestCase>() {
    let table = myisam_table(
        "fulltext_table",
        "text",
        "fulltext_text",
        IndexType::Fulltext,
    );

    let sql = platform::<T>().get_create_table_sql(&table).unwrap();
    assert_eq!(
        sql,
        vec![
            "CREATE TABLE fulltext_table (text LONGTEXT NOT NULL, \
             FULLTEXT INDEX fulltext_text (text)) ENGINE = MyISAM"
        ]
    );
}

pub fn testCreateTableWithSpatialIndex<T: MySQLPlatformTestCase>() {
    // This should be a point type
    let table = myisam_table("spatial_table", "point", "spatial_text", IndexType::Spatial);

    let sql = platform::<T>().get_create_table_sql(&table).unwrap();
    assert_eq!(
        sql,
        vec![
            "CREATE TABLE spatial_table (point LONGTEXT NOT NULL, SPATIAL INDEX spatial_text (point)) \
             ENGINE = MyISAM"
        ]
    );
}

fn length(length: i64) -> PhpArray {
    php_array!["length" => length]
}

pub fn testClobTypeDeclarationSQL<T: MySQLPlatformTestCase>() {
    let platform = platform::<T>();
    for (column, expected) in [
        (length(1), "TINYTEXT"),
        (length(255), "TINYTEXT"),
        (length(256), "TEXT"),
        (length(65535), "TEXT"),
        (length(65536), "MEDIUMTEXT"),
        (length(16_777_215), "MEDIUMTEXT"),
        (length(16_777_216), "LONGTEXT"),
        (PhpArray::new(), "LONGTEXT"),
    ] {
        assert_eq!(
            platform.get_clob_type_declaration_sql(&column).unwrap(),
            sql(expected)
        );
    }
}

pub fn testBlobTypeDeclarationSQL<T: MySQLPlatformTestCase>() {
    let platform = platform::<T>();
    for (column, expected) in [
        (length(1), "TINYBLOB"),
        (length(255), "TINYBLOB"),
        (length(256), "BLOB"),
        (length(65535), "BLOB"),
        (length(65536), "MEDIUMBLOB"),
        (length(16_777_215), "MEDIUMBLOB"),
        (length(16_777_216), "LONGBLOB"),
        (PhpArray::new(), "LONGBLOB"),
    ] {
        assert_eq!(
            platform.get_blob_type_declaration_sql(&column).unwrap(),
            sql(expected)
        );
    }
}

pub fn testInitializesDoctrineTypeMappings<T: MySQLPlatformTestCase>() {
    let platform = platform::<T>();
    assert!(platform.has_doctrine_type_mapping_for("binary").unwrap());
    assert_eq!(
        platform.get_doctrine_type_mapping("binary").unwrap(),
        Types::BINARY
    );

    assert!(platform.has_doctrine_type_mapping_for("varbinary").unwrap());
    assert_eq!(
        platform.get_doctrine_type_mapping("varbinary").unwrap(),
        Types::BINARY
    );
}

pub fn testGetVariableLengthStringTypeDeclarationSQLNoLength<T: MySQLPlatformTestCase>() {
    assert_invalid_column_declaration(
        platform::<T>().get_string_type_declaration_sql(&php_array!["name" => "email"]),
    );
}

pub fn testGetVariableLengthBinaryTypeDeclarationSQLNoLength<T: MySQLPlatformTestCase>() {
    assert_invalid_column_declaration(
        platform::<T>().get_binary_type_declaration_sql(&php_array!["name" => "attachment"]),
    );
}

pub fn testIgnoresDifferenceInDefaultValuesForUnsupportedColumnTypes<T: MySQLPlatformTestCase>() {
    let platform = platform::<T>();
    let def = || Value::from("def");
    let table = Table::editor()
        .set_unquoted_name("text_blob_default_value", None)
        .set_columns(vec![
            column("def_text", Types::TEXT)
                .set_default_value(def())
                .create()
                .unwrap(),
            column("def_text_null", Types::TEXT)
                .set_not_null(false)
                .set_default_value(def())
                .create()
                .unwrap(),
            column("def_blob", Types::BLOB)
                .set_default_value(def())
                .create()
                .unwrap(),
            column("def_blob_null", Types::BLOB)
                .set_not_null(false)
                .set_default_value(def())
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    assert_eq!(
        platform.get_create_table_sql(&table).unwrap(),
        vec![
            "CREATE TABLE text_blob_default_value (def_text LONGTEXT NOT NULL, \
             def_text_null LONGTEXT DEFAULT NULL, \
             def_blob LONGBLOB NOT NULL, \
             def_blob_null LONGBLOB DEFAULT NULL)"
        ]
    );

    let diff_table = table
        .edit()
        .unwrap()
        .modify_column_by_unquoted_name("def_text", |editor| editor.set_default_value(Value::Null))
        .modify_column_by_unquoted_name("def_text_null", |editor| {
            editor.set_default_value(Value::Null)
        })
        .modify_column_by_unquoted_name("def_blob", |editor| editor.set_default_value(Value::Null))
        .modify_column_by_unquoted_name("def_blob_null", |editor| {
            editor.set_default_value(Value::Null)
        })
        .create()
        .unwrap();

    let comparator = create_comparator(&platform);

    assert!(
        comparator
            .compare_tables(&table, &diff_table)
            .unwrap()
            .is_empty()
    );
    assert!(
        comparator
            .compare_tables(&table, &diff_table)
            .unwrap()
            .is_empty()
    );
}

pub fn testReturnsGuidTypeDeclarationSQL<T: MySQLPlatformTestCase>() {
    assert_eq!(
        platform::<T>()
            .get_guid_type_declaration_sql(&PhpArray::new())
            .unwrap(),
        sql("CHAR(36)")
    );
}

pub fn testQuotesDatabaseNameInListViewsSQL<T: MySQLPlatformTestCase>() {
    let sql = platform::<T>().get_list_views_sql("Foo'Bar\\").unwrap();
    assert!(
        sql.to_lowercase()
            .contains(&"'Foo''Bar\\\\'".to_lowercase()),
        "{sql}"
    );
}

pub fn testColumnCharsetDeclarationSQL<T: MySQLPlatformTestCase>() {
    assert_eq!(
        platform::<T>().get_column_charset_declaration_sql("ascii"),
        "CHARACTER SET ascii"
    );
}

pub fn testSupportsColumnCollation<T: MySQLPlatformTestCase>() {
    assert!(platform::<T>().supports_column_collation());
}

pub fn testColumnCollationDeclarationSQL<T: MySQLPlatformTestCase>() {
    assert_eq!(
        platform::<T>().get_column_collation_declaration_sql("ascii_general_ci"),
        "COLLATE `ascii_general_ci`"
    );
}

pub fn testGetCreateTableSQLWithColumnCollation<T: MySQLPlatformTestCase>() {
    let table = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![
            column("no_collation", Types::STRING)
                .set_length(Some(255))
                .create()
                .unwrap(),
            column("column_collation", Types::STRING)
                .set_length(Some(255))
                .set_collation(Some("ascii_general_ci"))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    assert_eq!(
        platform::<T>().get_create_table_sql(&table).unwrap(),
        vec![
            "CREATE TABLE foo (no_collation VARCHAR(255) NOT NULL, \
             column_collation VARCHAR(255) NOT NULL COLLATE `ascii_general_ci`)"
        ]
    );
}

pub fn testQuoteIdentifier<T: MySQLPlatformTestCase>() {
    assert_eq!(
        platform::<T>().quote_identifier("test.test"),
        "`test`.`test`"
    );
}

// MySQLPlatformTest's tests.

pub fn testReturnsJsonTypeDeclarationSQL<T: MySQLPlatformTestCase>() {
    assert_eq!(
        platform::<T>()
            .get_json_type_declaration_sql(&PhpArray::new())
            .unwrap(),
        sql("JSON")
    );
}

pub fn testInitializesJsonTypeMapping<T: MySQLPlatformTestCase>() {
    let platform = platform::<T>();
    assert!(platform.has_doctrine_type_mapping_for("json").unwrap());
    assert_eq!(
        platform.get_doctrine_type_mapping("json").unwrap(),
        Types::JSON
    );
}

pub fn testHasCorrectDefaultTransactionIsolationLevel<T: MySQLPlatformTestCase>() {
    assert_eq!(
        platform::<T>().get_default_transaction_isolation_level(),
        TransactionIsolationLevel::RepeatableRead
    );
}

pub fn testCollationOptionIsTakenIntoAccount<T: MySQLPlatformTestCase>() {
    let table = Table::editor()
        .set_unquoted_name("quotations", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_options(php_array!["collation" => "my_collation"])
        .create()
        .unwrap();

    assert!(platform::<T>().get_create_table_sql(&table).unwrap()[0].contains("my_collation"));
}
