import { create } from "zustand";
import { listProducts } from "@/lib/api";
import type { Product } from "@/types";

type ProductStore = {
  products: Product[];
  loading: boolean;
  searchQuery: string;
  fetchProducts: () => Promise<void>;
  setSearchQuery: (q: string) => void;
};

export const useProductStore = create<ProductStore>((set) => ({
  products: [],
  loading: false,
  searchQuery: "",

  fetchProducts: async () => {
    set({ loading: true });
    try {
      const products = await listProducts();
      set({ products, loading: false });
    } catch {
      set({ products: [], loading: false });
    }
  },

  setSearchQuery: (q) => set({ searchQuery: q }),
}));
