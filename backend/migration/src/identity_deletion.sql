-- This ledger preserves actual local/central bindings across profile deletion.
-- The deployment issuer is bound separately by the owner adapter; email and
-- UUID resemblance are never evidence of a central identity association.
CREATE TABLE tracker_identity_aliases (
    local_user_id uuid PRIMARY KEY,
    kind text NOT NULL DEFAULT 'central' CHECK(kind IN ('local','central')),
    central_subject text,
    recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CHECK((kind='local' AND central_subject IS NULL) OR
        (kind='central' AND central_subject IS NOT NULL AND length(btrim(central_subject)) BETWEEN 1 AND 2048))
);
CREATE INDEX tracker_identity_aliases_subject ON tracker_identity_aliases(central_subject,local_user_id);

INSERT INTO tracker_identity_aliases(local_user_id,kind,central_subject)
SELECT id,CASE WHEN central_sub IS NULL THEN 'local' ELSE 'central' END,central_sub FROM users;

CREATE FUNCTION tracker_identity_alias_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    -- A verified link updates the still-existing local profile first. Moving
    -- its durable alias afterwards preserves history in the same transaction.
    IF TG_OP='UPDATE' AND OLD.kind='local' AND NEW.kind='central'
        AND NEW.local_user_id=OLD.local_user_id
        AND EXISTS(SELECT 1 FROM users WHERE id=NEW.local_user_id AND central_sub=NEW.central_subject)
    THEN RETURN NEW; END IF;
    RAISE EXCEPTION 'identity_alias_immutable' USING ERRCODE='23514';
END;
$$;
CREATE TRIGGER tracker_identity_alias_immutable BEFORE UPDATE OR DELETE
    ON tracker_identity_aliases FOR EACH ROW EXECUTE FUNCTION tracker_identity_alias_immutable();

CREATE FUNCTION tracker_identity_alias_insert_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE current_id uuid;current_subject text;
BEGIN
    SELECT id,central_sub INTO current_id,current_subject FROM users WHERE id=NEW.local_user_id;
    IF current_id IS NOT NULL THEN
        IF (current_subject IS NULL AND (NEW.kind<>'local' OR NEW.central_subject IS NOT NULL))
            OR (current_subject IS NOT NULL AND (NEW.kind<>'central' OR NEW.central_subject IS DISTINCT FROM current_subject)) THEN
            RAISE EXCEPTION 'identity_alias_rebind_denied' USING ERRCODE='23514';
        END IF;
    ELSIF NEW.kind='local' THEN
        RAISE EXCEPTION 'identity_local_profile_unknown' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER tracker_identity_alias_insert_guard BEFORE INSERT
    ON tracker_identity_aliases FOR EACH ROW EXECUTE FUNCTION tracker_identity_alias_insert_guard();

CREATE FUNCTION tracker_identity_profile_binding_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='UPDATE' THEN
        IF OLD.id IS DISTINCT FROM NEW.id THEN
            RAISE EXCEPTION 'identity_local_id_immutable' USING ERRCODE='23514';
        END IF;
        IF OLD.central_sub IS NOT NULL AND NEW.central_sub IS DISTINCT FROM OLD.central_sub THEN
            RAISE EXCEPTION 'identity_alias_rebind_denied' USING ERRCODE='23514';
        END IF;
    END IF;
    IF EXISTS(SELECT 1 FROM tracker_identity_aliases a WHERE a.local_user_id=NEW.id
        AND ((a.kind='central' AND NEW.central_sub IS DISTINCT FROM a.central_subject)
            OR (TG_OP='INSERT' AND a.kind='local' AND NOT EXISTS(SELECT 1 FROM users WHERE id=NEW.id)))) THEN
        RAISE EXCEPTION 'identity_alias_rebind_denied' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER tracker_identity_profile_binding_guard BEFORE INSERT OR UPDATE
    ON users FOR EACH ROW EXECUTE FUNCTION tracker_identity_profile_binding_guard();

CREATE FUNCTION tracker_preserve_identity_alias() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE expected_kind text;
BEGIN
    expected_kind:=CASE WHEN NEW.central_sub IS NULL THEN 'local' ELSE 'central' END;
    IF NEW.central_sub IS NOT NULL THEN
        UPDATE tracker_identity_aliases SET kind='central',central_subject=NEW.central_sub
            WHERE local_user_id=NEW.id AND kind='local';
    END IF;
    INSERT INTO tracker_identity_aliases(local_user_id,kind,central_subject)
        VALUES(NEW.id,expected_kind,NEW.central_sub) ON CONFLICT(local_user_id) DO NOTHING;
    IF EXISTS(SELECT 1 FROM tracker_identity_aliases WHERE local_user_id=NEW.id
        AND (kind<>expected_kind OR central_subject IS DISTINCT FROM NEW.central_sub)) THEN
        RAISE EXCEPTION 'identity_alias_rebind_denied' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER tracker_preserve_identity_alias AFTER INSERT OR UPDATE
    ON users FOR EACH ROW EXECUTE FUNCTION tracker_preserve_identity_alias();
