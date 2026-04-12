import { useCallback, useEffect, useMemo, useState } from "react";
import { Loader2, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ProductForm } from "@/components/products/product-form";
import { listClipProducts, listProducts, tagClipProduct, untagClipProduct } from "@/lib/api";
import { useProductStore } from "@/stores/product-store";
import type { Product } from "@/types";
import { cn } from "@/lib/utils";

export function ProductPicker({
  clipId,
  open,
  onClose,
  onUpdated,
}: {
  clipId: number;
  open: boolean;
  onClose: () => void;
  onUpdated?: () => void;
}) {
  const fetchStoreProducts = useProductStore((s) => s.fetchProducts);
  const [loading, setLoading] = useState(false);
  const [catalog, setCatalog] = useState<Product[]>([]);
  const [taggedIds, setTaggedIds] = useState<Set<number>>(() => new Set());
  const [search, setSearch] = useState("");
  const [togglingId, setTogglingId] = useState<number | null>(null);
  const [quickCreateOpen, setQuickCreateOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [all, tagged] = await Promise.all([listProducts(), listClipProducts(clipId)]);
      setCatalog(all);
      setTaggedIds(new Set(tagged.map((p) => p.id)));
    } catch {
      setCatalog([]);
      setTaggedIds(new Set());
    } finally {
      setLoading(false);
    }
  }, [clipId]);

  useEffect(() => {
    if (open) {
      void load();
      setSearch("");
    }
  }, [open, load]);

  const visible = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) {
      return catalog;
    }
    return catalog.filter((p) => {
      const name = p.name.toLowerCase();
      const sku = (p.sku ?? "").toLowerCase();
      return name.includes(q) || sku.includes(q);
    });
  }, [catalog, search]);

  const toggle = async (productId: number, nextTagged: boolean) => {
    setTogglingId(productId);
    try {
      if (nextTagged) {
        await tagClipProduct(clipId, productId);
        setTaggedIds((prev) => new Set(prev).add(productId));
      } else {
        await untagClipProduct(clipId, productId);
        setTaggedIds((prev) => {
          const n = new Set(prev);
          n.delete(productId);
          return n;
        });
      }
      onUpdated?.();
    } finally {
      setTogglingId(null);
    }
  };

  return (
    <>
      <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
        <DialogContent className="max-h-[85vh] overflow-hidden sm:max-w-md" showCloseButton>
          <DialogHeader>
            <DialogTitle>Tag products</DialogTitle>
          </DialogHeader>
          <div className="flex flex-col gap-3">
            <Input
              placeholder="Search products…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="w-full"
              onClick={() => setQuickCreateOpen(true)}
            >
              <Plus className="mr-2 h-4 w-4" />
              Quick create product
            </Button>
            <div className="max-h-[50vh] overflow-y-auto rounded-md border border-[var(--color-border)]">
              {loading ? (
                <div className="flex items-center justify-center py-12 text-muted-foreground">
                  <Loader2 className="h-6 w-6 animate-spin" />
                </div>
              ) : visible.length === 0 ? (
                <p className="p-4 text-sm text-[var(--color-text-muted)]">No products to show.</p>
              ) : (
                <ul className="divide-y divide-[var(--color-border)]">
                  {visible.map((p) => {
                    const checked = taggedIds.has(p.id);
                    const busy = togglingId === p.id;
                    return (
                      <li key={p.id}>
                        <label
                          className={cn(
                            "flex cursor-pointer items-center gap-3 px-3 py-2.5 text-sm transition-colors",
                            "hover:bg-muted/50",
                          )}
                        >
                          <input
                            type="checkbox"
                            checked={checked}
                            disabled={busy}
                            onChange={(e) => void toggle(p.id, e.target.checked)}
                            className="h-4 w-4 rounded border-input"
                          />
                          {p.image_url ? (
                            <img src={p.image_url} alt="" className="h-9 w-9 rounded object-cover" />
                          ) : (
                            <span className="flex h-9 w-9 items-center justify-center rounded bg-muted text-xs">
                              📦
                            </span>
                          )}
                          <span className="min-w-0 flex-1 truncate font-medium text-[var(--color-text)]">
                            {p.name}
                          </span>
                          {busy ? <Loader2 className="h-4 w-4 shrink-0 animate-spin" /> : null}
                        </label>
                      </li>
                    );
                  })}
                </ul>
              )}
            </div>
          </div>
        </DialogContent>
      </Dialog>

      <ProductForm
        open={quickCreateOpen}
        product={null}
        onClose={() => setQuickCreateOpen(false)}
        onSaved={async () => {
          await fetchStoreProducts();
          void load();
          onUpdated?.();
        }}
      />
    </>
  );
}
