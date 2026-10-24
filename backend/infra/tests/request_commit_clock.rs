//! Small native SQL adapter boundary; no monolithic infrastructure unit target.
//! Both modules below are the unchanged production source, not a test reimplementation.
#[path = "../src/db_error.rs"]
mod db_error;
#[allow(dead_code)]
#[path = "../src/guarded_db.rs"]
mod guarded_db;
