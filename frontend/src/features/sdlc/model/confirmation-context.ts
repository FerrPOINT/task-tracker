import type { SdlcContext } from '@/api/sdlc'

export function isRequirementsReviewContext(context: SdlcContext): boolean {
  return (
    context.stage === 'Clarification' ||
    (context.stage === 'Draft' &&
      (context.draft_submode === 'awaiting_confirmation' ||
        context.draft_submode === 'clarification' ||
        // Tracker also projects legacy Clarification as Draft without a submode.
        (context.draft_submode == null && context.permissions.can_confirm)))
  )
}

export function canConfirmRequirementsContext(context?: SdlcContext): boolean {
  // Tracker's owner(actor) && ready() is authoritative for questions/evidence.
  return Boolean(context?.permissions.can_confirm && isRequirementsReviewContext(context))
}
