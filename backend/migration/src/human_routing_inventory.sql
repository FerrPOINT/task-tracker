-- Human consent is a real frozen-routing origin, independent of a PM review.
-- Keep the payload in its existing immutable owner ledger; the inventory is a projection.
CREATE TABLE task_handoff_routing_generations (
    task_id uuid NOT NULL,
    handoff_id uuid PRIMARY KEY,
    snapshot_id uuid NOT NULL UNIQUE,
    generation bigint NOT NULL CHECK(generation BETWEEN 1 AND 9007199254740991),
    UNIQUE(task_id,generation),
    FOREIGN KEY(task_id,handoff_id) REFERENCES task_handoff_materials(task_id,handoff_id) ON DELETE RESTRICT
);
-- Existing genuine human records gain only a routing index, never an execution or review.
INSERT INTO task_handoff_routing_generations(task_id,handoff_id,snapshot_id,generation)
SELECT m.task_id,m.handoff_id,(m.receipt->>'routing_snapshot_id')::uuid,
    COALESCE((SELECT max(r.generation) FROM sdlc_routing_snapshot_inventory r WHERE r.task_id=m.task_id),0)
        +row_number() OVER(PARTITION BY m.task_id ORDER BY m.created_at,m.handoff_id)
FROM task_handoff_materials m;

CREATE OR REPLACE VIEW sdlc_routing_snapshot_inventory AS
    SELECT snapshot_id,task_id,project_id,policy_version,confirmation_id,requirement_revision,content_hash,payload,
        1::bigint AS generation FROM sdlc_task_routing_snapshots
    UNION ALL SELECT snapshot_id,task_id,project_id,policy_version,confirmation_id,requirement_revision,content_hash,payload,
        generation FROM sdlc_task_routing_generations
    UNION ALL SELECT g.snapshot_id,m.task_id,(m.receipt->>'project_id')::uuid,
        (m.routing->'policy'->>'version')::bigint,m.confirmation_id,m.requirement_revision,m.requirements_sha256,
        m.routing AS payload,g.generation
    FROM task_handoff_routing_generations g JOIN task_handoff_materials m USING(task_id,handoff_id);

CREATE FUNCTION task_handoff_routing_generation_gate() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE material task_handoff_materials; next_generation bigint;
BEGIN
    PERFORM 1 FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT * INTO STRICT material FROM task_handoff_materials WHERE task_id=NEW.task_id AND handoff_id=NEW.handoff_id;
    SELECT COALESCE(max(generation),0)+1 INTO next_generation FROM sdlc_routing_snapshot_inventory WHERE task_id=NEW.task_id;
    IF NEW.snapshot_id::text IS DISTINCT FROM material.receipt->>'routing_snapshot_id'
        OR NEW.generation<>next_generation
        OR EXISTS(SELECT 1 FROM sdlc_routing_snapshot_inventory WHERE snapshot_id=NEW.snapshot_id) THEN
        RAISE EXCEPTION 'human frozen routing identity or generation mismatch' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER task_handoff_routing_generation_gate BEFORE INSERT ON task_handoff_routing_generations
FOR EACH ROW EXECUTE FUNCTION task_handoff_routing_generation_gate();
CREATE TRIGGER task_handoff_routing_generation_immutable BEFORE UPDATE OR DELETE ON task_handoff_routing_generations
FOR EACH ROW EXECUTE FUNCTION sdlc_history_immutable();

CREATE FUNCTION task_handoff_index_frozen_routing() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE next_generation bigint;
BEGIN
    PERFORM 1 FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    SELECT COALESCE(max(generation),0)+1 INTO next_generation FROM sdlc_routing_snapshot_inventory WHERE task_id=NEW.task_id;
    INSERT INTO task_handoff_routing_generations(task_id,handoff_id,snapshot_id,generation)
        VALUES(NEW.task_id,NEW.handoff_id,(NEW.receipt->>'routing_snapshot_id')::uuid,next_generation);
    RETURN NULL;
END $$;
CREATE TRIGGER task_handoff_index_frozen_routing AFTER INSERT ON task_handoff_materials
FOR EACH ROW EXECUTE FUNCTION task_handoff_index_frozen_routing();

-- Old producers retain their guards and serialize on the same owner task row.
-- An initial or critical snapshot must not collide with a human routing generation.
CREATE FUNCTION sdlc_routing_origin_unique() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE candidate_generation bigint;
BEGIN
    PERFORM 1 FROM sdlc_tasks WHERE task_id=NEW.task_id FOR UPDATE;
    candidate_generation:=CASE WHEN TG_TABLE_NAME='sdlc_task_routing_snapshots' THEN 1 ELSE (to_jsonb(NEW)->>'generation')::bigint END;
    IF EXISTS(SELECT 1 FROM sdlc_routing_snapshot_inventory r
        WHERE r.snapshot_id=NEW.snapshot_id OR (r.task_id=NEW.task_id AND r.generation=candidate_generation)) THEN
        RAISE EXCEPTION 'frozen routing origin identity or generation reused' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sdlc_routing_origin_unique BEFORE INSERT ON sdlc_task_routing_snapshots
FOR EACH ROW EXECUTE FUNCTION sdlc_routing_origin_unique();
CREATE TRIGGER sdlc_routing_origin_unique BEFORE INSERT ON sdlc_task_routing_generations
FOR EACH ROW EXECUTE FUNCTION sdlc_routing_origin_unique();
