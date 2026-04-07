import { AppShell } from "@/components/layout/app-shell";
import { Toaster } from "sonner";

function App() {
  return (
    <>
      <AppShell />
      <Toaster
        position="top-right"
        theme="dark"
        richColors
        closeButton
        toastOptions={{
          classNames: {
            toast:
              "border-[var(--color-border)] bg-[var(--color-surface)] text-[var(--color-text)]",
            title: "text-[var(--color-text)]",
            description: "text-[var(--color-text-muted)]",
          },
        }}
      />
    </>
  );
}

export default App;
