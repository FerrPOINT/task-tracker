-- Minimal SQL-owner fixture; no application identity or receipt is fabricated.
CREATE TABLE projects(id uuid PRIMARY KEY,owner_id uuid,archived_at timestamptz);
CREATE TABLE issues(id uuid PRIMARY KEY,project_id uuid,deleted_at timestamptz);
CREATE TABLE sdlc_tasks(task_id uuid PRIMARY KEY,project_id uuid);
CREATE TABLE users(id uuid PRIMARY KEY,central_sub text,is_active boolean);
CREATE TABLE project_members(project_id uuid,user_id uuid);
CREATE TABLE tracker_namespace_bindings(resource_id uuid,registry_instance_id uuid,namespace_id uuid);
CREATE TABLE tracker_space_catalog(registry_instance_id uuid,space_id uuid,admin_revision bigint,state text);
CREATE TABLE sdlc_task_source_bindings(task_id uuid,requirement_revision bigint,payload jsonb);
CREATE TABLE sdlc_assignments(task_id uuid);

INSERT INTO users VALUES('00000000-0000-4000-8000-000000000001','native-scope-producer',true);
INSERT INTO projects VALUES('00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000009',NULL);
INSERT INTO project_members VALUES('00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000001');
INSERT INTO issues VALUES('00000000-0000-4000-8000-000000000003','00000000-0000-4000-8000-000000000002',NULL);
INSERT INTO sdlc_tasks SELECT id,project_id FROM issues;
INSERT INTO tracker_namespace_bindings VALUES('00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000004','00000000-0000-4000-8000-000000000005');
INSERT INTO tracker_space_catalog VALUES('00000000-0000-4000-8000-000000000004','00000000-0000-4000-8000-000000000005',1,'active');

CREATE FUNCTION qa_bind_scope(caller_write jsonb,scope_write jsonb,changes jsonb DEFAULT '{}',ttl integer DEFAULT 30)
RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE caller jsonb; scope jsonb; now_seconds bigint;
BEGIN
 now_seconds:=floor(extract(epoch FROM clock_timestamp()))::bigint;
 caller:=jsonb_build_object('schema_version',1,'identity',jsonb_build_object('issuer','https://native-scope.test','subject','native-scope-producer'),
   'scopes',jsonb_build_array('task-tracker:read','task-tracker:write'),'writable',caller_write,
   'validity',jsonb_build_object('observed_at',now_seconds,'valid_until',now_seconds+ttl));
 PERFORM set_config('pdlc.tracker.native_caller',caller::text,true);
 scope:=jsonb_build_object('schema_version',1,'task_id','00000000-0000-4000-8000-000000000003',
   'project_id','00000000-0000-4000-8000-000000000002','identity',caller->'identity','validity',caller->'validity','writable',scope_write)||changes;
 PERFORM tracker_bind_native_task(scope);
 RETURN current_setting('pdlc.tracker.native_task_scope')::jsonb;
END $$;

CREATE FUNCTION qa_expect_scope_denial(caller_write jsonb,scope_write jsonb,changes jsonb,expected text,ttl integer DEFAULT 30)
RETURNS void LANGUAGE plpgsql AS $$ BEGIN
 BEGIN
  PERFORM qa_bind_scope(caller_write,scope_write,changes,ttl);
  RAISE EXCEPTION 'unexpected native scope admission';
 EXCEPTION WHEN insufficient_privilege THEN
  IF SQLERRM<>expected THEN RAISE EXCEPTION 'wrong denial: %',SQLERRM;END IF;
 END;
END $$;
