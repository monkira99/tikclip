import { invoke } from "@tauri-apps/api/core";

import { formatInvokeError } from "@/lib/invoke-error";
import { sidecarJson } from "@/lib/api/sidecar-client";
import type { CreateProductInput, Product, UpdateProductInput } from "@/types";

export async function listProducts(): Promise<Product[]> {
  return invoke<Product[]>("list_products");
}

export async function createProduct(input: CreateProductInput): Promise<number> {
  return invoke<number>("create_product", { input });
}

export async function updateProduct(productId: number, input: UpdateProductInput): Promise<void> {
  await invoke("update_product", { productId, input });
}

export async function deleteProduct(productId: number): Promise<void> {
  const id = Number(productId);
  if (!Number.isInteger(id) || id < 1) {
    throw new Error(`Invalid product id: ${String(productId)}`);
  }
  try {
    await invoke("delete_product", { productId: id });
  } catch (e) {
    throw new Error(formatInvokeError(e));
  }
}

export async function tagClipProduct(clipId: number, productId: number): Promise<void> {
  await invoke("tag_clip_product", { clipId, productId });
}

export type FetchedProductMediaFile = {
  kind: "image" | "video";
  path: string;
  source_url: string;
};

export type FetchProductFromUrlResult = {
  success: boolean;
  incomplete: boolean;
  data: {
    name: string | null;
    description: string | null;
    price: number | null;
    image_url: string | null;
    category: string | null;
    tiktok_shop_id: string | null;
    image_urls: string[];
    video_urls: string[];
    media_files: FetchedProductMediaFile[];
  } | null;
  error: string | null;
};

export async function fetchProductFromUrl(
  url: string,
  cookiesJson?: string | null,
  options?: { downloadMedia?: boolean },
): Promise<FetchProductFromUrlResult> {
  return sidecarJson<FetchProductFromUrlResult>("/api/products/fetch-from-url", {
    method: "POST",
    body: JSON.stringify({
      url,
      cookies_json: cookiesJson ?? null,
      download_media: options?.downloadMedia !== false,
    }),
  });
}

export type ProductEmbeddingMediaItem = {
  kind: "image" | "video";
  path: string;
  source_url?: string;
};

export type IndexProductEmbeddingsResult = {
  indexed: number;
  skipped: number;
  errors: string[];
  message: string | null;
};

export async function indexProductEmbeddings(
  productId: number,
  body: {
    product_name: string;
    product_description?: string;
    items: ProductEmbeddingMediaItem[];
  },
): Promise<IndexProductEmbeddingsResult> {
  return sidecarJson<IndexProductEmbeddingsResult>("/api/products/embeddings/index", {
    method: "POST",
    body: JSON.stringify({
      product_id: productId,
      product_name: body.product_name,
      product_description: body.product_description ?? "",
      items: body.items.map((x) => ({
        kind: x.kind,
        path: x.path,
        source_url: x.source_url ?? "",
      })),
    }),
  });
}

export async function deleteProductEmbeddings(productId: number): Promise<void> {
  await sidecarJson<{ ok: boolean }>("/api/products/embeddings/delete", {
    method: "POST",
    body: JSON.stringify({ product_id: productId }),
  });
}
