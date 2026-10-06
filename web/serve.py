#!/usr/bin/env python3
# Serves web/ (a text area in the Heelee ASCII web font) on a local port.
#
#   python3 web/serve.py [--port 8000] [--host 127.0.0.1]
#
# Make the font first: scripts/make_webfont.sh <font.ttf> web/heelee-ascii.woff

import argparse
import functools
import http.server
import os

parser = argparse.ArgumentParser()
parser.add_argument("--port", type=int, default=8000)
parser.add_argument("--host", default="127.0.0.1")
args = parser.parse_args()


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".woff": "font/woff", ".woff2": "font/woff2"}

    # Always fetch the latest font and page after a rebuild.
    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


handler = functools.partial(Handler, directory=os.path.dirname(os.path.abspath(__file__)))
with http.server.ThreadingHTTPServer((args.host, args.port), handler) as server:
    print(f"Serving on http://{args.host}:{args.port}/")
    server.serve_forever()
