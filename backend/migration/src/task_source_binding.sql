-- No data backfill: old tasks/confirmations acquire no source authority.
CREATE TABLE sdlc_source_policy_revisions (
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    source_revision bigint NOT NULL CHECK (source_revision BETWEEN 1 AND 9007199254740991),
    source_hash text NOT NULL CHECK (source_hash ~ '^[0-9a-f]{64}$'),
    payload jsonb NOT NULL,
    PRIMARY KEY(project_id,source_revision),
    UNIQUE(project_id,source_revision,source_hash),
    CHECK ((payload->>'project_id'=project_id::text
        AND payload->>'source_revision'=source_revision::text
        AND payload->>'source_hash'=source_hash AND payload->>'contract_version'='1') IS TRUE)
);
CREATE TABLE sdlc_source_policy_heads (
    project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE RESTRICT,
    source_revision bigint NOT NULL,
    FOREIGN KEY(project_id,source_revision)
        REFERENCES sdlc_source_policy_revisions(project_id,source_revision)
);
CREATE FUNCTION sdlc_source_head_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        RAISE EXCEPTION 'SDLC source head history must be preserved' USING ERRCODE='23514';
    ELSIF TG_OP='INSERT' THEN
        IF NEW.source_revision<>1 THEN
            RAISE EXCEPTION 'SDLC initial source revision must be one' USING ERRCODE='23514';
        END IF;
    ELSIF NEW.project_id<>OLD.project_id OR NEW.source_revision<>OLD.source_revision+1 THEN
        RAISE EXCEPTION 'SDLC source revision CAS differs' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_source_head_gate BEFORE INSERT OR UPDATE OR DELETE ON sdlc_source_policy_heads
    FOR EACH ROW EXECUTE FUNCTION sdlc_source_head_gate();
CREATE TABLE sdlc_source_policy_operations (
    project_id uuid NOT NULL,
    actor_subject text NOT NULL,
    idempotency_key text NOT NULL CHECK (length(idempotency_key) BETWEEN 1 AND 128),
    payload_hash text NOT NULL CHECK (payload_hash ~ '^[0-9a-f]{64}$'),
    source_revision bigint NOT NULL,
    PRIMARY KEY(project_id,actor_subject,idempotency_key),
    FOREIGN KEY(project_id,source_revision)
        REFERENCES sdlc_source_policy_revisions(project_id,source_revision)
);
CREATE TABLE sdlc_task_source_selections (
    task_id uuid PRIMARY KEY,
    project_id uuid NOT NULL,
    source_revision bigint,
    policy_hash text,
    FOREIGN KEY(task_id,project_id) REFERENCES sdlc_tasks(task_id,project_id),
    FOREIGN KEY(project_id,source_revision,policy_hash)
        REFERENCES sdlc_source_policy_revisions(project_id,source_revision,source_hash),
    UNIQUE(task_id,project_id,source_revision,policy_hash),
    CHECK ((source_revision IS NULL)=(policy_hash IS NULL))
);
-- An explicit initial enrollment overlays an absent/unconfigured selection; never rewrites it.
CREATE TABLE sdlc_draft_source_enrollments (
    task_id uuid PRIMARY KEY,
    project_id uuid NOT NULL,
    enrollment_id uuid NOT NULL UNIQUE,
    actor_subject text NOT NULL,
    idempotency_key text NOT NULL CHECK(length(idempotency_key) BETWEEN 1 AND 128),
    request_sha256 text NOT NULL CHECK(request_sha256 ~ '^[0-9a-f]{64}$'),
    source_revision bigint NOT NULL,
    policy_hash text NOT NULL,
    source_hash text NOT NULL CHECK(source_hash ~ '^[0-9a-f]{64}$'),
    payload jsonb NOT NULL,
    UNIQUE(task_id,actor_subject,idempotency_key),
    FOREIGN KEY(task_id,project_id) REFERENCES sdlc_tasks(task_id,project_id),
    FOREIGN KEY(project_id,source_revision,policy_hash)
        REFERENCES sdlc_source_policy_revisions(project_id,source_revision,source_hash),
    CHECK((payload->>'task_id'=task_id::text AND payload->>'project_id'=project_id::text
        AND payload->>'enrollment_id'=enrollment_id::text AND payload->>'owner_subject'=actor_subject
        AND payload->>'source_revision'=source_revision::text AND payload->>'policy_hash'=policy_hash
        AND payload->>'source_hash'=source_hash AND payload->>'request_sha256'=request_sha256) IS TRUE)
);
CREATE VIEW sdlc_effective_task_source_selections AS
    SELECT task_id,project_id,source_revision,policy_hash FROM sdlc_draft_source_enrollments
    UNION ALL
    SELECT s.task_id,s.project_id,s.source_revision,s.policy_hash FROM sdlc_task_source_selections s
        WHERE NOT EXISTS(SELECT 1 FROM sdlc_draft_source_enrollments e WHERE e.task_id=s.task_id);
CREATE FUNCTION sdlc_draft_source_enrollment_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    -- Metadata-only policy pin; PM may brainstorm. Technical execution/publication closes enrollment.
    IF NOT EXISTS(SELECT 1 FROM sdlc_tasks t
        JOIN sdlc_source_policy_heads h ON h.project_id=t.project_id
        JOIN sdlc_source_policy_revisions p ON p.project_id=h.project_id AND p.source_revision=h.source_revision
        WHERE t.task_id=NEW.task_id AND t.project_id=NEW.project_id AND t.root_task_id=t.task_id
        AND t.owner_subject=NEW.actor_subject AND t.state->>'stage'='Draft'
        AND t.state->>'flow'='single-task-v1'
        AND t.state->'confirmations'='[]'::jsonb
        AND p.source_revision=NEW.source_revision AND p.source_hash=NEW.policy_hash)
        OR EXISTS(SELECT 1 FROM sdlc_task_source_selections WHERE task_id=NEW.task_id AND source_revision IS NOT NULL)
        OR EXISTS(SELECT 1 FROM sdlc_confirmations WHERE task_id=NEW.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_task_source_bindings WHERE task_id=NEW.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_analysis_intents WHERE task_id=NEW.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_analysis_reservations WHERE task_id=NEW.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_analysis_results WHERE task_id=NEW.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=NEW.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_assignments WHERE task_id=NEW.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_proposals WHERE task_id=NEW.task_id)
        OR EXISTS(SELECT 1 FROM sdlc_delivery_results WHERE task_id=NEW.task_id)
    THEN
        RAISE EXCEPTION 'SDLC explicit unconfirmed Draft initial source pin required' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_draft_source_enrollment_gate BEFORE INSERT ON sdlc_draft_source_enrollments
    FOR EACH ROW EXECUTE FUNCTION sdlc_draft_source_enrollment_gate();
-- A candidate ref cannot be reassigned to another root, even after a new confirmation.
CREATE TABLE sdlc_source_ref_owners (
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    forge_instance_id uuid NOT NULL,
    repository_id uuid NOT NULL,
    candidate_ref text NOT NULL CHECK (candidate_ref LIKE 'refs/heads/%'),
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    PRIMARY KEY(forge_instance_id,repository_id,candidate_ref),
    UNIQUE(forge_instance_id,repository_id,candidate_ref,task_id),
    FOREIGN KEY(task_id,project_id) REFERENCES sdlc_tasks(task_id,project_id),
    CHECK(candidate_ref='refs/heads/sdlc/'||task_id::text),
    FOREIGN KEY(task_id,forge_instance_id,repository_id)
        REFERENCES task_repository_links(issue_id,forge_instance_id,repository_id) ON DELETE RESTRICT
);
CREATE TABLE sdlc_task_source_bindings (
    task_id uuid NOT NULL,
    project_id uuid NOT NULL,
    confirmation_id uuid NOT NULL,
    requirement_revision bigint NOT NULL,
    content_hash text NOT NULL,
    -- The exact reference spans immutable initial and review snapshots. The INSERT gate
    -- below enforces it through their inventory; a FK to only generation one is invalid.
    routing_snapshot_id uuid NOT NULL,
    forge_instance_id uuid NOT NULL,
    repository_id uuid NOT NULL,
    candidate_ref text NOT NULL,
    source_revision bigint NOT NULL,
    policy_hash text NOT NULL,
    source_hash text NOT NULL CHECK (source_hash ~ '^[0-9a-f]{64}$'),
    payload jsonb NOT NULL,
    PRIMARY KEY(task_id,requirement_revision),
    UNIQUE(confirmation_id),
    FOREIGN KEY(task_id,project_id) REFERENCES sdlc_tasks(task_id,project_id),
    FOREIGN KEY(task_id,confirmation_id,requirement_revision,content_hash)
        REFERENCES sdlc_confirmations(task_id,id,revision,content_hash),
    FOREIGN KEY(project_id,source_revision,policy_hash)
        REFERENCES sdlc_source_policy_revisions(project_id,source_revision,source_hash),
    FOREIGN KEY(forge_instance_id,repository_id,candidate_ref,task_id)
        REFERENCES sdlc_source_ref_owners(forge_instance_id,repository_id,candidate_ref,task_id),
    CHECK ((payload->>'task_id'=task_id::text AND payload->>'root_task_id'=task_id::text
        AND payload->>'project_id'=project_id::text AND payload->>'confirmation_id'=confirmation_id::text
        AND payload->>'requirement_revision'=requirement_revision::text
        AND payload->>'content_hash'=content_hash AND payload->>'routing_snapshot_id'=routing_snapshot_id::text
        AND payload->>'forge_instance_id'=forge_instance_id::text AND payload->>'repository_id'=repository_id::text
        AND payload->>'candidate_ref'=candidate_ref AND payload->>'source_revision'=source_revision::text
        AND payload->>'policy_hash'=policy_hash AND payload->>'source_hash'=source_hash
        AND payload->>'contract_version'='1'
        AND payload->>'base_commit' ~ '^[0-9a-f]{40}$') IS TRUE)
);
CREATE FUNCTION sdlc_source_binding_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM sdlc_routing_snapshot_inventory r JOIN sdlc_tasks t USING(task_id)
        JOIN tracker_namespace_bindings b ON b.resource_id=t.project_id AND b.state='active'
        JOIN sdlc_effective_task_source_selections s ON s.task_id=t.task_id AND s.project_id=t.project_id
        JOIN sdlc_source_policy_revisions p ON p.project_id=s.project_id AND p.source_revision=s.source_revision AND p.source_hash=s.policy_hash
        WHERE r.snapshot_id=NEW.routing_snapshot_id AND r.task_id=NEW.task_id
        AND r.confirmation_id=NEW.confirmation_id AND r.requirement_revision=NEW.requirement_revision
        AND r.content_hash=NEW.content_hash AND t.project_id=NEW.project_id
        AND t.root_task_id=t.task_id AND t.state->>'flow'='single-task-v1'
        AND t.owner_subject=NEW.payload->>'owner_subject'
        AND t.tracker_instance_id=NEW.payload->>'tracker_instance_id'
        AND p.source_revision=NEW.source_revision AND p.source_hash=NEW.policy_hash
        AND p.payload->'source'->>'forge_instance_id'=NEW.forge_instance_id::text
        AND p.payload->'source'->>'repository_id'=NEW.repository_id::text
        AND p.payload->'source'->>'base_ref'=NEW.payload->>'base_ref'
        AND p.payload->'source'->>'base_commit'=NEW.payload->>'base_commit'
        AND p.payload->'verification'->'namespace'=NEW.payload->'namespace'
        AND NEW.payload->'namespace'->>'registry_instance_id'=b.registry_instance_id::text
        AND NEW.payload->'namespace'->>'namespace_id'=b.namespace_id::text
        AND NEW.candidate_ref='refs/heads/sdlc/'||NEW.task_id::text
        AND NEW.candidate_ref<>p.payload->'verification'->>'default_ref'
        AND NEW.candidate_ref<>p.payload->'source'->>'base_ref'
        AND NOT (p.payload->'verification'->'protected_refs' ? NEW.candidate_ref)
    ) THEN
        RAISE EXCEPTION 'SDLC exact confirmed source binding required' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_source_binding_gate BEFORE INSERT ON sdlc_task_source_bindings
    FOR EACH ROW EXECUTE FUNCTION sdlc_source_binding_gate();
DO $$ DECLARE name text; BEGIN
    FOREACH name IN ARRAY ARRAY['sdlc_source_policy_revisions','sdlc_source_policy_operations',
        'sdlc_task_source_selections','sdlc_draft_source_enrollments','sdlc_source_ref_owners','sdlc_task_source_bindings'] LOOP
        EXECUTE format('CREATE TRIGGER sdlc_history_immutable BEFORE UPDATE OR DELETE ON %I
            FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable()',name);
    END LOOP;
    FOREACH name IN ARRAY ARRAY['sdlc_source_policy_revisions','sdlc_source_policy_heads',
        'sdlc_source_policy_operations','sdlc_task_source_selections','sdlc_draft_source_enrollments','sdlc_source_ref_owners','sdlc_task_source_bindings'] LOOP
        EXECUTE format('CREATE TRIGGER namespace_write_guard BEFORE INSERT OR UPDATE OR DELETE ON %I
            FOR EACH ROW EXECUTE FUNCTION tracker_namespace_write_guard()',name);
    END LOOP;
END $$;
