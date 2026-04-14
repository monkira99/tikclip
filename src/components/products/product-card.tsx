import { useState } from "react";
import { Pencil, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { deleteProduct, deleteProductEmbeddings, getSidecarBaseUrl } from "@/lib/api";
import { formatInvokeError } from "@/lib/invoke-error";
import { productImageSrc } from "@/lib/product-image";
import { useProductStore } from "@/stores/product-store";
import type { Product } from "@/types";
import { cn } from "@/lib/utils";

function formatPrice(price: number | null): string {
  if (price == null || Number.isNaN(price)) {
    return "—";
  }
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 }).format(price);
}

export function ProductCard({
  product,
  onEdit,
}: {
  product: Product;
  onEdit: (p: Product) => void;
}) {
  const fetchProducts = useProductStore((s) => s.fetchProducts);
  const [imgOk, setImgOk] = useState(true);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const displaySrc = productImageSrc(product.image_url);

  const runDelete = async () => {
    setDeleting(true);
    try {
      await deleteProduct(product.id);
      if (getSidecarBaseUrl()) {
        void deleteProductEmbeddings(product.id).catch(() => {
          /* vector store optional */
        });
      }
      setConfirmOpen(false);
      void fetchProducts();
    } catch (e) {
      toast.error(formatInvokeError(e));
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div
      className={cn(
        "flex flex-col overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)]",
        "transition-shadow hover:shadow-md",
      )}
    >
      <div className="relative aspect-square bg-muted/40">
        {displaySrc && imgOk ? (
          <img
            src={displaySrc}
            alt=""
            className="h-full w-full object-cover"
            onError={() => setImgOk(false)}
          />
        ) : (
          <div className="flex h-full items-center justify-center text-4xl text-muted-foreground">
            📦
          </div>
        )}
      </div>
      <div className="flex flex-1 flex-col gap-2 p-3">
        <div className="min-w-0">
          <h3 className="truncate font-medium text-[var(--color-text)]">{product.name}</h3>
          <p className="mt-0.5 font-mono text-sm text-[var(--color-text-muted)]">
            {formatPrice(product.price)}
          </p>
        </div>
        {product.category ? (
          <span className="w-fit rounded-md bg-primary/15 px-2 py-0.5 text-xs text-primary">
            {product.category}
          </span>
        ) : null}
        <div className="mt-auto flex gap-2 pt-1">
          <Button type="button" variant="outline" size="sm" className="flex-1" onClick={() => onEdit(product)}>
            <Pencil className="mr-1 h-3.5 w-3.5" />
            Edit
          </Button>
          <Button type="button" variant="destructive" size="sm" onClick={() => setConfirmOpen(true)}>
            <Trash2 className="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>

      <Dialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <DialogContent showCloseButton className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Delete product?</DialogTitle>
            <DialogDescription>
              “{product.name}” will be removed permanently. This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter className="border-0 bg-transparent p-0 sm:justify-end">
            <Button type="button" variant="outline" disabled={deleting} onClick={() => setConfirmOpen(false)}>
              Cancel
            </Button>
            <Button type="button" variant="destructive" disabled={deleting} onClick={() => void runDelete()}>
              {deleting ? "Deleting…" : "Delete"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
