//! Helpers of the tests that need a live `PostgreSQL` server: what
//! tests/TestUtil.php gives them when `phpunit.xml` names a `pdo_pgsql`
//! database. The server URL is read from `DBAL_TEST_PGSQL_URL` (for
//! example `postgres://postgres:postgres@127.0.0.1:5432/doctrine_tests`);
//! without it each test returns early and passes, with a note on stderr.
//! The database of the URL is created when it is missing. The tests hold
//! one lock while they run, as PHPUnit runs them one after the other on
//! the same database.
//!
//! Upstream: none
#![allow(dead_code)]

use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::tools::DsnParser;
use php2rust_doctrine_dbal::{Configuration, Connection, DriverManager, ParameterTypes};
use tokio::sync::{Mutex, MutexGuard};

/// The variable that names the server.
pub const ENV: &str = "DBAL_TEST_PGSQL_URL";

static LOCK: Mutex<()> = Mutex::const_new(());

/// A connection to the test database, and the lock the test holds.
pub struct Live {
    pub connection: Connection,
    _guard: MutexGuard<'static, ()>,
}

impl std::ops::Deref for Live {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        &self.connection
    }
}

/// The connection parameters of `DBAL_TEST_PGSQL_URL`, or `None` (with a
/// note) when it is not set.
pub fn params(test: &str) -> Option<PhpArray> {
    let Ok(url) = std::env::var(ENV) else {
        eprintln!("{test}: skipped, {ENV} is not set (needs a live PostgreSQL server)");
        return None;
    };
    let mut params = DsnParser::new([("postgres", "pdo_pgsql"), ("postgresql", "pdo_pgsql")])
        .parse(url.as_bytes())
        .unwrap_or_else(|e| panic!("{ENV}: {e}"));
    if params.get(&Key::from("driver")).is_none() {
        params.insert("driver", Value::string("pdo_pgsql"));
    }
    Some(params)
}

/// `TestUtil::getConnection()` with a configuration.
pub async fn pgsql_with(test: &str, configuration: Option<Configuration>) -> Option<Live> {
    let params = params(test)?;
    let guard = LOCK.lock().await;
    create_database(&params).await;
    let connection = DriverManager::get_connection(params, configuration).unwrap();
    Some(Live {
        connection,
        _guard: guard,
    })
}

/// `TestUtil::getConnection()`: the functional tests' connection, or
/// `None` when no server is named.
pub async fn pgsql(test: &str) -> Option<Live> {
    pgsql_with(test, None).await
}

/// Creates the database of the parameters if it is missing (`PostgreSQL`
/// has no `CREATE DATABASE IF NOT EXISTS`).
async fn create_database(params: &PhpArray) {
    let Some(Value::String(dbname)) = params.get(&Key::from("dbname")).cloned() else {
        return;
    };
    let dbname = dbname.to_string_lossy().into_owned();
    let mut server = params.clone();
    server.remove(&Key::from("dbname"));
    let connection = DriverManager::get_connection(server, None).unwrap();
    let exists = connection
        .fetch_one(
            "SELECT 1 FROM pg_database WHERE datname = ?",
            &PhpArray::list([Value::string(dbname.as_str())]),
            &ParameterTypes::new(),
        )
        .await
        .unwrap();
    if exists.is_some() {
        return;
    }
    let name = connection
        .get_database_platform()
        .await
        .unwrap()
        .quote_single_identifier(&dbname);
    connection
        .execute_statement(
            &format!("CREATE DATABASE {name}"),
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .unwrap();
}
