type FlowCanvasEdgeProps = {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  markerEnd?: string;
};

export function FlowCanvasEdge({ x1, y1, x2, y2, markerEnd }: FlowCanvasEdgeProps) {
  return (
    <line
      x1={x1}
      y1={y1}
      x2={x2}
      y2={y2}
      stroke="rgba(255,255,255,0.18)"
      strokeWidth={2}
      strokeLinecap="round"
      markerEnd={markerEnd}
    />
  );
}
