//! Diagnostics: prints the raw usage response of one Claude profile folder (no tokens).
//! `cargo run -p launcher-usage --example dump_usage -- "%APPDATA%\Claude"`

fn main() {
    let dir = std::env::args().nth(1).expect("profile folder");
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let session = launcher_usage::claude::read_session(std::path::Path::new(&dir), chrono::Utc::now().timestamp_millis())
            .expect("session");
        let client = reqwest::Client::new();
        let t = &session.tokens[0].token;
        let res = client
            .get("https://api.anthropic.com/api/oauth/usage")
            .bearer_auth(t)
            .header("anthropic-beta", "oauth-2025-04-20")
            .send()
            .await
            .unwrap();
        let v: serde_json::Value = res.json().await.unwrap();
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
    });
}
