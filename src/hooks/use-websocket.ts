import { useEffect, useRef } from "react";
import { wsClient } from "@/lib/ws";

/**
 * Subscribe to a sidecar WebSocket event. Handler always sees latest closure via ref.
 */
export function useWebSocketEvent(
  eventType: string,
  handler: (data: Record<string, unknown>) => void,
): void {
  const ref = useRef(handler);
  ref.current = handler;

  useEffect(() => {
    return wsClient.on(eventType, (data) => {
      ref.current(data);
    });
  }, [eventType]);
}
