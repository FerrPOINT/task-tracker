-- The locked parent issue serializes saves, including the first revision.
CREATE TABLE issue_plan_revisions (
 issue_id uuid NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
 revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
 author_id uuid NOT NULL,
 payload jsonb NOT NULL,
 PRIMARY KEY(issue_id,revision)
);
CREATE TABLE issue_plan_operations (
 issue_id uuid NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
 actor_subject text NOT NULL,
 operation_key text NOT NULL,
 author_id uuid NOT NULL,
 payload_hash text NOT NULL,
 result jsonb NOT NULL,
 PRIMARY KEY(issue_id,actor_subject,operation_key)
);
-- Rows are immutable. The history also reserves removed section IDs forever
-- within this issue; no parallel ID registry or mutable head is needed.
ALTER FUNCTION tracker_is_issue_purge_child(text,text,jsonb)
 RENAME TO tracker_is_issue_purge_child_before_decomposition;
CREATE FUNCTION tracker_is_issue_purge_child(table_name text,operation text,previous jsonb)
RETURNS boolean LANGUAGE sql VOLATILE AS $$
 SELECT tracker_is_issue_purge_child_before_decomposition(
  CASE WHEN table_name IN ('issue_plan_revisions','issue_plan_operations') THEN 'comments' ELSE table_name END,
  operation,previous);
$$;
CREATE FUNCTION tracker_plan_write_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE data jsonb;
BEGIN
 IF TG_OP='UPDATE' THEN RAISE EXCEPTION 'plan_history_is_immutable' USING ERRCODE='23514'; END IF;
 data:=CASE WHEN TG_OP='DELETE' THEN to_jsonb(OLD) ELSE to_jsonb(NEW) END;
 IF TG_OP='DELETE' THEN
  -- V2 removes children under the existing exact parent-purge proof. Legacy
  -- installation mode retains its already-authorized parent FK cascade.
  IF tracker_is_issue_purge_child(TG_TABLE_NAME,TG_OP,data) THEN RETURN OLD; END IF;
  IF EXISTS(SELECT 1 FROM issues WHERE id=(data->>'issue_id')::uuid) THEN
   RAISE EXCEPTION 'plan_history_is_immutable' USING ERRCODE='23514';
  END IF;
  RETURN OLD;
 END IF;
 RETURN NEW;
END $$;
-- V2 purge must authorize child deletion while the trashed parent still exists.
-- Run after the ordinary issue guards; never mint a new purge scope here.
CREATE FUNCTION tracker_purge_issue_plan() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
 IF (SELECT active FROM tracker_space_admission_state WHERE singleton)
    OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL) THEN
  DELETE FROM issue_plan_operations WHERE issue_id=OLD.id;
  DELETE FROM issue_plan_revisions WHERE issue_id=OLD.id;
 END IF;
 RETURN OLD;
END $$;
CREATE TRIGGER zz_purge_issue_plan BEFORE DELETE ON issues
 FOR EACH ROW EXECUTE FUNCTION tracker_purge_issue_plan();
-- Adopt the existing ordinary-content and exact native-comment task port.
ALTER FUNCTION tracker_authorize_project_content(text,text,jsonb,jsonb)
 RENAME TO tracker_authorize_project_content_before_decomposition;
CREATE FUNCTION tracker_authorize_project_content(table_name text,operation text,previous jsonb,desired jsonb)
RETURNS void LANGUAGE plpgsql VOLATILE AS $$ BEGIN
 IF table_name IN ('issue_plan_revisions','issue_plan_operations') THEN
  PERFORM tracker_authorize_project_content_before_decomposition('comments',operation,previous,desired);
 ELSE
  PERFORM tracker_authorize_project_content_before_decomposition(table_name,operation,previous,desired);
 END IF;
END $$;
DO $$ DECLARE name text; BEGIN
 FOREACH name IN ARRAY ARRAY['issue_plan_revisions','issue_plan_operations'] LOOP
  EXECUTE format('CREATE TRIGGER plan_write_guard BEFORE INSERT OR UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION tracker_plan_write_guard()',name);
  EXECUTE format('CREATE TRIGGER namespace_write_guard BEFORE INSERT OR UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION tracker_namespace_write_guard()',name);
  EXECUTE format('CREATE TRIGGER project_archive_write_guard BEFORE INSERT OR UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION tracker_project_archive_write_guard()',name);
  EXECUTE format('CREATE CONSTRAINT TRIGGER plan_commit_clock AFTER INSERT OR UPDATE OR DELETE ON %I DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION tracker_sdlc_commit_clock()',name);
 END LOOP;
END $$;
