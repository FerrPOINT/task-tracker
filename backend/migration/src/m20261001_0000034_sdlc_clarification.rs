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
    CHECK (state->>'stage' IN ('Draft', 'Clarification', 'Backlog'))
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
    UNIQUE(task_id, revision),
    FOREIGN KEY(task_id, revision, content_hash) REFERENCES sdlc_requirements(task_id, revision, content_hash)
);
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
INSERT INTO statuses (id, name, category, position, is_default, is_closed)
SELECT gen_random_uuid(), name, 'todo', 0, false, false
FROM (VALUES ('SDLC Draft'), ('SDLC Clarification'), ('Backlog')) AS names(name)
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
        'sdlc_draft_creations']
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
