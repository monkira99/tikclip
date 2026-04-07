import socket

import uvicorn

from app import create_app
from config import settings
from logging_config import setup_sidecar_logging


def bind_sidecar_socket() -> tuple[socket.socket, int]:
    """Bind with SO_REUSEADDR: primary port, fallback range, then ephemeral (port 0)."""
    host = settings.host
    ports_to_try: list[int] = [settings.port]

    start = settings.port_fallback_range_start
    end = settings.port_fallback_range_end
    if start <= end:
        for p in range(start, end + 1):
            if p != settings.port:
                ports_to_try.append(p)

    seen: set[int] = set()
    ordered: list[int] = []
    for p in ports_to_try:
        if p not in seen:
            seen.add(p)
            ordered.append(p)
    ordered.append(0)

    last_err: OSError | None = None
    for port in ordered:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            s.bind((host, port))
            actual_port = s.getsockname()[1]
            s.setblocking(False)
            return s, actual_port
        except OSError as e:
            last_err = e
            s.close()
            continue

    msg = f"Could not bind sidecar on {host!r} (tried configured ports + ephemeral)"
    if last_err is not None:
        msg += f": {last_err}"
    raise RuntimeError(msg)


def main():
    setup_sidecar_logging()
    sock, port = bind_sidecar_socket()
    print(f"SIDECAR_PORT={port}", flush=True)

    app = create_app()
    try:
        uvicorn.run(app, fd=sock.fileno(), log_level=settings.log_level)
    finally:
        try:
            sock.close()
        except OSError:
            pass


if __name__ == "__main__":
    main()
