"use client"

import { useEffect, useState } from "react"
import { toast } from "sonner"

export function NetworkStatus() {
  const [isOnline, setIsOnline] = useState(true)

  useEffect(() => {
    const handleOnline = () => {
      setIsOnline(true)
      toast.success("Connection restored")
    }

    const handleOffline = () => {
      setIsOnline(false)
      toast.error("No internet connection")
    }

    window.addEventListener("online", handleOnline)
    window.addEventListener("offline", handleOffline)

    return () => {
      window.removeEventListener("online", handleOnline)
      window.removeEventListener("offline", handleOffline)
    }
  }, [])

  return null
}
