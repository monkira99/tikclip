import { useEffect } from "react";
import { ProductList } from "@/components/products/product-list";
import { useProductStore } from "@/stores/product-store";

export function ProductsPage() {
  const fetchProducts = useProductStore((s) => s.fetchProducts);

  useEffect(() => {
    void fetchProducts();
  }, [fetchProducts]);

  return (
    <div className="space-y-6">
      <ProductList />
    </div>
  );
}
