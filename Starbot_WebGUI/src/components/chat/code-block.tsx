"use client"

import { useState } from "react"
import { Check, Copy } from "lucide-react"
import { cn } from "@/lib/utils"

interface CodeBlockProps {
  children: string
  language?: string
  className?: string
}

export function CodeBlock({ children, language, className }: CodeBlockProps) {
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    await navigator.clipboard.writeText(children)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className="group relative my-4">
      {language && (
        <div className="flex items-center justify-between rounded-t-lg bg-muted px-4 py-2 font-mono text-xs text-muted-foreground">
          <span>{language}</span>
        </div>
      )}
      <div className="relative">
        <pre
          className={cn(
            "overflow-x-auto rounded-b-lg bg-muted p-4",
            !language && "rounded-lg",
            className
          )}
        >
          <code className={language ? `language-${language}` : ""}>
            {children}
          </code>
        </pre>
        <button
          onClick={handleCopy}
          className="absolute right-2 top-2 rounded-md bg-background p-2 opacity-0 transition-opacity hover:bg-accent group-hover:opacity-100"
          aria-label="Copy code"
        >
          {copied ? (
            <Check className="h-4 w-4 text-green-500" />
          ) : (
            <Copy className="h-4 w-4" />
          )}
        </button>
      </div>
    </div>
  )
}
