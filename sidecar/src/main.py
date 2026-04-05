import socket

import uvicorn

from app import create_app
from config import settings
from logging_config import setup_sidecar_logging


def find_available_port() -> int:
    if is_port_available(settings.port):
        return settings.port
    for port in range(settings.port_fallback_range_start, settings.port_fallback_range_end + 1):
        if is_port_available(port):
            return port
    raise RuntimeError("No available port found in configured range")


def is_port_available(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        try:
            s.bind((settings.host, port))
            return True
        except OSError:
            return False


def main():
    setup_sidecar_logging()
    port = find_available_port()
    print(f"SIDECAR_PORT={port}", flush=True)

    app = create_app()
    uvicorn.run(app, host=settings.host, port=port, log_level=settings.log_level)


if __name__ == "__main__":
    main()
