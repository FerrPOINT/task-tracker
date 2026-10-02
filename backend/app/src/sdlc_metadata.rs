use crate::sdlc::{canonical_hash, pm_draft_input_hash};
use domain::{sdlc::*, sdlc_metadata::*};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use shared::AppError;
use uuid::Uuid;

type ProjectionResult<T> = Result<T, MetadataErrorCode>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourcePayload {
    contract_version: u8,
    tracker_instance_id: String,
    project_id: Uuid,
    task_id: Uuid,
    root_task_id: Uuid,
    owner_subject: String,
    stage: Stage,
    requirement_revision: Option<i64>,
    result: Value,
}

pub struct MetadataReferences {
    pub input: Option<PmDraftInput>,
    pub question: Option<Question>,
    pub requirement_hash: Option<String>,
}

fn valid(condition: bool) -> ProjectionResult<()> {
    if condition {
        Ok(())
    } else {
        Err(MetadataErrorCode::MetadataSourceInvalid)
    }
}

fn decode<T: DeserializeOwned>(value: &Value) -> ProjectionResult<T> {
    serde_json::from_value(value.clone()).map_err(|_| MetadataErrorCode::MetadataSourceInvalid)
}

fn value<T: Serialize>(data: T) -> ProjectionResult<Value> {
    serde_json::to_value(data).map_err(|_| MetadataErrorCode::MetadataSourceInvalid)
}

fn version(number: i64) -> ProjectionResult<()> {
    valid((1..=MAX_SAFE_VERSION).contains(&number))
}

fn hash(text: &str) -> ProjectionResult<()> {
    valid(
        text.len() == 64
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
    )
}

fn fence(q: &Question) -> ProjectionResult<MachineFence> {
    valid(!q.assignment_id.is_nil() && !q.execution_id.is_nil() && !q.agent_id.is_nil())?;
    version(q.assignment_version)?;
    Ok(MachineFence {
        assignment_id: q.assignment_id,
        execution_id: q.execution_id,
        agent_id: q.agent_id,
        assignment_version: q.assignment_version,
    })
}

fn question(
    q: &Question,
    source: &SourcePayload,
    refs: &MetadataReferences,
) -> ProjectionResult<()> {
    valid(!q.id.is_nil() && !q.request_id.is_nil() && !q.checkpoint_id.is_nil())?;
    valid(q.task_id == source.task_id && q.root_task_id == source.root_task_id)?;
    version(q.version)?;
    version(q.requirement_revision)?;
    valid(refs.requirement_hash.is_some())?;
    fence(q)?;
    Ok(())
}

fn revision(number: i64, content_hash: &str, refs: &MetadataReferences) -> ProjectionResult<()> {
    version(number)?;
    hash(content_hash)?;
    valid(refs.requirement_hash.as_deref() == Some(content_hash))
}

pub fn project(event: &OutboxEvent, refs: MetadataReferences) -> ProjectionResult<MetadataEvent> {
    let source: SourcePayload = decode(&event.payload)?;
    valid(source.contract_version == 1 && event.sequence > 0)?;
    valid(!event.event_id.is_nil() && !event.task_id.is_nil())?;
    valid(
        source.task_id == event.task_id
            && !source.project_id.is_nil()
            && !source.root_task_id.is_nil(),
    )?;
    valid(
        !source.tracker_instance_id.trim().is_empty() && !source.owner_subject.trim().is_empty(),
    )?;
    if let Some(number) = source.requirement_revision {
        version(number)?;
    }
    let resource = match event.event_type.as_str() {
        "task.created" => {
            let created: CreatedDraft = decode(&source.result)?;
            valid(
                created.task_id == event.task_id
                    && created.root_task_id == event.task_id
                    && created.project_id == source.project_id
                    && created.tracker_instance_id == source.tracker_instance_id
                    && created.owner_subject == source.owner_subject
                    && source.root_task_id == event.task_id
                    && matches!(source.stage, Stage::Draft)
                    && source.requirement_revision.is_none(),
            )?;
            let input = if let Some(input) = refs.input {
                valid(!input.snapshot_ref.is_nil())?;
                hash(&input.sha256)?;
                valid(
                    pm_draft_input_hash(&input.title, &input.description)
                        .map_err(|_| MetadataErrorCode::MetadataSourceInvalid)?
                        == input.sha256,
                )?;
                Some(MetadataInputRef {
                    snapshot_ref: input.snapshot_ref,
                    sha256: input.sha256,
                })
            } else {
                None
            };
            value(CreatedResource { input })?
        }
        "task.bound" => {
            let state: TaskState = decode(&source.result)?;
            valid(
                state.task_id == event.task_id
                    && state.project_id == source.project_id
                    && state.root_task_id == source.root_task_id
                    && state.tracker_instance_id == source.tracker_instance_id
                    && state.owner_subject == source.owner_subject
                    && matches!(state.stage, Stage::Draft)
                    && matches!(source.stage, Stage::Draft)
                    && state.current_revision().is_none()
                    && source.requirement_revision.is_none(),
            )?;
            value(BoundResource {})?
        }
        "pm.assigned" => {
            let assignment: PmAssignment = decode(&source.result)?;
            valid(
                !assignment.assignment_id.is_nil()
                    && !assignment.execution_id.is_nil()
                    && !assignment.agent_id.is_nil()
                    && !assignment.machine_subject.trim().is_empty(),
            )?;
            version(assignment.version)?;
            value(MachineFence {
                assignment_id: assignment.assignment_id,
                execution_id: assignment.execution_id,
                agent_id: assignment.agent_id,
                assignment_version: assignment.version,
            })?
        }
        "clarification.published" | "clarification.cancelled" => {
            let q: Question = decode(&source.result)?;
            question(&q, &source, &refs)?;
            let fence = fence(&q)?;
            if event.event_type == "clarification.published" {
                valid(
                    q.state == QuestionState::Open
                        && q.answer.is_none()
                        && source.requirement_revision == Some(q.requirement_revision),
                )?;
                value(QuestionResource {
                    question_id: q.id,
                    question_version: q.version,
                    request_id: q.request_id,
                    checkpoint_id: q.checkpoint_id,
                    requirement_revision: q.requirement_revision,
                    state: PublishedQuestionState::Open,
                    fence,
                })?
            } else {
                valid(q.state == QuestionState::Cancelled && q.answer.is_none())?;
                value(QuestionResource {
                    question_id: q.id,
                    question_version: q.version,
                    request_id: q.request_id,
                    checkpoint_id: q.checkpoint_id,
                    requirement_revision: q.requirement_revision,
                    state: CancelledQuestionState::Cancelled,
                    fence,
                })?
            }
        }
        "clarification.answered" => {
            let answer: Answer = decode(&source.result)?;
            valid(!answer.id.is_nil() && !answer.question_id.is_nil())?;
            version(answer.question_version)?;
            version(answer.requirement_revision)?;
            let q = refs
                .question
                .as_ref()
                .ok_or(MetadataErrorCode::MetadataSourceInvalid)?;
            question(q, &source, &refs)?;
            valid(
                q.id == answer.question_id
                    && q.version == answer.question_version
                    && q.requirement_revision == answer.requirement_revision
                    && source.requirement_revision == Some(answer.requirement_revision)
                    && answer.author_subject == source.owner_subject,
            )?;
            value(AnswerResource {
                answer_id: answer.id,
                question_id: answer.question_id,
                question_version: answer.question_version,
                request_id: q.request_id,
                checkpoint_id: q.checkpoint_id,
                requirement_revision: answer.requirement_revision,
                fence: fence(q)?,
            })?
        }
        "requirements.published" => {
            let r: RequirementsRevision = decode(&source.result)?;
            revision(r.revision, &r.content_hash, &refs)?;
            valid(
                source.requirement_revision == Some(r.revision)
                    && canonical_hash(&r.document)
                        .map_err(|_| MetadataErrorCode::MetadataSourceInvalid)?
                        == r.content_hash,
            )?;
            value(RevisionResource {
                requirement_revision: r.revision,
                content_hash: r.content_hash,
            })?
        }
        "requirements.evidence_recorded" => {
            let evidence: Evidence = decode(&source.result)?;
            revision(evidence.requirement_revision, &evidence.content_hash, &refs)?;
            valid(
                source.requirement_revision == Some(evidence.requirement_revision)
                    && !evidence.check_id.trim().is_empty(),
            )?;
            value(EvidenceResource {
                requirement_revision: evidence.requirement_revision,
                content_hash: evidence.content_hash,
                check_id_sha256: canonical_hash(&evidence.check_id)
                    .map_err(|_| MetadataErrorCode::MetadataSourceInvalid)?,
            })?
        }
        "requirements.confirmed" => {
            let confirmation: Confirmation = decode(&source.result)?;
            revision(confirmation.revision, &confirmation.content_hash, &refs)?;
            valid(
                !confirmation.id.is_nil()
                    && confirmation.task_id == event.task_id
                    && confirmation.owner_subject == source.owner_subject
                    && matches!(confirmation.stage, Stage::Backlog)
                    && matches!(source.stage, Stage::Backlog)
                    && source.requirement_revision == Some(confirmation.revision),
            )?;
            value(ConfirmationResource {
                confirmation_id: confirmation.id,
                requirement_revision: confirmation.revision,
                content_hash: confirmation.content_hash,
            })?
        }
        _ => return Err(MetadataErrorCode::MetadataSourceInvalid),
    };
    let mut projected = json!({"sequence":event.sequence.to_string(),"event_id":event.event_id,
        "task_id":event.task_id,"event_type":event.event_type,
        "created_at":event.created_at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        "payload":{"tracker_instance_id":source.tracker_instance_id,"project_id":source.project_id,
            "root_task_id":source.root_task_id,"owner_subject":source.owner_subject,
            "stage":source.stage,"current_requirement_revision":source.requirement_revision,
            "resource":resource}});
    let digest =
        canonical_hash(&json!({"contract_version":1,"projection":"metadata_v1","event":projected}))
            .map_err(|_| MetadataErrorCode::MetadataSourceInvalid)?;
    projected["metadata_sha256"] = Value::String(digest);
    decode(&projected)
}

pub struct MetadataWireResponse {
    pub status: u16,
    pub bytes: Vec<u8>,
}

pub fn serialize_page(
    after: i64,
    limit: u16,
    max_bytes: usize,
    candidates: Vec<MetadataCandidate>,
) -> Result<MetadataWireResponse, AppError> {
    let mut page = MetadataPage {
        contract_version: 1,
        projection: MetadataProjection::MetadataV1,
        after: after.to_string(),
        next_after: after.to_string(),
        has_more: !candidates.is_empty(),
        events: vec![],
    };
    let total = candidates.len();
    let mut bytes = serde_json::to_vec(&page).map_err(AppError::internal)?;
    for candidate in candidates.into_iter().take(usize::from(limit)) {
        let event = match candidate.event {
            Ok(event) => event,
            Err(code) => {
                if !page.events.is_empty() {
                    break;
                }
                return serialize_error(
                    after,
                    &candidate.sequence.to_string(),
                    candidate.event_id,
                    code,
                    None,
                    max_bytes,
                );
            }
        };
        let previous_cursor = page.next_after.clone();
        page.next_after = event.sequence().to_owned();
        page.events.push(event);
        page.has_more = page.events.len() < total;
        let candidate_bytes = serde_json::to_vec(&page).map_err(AppError::internal)?;
        if candidate_bytes.len() > max_bytes {
            if page.events.len() == 1 {
                let required = candidate_bytes.len();
                let code = if required <= METADATA_MAX_BYTES {
                    MetadataErrorCode::MetadataBudgetTooSmall
                } else {
                    MetadataErrorCode::MetadataEventUnrepresentable
                };
                return serialize_error(
                    after,
                    &candidate.sequence.to_string(),
                    candidate.event_id,
                    code,
                    Some(required as u64),
                    max_bytes,
                );
            }
            page.events.pop();
            page.next_after = previous_cursor;
            break;
        }
        bytes = candidate_bytes;
    }
    Ok(MetadataWireResponse { status: 200, bytes })
}

fn serialize_error(
    after: i64,
    blocked_sequence: &str,
    event_id: Uuid,
    code: MetadataErrorCode,
    required_bytes: Option<u64>,
    max_bytes: usize,
) -> Result<MetadataWireResponse, AppError> {
    let error = MetadataError {
        contract_version: 1,
        projection: MetadataProjection::MetadataV1,
        code,
        after: after.to_string(),
        blocked_sequence: blocked_sequence.into(),
        event_id,
        required_bytes,
        max_bytes: max_bytes as u32,
    };
    let bytes = serde_json::to_vec(&error).map_err(AppError::internal)?;
    if bytes.len() > METADATA_MIN_BYTES {
        return Err(AppError::internal("metadata error serialization bound"));
    }
    Ok(MetadataWireResponse {
        status: if matches!(code, MetadataErrorCode::MetadataBudgetTooSmall) {
            422
        } else {
            409
        },
        bytes,
    })
}

impl crate::sdlc::SdlcService {
    pub async fn metadata_outbox(
        &self,
        task: Uuid,
        actor: &Principal,
        after: i64,
        limit: u16,
        max_bytes: usize,
    ) -> Result<MetadataWireResponse, AppError> {
        serialize_page(
            after,
            limit,
            max_bytes,
            self.repository
                .metadata_outbox(task, actor, after, limit)
                .await?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(sequence: i64, owner: &str) -> OutboxEvent {
        let task = Uuid::from_u128(1);
        let state = TaskState {
            tracker_instance_id: "test".into(),
            project_id: Uuid::from_u128(2),
            task_id: task,
            root_task_id: task,
            owner_subject: owner.into(),
            stage: Stage::Draft,
            confirmation_revision: None,
            assignment: None,
            questions: vec![],
            revisions: vec![],
            confirmations: vec![],
            evidence: vec![],
        };
        OutboxEvent {
            sequence,
            event_id: Uuid::from_u128(sequence as u128 + 10),
            task_id: task,
            event_type: "task.bound".into(),
            created_at: "2026-10-02T01:02:03.123456Z".parse().unwrap(),
            payload: json!({"contract_version":1,"tracker_instance_id":"test","project_id":state.project_id,
                "task_id":task,"root_task_id":task,"owner_subject":owner,"stage":"Draft",
                "requirement_revision":null,"result":state}),
        }
    }

    fn refs() -> MetadataReferences {
        MetadataReferences {
            input: None,
            question: None,
            requirement_hash: None,
        }
    }

    fn candidate(sequence: i64, owner: &str) -> MetadataCandidate {
        let event = source(sequence, owner);
        MetadataCandidate {
            sequence,
            event_id: event.event_id,
            event: project(&event, refs()),
        }
    }

    fn body(wire: &MetadataWireResponse) -> Value {
        serde_json::from_slice(&wire.bytes).unwrap()
    }

    #[test]
    fn hash_uses_canonical_value_and_persisted_utc_timestamp() {
        let event = source(i64::MAX, "owner\r\n\"\\\t\u{1f680} e\u{301}");
        let first = value(project(&event, refs()).unwrap()).unwrap();
        assert_eq!(first["sequence"], i64::MAX.to_string());
        assert_eq!(first["created_at"], "2026-10-02T01:02:03.123456000Z");
        let mut without_digest = first.clone();
        let digest = without_digest
            .as_object_mut()
            .unwrap()
            .remove("metadata_sha256")
            .unwrap();
        assert_eq!(
            digest,
            canonical_hash(
                &json!({"contract_version":1,"projection":"metadata_v1","event":without_digest})
            )
            .unwrap()
        );
        let pretty: Value =
            serde_json::from_str(&serde_json::to_string_pretty(&event.payload).unwrap()).unwrap();
        assert_eq!(
            first,
            value(
                project(
                    &OutboxEvent {
                        payload: pretty,
                        ..event
                    },
                    refs()
                )
                .unwrap()
            )
            .unwrap()
        );
        assert!(first["payload"].get("result").is_none());
        assert_eq!(first["payload"]["resource"], json!({}));
        let mut unknown = first.clone();
        unknown["payload"]["resource"]["text"] = json!("secret");
        assert!(serde_json::from_value::<MetadataEvent>(unknown).is_err());
        let mut other = first;
        other["event_type"] = json!("unknown");
        assert!(serde_json::from_value::<MetadataEvent>(other).is_err());
    }

    #[test]
    fn exact_full_envelope_utf8_escaping_budget_and_contiguous_prefix() {
        let owner = "\u{416}\u{1f680}\"\\\r\n\t\u{1}".repeat(70);
        let full = serialize_page(
            0,
            100,
            METADATA_MAX_BYTES,
            vec![candidate(2, &owner), candidate(8, &owner)],
        )
        .unwrap();
        let budget = full.bytes.len();
        assert!(budget > 1024);
        for bound in [budget - 1, budget, budget + 1] {
            let wire = serialize_page(
                0,
                100,
                bound,
                vec![candidate(2, &owner), candidate(8, &owner)],
            )
            .unwrap();
            assert_eq!(wire.status, 200);
            assert!(wire.bytes.len() <= bound);
            let value = body(&wire);
            assert_eq!(
                value["events"].as_array().unwrap().len(),
                if bound < budget { 1 } else { 2 }
            );
            assert_eq!(value["has_more"], bound < budget);
            assert_eq!(value["next_after"], if bound < budget { "2" } else { "8" });
            if bound >= budget {
                assert_eq!(wire.bytes, full.bytes);
            }
        }
        let one = serialize_page(
            0,
            1,
            METADATA_MAX_BYTES,
            vec![candidate(2, &owner), candidate(8, &owner)],
        )
        .unwrap();
        assert_eq!(body(&one)["has_more"], true);
        let empty = serialize_page(i64::MAX, 100, 1024, vec![]).unwrap();
        assert_eq!(body(&empty)["next_after"], i64::MAX.to_string());
        assert_eq!(body(&empty)["has_more"], false);
    }

    #[test]
    fn first_and_middle_oversize_or_corrupt_events_never_advance_past_blocker() {
        let large = "\"\u{1f680}".repeat(400);
        let too_small = serialize_page(0, 100, 1024, vec![candidate(3, &large)]).unwrap();
        assert_eq!(too_small.status, 422);
        assert_eq!(body(&too_small)["code"], "metadata_budget_too_small");
        assert_eq!(body(&too_small)["after"], "0");
        assert_eq!(body(&too_small)["blocked_sequence"], "3");
        assert!(too_small.bytes.len() <= 1024);
        let huge = "x".repeat(METADATA_MAX_BYTES);
        let impossible =
            serialize_page(0, 100, METADATA_MAX_BYTES, vec![candidate(3, &huge)]).unwrap();
        assert_eq!(impossible.status, 409);
        assert_eq!(body(&impossible)["code"], "metadata_event_unrepresentable");
        let corrupt = || MetadataCandidate {
            sequence: 8,
            event_id: Uuid::from_u128(18),
            event: Err(MetadataErrorCode::MetadataSourceInvalid),
        };
        for second in [corrupt(), candidate(8, &huge)] {
            let prefix = serialize_page(
                0,
                100,
                1024,
                vec![candidate(2, "owner"), second, candidate(9, "owner")],
            )
            .unwrap();
            assert_eq!(body(&prefix)["next_after"], "2");
            assert_eq!(body(&prefix)["has_more"], true);
            assert_eq!(body(&prefix)["events"].as_array().unwrap().len(), 1);
        }
        let blocked = serialize_page(2, 100, 1024, vec![corrupt(), candidate(9, "owner")]).unwrap();
        assert_eq!(blocked.status, 409);
        assert_eq!(body(&blocked)["after"], "2");
        assert_eq!(body(&blocked)["code"], "metadata_source_invalid");
    }

    #[test]
    fn unknown_nil_partial_and_conflicting_sources_are_static_errors() {
        for mutate in 0..5 {
            let mut event = source(1, "owner");
            match mutate {
                0 => event.event_type = "unbounded-secret-type".into(),
                1 => event.event_id = Uuid::nil(),
                2 => event.payload["root_task_id"] = json!(Uuid::nil()),
                3 => {
                    event
                        .payload
                        .as_object_mut()
                        .unwrap()
                        .remove("owner_subject");
                }
                _ => event.payload["result"]["project_id"] = json!(Uuid::from_u128(99)),
            }
            assert!(matches!(
                project(&event, refs()),
                Err(MetadataErrorCode::MetadataSourceInvalid)
            ));
        }
    }

    #[test]
    fn immutable_input_refs_reject_nil_or_conflicting_hash_without_content_disclosure() {
        let mut event = source(1, "owner");
        event.event_type = "task.created".into();
        event.payload["result"] = json!({"tracker_instance_id":"test","project_id":Uuid::from_u128(2),
            "task_id":event.task_id,"root_task_id":event.task_id,"owner_subject":"owner",
            "task_key":"TEST-1","stage":"Draft"});
        let title = "original \u{416}\u{1f680}";
        let description = "private\r\n\"\\ e\u{301}";
        let input = PmDraftInput {
            snapshot_ref: Uuid::from_u128(3),
            title: title.into(),
            description: description.into(),
            sha256: pm_draft_input_hash(title, description).unwrap(),
        };
        let projected = value(
            project(
                &event,
                MetadataReferences {
                    input: Some(input.clone()),
                    ..refs()
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            projected["payload"]["resource"]["input"],
            json!({"snapshot_ref":input.snapshot_ref,"sha256":input.sha256})
        );
        assert!(
            !serde_json::to_string(&projected)
                .unwrap()
                .contains("private")
        );
        assert_eq!(
            value(project(&event, refs()).unwrap()).unwrap()["payload"]["resource"]["input"],
            Value::Null
        );
        for mut bad in [input.clone(), input] {
            if bad.snapshot_ref.is_nil() {
                unreachable!();
            }
            bad.snapshot_ref = Uuid::nil();
            assert!(matches!(
                project(
                    &event,
                    MetadataReferences {
                        input: Some(bad.clone()),
                        ..refs()
                    }
                ),
                Err(MetadataErrorCode::MetadataSourceInvalid)
            ));
            bad.snapshot_ref = Uuid::from_u128(3);
            bad.sha256 = "a".repeat(64);
            assert!(matches!(
                project(
                    &event,
                    MetadataReferences {
                        input: Some(bad),
                        ..refs()
                    }
                ),
                Err(MetadataErrorCode::MetadataSourceInvalid)
            ));
        }
    }
}
