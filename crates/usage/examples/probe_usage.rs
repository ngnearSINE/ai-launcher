//! Diagnostics: what the launcher would show for each profile folder given on the command line
//! (plans, windows and percentages; no tokens, no e-mail addresses).
//! `cargo run -p launcher-usage --example probe_usage -- "<profile dir>" ...`

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let client = reqwest::Client::new();
    for dir in std::env::args().skip(1) {
        println!("== {dir}");
        let path = std::path::Path::new(&dir);
        println!("   has_session: {}", launcher_usage::claude::has_session(path));
        match rt.block_on(launcher_usage::claude::fetch(&client, path)) {
            Ok(r) => {
                for o in &r.orgs {
                    println!("   org {} ({}) type={}", &o.org_uuid[..8.min(o.org_uuid.len())], o.plan, o.org_type);
                    for q in &o.quotas {
                        println!("     {:?} '{}' used={:.1}% resets={:?}", q.kind, q.label, q.used_percent, q.resets_at);
                    }
                }
            }
            Err(e) => println!("   error: {e:?}"),
        }
    }
}
