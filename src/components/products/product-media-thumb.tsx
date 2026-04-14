import { useEffect, useMemo, useState } from "react";
import { productImageSrc } from "@/lib/product-image";
import { cn } from "@/lib/utils";

/** Product cover for lists/chips: remote URL or Tauri ``convertFileSrc`` for local paths. */
export function ProductMediaThumb({
  imageUrl,
  frameClassName,
  imgClassName,
  fallbackClassName,
}: {
  imageUrl: string | null;
  /** Wrapper size/shape, e.g. ``h-9 w-9 rounded`` or ``h-6 w-6 rounded-full`` */
  frameClassName: string;
  imgClassName?: string;
  /** Optional extra classes for the 📦 fallback span */
  fallbackClassName?: string;
}) {
  const [failed, setFailed] = useState(false);
  const src = useMemo(() => productImageSrc(imageUrl), [imageUrl]);

  useEffect(() => {
    setFailed(false);
  }, [imageUrl]);

  if (!src || failed) {
    return (
      <span
        className={cn(
          "flex shrink-0 items-center justify-center bg-muted",
          frameClassName,
          fallbackClassName,
        )}
      >
        📦
      </span>
    );
  }

  return (
    <span className={cn("relative shrink-0 overflow-hidden", frameClassName)}>
      <img
        src={src}
        alt=""
        className={cn("absolute inset-0 h-full w-full object-cover", imgClassName)}
        onError={() => setFailed(true)}
      />
    </span>
  );
}
