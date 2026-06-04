use std::env;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let path = env::var("PATH").unwrap_or_else(|_| "<unset>".to_string());
    let argv0 = env::args().next().unwrap_or_else(|| "<unknown>".to_string());

    println!("hello-rust: ELF binary built from Rust, running on NullVoidOS");
    println!("  pid:       {}", process::id());
    println!("  argv[0]:   {}", argv0);
    println!("  unix_ts:   {}", now);
    println!("  path_head: {}", path.split(':').next().unwrap_or(""));
}
