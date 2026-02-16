import { Skeleton } from "@/components/ui/skeleton"

export function MessageSkeleton() {
  return (
    <div className="space-y-4 max-w-3xl mx-auto p-4">
      {/* Assistant message skeleton */}
      <div className="flex flex-col gap-1 items-start">
        <Skeleton className="h-3 w-16" />
        <Skeleton className="h-20 w-[70%] rounded-lg" />
      </div>

      {/* User message skeleton */}
      <div className="flex flex-col gap-1 items-end">
        <Skeleton className="h-3 w-12" />
        <Skeleton className="h-16 w-[60%] rounded-lg" />
      </div>

      {/* Assistant message skeleton */}
      <div className="flex flex-col gap-1 items-start">
        <Skeleton className="h-3 w-16" />
        <Skeleton className="h-24 w-[75%] rounded-lg" />
      </div>
    </div>
  )
}
