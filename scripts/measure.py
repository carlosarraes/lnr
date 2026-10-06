#!/usr/bin/env python3
"""Measure local binary startup and one fixture-backed context request."""
import http.server
import json
import os
from pathlib import Path
import statistics
import subprocess
import threading
import time

binary = Path(__file__).resolve().parents[1] / 'target/release/lnr'
if not binary.exists():
    raise SystemExit('Run cargo build --release --locked first')
timings = []
for _ in range(10):
    start = time.perf_counter()
    subprocess.run([binary, '--help'], check=True, stdout=subprocess.DEVNULL)
    timings.append((time.perf_counter() - start) * 1000)
metrics = {'help_process_median_ms': round(statistics.median(timings), 3)}
page = {'nodes': [], 'pageInfo': {'hasNextPage': False, 'endCursor': None}}
body = json.dumps({'data': {'issue': {'id': 'fixture', 'identifier': 'ENG-1',
    'title': 'Fixture issue', 'parent': None, 'project': None,
    'children': page, 'comments': page, 'relations': page, 'inverseRelations': page}}}).encode()

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        request = self.rfile.read(int(self.headers['Content-Length']))
        metrics['requests'] = metrics.get('requests', 0) + 1
        metrics['request_bytes'] = len(request)
        metrics['response_bytes'] = len(body)
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass

server = http.server.HTTPServer(('127.0.0.1', 0), Handler)
thread = threading.Thread(target=server.serve_forever, daemon=True)
thread.start()
try:
    env = dict(os.environ, LINEAR_API_KEY='fixture-key',
               LNR_API_URL=f'http://127.0.0.1:{server.server_port}', NO_PROXY='127.0.0.1')
    start = time.perf_counter()
    result = subprocess.run([binary, 'issue', 'context', 'ENG-1', '--json'],
                            env=env, capture_output=True, check=True)
    metrics['context_fixture_ms'] = round((time.perf_counter() - start) * 1000, 3)
    metrics['output_bytes'] = len(result.stdout)
    metrics['rustc'] = subprocess.check_output(['rustc', '--version'], text=True).strip()
    metrics['profile'] = 'release'
    metrics['note'] = 'Local fixture timings, including process startup; not live API latency or an upstream comparison.'
    print(json.dumps(metrics, indent=2))
finally:
    server.shutdown()
    server.server_close()
    thread.join()
