import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

const buttonVariants = cva(
  "group/button inline-flex shrink-0 items-center justify-center border whitespace-nowrap font-semibold tracking-[0.02em] transition-opacity outline-none select-none focus-visible:border-[var(--color-accent)] focus-visible:ring-2 focus-visible:ring-[color-mix(in_oklab,var(--color-accent)_25%,transparent)] active:not-aria-[haspopup]:translate-y-px disabled:pointer-events-none disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-2 aria-invalid:ring-destructive/30 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default:
          "rounded-full border-white/10 bg-white/[0.82] text-[var(--color-button-foreground)] shadow-[inset_0_1px_0_rgba(255,255,255,0.55),0_10px_28px_rgba(0,0,0,0.24)] hover:opacity-85",
        outline:
          "rounded-xl border-white/10 bg-white/[0.03] text-[var(--color-text)] shadow-[inset_0_1px_0_rgba(255,255,255,0.07),0_8px_22px_rgba(0,0,0,0.18)] hover:opacity-80",
        secondary:
          "rounded-full border-white/10 bg-[rgba(85,179,255,0.12)] text-[var(--color-text)] shadow-[inset_0_1px_0_rgba(255,255,255,0.05),0_8px_20px_rgba(0,0,0,0.16)] hover:opacity-80",
        ghost:
          "rounded-full border-transparent bg-transparent text-[var(--color-text-muted)] hover:opacity-70 hover:text-[var(--color-text)]",
        destructive:
          "rounded-full border-[rgba(255,99,99,0.2)] bg-[rgba(255,99,99,0.15)] text-[var(--color-primary)] shadow-[inset_0_1px_0_rgba(255,255,255,0.04),0_8px_20px_rgba(0,0,0,0.18)] hover:opacity-80",
        link: "rounded-md border-transparent bg-transparent px-0 text-[var(--color-accent)] hover:opacity-70 hover:underline",
      },
      size: {
        default:
          "h-10 gap-2 px-4 text-[0.95rem] has-data-[icon=inline-end]:pr-3 has-data-[icon=inline-start]:pl-3",
        xs: "h-7 gap-1 rounded-lg px-2.5 text-[11px] has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2 [&_svg:not([class*='size-'])]:size-3",
        sm: "h-8 gap-1.5 rounded-xl px-3 text-xs has-data-[icon=inline-end]:pr-2.5 has-data-[icon=inline-start]:pl-2.5 [&_svg:not([class*='size-'])]:size-3.5",
        lg: "h-11 gap-2 px-5 text-base has-data-[icon=inline-end]:pr-4 has-data-[icon=inline-start]:pl-4",
        icon: "size-10 rounded-xl",
        "icon-xs":
          "size-7 rounded-lg [&_svg:not([class*='size-'])]:size-3",
        "icon-sm":
          "size-8 rounded-xl",
        "icon-lg": "size-11 rounded-2xl",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Button({
  className,
  variant = "default",
  size = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }) {
  const Comp = asChild ? Slot.Root : "button"

  return (
    <Comp
      data-slot="button"
      data-variant={variant}
      data-size={size}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button, buttonVariants }
