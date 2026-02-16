"use client"

import { Copy, MoreVertical, Edit, Trash2, RotateCcw } from "lucide-react"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { toast } from "sonner"
import { Message } from "@/lib/types"

interface MessageActionsProps {
  message: Message
  onEdit?: (message: Message) => void
  onDelete?: (messageId: string) => void
  onRegenerate?: (messageId: string) => void
}

export function MessageActions({
  message,
  onEdit,
  onDelete,
  onRegenerate,
}: MessageActionsProps) {
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(message.content)
      toast.success("Copied to clipboard")
    } catch (error) {
      toast.error("Failed to copy")
    }
  }

  const handleEdit = () => {
    if (onEdit) {
      onEdit(message)
    }
  }

  const handleDelete = () => {
    if (onDelete) {
      onDelete(message.id)
    }
  }

  const handleRegenerate = () => {
    if (onRegenerate) {
      onRegenerate(message.id)
    }
  }

  return (
    <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
      <button
        onClick={handleCopy}
        className="rounded-md p-1 hover:bg-accent transition-colors"
        aria-label="Copy message"
      >
        <Copy className="h-3.5 w-3.5" />
      </button>

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            className="rounded-md p-1 hover:bg-accent transition-colors"
            aria-label="More actions"
          >
            <MoreVertical className="h-3.5 w-3.5" />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onClick={handleCopy}>
            <Copy className="mr-2 h-4 w-4" />
            Copy
          </DropdownMenuItem>

          {message.role === "user" && onEdit && (
            <DropdownMenuItem onClick={handleEdit}>
              <Edit className="mr-2 h-4 w-4" />
              Edit
            </DropdownMenuItem>
          )}

          {message.role === "assistant" && onRegenerate && (
            <DropdownMenuItem onClick={handleRegenerate}>
              <RotateCcw className="mr-2 h-4 w-4" />
              Regenerate
            </DropdownMenuItem>
          )}

          <DropdownMenuSeparator />

          {onDelete && (
            <DropdownMenuItem
              onClick={handleDelete}
              className="text-destructive focus:text-destructive"
            >
              <Trash2 className="mr-2 h-4 w-4" />
              Delete
            </DropdownMenuItem>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  )
}
