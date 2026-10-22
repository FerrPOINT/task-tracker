-- Only a fresh disposable database may run this focused invariant fixture.
CREATE TABLE projects(id uuid PRIMARY KEY,owner_id uuid NOT NULL DEFAULT '00000000-0000-4000-8000-000000000030',namespace_managed boolean NOT NULL DEFAULT false);
CREATE TABLE users(id uuid PRIMARY KEY,is_active boolean NOT NULL,central_sub text);
INSERT INTO users VALUES('00000000-0000-4000-8000-000000000030',true,'owner-subject');
CREATE TABLE tracker_namespace_bindings(resource_id uuid PRIMARY KEY,registry_instance_id uuid,namespace_id uuid,state text,command jsonb);
INSERT INTO projects(id) VALUES ('00000000-0000-4000-8000-000000000001'),('00000000-0000-4000-8000-000000000002');
INSERT INTO tracker_namespace_bindings VALUES
('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000011','active','{"operation_id":"00000000-0000-4000-8000-000000000021"}'),
('00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000012','active','{"operation_id":"00000000-0000-4000-8000-000000000022"}');

-- Focused SQL fixture contains the exact dependency columns used by the new
-- migration. Full product constraints are exercised separately by infra/server QA.
CREATE TABLE sdlc_tasks(task_id uuid PRIMARY KEY,project_id uuid NOT NULL REFERENCES projects(id));
CREATE TABLE sdlc_assignments(task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id),version bigint NOT NULL,payload jsonb NOT NULL,PRIMARY KEY(task_id,version));
CREATE TABLE sdlc_analysis_capacity_holds(assignment_id uuid PRIMARY KEY,root_task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id));

CREATE TABLE sdlc_task_source_bindings(task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id),payload jsonb NOT NULL);
CREATE TABLE sdlc_pm_completions(task_id uuid NOT NULL REFERENCES sdlc_tasks(task_id),assignment_id uuid NOT NULL,execution_id uuid NOT NULL,assignment_version bigint NOT NULL);
