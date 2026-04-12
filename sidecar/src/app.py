from contextlib import asynccontextmanager

from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware

from core.cleanup import cleanup_worker
from core.watcher import account_watcher
from embeddings.zvec_runtime import setup_zvec
from routes import accounts, health, recordings
from routes import clips as clips_routes
from routes import products as product_routes
from routes import storage as storage_routes
from routes import trim as trim_routes
from ws.manager import ws_manager


@asynccontextmanager
async def lifespan(app: FastAPI):
    setup_zvec()
    await account_watcher.start()
    await cleanup_worker.start()
    yield
    await cleanup_worker.stop()
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
    app.include_router(clips_routes.router)
    app.include_router(trim_routes.router)
    app.include_router(product_routes.router, tags=["products"])
    app.include_router(storage_routes.router, tags=["storage"])
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
