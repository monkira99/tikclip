import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "group/badge inline-flex h-6 w-fit shrink-0 items-center justify-center gap-1 overflow-hidden rounded-md border px-2.5 py-0.5 text-[11px] font-semibold tracking-[0.04em] uppercase whitespace-nowrap transition-opacity focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2 aria-invalid:border-destructive aria-invalid:ring-destructive/20 [&>svg]:pointer-events-none [&>svg]:size-3!",
  {
    variants: {
      variant: {
        default:
          "border-white/8 bg-[var(--color-bg-subtle)] text-[var(--color-text)] [a]:hover:opacity-80",
        secondary:
          "border-white/8 bg-white/[0.04] text-[var(--color-text-soft)] [a]:hover:opacity-80",
        destructive:
          "border-[rgba(255,99,99,0.2)] bg-[rgba(255,99,99,0.15)] text-[var(--color-primary)] focus-visible:ring-destructive/20 [a]:hover:opacity-80",
        outline:
          "border-white/10 bg-transparent text-[var(--color-text-muted)] [a]:hover:opacity-80",
        ghost:
          "border-transparent bg-transparent text-[var(--color-text-muted)] hover:opacity-70",
        link: "border-transparent bg-transparent px-0 text-[var(--color-accent)] hover:underline",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

function Badge({
  className,
  variant = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"span"> &
  VariantProps<typeof badgeVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot.Root : "span"

  return (
    <Comp
      data-slot="badge"
      data-variant={variant}
      className={cn(badgeVariants({ variant }), className)}
      {...props}
    />
  )
}

export { Badge, badgeVariants }
