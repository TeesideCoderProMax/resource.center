from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread
import unittest
from test_release import load
probe = load("load-check")

class LoadTests(unittest.TestCase):
    def test_bounded_http_results_and_failures(self):
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                self.send_response(503 if self.path == "/fail" else 200)
                self.end_headers()
                self.wfile.write(b"{}")
            def log_message(self, *args):
                pass
        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            url = f"http://127.0.0.1:{server.server_port}"
            good = probe.measure(url, 12, 3)
            self.assertEqual(good["errors"], 0)
            self.assertEqual(good["requests"], 12)
            self.assertGreater(good["p95_ms"], 0)
            self.assertEqual(probe.measure(url + "/fail", 4, 2)["errors"], 4)
            with self.assertRaises(ValueError):
                probe.measure(url, 101, 1)
        finally:
            server.shutdown()
            server.server_close()
            thread.join()
