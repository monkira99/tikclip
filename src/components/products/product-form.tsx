import { useEffect, useMemo, useState } from "react";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  createProduct,
  fetchProductFromUrl,
  type FetchProductFromUrlResult,
  indexProductEmbeddings,
  updateProduct,
} from "@/lib/api";
import { useProductStore } from "@/stores/product-store";
import type { Product } from "@/types";

function emptyForm() {
  return {
    name: "",
    description: "",
    sku: "",
    image_url: "",
    tiktok_shop_id: "",
    tiktok_url: "",
    price: "",
    category: "",
    importUrl: "",
    importCookies: "",
    mediaFilesJson: "",
    downloadMedia: true,
  };
}

export function ProductForm({
  open,
  onClose,
  product,
  onSaved,
}: {
  open: boolean;
  onClose: () => void;
  product: Product | null;
  onSaved?: () => void;
}) {
  const fetchProducts = useProductStore((s) => s.fetchProducts);
  const [tab, setTab] = useState("import");
  const [saving, setSaving] = useState(false);
  const [fetching, setFetching] = useState(false);
  const [fetchHint, setFetchHint] = useState<string | null>(null);
  const [form, setForm] = useState(() => emptyForm());

  useEffect(() => {
    if (!open) {
      return;
    }
    setFetchHint(null);
    if (product) {
      setTab("manual");
      setForm({
        name: product.name,
        description: product.description ?? "",
        sku: product.sku ?? "",
        image_url: product.image_url ?? "",
        tiktok_shop_id: product.tiktok_shop_id ?? "",
        tiktok_url: product.tiktok_url ?? "",
        price: product.price != null ? String(product.price) : "",
        category: product.category ?? "",
        importUrl: product.tiktok_url ?? "",
        importCookies: "",
        mediaFilesJson: product.media_files_json ?? "",
        downloadMedia: true,
      });
    } else {
      setTab("import");
      setForm(emptyForm());
    }
  }, [open, product]);

  const parsedMediaFiles = useMemo(() => {
    try {
      const j: unknown = JSON.parse(form.mediaFilesJson || "[]");
      if (!Array.isArray(j)) {
        return [];
      }
      return j.filter(
        (x): x is { kind: string; path: string; source_url: string } =>
          x !== null &&
          typeof x === "object" &&
          "kind" in x &&
          "path" in x &&
          typeof (x as { path: string }).path === "string",
      );
    } catch {
      return [];
    }
  }, [form.mediaFilesJson]);

  const applyFetched = (data: NonNullable<FetchProductFromUrlResult["data"]>) => {
    setForm((f) => ({
      ...f,
      name: data.name ?? f.name,
      description: data.description ?? f.description,
      sku: data.tiktok_shop_id ?? f.sku,
      image_url: data.image_url ?? f.image_url,
      tiktok_shop_id: data.tiktok_shop_id ?? f.tiktok_shop_id,
      price: data.price != null ? String(data.price) : f.price,
      category: data.category ?? f.category,
      tiktok_url: f.importUrl.trim() || f.tiktok_url,
      mediaFilesJson: JSON.stringify(data.media_files ?? []),
    }));
  };

  const onFetch = async () => {
    const url = form.importUrl.trim();
    if (!url) {
      setFetchHint("Enter a product URL.");
      return;
    }
    setFetching(true);
    setFetchHint(null);
    try {
      const res = await fetchProductFromUrl(
        url,
        form.importCookies.trim() ? form.importCookies : null,
        { downloadMedia: form.downloadMedia },
      );
      if (!res.success) {
        setFetchHint(res.error ?? "Could not read product from this page.");
        return;
      }
      if (res.data) {
        applyFetched(res.data);
      }
      const mf = res.data?.media_files ?? [];
      const nImg = mf.filter((m) => m.kind === "image").length;
      const nVid = mf.filter((m) => m.kind === "video").length;
      const remoteOnly =
        form.downloadMedia === false && (res.data?.image_urls?.length || res.data?.video_urls?.length);
      let hint = "";
      if (res.incomplete) {
        hint = "Some fields could not be detected; check and complete manually.";
      } else {
        hint = "Imported — review the Manual tab before saving.";
      }
      if (mf.length > 0) {
        hint += ` Downloaded ${nImg} image(s) and ${nVid} video(s) under your storage folder.`;
      } else if (remoteOnly) {
        hint += ` Found ${res.data?.image_urls?.length ?? 0} image URL(s) and ${res.data?.video_urls?.length ?? 0} video URL(s) (download disabled).`;
      }
      setFetchHint(hint);
      setTab("manual");
    } catch (e) {
      setFetchHint(e instanceof Error ? e.message : "Fetch failed");
    } finally {
      setFetching(false);
    }
  };

  const onSave = async () => {
    const name = form.name.trim();
    if (!name) {
      setFetchHint("Name is required.");
      setTab("manual");
      return;
    }
    const priceRaw = form.price.trim();
    const price = priceRaw === "" ? null : Number(priceRaw);
    if (priceRaw !== "" && Number.isNaN(price)) {
      setFetchHint("Invalid price.");
      setTab("manual");
      return;
    }

    const mfRaw = form.mediaFilesJson.trim();
    let media_files_json: string | null = null;
    let parsedMedia: unknown[] | null = null;
    if (mfRaw) {
      try {
        const parsed: unknown = JSON.parse(mfRaw);
        if (!Array.isArray(parsed)) {
          throw new Error("not array");
        }
        parsedMedia = parsed;
        media_files_json = mfRaw;
      } catch {
        setFetchHint("Media files field must be valid JSON array.");
        setTab("manual");
        return;
      }
    }

    setSaving(true);
    setFetchHint(null);
    try {
      const base = {
        description: form.description.trim() || null,
        sku: form.sku.trim() || null,
        image_url: form.image_url.trim() || null,
        tiktok_shop_id: form.tiktok_shop_id.trim() || null,
        tiktok_url: form.tiktok_url.trim() || null,
        category: form.category.trim() || null,
        price,
        media_files_json,
      };
      let savedId: number;
      if (product) {
        await updateProduct(product.id, { name, ...base });
        savedId = product.id;
      } else {
        savedId = await createProduct({ name, ...base });
      }
      if (parsedMedia?.length) {
        const items = parsedMedia
          .filter((row): row is Record<string, unknown> => row !== null && typeof row === "object")
          .filter(
            (row) =>
              row.kind === "image" || row.kind === "video",
          )
          .map((row) => ({
            kind: row.kind as "image" | "video",
            path: String(row.path ?? ""),
            source_url: typeof row.source_url === "string" ? row.source_url : "",
          }))
          .filter((x) => x.path.length > 0);
        if (items.length > 0) {
          void indexProductEmbeddings(savedId, {
            product_name: name,
            product_description: form.description,
            items,
          }).catch(() => {
            /* optional: indexing disabled or Gemini/vector error */
          });
        }
      }
      void fetchProducts();
      onSaved?.();
      onClose();
    } catch (e) {
      setFetchHint(e instanceof Error ? e.message : "Save failed");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg" showCloseButton>
        <DialogHeader>
          <DialogTitle>{product ? "Edit product" : "Add product"}</DialogTitle>
        </DialogHeader>

        <Tabs value={tab} onValueChange={setTab}>
          <TabsList className="w-full">
            <TabsTrigger value="import" className="flex-1" disabled={!!product}>
              Import from TikTok
            </TabsTrigger>
            <TabsTrigger value="manual" className="flex-1">
              Manual
            </TabsTrigger>
          </TabsList>

          <TabsContent value="import" className="mt-4 space-y-3">
            <div className="space-y-1.5">
              <Label htmlFor="product-import-url">Product URL</Label>
              <Input
                id="product-import-url"
                value={form.importUrl}
                onChange={(e) => setForm((f) => ({ ...f, importUrl: e.target.value }))}
                placeholder="https://…"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="product-import-cookies">Cookies JSON (optional)</Label>
              <textarea
                id="product-import-cookies"
                value={form.importCookies}
                onChange={(e) => setForm((f) => ({ ...f, importCookies: e.target.value }))}
                rows={3}
                className="w-full resize-y rounded-md border border-input bg-background px-2 py-2 text-sm"
                placeholder='{"sessionid": "…"} or [{"name":"…","value":"…"}]'
              />
            </div>
            <label className="flex cursor-pointer items-start gap-2 text-sm">
              <input
                type="checkbox"
                className="mt-1"
                checked={form.downloadMedia}
                onChange={(e) => setForm((f) => ({ ...f, downloadMedia: e.target.checked }))}
              />
              <span>
                Download all gallery images and videos to your TikClip storage (
                <code className="text-xs">products/fetched/…</code>). Uncheck to only read metadata
                and remote URLs.
              </span>
            </label>
            <Button type="button" disabled={fetching} onClick={() => void onFetch()}>
              {fetching ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : null}
              Fetch
            </Button>
          </TabsContent>

          <TabsContent value="manual" className="mt-4 space-y-3">
            <div className="space-y-1.5">
              <Label htmlFor="product-name">Name</Label>
              <Input
                id="product-name"
                value={form.name}
                onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="product-desc">Description</Label>
              <textarea
                id="product-desc"
                value={form.description}
                onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
                rows={3}
                className="w-full resize-y rounded-md border border-input bg-background px-2 py-2 text-sm"
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label htmlFor="product-sku">SKU</Label>
                <Input
                  id="product-sku"
                  value={form.sku}
                  onChange={(e) => setForm((f) => ({ ...f, sku: e.target.value }))}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="product-price">Price</Label>
                <Input
                  id="product-price"
                  value={form.price}
                  onChange={(e) => setForm((f) => ({ ...f, price: e.target.value }))}
                  inputMode="decimal"
                />
              </div>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="product-category">Category</Label>
              <Input
                id="product-category"
                value={form.category}
                onChange={(e) => setForm((f) => ({ ...f, category: e.target.value }))}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="product-image">Image URL</Label>
              <Input
                id="product-image"
                value={form.image_url}
                onChange={(e) => setForm((f) => ({ ...f, image_url: e.target.value }))}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="product-tt-url">TikTok URL</Label>
              <Input
                id="product-tt-url"
                value={form.tiktok_url}
                onChange={(e) => setForm((f) => ({ ...f, tiktok_url: e.target.value }))}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="product-shop-id">TikTok shop ID</Label>
              <Input
                id="product-shop-id"
                value={form.tiktok_shop_id}
                onChange={(e) => setForm((f) => ({ ...f, tiktok_shop_id: e.target.value }))}
              />
            </div>
            {parsedMediaFiles.length > 0 ? (
              <div className="space-y-1.5">
                <Label>Downloaded media ({parsedMediaFiles.length})</Label>
                <ul className="max-h-36 list-inside list-disc overflow-y-auto rounded-md border border-input bg-muted/30 px-2 py-2 font-mono text-xs text-muted-foreground">
                  {parsedMediaFiles.map((m, i) => (
                    <li key={`${m.path}-${i}`}>
                      <span className="text-foreground">{m.kind}</span> —{" "}
                      {m.path.replace(/^.*[/\\]/, "")}
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
            <div className="space-y-1.5">
              <Label htmlFor="product-media-json">Media manifest JSON (advanced)</Label>
              <textarea
                id="product-media-json"
                value={form.mediaFilesJson}
                onChange={(e) => setForm((f) => ({ ...f, mediaFilesJson: e.target.value }))}
                rows={4}
                className="w-full resize-y rounded-md border border-input bg-background px-2 py-2 font-mono text-xs"
                placeholder='[{"kind":"image","path":"/…","source_url":"https://…"}]'
              />
            </div>
          </TabsContent>
        </Tabs>

        {fetchHint ? <p className="text-sm text-amber-600 dark:text-amber-400">{fetchHint}</p> : null}

        <DialogFooter>
          <Button type="button" variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button type="button" disabled={saving} onClick={() => void onSave()}>
            {saving ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : null}
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
