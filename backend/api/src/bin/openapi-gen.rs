use std::fs;
use std::path::PathBuf;
use utoipa::OpenApi;

fn main() {
    let json = api::ApiDoc::openapi().to_pretty_json().unwrap();
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "openapi/openapi.json".to_string()),
    );
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    fs::write(&out, json).unwrap();
    println!("Wrote {}", out.display());
}
