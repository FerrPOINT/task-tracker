-- Created before first migration only for a proven uninitialized Tracker DB.
-- An empty row set on an existing installation grants no automatic activation.
CREATE TABLE IF NOT EXISTS task_model_bootstrap_provenance (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    bootstrap_id uuid NOT NULL UNIQUE,
    origin text NOT NULL CHECK(origin='uninitialized_tracker_database'),
    pending boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    completed_at timestamptz,
    CHECK(pending=(completed_at IS NULL))
);
