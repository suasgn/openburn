import { useEffect, useRef } from "react"
import type QrScanner from "qr-scanner"
import { X } from "lucide-react"
import { Button } from "@/components/ui/button"

type QrScannerDialogProps = {
  open: boolean
  onScan: (content: string) => void
  onClose: () => void
  onError: (error: unknown) => void
}

export function QrScannerDialog({ open, onScan, onClose, onError }: QrScannerDialogProps) {
  const videoRef = useRef<HTMLVideoElement>(null)
  const onScanRef = useRef(onScan)
  const onErrorRef = useRef(onError)
  onScanRef.current = onScan
  onErrorRef.current = onError

  useEffect(() => {
    if (!open || !videoRef.current) return

    let cancelled = false
    let scanner: QrScanner | null = null

    const startScanner = async () => {
      try {
        const { default: QrScanner } = await import("qr-scanner")
        if (cancelled || !videoRef.current) return

        scanner = new QrScanner(
          videoRef.current,
          (result) => {
            scanner?.stop()
            onScanRef.current(result.data)
          },
          {
            returnDetailedScanResult: true,
            preferredCamera: "environment",
            highlightScanRegion: true,
            highlightCodeOutline: true,
            onDecodeError: () => {},
          },
        )

        await scanner.start()
      } catch (error) {
        if (!cancelled) onErrorRef.current(error)
      }
    }

    void startScanner()
    return () => {
      cancelled = true
      scanner?.destroy()
    }
  }, [open])

  if (!open) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div role="dialog" aria-label="Scan account transfer QR code" className="w-full max-w-sm rounded-lg border bg-background p-4 shadow-xl">
        <div className="flex items-start justify-between gap-2">
          <div>
            <h4 className="font-semibold">Scan account transfer</h4>
            <p className="text-xs text-muted-foreground">Point the camera at the QR code on the other device.</p>
          </div>
          <Button type="button" variant="ghost" size="icon-xs" onClick={onClose} aria-label="Cancel QR scan">
            <X className="size-4" />
          </Button>
        </div>
        <video ref={videoRef} className="mt-3 aspect-square w-full rounded-md bg-black object-cover" muted playsInline />
        <Button type="button" variant="outline" size="xs" className="mt-3 w-full" onClick={onClose}>
          Cancel
        </Button>
      </div>
    </div>
  )
}
