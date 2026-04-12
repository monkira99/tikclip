import { useMemo, useState } from "react";
import { Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ProductCard } from "@/components/products/product-card";
import { ProductForm } from "@/components/products/product-form";
import { useProductStore } from "@/stores/product-store";
import type { Product } from "@/types";

export function ProductList() {
  const products = useProductStore((s) => s.products);
  const searchQuery = useProductStore((s) => s.searchQuery);
  const setSearchQuery = useProductStore((s) => s.setSearchQuery);
  const [formOpen, setFormOpen] = useState(false);
  const [editProduct, setEditProduct] = useState<Product | null>(null);

  const filtered = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) {
      return products;
    }
    return products.filter((p) => {
      const name = p.name.toLowerCase();
      const sku = (p.sku ?? "").toLowerCase();
      return name.includes(q) || sku.includes(q);
    });
  }, [products, searchQuery]);

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <Input
          placeholder="Search by name or SKU…"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="max-w-md"
        />
        <Button
          type="button"
          onClick={() => {
            setEditProduct(null);
            setFormOpen(true);
          }}
        >
          <Plus className="mr-2 h-4 w-4" />
          Add product
        </Button>
      </div>

      {filtered.length === 0 ? (
        <p className="text-sm text-[var(--color-text-muted)]">
          {products.length === 0 ? "No products yet." : "No matches for your search."}
        </p>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
          {filtered.map((p) => (
            <ProductCard
              key={p.id}
              product={p}
              onEdit={(prod) => {
                setEditProduct(prod);
                setFormOpen(true);
              }}
            />
          ))}
        </div>
      )}

      <ProductForm
        open={formOpen}
        product={editProduct}
        onClose={() => {
          setFormOpen(false);
          setEditProduct(null);
        }}
      />
    </div>
  );
}
