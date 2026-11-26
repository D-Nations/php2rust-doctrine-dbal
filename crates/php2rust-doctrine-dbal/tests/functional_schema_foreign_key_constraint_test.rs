//! Port of tests/Functional/Schema/ForeignKeyConstraintTest.php on
//! `pdo_sqlite` in memory. A data provider's cases run in one test, in
//! order, without the `RESTRICT` case that upstream marks incomplete on
//! `SQLite`. testDeferrabilityIntrospection is marked incomplete on
//! `SQLite` altogether and is not here (parity marks it n/a).
//!
//! Upstream: doctrine/dbal tests/Functional/Schema/ForeignKeyConstraintTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php2rust_doctrine_dbal::Connection;
use php2rust_doctrine_dbal::schema::{
    AbstractSchemaManager, ForeignKeyConstraint, ForeignKeyConstraintEditor, ReferentialAction,
    Table,
};
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{
    assert_foreign_key_constraint_list_equals, column, drop_table_if_exists, foreign_key,
    primary_key, schema_manager, sqlite,
};

#[tokio::test]
async fn testUnnamedForeignKeyConstraint() {
    let connection = sqlite();
    drop_table_if_exists(&connection, "users").await;
    drop_table_if_exists(&connection, "roles").await;
    drop_table_if_exists(&connection, "teams").await;

    let roles = Table::editor()
        .set_unquoted_name("roles", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    let teams = Table::editor()
        .set_unquoted_name("teams", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    let users = Table::editor()
        .set_unquoted_name("users", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("role_id", Types::INTEGER).create().unwrap(),
            column("team_id", Types::INTEGER).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .set_foreign_key_constraints(vec![
            foreign_key(None, &["role_id"], "roles", &["id"]),
            foreign_key(None, &["team_id"], "teams", &["id"]),
        ])
        .create()
        .unwrap();

    let sm = schema_manager(&connection).await;
    sm.create_table(&roles).await.unwrap();
    sm.create_table(&teams).await.unwrap();
    sm.create_table(&users).await.unwrap();

    let table = sm
        .introspect_table_by_unquoted_name("users", None)
        .await
        .unwrap();
    assert_eq!(2, table.get_foreign_keys().len());
}

#[tokio::test]
async fn testColumnIntrospection() {
    let connection = sqlite();
    drop_table_if_exists(&connection, "users").await;
    drop_table_if_exists(&connection, "roles").await;
    drop_table_if_exists(&connection, "teams").await;

    let roles = Table::editor()
        .set_unquoted_name("roles", None)
        .set_columns(vec![
            column("r_id1", Types::INTEGER).create().unwrap(),
            column("r_id2", Types::INTEGER).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["r_id1", "r_id2"]))
        .create()
        .unwrap();

    let teams = Table::editor()
        .set_unquoted_name("teams", None)
        .set_columns(vec![
            column("t_id1", Types::INTEGER).create().unwrap(),
            column("t_id2", Types::INTEGER).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["t_id1", "t_id2"]))
        .create()
        .unwrap();

    let foreign_key_constraints = vec![
        foreign_key(
            Some("fk_roles"),
            &["role_id1", "role_id2"],
            "roles",
            &["r_id1", "r_id2"],
        ),
        foreign_key(
            Some("fk_teams"),
            &["team_id1", "team_id2"],
            "teams",
            &["t_id1", "t_id2"],
        ),
    ];

    let users = Table::editor()
        .set_unquoted_name("users", None)
        .set_columns(vec![
            column("u_id1", Types::INTEGER).create().unwrap(),
            column("u_id2", Types::INTEGER).create().unwrap(),
            column("role_id1", Types::INTEGER).create().unwrap(),
            column("role_id2", Types::INTEGER).create().unwrap(),
            column("team_id1", Types::INTEGER).create().unwrap(),
            column("team_id2", Types::INTEGER).create().unwrap(),
        ])
        .set_foreign_key_constraints(foreign_key_constraints.clone())
        .set_primary_key_constraint(primary_key(&["u_id1", "u_id2"]))
        .create()
        .unwrap();

    let sm = schema_manager(&connection).await;
    sm.create_table(&roles).await.unwrap();
    sm.create_table(&teams).await.unwrap();
    sm.create_table(&users).await.unwrap();

    let table = sm.introspect_table("users").await.unwrap();

    assert_foreign_key_constraint_list_equals(
        &foreign_key_constraints.iter().collect::<Vec<_>>(),
        &table.get_foreign_keys().values().collect::<Vec<_>>(),
    );
}

/// `referentialActionProvider()` without `RESTRICT`, whose introspection
/// upstream marks incomplete on `SQLite`.
const ACTIONS: [ReferentialAction; 4] = [
    ReferentialAction::Cascade,
    ReferentialAction::NoAction,
    ReferentialAction::SetDefault,
    ReferentialAction::SetNull,
];

/// `testReferentialActionIntrospection($setter, $action, $getter)`.
async fn test_referential_action_introspection(
    connection: &Connection,
    setter: impl FnOnce(ForeignKeyConstraintEditor, ReferentialAction) -> ForeignKeyConstraintEditor,
    action: ReferentialAction,
    getter: impl FnOnce(&ForeignKeyConstraint) -> ReferentialAction,
) {
    drop_table_if_exists(connection, "users").await;
    drop_table_if_exists(connection, "roles").await;

    let roles = Table::editor()
        .set_unquoted_name("roles", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    let editor = ForeignKeyConstraint::editor()
        .set_unquoted_referencing_column_names(&["role_id"])
        .set_unquoted_referenced_table_name("roles", None)
        .set_unquoted_referenced_column_names(&["id"]);

    let editor = setter(editor, action);

    let users = Table::editor()
        .set_unquoted_name("users", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("role_id", Types::INTEGER)
                .set_not_null(false)
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .set_foreign_key_constraints(vec![editor.create().unwrap()])
        .create()
        .unwrap();

    let sm = schema_manager(connection).await;
    sm.create_table(&roles).await.unwrap();
    sm.create_table(&users).await.unwrap();

    let constraints = sm.list_table_foreign_keys("users").await.unwrap();
    assert_eq!(1, constraints.len());

    assert_eq!(action, getter(&constraints[0]), "{}", action.value());
}

#[tokio::test]
async fn testOnUpdateIntrospection() {
    for action in ACTIONS {
        let connection = sqlite();
        test_referential_action_introspection(
            &connection,
            ForeignKeyConstraintEditor::set_on_update_action,
            action,
            |constraint| constraint.get_on_update_action().unwrap(),
        )
        .await;
    }
}

#[tokio::test]
async fn testOnDeleteIntrospection() {
    for action in ACTIONS {
        let connection = sqlite();
        test_referential_action_introspection(
            &connection,
            ForeignKeyConstraintEditor::set_on_delete_action,
            action,
            |constraint| constraint.get_on_delete_action().unwrap(),
        )
        .await;
    }
}
