from __future__ import annotations

import asyncio
import logging
from collections.abc import Callable, Coroutine
from typing import Any

import orjson
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

log = logging.getLogger("hft.ingest")


def start_ingest_server(
    port: int,
    loop: asyncio.AbstractEventLoop,
    handler: Callable[[dict[str, Any]], Coroutine[Any, Any, None]],
) -> ThreadingHTTPServer:
    class H(BaseHTTPRequestHandler):
        def log_message(self, fmt: str, *args) -> None:  # noqa: A003
            log.debug(fmt, *args)

        def do_POST(self) -> None:  # noqa: N802
            if self.path not in {"/ingest", "/signal", "/"}:
                self.send_error(404)
                return
            n = int(self.headers.get("Content-Length") or 0)
            body = self.rfile.read(n)
            try:
                payload = orjson.loads(body)
            except Exception:
                self.send_error(400, "json")
                return
            if isinstance(payload, dict):
                asyncio.run_coroutine_threadsafe(handler(payload), loop)
            self.send_response(202)
            self.end_headers()
            self.wfile.write(b'{"ok":true}')

        def do_GET(self) -> None:  # noqa: N802
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'{"ok":true,"path":"/ingest"}')

    httpd = ThreadingHTTPServer(("0.0.0.0", port), H)
    threading_run(httpd)
    log.info("ingest http :%s/ingest", port)
    return httpd


def threading_run(httpd: ThreadingHTTPServer) -> None:
    import threading

    t = threading.Thread(target=httpd.serve_forever, name="ingest-http", daemon=True)
    t.start()
