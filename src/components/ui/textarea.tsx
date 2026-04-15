import * as React from "react"

import { cn } from "@/lib/utils"

function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "flex min-h-24 w-full rounded-xl border border-white/8 bg-[rgb(7_8_10_/_0.9)] px-3.5 py-2 text-[15px] font-medium tracking-[0.02em] text-[var(--color-text)] shadow-[inset_0_1px_0_rgba(255,255,255,0.04)] transition-[border-color,box-shadow,opacity] outline-none placeholder:text-[var(--color-text-muted)] focus-visible:border-[var(--color-accent)] focus-visible:ring-2 focus-visible:ring-[color-mix(in_oklab,var(--color-accent)_18%,transparent)] disabled:pointer-events-none disabled:cursor-not-allowed disabled:bg-white/[0.04] disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-2 aria-invalid:ring-destructive/25 md:text-sm",
        className,
      )}
      {...props}
    />
  )
}

export { Textarea }
