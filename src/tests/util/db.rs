use sqlx::{
    Pool, Postgres,
    testing::{TestArgs, TestSupport},
};
use uuid::Uuid;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

pub async fn new_test_pool() -> Pool<Postgres> {
    let test_path = Uuid::new_v4().to_string();
    let args = TestArgs {
        // We might want to use the real test path here for debuggability
        // but that requires some macro magic that I'm not willing to
        // investigate for now, leaking is fine since we're in a test context
        test_path: test_path.leak(),
        migrator: Some(&MIGRATOR),
        fixtures: &[],
    };
    let cx_a = Postgres::test_context(&args)
        .await
        .expect("Failed to create DB test context");

    Pool::connect_with(cx_a.connect_opts.clone())
        .await
        .expect("Failed to connect to database")
}
