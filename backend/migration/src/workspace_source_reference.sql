-- Optional metadata in existing immutable receipts; no backfill or new authority.
CREATE FUNCTION sdlc_workspace_source_reference_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE selected text;
BEGIN
    IF TG_TABLE_NAME='sdlc_source_policy_revisions' THEN
        selected := NEW.payload->'source'->>'development_workspace_id';
        IF selected IS DISTINCT FROM NEW.payload->'verification'->>'development_workspace_id' THEN
            RAISE EXCEPTION 'SDLC workspace source reference differs' USING ERRCODE='23514';
        END IF;
    ELSE
        selected := NEW.payload->>'development_workspace_id';
        IF NOT EXISTS (
            SELECT 1 FROM sdlc_source_policy_revisions p
            WHERE p.project_id=NEW.project_id AND p.source_revision=NEW.source_revision
                AND p.source_hash=NEW.policy_hash
                AND p.payload->'source'->>'development_workspace_id' IS NOT DISTINCT FROM selected
        ) THEN
            RAISE EXCEPTION 'SDLC workspace source reference differs' USING ERRCODE='23514';
        END IF;
    END IF;
    IF selected IS NOT NULL AND (selected !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
        OR selected='00000000-0000-0000-0000-000000000000') THEN
        RAISE EXCEPTION 'SDLC workspace source reference invalid' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_workspace_source_policy_gate BEFORE INSERT ON sdlc_source_policy_revisions
    FOR EACH ROW EXECUTE FUNCTION sdlc_workspace_source_reference_gate();
CREATE TRIGGER sdlc_workspace_source_binding_gate BEFORE INSERT ON sdlc_task_source_bindings
    FOR EACH ROW EXECUTE FUNCTION sdlc_workspace_source_reference_gate();
