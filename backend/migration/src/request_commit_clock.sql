-- Freshness only. Product admission/role checks remain at their owning boundary.
-- Insert/delete in the same transaction keeps the deferred event without durable rows.
CREATE TABLE tracker_request_commit_fences (
    fence_id uuid PRIMARY KEY,
    valid_until timestamptz NOT NULL
);
CREATE FUNCTION tracker_request_commit_clock() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF clock_timestamp() >= NEW.valid_until THEN
        RAISE EXCEPTION 'pdlc_request_authority_expired' USING ERRCODE='42501';
    END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER tracker_request_commit_clock
AFTER INSERT ON tracker_request_commit_fences
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
EXECUTE FUNCTION tracker_request_commit_clock();
