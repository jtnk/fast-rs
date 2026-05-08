#!/usr/bin/env python3
import http.server
import subprocess
import json
import os
import threading

HTML = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Speed Test</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: system-ui, sans-serif;
      background: #0f0f0f;
      color: #e0e0e0;
      min-height: 100vh;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: 2rem;
      padding: 2rem;
    }
    h1 { font-size: 1.8rem; font-weight: 600; color: #fff; letter-spacing: -0.5px; }
    button {
      background: #2563eb;
      color: #fff;
      border: none;
      padding: 0.85rem 2.5rem;
      font-size: 1.05rem;
      font-weight: 600;
      border-radius: 8px;
      cursor: pointer;
      transition: background 0.15s, transform 0.1s;
    }
    button:hover:not(:disabled) { background: #1d4ed8; transform: translateY(-1px); }
    button:active:not(:disabled) { transform: translateY(0); }
    button:disabled { background: #374151; cursor: not-allowed; }
    #output-box {
      width: 100%;
      max-width: 700px;
      background: #1a1a1a;
      border: 1px solid #2a2a2a;
      border-radius: 10px;
      padding: 1.25rem 1.5rem;
      display: none;
    }
    #output-box h2 { font-size: 0.8rem; text-transform: uppercase; letter-spacing: 1px; color: #6b7280; margin-bottom: 0.75rem; }
    #status { font-size: 0.85rem; color: #9ca3af; margin-bottom: 0.5rem; }
    #output {
      font-family: 'SF Mono', 'Fira Code', monospace;
      font-size: 0.88rem;
      white-space: pre-wrap;
      word-break: break-word;
      line-height: 1.6;
      color: #d1fae5;
    }
    .spinner {
      display: inline-block;
      width: 14px;
      height: 14px;
      border: 2px solid #4b5563;
      border-top-color: #2563eb;
      border-radius: 50%;
      animation: spin 0.7s linear infinite;
      margin-right: 6px;
      vertical-align: middle;
    }
    @keyframes spin { to { transform: rotate(360deg); } }
  </style>
</head>
<body>
  <h1>Speed Test</h1>
  <button id="btn" onclick="runTest()">Run Speed Test</button>
  <div id="output-box">
    <h2>Output</h2>
    <div id="status"></div>
    <div id="output"></div>
  </div>

  <script>
    async function runTest() {
      const btn = document.getElementById('btn');
      const box = document.getElementById('output-box');
      const status = document.getElementById('status');
      const output = document.getElementById('output');

      btn.disabled = true;
      btn.textContent = 'Running…';
      box.style.display = 'block';
      output.textContent = '';
      status.innerHTML = '<span class="spinner"></span>Running speed test…';

      try {
        const res = await fetch('/run', { method: 'POST' });
        const data = await res.json();
        if (data.error) {
          status.textContent = 'Error';
          output.style.color = '#fca5a5';
          output.textContent = data.error + (data.output ? '\\n\\n' + data.output : '');
        } else {
          status.textContent = 'Done';
          output.style.color = '#d1fae5';
          output.textContent = data.output || '(no output)';
        }
      } catch (e) {
        status.textContent = 'Request failed';
        output.style.color = '#fca5a5';
        output.textContent = String(e);
      } finally {
        btn.disabled = false;
        btn.textContent = 'Run Speed Test';
      }
    }
  </script>
</body>
</html>
"""

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(fmt % args)

    def do_GET(self):
        if self.path == '/':
            body = HTML.encode()
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        if self.path == '/run':
            result = subprocess.run(
                ['min', 'run', 'run'],
                capture_output=True,
                text=True,
                cwd=os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
            )
            combined = result.stdout
            if result.stderr:
                combined += result.stderr
            payload = json.dumps({
                'output': combined,
                'error': None if result.returncode == 0 else f'Process exited with code {result.returncode}'
            }).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
        elif self.path == '/shutdown':
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(b'{"ok": true}')
            threading.Thread(target=self.server.shutdown, daemon=True).start()
        else:
            self.send_response(404)
            self.end_headers()

if __name__ == '__main__':
    port = int(os.environ.get('PORT', 8080))
    http.server.HTTPServer.allow_reuse_address = True
    server = http.server.HTTPServer(('0.0.0.0', port), Handler)
    print(f'Speed test webapp running at http://localhost:{port}', flush=True)
    print(f'To stop: curl -X POST http://localhost:{port}/shutdown', flush=True)
    server.serve_forever()
