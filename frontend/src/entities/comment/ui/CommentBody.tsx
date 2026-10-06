import Markdown from 'react-markdown'

export function CommentBody({ body }: { body: string }) {
  return (
    <div className="min-w-0 break-words text-sm leading-6 text-text-secondary [&_h1]:text-xl [&_h2]:text-lg [&_h3]:text-base [&_h1]:font-semibold [&_h2]:font-semibold [&_h3]:font-semibold [&_h4]:font-semibold [&_h5]:font-semibold [&_h6]:font-semibold [&_p]:my-2 [&_p]:whitespace-pre-line [&_li]:whitespace-pre-line [&_ul]:list-disc [&_ul]:pl-6 [&_ol]:list-decimal [&_ol]:pl-6 [&_a]:text-accent [&_a]:underline [&_blockquote]:border-l-2 [&_blockquote]:border-border [&_blockquote]:pl-4 [&_pre]:overflow-x-auto [&_pre]:rounded-md [&_pre]:bg-surface-raised [&_pre]:p-3 [&_code]:font-mono [&_img]:max-w-full">
      <Markdown skipHtml>{body}</Markdown>
    </div>
  )
}
