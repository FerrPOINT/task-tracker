import Markdown, { type Components } from 'react-markdown'
import { CodeBlock, Prose } from '@pdlc/ui/ui'

const markdownComponents: Components = {
  pre: ({ children }) => <CodeBlock wrap={false}>{children}</CodeBlock>,
}

export function CommentBody({ body }: { body: string }) {
  return (
    <Prose preserveLineBreaks>
      <Markdown skipHtml components={markdownComponents}>
        {body}
      </Markdown>
    </Prose>
  )
}
