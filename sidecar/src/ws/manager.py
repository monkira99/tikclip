import asyncio
import json
import time

from fastapi import WebSocket


class ConnectionManager:
    def __init__(self):
        self._connections: list[WebSocket] = []
        self._lock = asyncio.Lock()

    async def connect(self, websocket: WebSocket):
        await websocket.accept()
        async with self._lock:
            self._connections.append(websocket)

    async def disconnect(self, websocket: WebSocket):
        async with self._lock:
            self._connections.remove(websocket)

    async def broadcast(self, event_type: str, data: dict):
        message = json.dumps(
            {
                "type": event_type,
                "data": data,
                "timestamp": int(time.time()),
            }
        )
        async with self._lock:
            stale = []
            for ws in self._connections:
                try:
                    await ws.send_text(message)
                except Exception:
                    stale.append(ws)
            for ws in stale:
                self._connections.remove(ws)

    @property
    def active_count(self) -> int:
        return len(self._connections)


ws_manager = ConnectionManager()
