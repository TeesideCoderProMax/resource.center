#!/usr/bin/env python3
"""Bounded GET load probe. Replica counts must come from separate Azure evidence."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import json
import time
import urllib.error
import urllib.request


def measure(url, count, concurrency):
    if not 1 <= count <= 100 or not 1 <= concurrency <= 10:
        raise ValueError("Use 1..100 requests and 1..10 concurrent requests")
    def request(_):
        started = time.monotonic()
        try:
            with urllib.request.urlopen(url, timeout=10) as response:
                response.read(1024 * 1024)
                ok = response.status == 200
        except urllib.error.HTTPError as error:
            error.close()
            ok = False
        except OSError:
            ok = False
        return (time.monotonic() - started) * 1000, ok
    started = time.monotonic()
    with ThreadPoolExecutor(max_workers=concurrency) as pool:
        rows = list(pool.map(request, range(count)))
    times = sorted(row[0] for row in rows)
    return {"time_utc": datetime.now(timezone.utc).isoformat(), "url": url,
            "requests": count, "concurrency": concurrency, "duration_seconds": time.monotonic()-started,
            "errors": sum(not row[1] for row in rows), "p50_ms": times[(count-1)//2],
            "p95_ms": times[min(count-1, (95*count+99)//100-1)], "replica_evidence": "not captured"}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("url")
    parser.add_argument("--requests", type=int, default=40)
    parser.add_argument("--concurrency", type=int, default=5)
    args = parser.parse_args()
    result = measure(args.url, args.requests, args.concurrency)
    print(json.dumps(result, indent=2))
    raise SystemExit(1 if result["errors"] else 0)
