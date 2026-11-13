import type { LifecycleReadback, SdlcContext, DeliveryInput, SdlcQuestion } from '../sdlc'
import { singleTaskRoutingPolicy } from './routing-policy'

export const lifecycleTask = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'
export const resultId = 'dddddddd-dddd-4ddd-8ddd-dddddddddddd'
const at = '2026-11-12T10:00:00Z'
export const lifecycleContext: SdlcContext = {
  contract_version: 1,
  tracker_instance_id: 'tracker-test',
  project_id: lifecycleTask,
  task_id: lifecycleTask,
  root_task_id: lifecycleTask,
  owner_subject: 'owner',
  stage: 'Development',
  status: 'Ready',
  permissions: { can_answer: false, can_confirm: false },
}
export function lifecycleReadback(): LifecycleReadback {
  return {
    observed_at: at,
    runtime_state: 'queued',
    receipt: {
      task: {
        task_id: lifecycleTask,
        owner_subject: 'owner',
        version: 2,
        stage: 'Development',
        status: 'Ready',
        cycle: 0,
        transient_retries: 0,
        runtime: 'queued',
        routing: {
          contract_version: 2,
          snapshot_id: lifecycleTask,
          tracker_instance_id: 'tracker-test',
          project_id: lifecycleTask,
          task_id: lifecycleTask,
          root_task_id: lifecycleTask,
          confirmation_id: lifecycleTask,
          requirement_revision: 2,
          content_hash: 'a'.repeat(64),
          policy: singleTaskRoutingPolicy(lifecycleTask),
          created_at: at,
        },
      },
      assignment: null,
      lease: null,
      binding: null,
      result: null,
    },
  }
}
export function acceptedResult(): DeliveryInput {
  const artifact = { reference: 'evidence/review.json', sha256: 'b'.repeat(64) }
  return {
    source: 'delivery',
    receipt: {
      result_id: resultId,
      previous_result_id: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
      assignment_id: lifecycleTask,
      task_id: lifecycleTask,
      terminal: {
        chat_id: lifecycleTask,
        run_id: lifecycleTask,
        fleet_receipt: artifact,
        workflow_receipt: artifact,
        output_sha256: 'c'.repeat(64),
      },
      output: { role: 'reviewer', verdict: 'approved', report: artifact },
      previous_stage: 'Review',
      next_stage: 'Testing',
      next_status: 'Ready',
      cycle: 0,
      created_at: at,
    },
  }
}

export function ownerQuestion(): SdlcQuestion {
  return {
    id: lifecycleTask,
    task_id: lifecycleTask,
    root_task_id: lifecycleTask,
    request_id: lifecycleTask,
    version: 4,
    requirement_revision: 2,
    assignment_id: lifecycleTask,
    execution_id: lifecycleTask,
    agent_id: lifecycleTask,
    assignment_version: 1,
    text: 'Which delivery method?',
    rationale: 'Needed to finalize scope.',
    required: true,
    mode: 'text',
    options: [],
    state: 'open',
    author_subject: 'pm',
    created_at: '2026-11-12T10:00:00Z',
  }
}
