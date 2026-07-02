#!/usr/bin/env python3
"""taodb-ziran auxiliary: MiniMax embedding helper.

Calls the MiniMax embedding API to return 1536-dim vectors.
taodb-ziran core does NOT require embeddings — this script is for users
who want to add vector search capabilities on top of the temporal-spatial engine.

Usage:
  MINIMAX_API_KEY=sk-... python3 embed.py "some text"
  MINIMAX_API_KEY=sk-... python3 embed.py --batch "text1" "text2" "text3"

Output: JSON array [[0.1, 0.2, ...], ...]
"""

import argparse
import json
import os
import sys
import urllib.request

API_URL = "https://api.minimaxi.com/v1/embeddings"
API_KEY = os.environ.get("MINIMAX_API_KEY", "")
MODEL = "embo-01"
TYPE = "db"  # db = document storage
DIM = 1536


def embed_batch(texts):
    """Batch embedding. Requires MINIMAX_API_KEY environment variable."""
    if not API_KEY:
        print("ERROR: MINIMAX_API_KEY environment variable is required.", file=sys.stderr)
        print("  Set it before running: MINIMAX_API_KEY=sk-... python3 embed.py ...", file=sys.stderr)
        sys.exit(1)
    if not texts:
        return []
    body = {
        "model": MODEL,
        "type": TYPE,
        "texts": texts,
    }
    req = urllib.request.Request(
        API_URL,
        data=json.dumps(body).encode(),
        headers={
            "Authorization": f"Bearer {API_KEY}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=60) as resp:
        data = json.loads(resp.read())
        vectors = data.get("vectors")
        if vectors is None:
            print(f"ERROR: {data}", file=sys.stderr)
            sys.exit(1)
        return vectors


def embed_one(text):
    """Single embedding."""
    return embed_batch([text])[0]


def main():
    p = argparse.ArgumentParser()
    p.add_argument("texts", nargs="+", help="要 embed 的文本")
    p.add_argument("--batch", action="store_true", help="批量模式（明确）")
    args = p.parse_args()

    vectors = embed_batch(args.texts)
    print(json.dumps(vectors, ensure_ascii=False))


if __name__ == "__main__":
    main()