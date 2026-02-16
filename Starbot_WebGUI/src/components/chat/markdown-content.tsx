"use client"

import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeHighlight from "rehype-highlight"
import rehypeRaw from "rehype-raw"
import { CodeBlock } from "./code-block"
import { cn } from "@/lib/utils"

interface MarkdownContentProps {
  content: string
  className?: string
}

export function MarkdownContent({ content, className }: MarkdownContentProps) {
  return (
    <div
      className={cn(
        "prose prose-sm dark:prose-invert max-w-none",
        "prose-p:leading-relaxed prose-pre:p-0 prose-pre:bg-transparent",
        "prose-headings:font-semibold prose-headings:tracking-tight",
        "prose-a:text-primary prose-a:no-underline hover:prose-a:underline",
        "prose-code:rounded prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:font-mono prose-code:text-sm prose-code:before:content-[''] prose-code:after:content-['']",
        "prose-img:rounded-lg prose-img:shadow-md",
        "prose-hr:border-border",
        "prose-blockquote:border-l-primary prose-blockquote:font-normal prose-blockquote:not-italic",
        "prose-table:border-border prose-th:border-border prose-td:border-border",
        className
      )}
    >
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeHighlight, rehypeRaw]}
        components={{
        code: (props: any) => {
          const { node, inline, className, children, ...rest } = props
          const match = /language-(\w+)/.exec(className || "")
          const language = match ? match[1] : undefined

          if (!inline && language) {
            return (
              <CodeBlock language={language}>
                {String(children).replace(/\n$/, "")}
              </CodeBlock>
            )
          }

          return (
            <code className={className} {...rest}>
              {children}
            </code>
          )
        },
        a: (props: any) => {
          const { node, children, href, ...rest } = props
          return (
            <a
              href={href}
              target="_blank"
              rel="noopener noreferrer"
              {...rest}
            >
              {children}
            </a>
          )
        },
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  )
}
