-- The marker is opt-in on creation only; existing aggregates and ledgers are untouched.
DO $$
DECLARE c record; definition text; changed text; n integer := 0;
BEGIN
    FOR c IN SELECT oid,conname FROM pg_constraint
        WHERE conrelid='sdlc_draft_creations'::regclass AND contype='c'
    LOOP
        definition := pg_get_constraintdef(c.oid);
        IF position('result - ARRAY[' IN definition)>0 THEN
            changed := replace(definition,'result - ARRAY[','(result - ''interaction_protocol'') - ARRAY[');
            EXECUTE format('ALTER TABLE sdlc_draft_creations DROP CONSTRAINT %I',c.conname);
            EXECUTE format('ALTER TABLE sdlc_draft_creations ADD CONSTRAINT %I %s',c.conname,changed);
            n := n+1;
        END IF;
    END LOOP;
    IF n<>1 THEN RAISE EXCEPTION 'configured CLI creation shape anchor changed'; END IF;
END $$;
ALTER TABLE sdlc_draft_creations ADD CONSTRAINT sdlc_creation_protocol CHECK
    ((NOT result ? 'interaction_protocol' OR result->'interaction_protocol'='"configured-cli-v1"'::jsonb) IS TRUE);

CREATE FUNCTION sdlc_configured_cli_marker_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='UPDATE' AND OLD.state->'interaction_protocol' IS DISTINCT FROM NEW.state->'interaction_protocol' THEN
        RAISE EXCEPTION 'interaction protocol is immutable; no legacy retrofit' USING ERRCODE='23514';
    END IF;
    IF NOT NEW.state ? 'interaction_protocol' THEN RETURN NEW; END IF;
    IF NOT ((NEW.state->'interaction_protocol'='"configured-cli-v1"'::jsonb
        AND NEW.state->'flow'='"single-task-v1"'::jsonb) IS TRUE) THEN
        RAISE EXCEPTION 'invalid configured CLI protocol' USING ERRCODE='23514';
    END IF;
    IF TG_OP='UPDATE' AND OLD.pm_execution_id IS NOT NULL
        AND OLD.state->'workflow_task_key' IS DISTINCT FROM NEW.state->'workflow_task_key' THEN
        RAISE EXCEPTION 'configured CLI workflow business key is immutable' USING ERRCODE='23514';
    END IF;
    IF NEW.pm_execution_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM sdlc_pm_executions e WHERE e.task_id=NEW.task_id AND e.execution_id=NEW.pm_execution_id
            AND e.result->'interaction_protocol'=NEW.state->'interaction_protocol'
            AND e.result->'assignment'=NEW.state->'assignment'
            AND NEW.state->>'workflow_task_key'='SDLC-'||e.ordinal::text
    ) THEN
        RAISE EXCEPTION 'configured CLI requires exact business reservation' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_configured_cli_marker_gate BEFORE INSERT OR UPDATE ON sdlc_tasks
    FOR EACH ROW EXECUTE FUNCTION sdlc_configured_cli_marker_gate();

CREATE FUNCTION sdlc_configured_cli_origin_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state ? 'interaction_protocol' AND NOT EXISTS(
        SELECT 1 FROM sdlc_draft_creations c WHERE c.task_id=NEW.task_id AND c.project_id=NEW.project_id
            AND c.actor_subject=NEW.owner_subject AND c.result->'interaction_protocol'=NEW.state->'interaction_protocol'
    ) THEN RAISE EXCEPTION 'configured CLI requires original owner creation ledger' USING ERRCODE='23514'; END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER sdlc_configured_cli_origin_gate AFTER INSERT ON sdlc_tasks
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION sdlc_configured_cli_origin_gate();

CREATE FUNCTION sdlc_configured_cli_creation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_tasks t WHERE t.task_id=NEW.task_id
        AND t.state->'interaction_protocol' IS NOT DISTINCT FROM NEW.result->'interaction_protocol') THEN
        RAISE EXCEPTION 'creation protocol must match original task' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_configured_cli_creation_gate BEFORE INSERT ON sdlc_draft_creations
    FOR EACH ROW EXECUTE FUNCTION sdlc_configured_cli_creation_gate();

CREATE FUNCTION sdlc_configured_cli_no_native_lease() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS(SELECT 1 FROM sdlc_tasks t WHERE t.pm_execution_id=NEW.execution_id
        AND t.state->'interaction_protocol'='"configured-cli-v1"'::jsonb) THEN
        RAISE EXCEPTION 'configured CLI has no native execution lease' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_configured_cli_no_native_lease BEFORE INSERT ON sdlc_pm_execution_leases
    FOR EACH ROW EXECUTE FUNCTION sdlc_configured_cli_no_native_lease();
