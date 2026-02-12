import { Tldraw } from "tldraw";
import "tldraw/tldraw.css";

import { type SyncStatus, useCanvasSync } from "./hooks/useCanvasSync";
import "./App.css";

const STATUS_LABELS: Record<SyncStatus, string> = {
  hydrating: "Hydrating",
  idle: "Synced",
  saving: "Saving",
  error: "Error",
};

function App() {
  const { handleMount, createQuickNote, syncStatus, syncError, trackedCount, runtimeMode } = useCanvasSync();

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="app-heading">
          <h1>CyberWeaver</h1>
          <p>Digital forensics canvas for structured clue mapping.</p>
        </div>

        <div className="sync-panel">
          <span className={`sync-pill sync-pill--${syncStatus}`} data-testid="sync-status">
            {STATUS_LABELS[syncStatus]}
          </span>

          <span className="tracked-count">
            <span data-testid="tracked-count-value">{trackedCount}</span>
            <span>tracked clues</span>
          </span>

          <button type="button" className="quick-action" data-testid="quick-note" onClick={createQuickNote}>
            Add note
          </button>

          <span className="runtime-mode" data-testid="runtime-mode">
            {runtimeMode}
          </span>
        </div>
      </header>

      {syncError ? (
        <div className="error-banner" role="alert">
          {syncError}
        </div>
      ) : null}

      <main className="canvas-root">
        <Tldraw
          onMount={(editor) => {
            void handleMount(editor);
          }}
        />
      </main>
    </div>
  );
}

export default App;
