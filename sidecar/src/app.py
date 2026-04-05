from contextlib import asynccontextmanager

from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware

from .core.watcher import account_watcher
from .routes import accounts, health, recordings
from .ws.manager import ws_manager


@asynccontextmanager
async def lifespan(app: FastAPI):
    await account_watcher.start()
    yield
    await account_watcher.stop()


def create_app() -> FastAPI:
    app = FastAPI(title="TikClip Sidecar", version="0.1.0", lifespan=lifespan)

    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],
        allow_methods=["*"],
        allow_headers=["*"],
    )

    app.include_router(health.router)
    app.include_router(recordings.router)
    app.include_router(accounts.router, tags=["accounts"])

    @app.websocket("/ws")
    async def websocket_endpoint(websocket: WebSocket):
        await ws_manager.connect(websocket)
        try:
            while True:
                await websocket.receive_text()
        except WebSocketDisconnect:
            await ws_manager.disconnect(websocket)

    return app
