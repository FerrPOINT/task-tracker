import { describe, expect, it } from 'vitest'
import type { SdlcContext } from '@/api/sdlc'
import { canConfirmRequirementsContext } from './confirmation-context'

const context: SdlcContext = {
  contract_version: 1,
  tracker_instance_id: 'tracker-test',
  project_id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  task_id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
  root_task_id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
  owner_subject: 'central-owner-not-local-user-id',
  stage: 'Draft',
  draft_submode: 'awaiting_confirmation',
  requirement_revision: 2,
  permissions: { can_answer: false, can_confirm: true },
}

describe('Tracker owner confirmation context', () => {
  it.each([
    ['legacy Clarification', 'Clarification', undefined, true, true],
    ['projected legacy', 'Draft', undefined, true, true],
    ['projected legacy null', 'Draft', null, true, true],
    ['awaiting owner', 'Draft', 'awaiting_confirmation', true, true],
    ['ready clarification', 'Draft', 'clarification', true, true],
    ['required open question', 'Draft', 'clarification', false, false],
    ['read-only review', 'Draft', 'awaiting_confirmation', false, false],
    ['read-only legacy', 'Clarification', undefined, false, false],
    ['brainstorming', 'Draft', 'brainstorming', true, false],
    ['unready missing mode', 'Draft', undefined, false, false],
    ['technical stage', 'Analysis', undefined, true, false],
  ] satisfies [string, SdlcContext['stage'], SdlcContext['draft_submode'], boolean, boolean][])(
    '%s keeps server permission and exact review stage semantics',
    (_name, stage, draft_submode, can_confirm, expected) => {
      expect(
        canConfirmRequirementsContext({
          ...context,
          stage,
          draft_submode,
          permissions: { can_confirm, can_answer: draft_submode === 'clarification' },
        }),
      ).toBe(expected)
    },
  )
  it('does not infer readiness from status or waiting reason, or a missing context', () => {
    expect(canConfirmRequirementsContext()).toBe(false)
    expect(
      canConfirmRequirementsContext({
        ...context,
        status: 'InProgress',
        waiting_reason: 'waiting_for_owner_answers',
        permissions: { can_answer: true, can_confirm: true },
      }),
    ).toBe(true)
    expect(
      canConfirmRequirementsContext({
        ...context,
        status: 'Ready',
        waiting_reason: 'waiting_for_owner_confirmation',
        permissions: { can_answer: false, can_confirm: false },
      }),
    ).toBe(false)
  })
})
