function App() {
  return (
    <div className="flex h-screen bg-[var(--color-bg)] text-[var(--color-text)]">
      <aside className="w-[220px] bg-[var(--color-surface)] border-r border-[var(--color-border)]">
        <div className="p-5 border-b border-[var(--color-border)]">
          <h1 className="text-lg font-bold">TikClip</h1>
          <p className="text-xs text-[var(--color-text-muted)]">Live Recorder</p>
        </div>
      </aside>
      <main className="flex-1 p-6">
        <h2 className="text-xl">Dashboard</h2>
        <p className="text-[var(--color-text-muted)]">App is running.</p>
      </main>
    </div>
  );
}

export default App;
