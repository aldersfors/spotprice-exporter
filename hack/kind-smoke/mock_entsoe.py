"""Mock ENTSO-E A44 endpoint for the kind smoke test. Synthetic prices, not market data."""

import math
import os
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlparse

TOKEN = os.environ.get("MOCK_TOKEN", "smoke-token")


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        url = urlparse(self.path)
        query = parse_qs(url.query)
        if url.path != "/api":
            self.send_response(404)
            self.end_headers()
            return
        if query.get("securityToken", [""])[0] != TOKEN:
            self.send_response(401)
            self.end_headers()
            self.wfile.write(b"unauthorized")
            return
        start = datetime.strptime(query["periodStart"][0], "%Y%m%d%H%M").replace(tzinfo=timezone.utc)
        end = datetime.strptime(query["periodEnd"][0], "%Y%m%d%H%M").replace(tzinfo=timezone.utc)
        steps = int((end - start).total_seconds() // 900)
        points = []
        for i in range(steps):
            price = 60 + 40 * math.sin(i / 8.0)
            if i % 37 == 5:
                price = -2.5
            points.append(f"<Point><position>{i + 1}</position><price.amount>{price:.2f}</price.amount></Point>")
        if steps == 0:
            body = ("<Acknowledgement_MarketDocument><Reason><code>999</code>"
                    "<text>No matching data found</text></Reason></Acknowledgement_MarketDocument>")
        else:
            body = f"""<?xml version="1.0" encoding="UTF-8"?>
<Publication_MarketDocument>
  <TimeSeries>
    <Period>
      <timeInterval><start>{start:%Y-%m-%dT%H:%M}Z</start><end>{end:%Y-%m-%dT%H:%M}Z</end></timeInterval>
      <resolution>PT15M</resolution>
      {''.join(points)}
    </Period>
  </TimeSeries>
</Publication_MarketDocument>"""
        self.send_response(200)
        self.send_header("Content-Type", "application/xml")
        self.end_headers()
        self.wfile.write(body.encode())

    def log_message(self, fmt, *args):
        print("mock-entsoe:", fmt % args, flush=True)


if __name__ == "__main__":
    HTTPServer(("0.0.0.0", 8080), Handler).serve_forever()
