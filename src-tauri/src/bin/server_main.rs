#[tokio::main]
async fn main() {
    tauri_app_lib::server::serve().await
}
