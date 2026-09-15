use aro_store::AroStore;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn rls_placeholder_test() -> TestResult {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping RLS integration test: DATABASE_URL is not set");
        return Ok(());
    };
    let _store = AroStore::connect(&database_url, 1).await?;
    Ok(())
}
