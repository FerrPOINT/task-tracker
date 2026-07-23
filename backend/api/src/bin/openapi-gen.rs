use std::fs;
use std::path::PathBuf;

fn main() {
    let json = api::openapi_json();
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "openapi/openapi.json".to_string()),
    );
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    fs::write(&out, json).unwrap();
    println!("Wrote {}", out.display());
}
