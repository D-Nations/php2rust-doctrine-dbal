//! Port of tests/Platforms/MySQLPlatformTest.php with the tests it
//! inherits from tests/Platforms/AbstractMySQLPlatformTestCase.php and
//! tests/Platforms/AbstractPlatformTestCase.php, at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. The test names are
//! PHPUnit's; each calls the shared port of the class that declares it
//! (`tests/platforms_support`, `tests/platforms_mysql_support`) on `Case`.
//!
//! Upstream: doctrine/dbal tests/Platforms/MySQLPlatformTest.php
//! Upstream: doctrine/dbal tests/Platforms/AbstractMySQLPlatformTestCase.php
//! Upstream: doctrine/dbal tests/Platforms/AbstractPlatformTestCase.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;
mod platforms_mysql_support;
mod platforms_support;

use php2rust_doctrine_dbal::platforms::MySQLPlatform;
use platforms_mysql_support::MySQLPlatformTestCase;

/// `MySQLPlatformTest`.
struct Case;

impl MySQLPlatformTestCase for Case {
    type Platform = MySQLPlatform;

    const PROVIDERS: &'static str = "Platforms/MySQLPlatformTest";

    fn create_platform() -> MySQLPlatform {
        MySQLPlatform::new()
    }
}

#[test]
fn testAlterStringToFixedString() {
    platforms_support::testAlterStringToFixedString::<Case>();
}

#[test]
fn testAlterTableChangeQuotedColumn() {
    platforms_support::testAlterTableChangeQuotedColumn::<Case>();
}

#[test]
fn testAlterTableRenameIndex() {
    platforms_support::testAlterTableRenameIndex::<Case>();
}

#[test]
fn testAlterTableRenameIndexInSchema() {
    platforms_support::testAlterTableRenameIndexInSchema::<Case>();
}

#[test]
fn testAsciiSQLDeclaration() {
    platforms_support::testAsciiSQLDeclaration::<Case>();
}

#[test]
fn testBlobTypeDeclarationSQL() {
    platforms_mysql_support::testBlobTypeDeclarationSQL::<Case>();
}

#[test]
fn testCaseInsensitiveDoctrineTypeMappingFromType() {
    platforms_support::testCaseInsensitiveDoctrineTypeMappingFromType::<Case>();
}

#[test]
fn testClobTypeDeclarationSQL() {
    platforms_mysql_support::testClobTypeDeclarationSQL::<Case>();
}

#[test]
fn testCollationOptionIsTakenIntoAccount() {
    platforms_mysql_support::testCollationOptionIsTakenIntoAccount::<Case>();
}

#[test]
fn testColumnCharsetDeclarationSQL() {
    platforms_mysql_support::testColumnCharsetDeclarationSQL::<Case>();
}

#[test]
fn testColumnCollationDeclarationSQL() {
    platforms_mysql_support::testColumnCollationDeclarationSQL::<Case>();
}

#[test]
fn testCreateTableWithFulltextIndex() {
    platforms_mysql_support::testCreateTableWithFulltextIndex::<Case>();
}

#[test]
fn testCreateTableWithSpatialIndex() {
    platforms_mysql_support::testCreateTableWithSpatialIndex::<Case>();
}

#[test]
fn testCreateWithNoColumns() {
    platforms_support::testCreateWithNoColumns::<Case>();
}

#[test]
fn testDoesSupportSavePoints() {
    platforms_mysql_support::testDoesSupportSavePoints::<Case>();
}

#[test]
fn testGenerateMixedCaseTableCreate() {
    platforms_mysql_support::testGenerateMixedCaseTableCreate::<Case>();
}

#[test]
fn testGenerateTableWithMultiColumnUniqueIndex() {
    platforms_support::testGenerateTableWithMultiColumnUniqueIndex::<Case>();
}

#[test]
fn testGeneratesAlterTableRenameIndexUsedByForeignKeySQL() {
    platforms_support::testGeneratesAlterTableRenameIndexUsedByForeignKeySQL::<Case>();
}

#[test]
fn testGeneratesBitAndComparisonExpressionSql() {
    platforms_support::testGeneratesBitAndComparisonExpressionSql::<Case>();
}

#[test]
fn testGeneratesBitOrComparisonExpressionSql() {
    platforms_support::testGeneratesBitOrComparisonExpressionSql::<Case>();
}

#[test]
fn testGeneratesDDLSnippets() {
    platforms_mysql_support::testGeneratesDDLSnippets::<Case>();
}

#[test]
fn testGeneratesDecimalTypeDeclarationSQL() {
    platforms_support::testGeneratesDecimalTypeDeclarationSQL::<Case>();
}

#[test]
fn testGeneratesFloatDeclarationSQL() {
    platforms_support::testGeneratesFloatDeclarationSQL::<Case>();
}

#[test]
fn testGeneratesForeignKeyCreationSql() {
    platforms_support::testGeneratesForeignKeyCreationSql::<Case>();
}

#[test]
fn testGeneratesIndexCreationSql() {
    platforms_support::testGeneratesIndexCreationSql::<Case>();
}

#[test]
fn testGeneratesInlineColumnCommentSQL() {
    platforms_support::testGeneratesInlineColumnCommentSQL::<Case>();
}

#[test]
fn testGeneratesPartialIndexesSqlOnlyWhenSupportingPartialIndexes() {
    platforms_support::testGeneratesPartialIndexesSqlOnlyWhenSupportingPartialIndexes::<Case>();
}

#[test]
fn testGeneratesSmallFloatDeclarationSQL() {
    platforms_support::testGeneratesSmallFloatDeclarationSQL::<Case>();
}

#[test]
fn testGeneratesSqlSnippets() {
    platforms_mysql_support::testGeneratesSqlSnippets::<Case>();
}

#[test]
fn testGeneratesTableCreationSql() {
    platforms_support::testGeneratesTableCreationSql::<Case>();
}

#[test]
fn testGeneratesTransactionsCommands() {
    platforms_mysql_support::testGeneratesTransactionsCommands::<Case>();
}

#[test]
fn testGeneratesTypeDeclarationForIntegers() {
    platforms_mysql_support::testGeneratesTypeDeclarationForIntegers::<Case>();
}

#[test]
fn testGeneratesUniqueIndexCreationSql() {
    platforms_support::testGeneratesUniqueIndexCreationSql::<Case>();
}

#[test]
fn testGetColumnDeclarationListSQLWithLegacyTypeKey() {
    platforms_support::testGetColumnDeclarationListSQLWithLegacyTypeKey::<Case>();
}

#[test]
fn testGetColumnDeclarationSQLWithTypeNameDoesNotTriggerDeprecation() {
    platforms_support::testGetColumnDeclarationSQLWithTypeNameDoesNotTriggerDeprecation::<Case>();
}

#[test]
fn testGetColumnDeclarationSQLWithoutType() {
    platforms_support::testGetColumnDeclarationSQLWithoutType::<Case>();
}

#[test]
fn testGetCommentOnColumnSQL() {
    platforms_support::testGetCommentOnColumnSQL::<Case>();
}

#[test]
fn testGetCommentOnColumnSQLWithQuoteCharacter() {
    platforms_support::testGetCommentOnColumnSQLWithQuoteCharacter::<Case>();
}

#[test]
fn testGetCommentOnColumnSQLWithoutQuoteCharacter() {
    platforms_support::testGetCommentOnColumnSQLWithoutQuoteCharacter::<Case>();
}

#[test]
fn testGetCreateSchemaSQL() {
    platforms_support::testGetCreateSchemaSQL::<Case>();
}

#[test]
fn testGetCreateTableSQLWithColumnCollation() {
    platforms_mysql_support::testGetCreateTableSQLWithColumnCollation::<Case>();
}

#[test]
fn testGetCustomColumnDeclarationSql() {
    platforms_support::testGetCustomColumnDeclarationSql::<Case>();
}

#[test]
fn testGetDateTimeTypeDeclarationSql() {
    platforms_mysql_support::testGetDateTimeTypeDeclarationSql::<Case>();
}

#[test]
fn testGetDecimalTypeDeclarationSQLNoPrecision() {
    platforms_support::testGetDecimalTypeDeclarationSQLNoPrecision::<Case>();
}

#[test]
fn testGetDecimalTypeDeclarationSQLNoScale() {
    platforms_support::testGetDecimalTypeDeclarationSQLNoScale::<Case>();
}

#[test]
fn testGetDefaultValueDeclarationSQL() {
    platforms_support::testGetDefaultValueDeclarationSQL::<Case>();
}

#[test]
fn testGetDefaultValueDeclarationSQLDateTime() {
    platforms_support::testGetDefaultValueDeclarationSQLDateTime::<Case>();
}

#[test]
fn testGetDefaultValueDeclarationSQLForDateType() {
    platforms_support::testGetDefaultValueDeclarationSQLForDateType::<Case>();
}

#[test]
fn testGetDefaultValueDeclarationSQLForIntegerTypes() {
    platforms_support::testGetDefaultValueDeclarationSQLForIntegerTypes::<Case>();
}

#[test]
fn testGetDefaultValueDeclarationSQLWithLegacyTypeKey() {
    platforms_support::testGetDefaultValueDeclarationSQLWithLegacyTypeKey::<Case>();
}

#[test]
fn testGetEnumDeclarationSQL() {
    platforms_support::testGetEnumDeclarationSQL::<Case>();
}

#[test]
fn testGetEnumDeclarationSQLWithInvalidValues() {
    platforms_support::testGetEnumDeclarationSQLWithInvalidValues::<Case>();
}

#[test]
fn testGetEnumDeclarationWithLengthSQL() {
    platforms_support::testGetEnumDeclarationWithLengthSQL::<Case>();
}

#[test]
fn testGetFixedLengthBinaryTypeDeclarationSQLNoLength() {
    platforms_support::testGetFixedLengthBinaryTypeDeclarationSQLNoLength::<Case>();
}

#[test]
fn testGetFixedLengthBinaryTypeDeclarationSQLWithLength() {
    platforms_support::testGetFixedLengthBinaryTypeDeclarationSQLWithLength::<Case>();
}

#[test]
fn testGetFixedLengthStringTypeDeclarationSQLNoLength() {
    platforms_support::testGetFixedLengthStringTypeDeclarationSQLNoLength::<Case>();
}

#[test]
fn testGetFixedLengthStringTypeDeclarationSQLWithLength() {
    platforms_support::testGetFixedLengthStringTypeDeclarationSQLWithLength::<Case>();
}

#[test]
fn testGetInvalidForeignKeyReferentialActionSQL() {
    platforms_support::testGetInvalidForeignKeyReferentialActionSQL::<Case>();
}

#[test]
fn testGetUnknownDoctrineMappingType() {
    platforms_support::testGetUnknownDoctrineMappingType::<Case>();
}

#[test]
fn testGetVariableLengthBinaryTypeDeclarationSQLNoLength() {
    platforms_mysql_support::testGetVariableLengthBinaryTypeDeclarationSQLNoLength::<Case>();
}

#[test]
fn testGetVariableLengthBinaryTypeDeclarationSQLWithLength() {
    platforms_support::testGetVariableLengthBinaryTypeDeclarationSQLWithLength::<Case>();
}

#[test]
fn testGetVariableLengthStringTypeDeclarationSQLNoLength() {
    platforms_mysql_support::testGetVariableLengthStringTypeDeclarationSQLNoLength::<Case>();
}

#[test]
fn testGetVariableLengthStringTypeDeclarationSQLWithLength() {
    platforms_support::testGetVariableLengthStringTypeDeclarationSQLWithLength::<Case>();
}

#[test]
fn testHasCorrectDefaultTransactionIsolationLevel() {
    platforms_mysql_support::testHasCorrectDefaultTransactionIsolationLevel::<Case>();
}

#[test]
fn testIgnoresDifferenceInDefaultValuesForUnsupportedColumnTypes() {
    platforms_mysql_support::testIgnoresDifferenceInDefaultValuesForUnsupportedColumnTypes::<Case>(
    );
}

#[test]
fn testInitializesDoctrineTypeMappings() {
    platforms_mysql_support::testInitializesDoctrineTypeMappings::<Case>();
}

#[test]
fn testInitializesJsonTypeMapping() {
    platforms_mysql_support::testInitializesJsonTypeMapping::<Case>();
}

#[test]
fn testItEscapesStringsForLike() {
    platforms_support::testItEscapesStringsForLike::<Case>();
}

#[test]
fn testKeywordList() {
    platforms_support::testKeywordList::<Case>();
}

#[test]
fn testModifyLimitQuery() {
    platforms_mysql_support::testModifyLimitQuery::<Case>();
}

#[test]
fn testModifyLimitQueryWithEmptyOffset() {
    platforms_mysql_support::testModifyLimitQueryWithEmptyOffset::<Case>();
}

#[test]
fn testModifyLimitQueryWithoutLimit() {
    platforms_mysql_support::testModifyLimitQueryWithoutLimit::<Case>();
}

#[test]
fn testQuoteIdentifier() {
    platforms_mysql_support::testQuoteIdentifier::<Case>();
}

#[test]
fn testQuoteStringLiteral() {
    platforms_support::testQuoteStringLiteral::<Case>();
}

#[test]
fn testQuotedColumnInForeignKeyPropagation() {
    platforms_support::testQuotedColumnInForeignKeyPropagation::<Case>();
}

#[test]
fn testQuotedColumnInIndexPropagation() {
    platforms_support::testQuotedColumnInIndexPropagation::<Case>();
}

#[test]
fn testQuotedColumnInPrimaryKeyPropagation() {
    platforms_support::testQuotedColumnInPrimaryKeyPropagation::<Case>();
}

#[test]
fn testQuotedNameInIndexSQL() {
    platforms_support::testQuotedNameInIndexSQL::<Case>();
}

#[test]
fn testQuotesAlterTableRenameIndex() {
    platforms_support::testQuotesAlterTableRenameIndex::<Case>();
}

#[test]
fn testQuotesAlterTableRenameIndexInSchema() {
    platforms_support::testQuotesAlterTableRenameIndexInSchema::<Case>();
}

#[test]
fn testQuotesDatabaseNameInListViewsSQL() {
    platforms_mysql_support::testQuotesDatabaseNameInListViewsSQL::<Case>();
}

#[test]
fn testQuotesReservedKeywordInIndexDeclarationSQL() {
    platforms_support::testQuotesReservedKeywordInIndexDeclarationSQL::<Case>();
}

#[test]
fn testQuotesReservedKeywordInTruncateTableSQL() {
    platforms_support::testQuotesReservedKeywordInTruncateTableSQL::<Case>();
}

#[test]
fn testQuotesReservedKeywordInUniqueConstraintDeclarationSQL() {
    platforms_support::testQuotesReservedKeywordInUniqueConstraintDeclarationSQL::<Case>();
}

#[test]
fn testRegisterDoctrineMappingType() {
    platforms_support::testRegisterDoctrineMappingType::<Case>();
}

#[test]
fn testRegisterUnknownDoctrineMappingType() {
    platforms_support::testRegisterUnknownDoctrineMappingType::<Case>();
}

#[test]
fn testReturnsForeignKeyReferentialActionSQL() {
    platforms_support::testReturnsForeignKeyReferentialActionSQL::<Case>();
}

#[test]
fn testReturnsGuidTypeDeclarationSQL() {
    platforms_mysql_support::testReturnsGuidTypeDeclarationSQL::<Case>();
}

#[test]
fn testReturnsJsonTypeDeclarationSQL() {
    platforms_mysql_support::testReturnsJsonTypeDeclarationSQL::<Case>();
}

#[test]
fn testReturnsJsonbTypeDeclarationSQL() {
    platforms_support::testReturnsJsonbTypeDeclarationSQL::<Case>();
}

#[test]
fn testSupportsColumnCollation() {
    platforms_mysql_support::testSupportsColumnCollation::<Case>();
}

#[test]
fn testSupportsCommentOnStatement() {
    platforms_support::testSupportsCommentOnStatement::<Case>();
}

#[test]
fn testSupportsIdentityColumns() {
    platforms_mysql_support::testSupportsIdentityColumns::<Case>();
}

#[test]
fn testUniquePrimaryKey() {
    platforms_mysql_support::testUniquePrimaryKey::<Case>();
}

#[test]
fn testZeroOffsetWithoutLimitIsIgnored() {
    platforms_support::testZeroOffsetWithoutLimitIsIgnored::<Case>();
}
