use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

pub const UP_SQL: &str = r#"
CREATE INDEX sdlc_project_members_user_idx ON project_members(user_id,project_id);
CREATE TABLE sdlc_instance (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    instance_id text NOT NULL UNIQUE CHECK (length(instance_id) BETWEEN 1 AND 128)
);
CREATE TABLE sdlc_tasks (
    task_id uuid PRIMARY KEY REFERENCES issues(id) ON DELETE RESTRICT,
    tracker_instance_id text NOT NULL REFERENCES sdlc_instance(instance_id),
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    root_task_id uuid NOT NULL REFERENCES issues(id) ON DELETE RESTRICT,
    owner_subject text NOT NULL CHECK (length(owner_subject) > 0),
    state jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (state->>'task_id' = task_id::text),
    CHECK (state->>'root_task_id' = root_task_id::text),
    CHECK (state->>'project_id' = project_id::text),
    CHECK (state->>'tracker_instance_id' = tracker_instance_id),
    CHECK (state->>'owner_subject' = owner_subject),
    CONSTRAINT sdlc_root_only CHECK (root_task_id = task_id),
    CHECK (state->>'stage' IN ('Draft', 'Clarification', 'Backlog', 'Analysis'))
);
CREATE INDEX sdlc_tasks_project ON sdlc_tasks(project_id);
CREATE INDEX sdlc_tasks_root ON sdlc_tasks(root_task_id);
ALTER TABLE sdlc_tasks ADD CONSTRAINT sdlc_tasks_creation_owner
    UNIQUE(task_id, project_id, owner_subject);
CREATE TABLE sdlc_draft_creations (
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    actor_subject text NOT NULL CHECK (length(actor_subject) > 0),
    idempotency_key text NOT NULL CHECK (octet_length(idempotency_key) BETWEEN 1 AND 128),
    payload_hash text NOT NULL CHECK (payload_hash ~ '^[0-9a-f]{64}$'),
    task_id uuid NOT NULL,
    result jsonb NOT NULL,
    input_snapshot_ref uuid UNIQUE,
    input_title text,
    input_description text,
    input_sha256 text,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(project_id, actor_subject, idempotency_key),
    UNIQUE(task_id),
    CHECK ((input_snapshot_ref IS NULL AND input_title IS NULL
        AND input_description IS NULL AND input_sha256 IS NULL)
        OR (input_snapshot_ref IS NOT NULL AND input_title IS NOT NULL
        AND input_description IS NOT NULL AND input_sha256 IS NOT NULL
        AND input_snapshot_ref != '00000000-0000-0000-0000-000000000000'::uuid
        AND char_length(input_title) BETWEEN 1 AND 500
        AND char_length(input_description) <= 100000
        AND input_sha256 ~ '^[0-9a-f]{64}$')),
    FOREIGN KEY(task_id, project_id, actor_subject)
        REFERENCES sdlc_tasks(task_id, project_id, owner_subject) ON DELETE RESTRICT,
    CHECK (jsonb_typeof(result) = 'object'
        AND result ?& ARRAY['tracker_instance_id','project_id','task_id','root_task_id','task_key','owner_subject','stage']
        AND result - ARRAY['tracker_instance_id','project_id','task_id','root_task_id','task_key','owner_subject','stage'] = '{}'::jsonb),
    CHECK ((jsonb_typeof(result->'tracker_instance_id') = 'string'
        AND length(result->>'tracker_instance_id') > 0) IS TRUE),
    CHECK ((jsonb_typeof(result->'task_key') = 'string'
        AND length(result->>'task_key') > 0) IS TRUE),
    CHECK ((result->>'task_id' = task_id::text) IS TRUE),
    CHECK ((result->>'root_task_id' = task_id::text) IS TRUE),
    CHECK ((result->>'project_id' = project_id::text) IS TRUE),
    CHECK ((result->>'owner_subject' = actor_subject) IS TRUE),
    CHECK ((result->>'stage' = 'Draft') IS TRUE)
);
CREATE TABLE sdlc_agent_bindings (
    tracker_instance_id text NOT NULL REFERENCES sdlc_instance(instance_id),
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id),
    agent_id uuid NOT NULL,
    binding_id uuid NOT NULL UNIQUE,
    PRIMARY KEY(tracker_instance_id, task_id, agent_id)
);
CREATE TABLE sdlc_assignments (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id),
    version bigint NOT NULL CHECK (version BETWEEN 1 AND 9007199254740991),
    payload jsonb NOT NULL,
    PRIMARY KEY(task_id, version)
);
ALTER TABLE sdlc_draft_creations ADD CONSTRAINT sdlc_draft_input_identity
    UNIQUE(task_id,input_snapshot_ref,input_sha256);
CREATE TABLE sdlc_pm_executions (
    execution_id uuid PRIMARY KEY CHECK (execution_id != '00000000-0000-0000-0000-000000000000'::uuid),
    ordinal bigint GENERATED ALWAYS AS IDENTITY UNIQUE CHECK (ordinal > 0),
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id) ON DELETE RESTRICT,
    assignment_id uuid NOT NULL UNIQUE CHECK (assignment_id != '00000000-0000-0000-0000-000000000000'::uuid),
    assignment_version bigint NOT NULL CHECK (assignment_version BETWEEN 1 AND 9007199254740991),
    expected_owner_version bigint NOT NULL CHECK (expected_owner_version >= 0),
    owner_version bigint NOT NULL CHECK (owner_version BETWEEN 1 AND 9007199254740991 AND owner_version=expected_owner_version+1),
    input_snapshot_ref uuid NOT NULL,
    input_sha256 text NOT NULL CHECK (input_sha256 ~ '^[0-9a-f]{64}$'),
    assignment_operation_key text NOT NULL UNIQUE,
    result jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(task_id,execution_id), UNIQUE(task_id,owner_version), UNIQUE(task_id,assignment_version),
    FOREIGN KEY(task_id,assignment_version) REFERENCES sdlc_assignments(task_id,version),
    FOREIGN KEY(task_id,input_snapshot_ref,input_sha256)
        REFERENCES sdlc_draft_creations(task_id,input_snapshot_ref,input_sha256),
    CHECK ((result->'assignment'->>'execution_id'=execution_id::text
        AND result->'assignment'->>'assignment_id'=assignment_id::text
        AND (result->'assignment'->>'version')::bigint=assignment_version
        AND result->'execution'->>'ordinal'=ordinal::text
        AND result->'execution'->>'key'='SDLC-'||ordinal::text
        AND result->'binding'->>'task_id'=task_id::text
        AND (result->'owner_cas'->>'expected_version')::bigint=expected_owner_version
        AND (result->'owner_cas'->>'version')::bigint=owner_version
        AND result->'input'->>'snapshot_ref'=input_snapshot_ref::text
        AND result->'input'->>'sha256'=input_sha256
        AND result->>'assignment_operation_key'=assignment_operation_key
        AND result->>'admission_state'='reserved'
        AND result->'dispatch_allowed'='false'::jsonb) IS TRUE)
);
ALTER TABLE sdlc_tasks ADD COLUMN pm_owner_version bigint NOT NULL DEFAULT 0,
    ADD COLUMN pm_execution_id uuid,
    ADD COLUMN pm_admission_state text,
    ADD CONSTRAINT sdlc_pm_current_execution FOREIGN KEY(task_id,pm_execution_id)
        REFERENCES sdlc_pm_executions(task_id,execution_id),
    ADD CONSTRAINT sdlc_pm_control CHECK (
        (pm_owner_version=0 AND pm_execution_id IS NULL AND pm_admission_state IS NULL)
        OR (pm_owner_version=1 AND pm_execution_id IS NOT NULL AND pm_admission_state='reserved'
            AND (state->'assignment'->>'execution_id'=pm_execution_id::text) IS TRUE)
    );
CREATE FUNCTION sdlc_pm_reservation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.pm_execution_id IS NOT NULL AND
        (NEW.state IS DISTINCT FROM OLD.state OR NEW.pm_owner_version IS DISTINCT FROM OLD.pm_owner_version
         OR NEW.pm_execution_id IS DISTINCT FROM OLD.pm_execution_id
         OR NEW.pm_admission_state IS DISTINCT FROM OLD.pm_admission_state) THEN
        RAISE EXCEPTION 'SDLC reserved PM requires verified admission' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_pm_reservation_gate BEFORE UPDATE ON sdlc_tasks
FOR EACH ROW EXECUTE FUNCTION sdlc_pm_reservation_gate();
CREATE TABLE sdlc_pm_execution_leases (
    execution_id uuid PRIMARY KEY REFERENCES sdlc_pm_executions(execution_id) ON DELETE RESTRICT,
    lease_id uuid NOT NULL UNIQUE CHECK (lease_id != '00000000-0000-0000-0000-000000000000'::uuid),
    version bigint NOT NULL CHECK (version BETWEEN 1 AND 9007199254740991),
    holder_subject text NOT NULL CHECK (length(holder_subject)>0),
    claimed_at timestamptz NOT NULL,
    heartbeat_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    result jsonb NOT NULL,
    CHECK (heartbeat_at>=claimed_at AND expires_at=heartbeat_at+interval '30 seconds'),
    CHECK (version!=1 OR heartbeat_at=claimed_at),
    CHECK ((result->'lease'->>'lease_id'=lease_id::text
        AND (result->'lease'->>'version')::bigint=version
        AND result->'lease'->>'holder_subject'=holder_subject
        AND (result->'lease'->>'claimed_at')::timestamptz=claimed_at
        AND (result->'lease'->>'heartbeat_at')::timestamptz=heartbeat_at
        AND (result->'lease'->>'expires_at')::timestamptz=expires_at
        AND result->'fence'->>'execution_id'=execution_id::text
        AND result->'ttl_seconds'='30'::jsonb
        AND result->'heartbeat_seconds'='10'::jsonb
        AND result->'dispatch_allowed'='false'::jsonb) IS TRUE)
);
CREATE TABLE sdlc_pm_lease_operations (
    execution_id uuid NOT NULL REFERENCES sdlc_pm_execution_leases(execution_id) ON DELETE RESTRICT,
    actor_subject text NOT NULL CHECK (length(actor_subject)>0),
    idempotency_key text NOT NULL CHECK (octet_length(idempotency_key) BETWEEN 1 AND 128),
    payload_hash text NOT NULL CHECK (payload_hash ~ '^[0-9a-f]{64}$'),
    lease_version bigint NOT NULL CHECK (lease_version BETWEEN 1 AND 9007199254740991),
    result jsonb NOT NULL,
    PRIMARY KEY(execution_id,actor_subject,idempotency_key),
    UNIQUE(execution_id,lease_version),
    CHECK ((result->'fence'->>'execution_id'=execution_id::text
        AND (result->'lease'->>'version')::bigint=lease_version
        AND result->'lease'->>'holder_subject'=actor_subject
        AND result->'dispatch_allowed'='false'::jsonb) IS TRUE)
);
CREATE FUNCTION sdlc_pm_lease_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        RAISE EXCEPTION 'SDLC ownership lease cannot be deleted/reacquired' USING ERRCODE='23514';
    END IF;
    IF TG_OP='INSERT' THEN
        IF NEW.version!=1 THEN
            RAISE EXCEPTION 'SDLC initial ownership lease version invalid' USING ERRCODE='23514';
        END IF;
        RETURN NEW;
    END IF;
    IF NEW.execution_id IS DISTINCT FROM OLD.execution_id
        OR NEW.lease_id IS DISTINCT FROM OLD.lease_id
        OR NEW.holder_subject IS DISTINCT FROM OLD.holder_subject
        OR NEW.claimed_at IS DISTINCT FROM OLD.claimed_at
        OR NEW.version!=OLD.version+1
        OR NEW.heartbeat_at<OLD.heartbeat_at
        OR OLD.expires_at<=clock_timestamp()
        OR (NEW.result-'lease') IS DISTINCT FROM (OLD.result-'lease') THEN
        RAISE EXCEPTION 'SDLC expired or inconsistent ownership lease requires quiescence' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_pm_lease_gate BEFORE INSERT OR UPDATE OR DELETE ON sdlc_pm_execution_leases
FOR EACH ROW EXECUTE FUNCTION sdlc_pm_lease_gate();
CREATE TABLE sdlc_requests (
    request_id uuid PRIMARY KEY,
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id),
    UNIQUE(task_id, request_id)
);
CREATE TABLE sdlc_question_versions (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id),
    question_id uuid NOT NULL,
    version bigint NOT NULL CHECK (version BETWEEN 1 AND 9007199254740991),
    request_id uuid NOT NULL,
    payload jsonb NOT NULL,
    PRIMARY KEY(task_id, question_id, version),
    FOREIGN KEY(task_id, request_id) REFERENCES sdlc_requests(task_id, request_id)
);
CREATE TABLE sdlc_options (
    task_id uuid NOT NULL, question_id uuid NOT NULL, question_version bigint NOT NULL,
    option_id uuid NOT NULL, payload jsonb NOT NULL,
    PRIMARY KEY(task_id, question_id, question_version, option_id),
    FOREIGN KEY(task_id, question_id, question_version) REFERENCES sdlc_question_versions(task_id, question_id, version)
);
CREATE TABLE sdlc_requirements (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id),
    revision bigint NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
    content_hash text NOT NULL CHECK (content_hash ~ '^[0-9a-f]{64}$'),
    payload jsonb NOT NULL,
    PRIMARY KEY(task_id, revision), UNIQUE(task_id, revision, content_hash)
);
CREATE INDEX sdlc_options_stable_identity ON sdlc_options(task_id, question_id, option_id, question_version);
CREATE TABLE sdlc_answers (
    id uuid PRIMARY KEY, task_id uuid NOT NULL, question_id uuid NOT NULL,
    question_version bigint NOT NULL, requirement_revision bigint NOT NULL,
    payload jsonb NOT NULL,
    UNIQUE(task_id, question_id, question_version),
    FOREIGN KEY(task_id, question_id, question_version) REFERENCES sdlc_question_versions(task_id, question_id, version),
    FOREIGN KEY(task_id, requirement_revision) REFERENCES sdlc_requirements(task_id, revision)
);
CREATE TABLE sdlc_evidence (
    id uuid PRIMARY KEY, task_id uuid NOT NULL, revision bigint NOT NULL,
    content_hash text NOT NULL, check_id text NOT NULL, payload jsonb NOT NULL,
    FOREIGN KEY(task_id, revision, content_hash) REFERENCES sdlc_requirements(task_id, revision, content_hash)
);
CREATE TABLE sdlc_confirmations (
    id uuid PRIMARY KEY, task_id uuid NOT NULL, revision bigint NOT NULL,
    content_hash text NOT NULL, payload jsonb NOT NULL,
    UNIQUE(task_id, revision), UNIQUE(task_id, id, revision, content_hash),
    FOREIGN KEY(task_id, revision, content_hash) REFERENCES sdlc_requirements(task_id, revision, content_hash)
);
-- Only public owner-declared references, never private package content or admission.
CREATE FUNCTION sdlc_valid_role_routes(routes jsonb) RETURNS boolean IMMUTABLE LANGUAGE plpgsql AS $$
DECLARE role text; route jsonb; suffix text; profile text; field text;
    agents text[] := '{}'; namespaces text[] := '{}'; workflows text[] := '{}';
BEGIN
    IF (jsonb_typeof(routes)='object'
        AND routes ?& ARRAY['project_manager','analyst','architect','developer','reviewer','tester','devops']
        AND routes - ARRAY['project_manager','analyst','architect','developer','reviewer','tester','devops']='{}'::jsonb) IS NOT TRUE THEN RETURN false; END IF;
    FOREACH role IN ARRAY ARRAY['project_manager','analyst','architect','developer','reviewer','tester','devops'] LOOP
        route := routes->role;
        IF (jsonb_typeof(route)='object'
            AND route ?& ARRAY['agent_id','fleet_config_revision','package_commit','package_manifest_sha256','namespace_id','namespace_name','workflow_id','workflow_key','profile','workflow_catalog_version','workflow_catalog_sha256']
            AND route - ARRAY['agent_id','fleet_config_revision','package_commit','package_manifest_sha256','namespace_id','namespace_name','workflow_id','workflow_key','profile','workflow_catalog_version','workflow_catalog_sha256']='{}'::jsonb) IS NOT TRUE THEN RETURN false; END IF;
        FOREACH field IN ARRAY ARRAY['agent_id','package_commit','package_manifest_sha256','namespace_id','namespace_name','workflow_id','workflow_key','profile','workflow_catalog_sha256'] LOOP
            IF jsonb_typeof(route->field) IS DISTINCT FROM 'string' THEN RETURN false; END IF;
        END LOOP;
        IF (route->>'agent_id' ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
            AND route->>'agent_id' != '00000000-0000-0000-0000-000000000000'
            AND jsonb_typeof(route->'fleet_config_revision')='number'
            AND route->>'fleet_config_revision' ~ '^[1-9][0-9]{0,15}$') IS NOT TRUE THEN RETURN false; END IF;
        IF (route->>'fleet_config_revision')::bigint > 9007199254740991 THEN RETURN false; END IF;
        FOREACH field IN ARRAY ARRAY['namespace_id','workflow_id'] LOOP
            IF (route->>field ~ '^[1-9][0-9]{0,18}$') IS NOT TRUE THEN RETURN false; END IF;
            IF length(route->>field)=19 AND (route->>field) COLLATE "C" > '9223372036854775807' COLLATE "C" THEN RETURN false; END IF;
        END LOOP;
        suffix := replace(role,'_','-');
        profile := CASE role WHEN 'tester' THEN 'hermes-sdlc-quality' WHEN 'devops' THEN 'hermes-sdlc-operations' ELSE 'hermes-sdlc-'||suffix END;
        IF (route->>'package_commit'='4b9b4c9297a13fb28a6ba2039af2f7cb719f2f58'
            AND route->>'package_manifest_sha256' ~ '^[0-9a-f]{64}$'
            AND route->>'package_manifest_sha256'=routes->'project_manager'->>'package_manifest_sha256'
            AND route->>'namespace_name'='hermes-'||suffix
            AND route->>'workflow_key'='hermes-sdlc:'||role
            AND route->>'profile'=profile
            AND route->'workflow_catalog_version'='3'::jsonb
            AND route->>'workflow_catalog_sha256' ~ '^[0-9a-f]{64}$'
            AND route->>'workflow_catalog_sha256'=routes->'project_manager'->>'workflow_catalog_sha256') IS NOT TRUE THEN RETURN false; END IF;
        IF route->>'agent_id'=ANY(agents) OR route->>'namespace_id'=ANY(namespaces) OR route->>'workflow_id'=ANY(workflows) THEN RETURN false; END IF;
        agents := array_append(agents,route->>'agent_id');
        namespaces := array_append(namespaces,route->>'namespace_id');
        workflows := array_append(workflows,route->>'workflow_id');
    END LOOP;
    RETURN true;
END $$;
CREATE TABLE sdlc_project_routing_revisions (
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    version bigint NOT NULL CHECK (version BETWEEN 1 AND 9007199254740991),
    tracker_instance_id text NOT NULL REFERENCES sdlc_instance(instance_id),
    routing_hash text NOT NULL CHECK (routing_hash ~ '^[0-9a-f]{64}$'),
    payload jsonb NOT NULL,
    PRIMARY KEY(project_id,version),
    CHECK ((payload->'contract_version'='1'::jsonb
        AND payload->>'project_id'=project_id::text
        AND payload->>'tracker_instance_id'=tracker_instance_id
        AND (payload->>'version')::bigint=version
        AND payload->>'routing_hash'=routing_hash
        AND payload->>'verification'='declared'
        AND payload->'native_ready'='false'::jsonb AND payload->'dispatch_allowed'='false'::jsonb
        AND sdlc_valid_role_routes(payload->'routes')) IS TRUE)
);
CREATE TABLE sdlc_project_routing_heads (
    project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE RESTRICT,
    version bigint NOT NULL,
    FOREIGN KEY(project_id,version) REFERENCES sdlc_project_routing_revisions(project_id,version)
);
CREATE FUNCTION sdlc_routing_head_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN RAISE EXCEPTION 'SDLC routing head cannot be deleted' USING ERRCODE='23514'; END IF;
    IF (TG_OP='INSERT' AND NEW.version!=1) OR
        (TG_OP='UPDATE' AND (NEW.project_id IS DISTINCT FROM OLD.project_id OR NEW.version!=OLD.version+1)) THEN
        RAISE EXCEPTION 'SDLC routing head must advance by one' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_routing_head_gate BEFORE INSERT OR UPDATE OR DELETE ON sdlc_project_routing_heads
FOR EACH ROW EXECUTE FUNCTION sdlc_routing_head_gate();
CREATE TABLE sdlc_project_routing_operations (
    project_id uuid NOT NULL,
    actor_subject text NOT NULL CHECK (length(actor_subject)>0),
    idempotency_key text NOT NULL CHECK (octet_length(idempotency_key) BETWEEN 1 AND 128),
    payload_hash text NOT NULL CHECK (payload_hash ~ '^[0-9a-f]{64}$'),
    version bigint NOT NULL,
    PRIMARY KEY(project_id,actor_subject,idempotency_key),
    UNIQUE(project_id,version),
    FOREIGN KEY(project_id,version) REFERENCES sdlc_project_routing_revisions(project_id,version)
);
ALTER TABLE sdlc_tasks ADD CONSTRAINT sdlc_task_project_identity UNIQUE(task_id,project_id);
CREATE TABLE sdlc_task_routing_snapshots (
    snapshot_id uuid PRIMARY KEY CHECK (snapshot_id!='00000000-0000-0000-0000-000000000000'::uuid),
    task_id uuid NOT NULL UNIQUE,
    project_id uuid NOT NULL,
    policy_version bigint NOT NULL,
    confirmation_id uuid NOT NULL UNIQUE,
    requirement_revision bigint NOT NULL,
    content_hash text NOT NULL,
    payload jsonb NOT NULL,
    FOREIGN KEY(task_id,project_id) REFERENCES sdlc_tasks(task_id,project_id),
    FOREIGN KEY(project_id,policy_version) REFERENCES sdlc_project_routing_revisions(project_id,version),
    FOREIGN KEY(task_id,confirmation_id,requirement_revision,content_hash) REFERENCES sdlc_confirmations(task_id,id,revision,content_hash),
    UNIQUE(task_id,confirmation_id,requirement_revision,content_hash,snapshot_id),
    CHECK ((payload->'contract_version'='1'::jsonb AND payload->>'snapshot_id'=snapshot_id::text
        AND payload->>'task_id'=task_id::text AND payload->>'root_task_id'=task_id::text
        AND payload->>'project_id'=project_id::text AND payload->>'confirmation_id'=confirmation_id::text
        AND (payload->>'requirement_revision')::bigint=requirement_revision
        AND payload->>'content_hash'=content_hash
        AND (payload->'policy'->>'version')::bigint=policy_version
        AND payload->'policy'->>'project_id'=project_id::text) IS TRUE)
);
CREATE INDEX sdlc_task_routing_policy_idx ON sdlc_task_routing_snapshots(project_id,policy_version);
CREATE FUNCTION sdlc_routing_snapshot_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM sdlc_tasks t WHERE t.task_id=NEW.task_id
        AND t.project_id=NEW.project_id AND t.root_task_id=t.task_id AND t.pm_execution_id IS NULL
        AND t.tracker_instance_id=NEW.payload->>'tracker_instance_id'
        AND t.state->>'stage' IN ('Draft','Clarification')) OR NOT EXISTS (
        SELECT 1 FROM sdlc_project_routing_revisions r JOIN sdlc_project_routing_heads h USING(project_id)
        WHERE r.project_id=NEW.project_id AND r.version=NEW.policy_version AND h.version=r.version
        AND r.payload=NEW.payload->'policy') THEN
        RAISE EXCEPTION 'SDLC routing snapshot requires current exact publication policy' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_routing_snapshot_gate BEFORE INSERT ON sdlc_task_routing_snapshots
FOR EACH ROW EXECUTE FUNCTION sdlc_routing_snapshot_gate();
CREATE TABLE sdlc_analysis_intents (
    intent_id uuid PRIMARY KEY CHECK (intent_id != '00000000-0000-0000-0000-000000000000'::uuid),
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id),
    requirement_revision bigint NOT NULL CHECK (requirement_revision BETWEEN 1 AND 9007199254740991),
    content_hash text NOT NULL,
    confirmation_id uuid NOT NULL UNIQUE,
    operation_key text NOT NULL UNIQUE,
    payload jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(task_id, requirement_revision),
    routing_snapshot_id uuid,
    FOREIGN KEY(task_id,confirmation_id,requirement_revision,content_hash,routing_snapshot_id)
        REFERENCES sdlc_task_routing_snapshots(task_id,confirmation_id,requirement_revision,content_hash,snapshot_id),
    FOREIGN KEY(task_id,confirmation_id,requirement_revision,content_hash)
        REFERENCES sdlc_confirmations(task_id,id,revision,content_hash),
    CHECK ((payload->>'intent_id'=intent_id::text
        AND payload->>'task_id'=task_id::text
        AND payload->>'confirmation_id'=confirmation_id::text
        AND (payload->>'requirement_revision')::bigint=requirement_revision
        AND payload->>'content_hash'=content_hash
        AND payload->>'operation_key'=operation_key
        AND operation_key='analysis:'||confirmation_id::text
        AND payload->'contract_version'='1'::jsonb
        AND payload->>'stage'='Analysis' AND payload->>'status'='Ready'
        AND payload->>'role'='Analyst' AND payload->>'workflow'='hermes-sdlc:analyst'
        AND payload->>'mode'='analysis' AND payload->>'scope'='business'
        AND payload->'cycle'='0'::jsonb AND payload->'attempt'='0'::jsonb) IS TRUE)
);
CREATE FUNCTION sdlc_analysis_routing_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE expected uuid;
BEGIN
    SELECT snapshot_id INTO expected FROM sdlc_task_routing_snapshots WHERE task_id=NEW.task_id;
    IF NEW.routing_snapshot_id IS DISTINCT FROM expected THEN
        RAISE EXCEPTION 'SDLC Analysis routing snapshot mismatch' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_analysis_routing_gate BEFORE INSERT ON sdlc_analysis_intents
FOR EACH ROW EXECUTE FUNCTION sdlc_analysis_routing_gate();
CREATE TABLE sdlc_reservation_capacity (singleton boolean PRIMARY KEY CHECK(singleton));
INSERT INTO sdlc_reservation_capacity VALUES(true);
CREATE SEQUENCE sdlc_analysis_reservation_fence MAXVALUE 9007199254740991 NO CYCLE;
CREATE SEQUENCE sdlc_analysis_workflow_task_ordinal AS bigint NO CYCLE;
CREATE TABLE sdlc_analysis_reservations (
    assignment_id uuid PRIMARY KEY CHECK(assignment_id!='00000000-0000-0000-0000-000000000000'),
    execution_id uuid NOT NULL UNIQUE CHECK(execution_id!='00000000-0000-0000-0000-000000000000'),
    workflow_task_ordinal bigint NOT NULL UNIQUE CHECK(workflow_task_ordinal>0),
    task_id uuid NOT NULL UNIQUE REFERENCES sdlc_tasks(task_id),
    root_task_id uuid NOT NULL UNIQUE REFERENCES sdlc_tasks(task_id),
    agent_id uuid NOT NULL UNIQUE CHECK(agent_id!='00000000-0000-0000-0000-000000000000'),
    intent_id uuid NOT NULL UNIQUE REFERENCES sdlc_analysis_intents(intent_id),
    routing_snapshot_id uuid NOT NULL REFERENCES sdlc_task_routing_snapshots(snapshot_id),
    fencing_token bigint NOT NULL UNIQUE CHECK(fencing_token BETWEEN 1 AND 9007199254740991),
    lease_id uuid NOT NULL UNIQUE CHECK(lease_id!='00000000-0000-0000-0000-000000000000'),
    pool_slot smallint NOT NULL UNIQUE CHECK(pool_slot BETWEEN 1 AND 2),
    payload jsonb NOT NULL,
    CHECK(root_task_id=task_id),
    CHECK((payload->'contract_version'='1'::jsonb AND payload->>'assignment_id'=assignment_id::text
        AND payload->>'execution_id'=execution_id::text AND payload->>'task_id'=task_id::text
        AND payload->>'workflow_task_ref'='SDLC-'||workflow_task_ordinal::text
        AND payload->>'root_task_id'=root_task_id::text AND payload->>'agent_id'=agent_id::text
        AND payload->>'intent_id'=intent_id::text AND payload->>'routing_snapshot_id'=routing_snapshot_id::text
        AND payload->'fencing_token'=to_jsonb(fencing_token) AND payload->>'lease_id'=lease_id::text
        AND payload->'reservation_version'='1'::jsonb AND payload->>'stage'='Analysis'
        AND payload->>'role_key'='analyst' AND payload->>'workflow_key'='hermes-sdlc:analyst'
        AND payload->>'mode_key'='analysis' AND payload->>'scope'='business'
        AND payload->'cycle_number'='0'::jsonb AND payload->'attempt_number'='1'::jsonb
        AND payload->>'assignment_operation_key'='analysis-reserve:'||assignment_id::text
        AND payload->>'assignment_hash' ~ '^[0-9a-f]{64}$') IS TRUE)
);
CREATE INDEX sdlc_pm_assignment_agent_capacity ON sdlc_assignments((payload->>'agent_id'),task_id);
CREATE FUNCTION sdlc_analysis_reservation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM task_id FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    PERFORM singleton FROM sdlc_reservation_capacity WHERE singleton FOR UPDATE;
    IF EXISTS(SELECT 1 FROM sdlc_assignments WHERE task_id=NEW.root_task_id OR payload->>'agent_id'=NEW.agent_id::text)
        OR EXISTS(SELECT 1 FROM sdlc_pm_executions WHERE task_id=NEW.root_task_id)
        OR EXISTS(SELECT 1 FROM sdlc_tasks WHERE task_id=NEW.root_task_id AND (pm_execution_id IS NOT NULL OR state->'assignment' IS DISTINCT FROM 'null'::jsonb)) THEN
        RAISE EXCEPTION 'SDLC pm_quiescence_unverified' USING ERRCODE='23514';
    END IF;
    IF NOT EXISTS(SELECT 1 FROM sdlc_analysis_intents i JOIN sdlc_tasks t USING(task_id)
        JOIN sdlc_task_routing_snapshots s ON s.snapshot_id=i.routing_snapshot_id
        WHERE i.intent_id=NEW.intent_id AND i.task_id=NEW.task_id AND s.snapshot_id=NEW.routing_snapshot_id
        AND t.state->>'stage'='Analysis' AND t.root_task_id=NEW.root_task_id
        AND i.requirement_revision=(t.state->'revisions'->-1->>'revision')::bigint
        AND i.content_hash=t.state->'revisions'->-1->>'content_hash'
        AND NEW.payload->>'tracker_instance_id'=t.tracker_instance_id
        AND NEW.payload->>'owner_subject'=t.owner_subject AND NEW.payload->>'project_id'=t.project_id::text
        AND NEW.payload->>'confirmation_id'=i.confirmation_id::text
        AND NEW.payload->'requirement_revision'=to_jsonb(i.requirement_revision)
        AND NEW.payload->>'content_hash'=i.content_hash
        AND NEW.payload->'routing_policy_version'=to_jsonb(s.policy_version)
        AND NEW.payload->>'routing_hash'=s.payload->'policy'->>'routing_hash'
        AND NEW.payload->'route'=s.payload->'policy'->'routes'->'analyst'
        AND NEW.agent_id::text=s.payload->'policy'->'routes'->'analyst'->>'agent_id') THEN
        RAISE EXCEPTION 'SDLC reservation requires exact routed current Analysis intent' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_analysis_reservation_gate BEFORE INSERT ON sdlc_analysis_reservations
FOR EACH ROW EXECUTE FUNCTION sdlc_analysis_reservation_gate();
CREATE FUNCTION sdlc_pm_capacity_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM task_id FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    PERFORM singleton FROM sdlc_reservation_capacity WHERE singleton FOR UPDATE;
    IF EXISTS(SELECT 1 FROM sdlc_analysis_reservations WHERE root_task_id=NEW.task_id OR agent_id::text=NEW.payload->>'agent_id') THEN
        RAISE EXCEPTION 'SDLC root/agent capacity held by Analysis reservation' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_pm_capacity_gate BEFORE INSERT ON sdlc_assignments
FOR EACH ROW EXECUTE FUNCTION sdlc_pm_capacity_gate();
CREATE TABLE sdlc_analysis_reservation_leases (
    assignment_id uuid PRIMARY KEY REFERENCES sdlc_analysis_reservations(assignment_id),
    version bigint NOT NULL CHECK(version BETWEEN 1 AND 9007199254740991),
    holder_subject text NOT NULL CHECK(length(holder_subject) BETWEEN 1 AND 128),
    claimed_at timestamptz NOT NULL,
    heartbeat_at timestamptz NOT NULL CHECK(heartbeat_at>=claimed_at),
    expires_at timestamptz NOT NULL CHECK(expires_at=heartbeat_at+interval '30 seconds')
);
CREATE TABLE sdlc_analysis_reservation_operations (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id),
    idempotency_key text NOT NULL CHECK(octet_length(idempotency_key) BETWEEN 1 AND 128),
    actor_subject text NOT NULL,
    payload_hash text NOT NULL CHECK(payload_hash ~ '^[0-9a-f]{64}$'),
    assignment_id uuid NOT NULL REFERENCES sdlc_analysis_reservations(assignment_id),
    lease_version bigint NOT NULL CHECK(lease_version BETWEEN 1 AND 9007199254740991),
    result jsonb NOT NULL,
    PRIMARY KEY(task_id,idempotency_key), UNIQUE(assignment_id,lease_version),
    CHECK((result->'lease'->'version'=to_jsonb(lease_version)
        AND result->'lease'->>'holder_subject'=actor_subject
        AND result->'assignment'->>'assignment_id'=assignment_id::text
        AND result->'assignment'->>'task_id'=task_id::text
        AND result->'ttl_seconds'='30'::jsonb AND result->'heartbeat_interval_seconds'='10'::jsonb
        AND result->>'admission_state'='awaiting_admission'
        AND result->'dispatch_allowed'='false'::jsonb AND result->'capacity_held'='true'::jsonb) IS TRUE)
);
ALTER TABLE sdlc_analysis_reservation_leases ADD CONSTRAINT sdlc_reservation_current_receipt
    FOREIGN KEY(assignment_id,version) REFERENCES sdlc_analysis_reservation_operations(assignment_id,lease_version) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE sdlc_analysis_reservations ADD CONSTRAINT sdlc_reservation_lease_required
    FOREIGN KEY(assignment_id) REFERENCES sdlc_analysis_reservation_leases(assignment_id) DEFERRABLE INITIALLY DEFERRED;
CREATE FUNCTION sdlc_analysis_lease_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN RAISE EXCEPTION 'SDLC reservation capacity cannot be released without verified stop' USING ERRCODE='23514'; END IF;
    IF TG_OP='INSERT' THEN
        IF NEW.version!=1 OR NEW.heartbeat_at!=NEW.claimed_at THEN
            RAISE EXCEPTION 'SDLC reservation initial lease invalid' USING ERRCODE='23514';
        END IF;
    ELSIF NEW.assignment_id IS DISTINCT FROM OLD.assignment_id OR NEW.holder_subject IS DISTINCT FROM OLD.holder_subject
        OR NEW.claimed_at IS DISTINCT FROM OLD.claimed_at OR NEW.version!=OLD.version+1
        OR OLD.expires_at<=clock_timestamp() OR NEW.heartbeat_at<OLD.heartbeat_at THEN
        RAISE EXCEPTION 'SDLC expired/stale reservation lease needs reconciliation' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_analysis_lease_gate BEFORE INSERT OR UPDATE OR DELETE ON sdlc_analysis_reservation_leases
FOR EACH ROW EXECUTE FUNCTION sdlc_analysis_lease_gate();
CREATE FUNCTION sdlc_analysis_operation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM sdlc_analysis_reservations r JOIN sdlc_analysis_reservation_leases l USING(assignment_id)
        WHERE r.assignment_id=NEW.assignment_id AND r.task_id=NEW.task_id AND r.payload=NEW.result->'assignment'
        AND r.pool_slot::integer=(NEW.result->>'technical_pool_slot')::integer
        AND l.version=NEW.lease_version AND l.holder_subject=NEW.actor_subject
        AND l.claimed_at=(NEW.result->'lease'->>'claimed_at')::timestamptz
        AND l.heartbeat_at=(NEW.result->'lease'->>'heartbeat_at')::timestamptz
        AND l.expires_at=(NEW.result->'lease'->>'expires_at')::timestamptz
        AND r.lease_id::text=NEW.result->'lease'->>'lease_id') THEN
        RAISE EXCEPTION 'SDLC reservation operation must match current exact owner ledger' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_analysis_operation_gate BEFORE INSERT ON sdlc_analysis_reservation_operations
FOR EACH ROW EXECUTE FUNCTION sdlc_analysis_operation_gate();
CREATE TABLE sdlc_idempotency (
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id),
    actor_subject text NOT NULL, idempotency_key text NOT NULL,
    payload_hash text NOT NULL CHECK (payload_hash ~ '^[0-9a-f]{64}$'),
    result jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(task_id, actor_subject, idempotency_key)
);
CREATE TABLE sdlc_outbox (
    sequence bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id uuid NOT NULL UNIQUE,
    task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id),
    event_type text NOT NULL, payload jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX sdlc_outbox_task_cursor ON sdlc_outbox(task_id, sequence);
CREATE UNIQUE INDEX sdlc_outbox_analysis_once
    ON sdlc_outbox(task_id, (payload->'result'->>'requirement_revision'))
    WHERE event_type='analysis.intent_created';
INSERT INTO statuses (id, name, category, position, is_default, is_closed)
SELECT gen_random_uuid(), name, 'todo', 0, false, false
FROM (VALUES ('SDLC Draft'), ('SDLC Clarification'), ('Backlog'), ('SDLC Analysis')) AS names(name)
WHERE NOT EXISTS (SELECT 1 FROM statuses s WHERE s.name = names.name);

CREATE FUNCTION sdlc_binding_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.task_id != OLD.task_id OR NEW.tracker_instance_id != OLD.tracker_instance_id
        OR NEW.project_id != OLD.project_id OR NEW.root_task_id != OLD.root_task_id
        OR NEW.owner_subject != OLD.owner_subject THEN
        RAISE EXCEPTION 'SDLC binding is immutable' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_binding_immutable BEFORE UPDATE ON sdlc_tasks
FOR EACH ROW EXECUTE FUNCTION sdlc_binding_immutable();

CREATE FUNCTION sdlc_analysis_gate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.state->>'stage'='Analysis' AND NEW IS DISTINCT FROM OLD THEN
        RAISE EXCEPTION 'SDLC queued Analysis requires guarded admission before lifecycle mutation'
            USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_analysis_gate BEFORE UPDATE ON sdlc_tasks
FOR EACH ROW EXECUTE FUNCTION sdlc_analysis_gate();

CREATE FUNCTION sdlc_issue_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE binding sdlc_tasks%ROWTYPE; target_name text;
BEGIN
    SELECT * INTO binding FROM sdlc_tasks WHERE task_id = NEW.id;
    IF NOT FOUND THEN RETURN NEW; END IF;
    IF NEW.project_id != binding.project_id THEN
        RAISE EXCEPTION 'SDLC project binding is immutable' USING ERRCODE = '23514';
    END IF;
    SELECT name INTO target_name FROM statuses WHERE id = NEW.status_id;
    IF binding.state->>'stage' IN ('Draft', 'Clarification') THEN
        IF target_name != 'SDLC ' || (binding.state->>'stage') OR NEW.sprint_id IS NOT NULL THEN
            RAISE EXCEPTION 'SDLC confirmation gate blocks transition/sprint' USING ERRCODE = '23514';
        END IF;
    ELSIF NOT EXISTS (
        SELECT 1 FROM sdlc_confirmations c WHERE c.task_id = NEW.id
        AND c.revision = (binding.state->'revisions'->-1->>'revision')::bigint
        AND c.content_hash = binding.state->'revisions'->-1->>'content_hash'
    ) THEN
        RAISE EXCEPTION 'SDLC exact revision confirmation absent' USING ERRCODE = '23514';
    END IF;
    IF binding.state->>'stage'='Analysis' THEN
        IF target_name != 'SDLC Analysis' OR NEW.sprint_id IS NOT NULL OR NOT EXISTS (
            SELECT 1 FROM sdlc_analysis_intents q WHERE q.task_id=NEW.id
                AND q.requirement_revision=(binding.state->'revisions'->-1->>'revision')::bigint
                AND q.content_hash=binding.state->'revisions'->-1->>'content_hash'
        ) THEN
            RAISE EXCEPTION 'SDLC queued Analysis requires guarded admission' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_issue_gate BEFORE UPDATE ON issues
FOR EACH ROW EXECUTE FUNCTION sdlc_issue_gate();

CREATE FUNCTION sdlc_history_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'SDLC history is append-only' USING ERRCODE = '23514';
END $$;
DO $$ DECLARE name text; BEGIN
    FOREACH name IN ARRAY ARRAY['sdlc_instance', 'sdlc_agent_bindings', 'sdlc_assignments',
        'sdlc_requests', 'sdlc_question_versions', 'sdlc_options', 'sdlc_requirements',
        'sdlc_answers', 'sdlc_evidence', 'sdlc_confirmations', 'sdlc_idempotency', 'sdlc_outbox',
        'sdlc_draft_creations', 'sdlc_pm_executions', 'sdlc_pm_lease_operations', 'sdlc_analysis_intents',
        'sdlc_project_routing_revisions', 'sdlc_project_routing_operations', 'sdlc_task_routing_snapshots',
        'sdlc_reservation_capacity', 'sdlc_analysis_reservations', 'sdlc_analysis_reservation_operations']
    LOOP
        EXECUTE format('CREATE TRIGGER sdlc_history_immutable BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable()', name);
    END LOOP;
END $$;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(UP_SQL).await?;
        Ok(())
    }
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "SDLC migration is additive; rollback requires preserving business history".into(),
        ))
    }
}
