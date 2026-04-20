import assert from "node:assert/strict";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";

import { FlowCanvasNode } from "./flow-canvas-node";

test("FlowCanvasNode renders running badge and active marker even without motion", () => {
  const markup = renderToStaticMarkup(
    <FlowCanvasNode
      nodeKey="record"
      selected
      hasDraftChanges={false}
      runtimeState="Recording live"
      summary="Max 5 min"
      visualState="running"
      badgeLabel="Running"
      inlineDetail={null}
      activeMarker
      onClick={() => {}}
    />,
  );

  assert.match(markup, /Running/);
  assert.match(markup, /Recording live/);
  assert.match(markup, /aria-pressed="true"/);
  assert.match(markup, /data-runtime-state="running"/);
  assert.match(markup, /data-active-marker="true"/);
  assert.match(markup, /runtime-pulse-glow/);
});

test("FlowCanvasNode renders error detail without changing node content structure", () => {
  const markup = renderToStaticMarkup(
    <FlowCanvasNode
      nodeKey="clip"
      selected
      hasDraftChanges={false}
      runtimeState="Clip failed"
      summary="15-45s clips"
      visualState="error"
      badgeLabel="Error"
      inlineDetail="clip timeout"
      activeMarker={false}
      onClick={() => {}}
    />,
  );

  assert.match(markup, /Error/);
  assert.match(markup, /clip timeout/);
  assert.match(markup, /line-clamp-1/);
  assert.match(markup, /data-runtime-state="error"/);
});

test("FlowCanvasNode keeps summary constrained to a two-line clamp", () => {
  const markup = renderToStaticMarkup(
    <FlowCanvasNode
      nodeKey="caption"
      selected={false}
      hasDraftChanges
      runtimeState="Caption complete"
      summary="A very long detail string that should still be constrained by the node surface"
      visualState="done"
      badgeLabel="Done"
      inlineDetail={null}
      activeMarker={false}
      onClick={() => {}}
    />,
  );

  assert.match(markup, /line-clamp-2/);
  assert.match(markup, /Done/);
});
