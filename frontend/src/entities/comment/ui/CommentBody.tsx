import Markdown from 'react-markdown'
import { Prose } from '@pdlc/ui/ui'

export function CommentBody({ body }: { body: string }) {
  return (
    <Prose preserveLineBreaks>
      <Markdown skipHtml>{body}</Markdown>
    </Prose>
  )
}
