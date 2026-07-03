#!/usr/bin/env python3
"""
demo_agent.py — A tiny "agent" that talks to taodb over MCP stdio.

This is the script that drives the demo gif. It does exactly what a real
agent does (Claude Code, Cursor, etc.) — starts the taodb MCP subprocess,
sends JSON-RPC frames, parses responses — but presents the conversation
in a human-friendly format suitable for a 30-second terminal recording.

Usage:
    demo_agent.py memorize "Decision text" --containers a,b --energy 0.3
    demo_agent.py recall   "What did we decide about X?" --containers a,b --days 14

The script auto-discovers the project's user/project from .taodb/ metadata
if it exists, otherwise falls back to user=demo, project=default.
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path

TAODB_BIN = os.environ.get("TAODB_BIN", "taodb")

# ─── Pretty printing ────────────────────────────────────────────────────

CYAN = "\033[1;36m"
GREEN = "\033[1;32m"
YELLOW = "\033[1;33m"
DIM = "\033[2m"
RESET = "\033[0m"


def header(label):
    bar = "─" * 60
    print(f"{DIM}{bar}{RESET}")
    print(f"  {label}")
    print(f"{DIM}{bar}{RESET}")


def discover_identity():
    """Read user/project from .mcp.json if present, else fall back."""
    cwd = Path.cwd()
    mcp_json = cwd / ".mcp.json"
    if mcp_json.exists():
        try:
            data = json.loads(mcp_json.read_text())
            servers = data.get("mcpServers", {})
            for name, cfg in servers.items():
                args = cfg.get("args", [])
                user = project = None
                for i, a in enumerate(args):
                    if a == "--user" and i + 1 < len(args):
                        user = args[i + 1]
                    if a == "--project" and i + 1 < len(args):
                        project = args[i + 1]
                if user and project:
                    return user, project
        except Exception:
            pass
    return "demo", "default"


# ─── MCP transport ──────────────────────────────────────────────────────


class TaodbMcp:
    def __init__(self):
        self.proc = subprocess.Popen(
            [TAODB_BIN, "mcp"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,  # silence [mcp] init logs
            text=True,
            bufsize=1,  # line-buffered: stable on macOS where bufsize=0 + text=True can race
        )
        self._next_id = 1

    def request(self, method, params=None):
        msg = {"jsonrpc": "2.0", "id": self._next_id, "method": method}
        if params is not None:
            msg["params"] = params
        self._next_id += 1
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError("taodb mcp closed unexpectedly")
        return json.loads(line)

    def notify(self, method):
        msg = {"jsonrpc": "2.0", "method": method}
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()

    def initialize(self):
        r = self.request(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "demo-agent", "version": "0.1.0"},
            },
        )
        self.notify("notifications/initialized")
        return r

    def call_tool(self, name, arguments):
        return self.request(
            "tools/call", {"name": name, "arguments": arguments}
        )

    def close(self):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        self.proc.wait(timeout=3)


# ─── Tool helpers ───────────────────────────────────────────────────────


def memorize(mcp, text, containers, energy):
    header("📝  AGENT  →  taodb_memorize")
    print(f"   text:        {text}")
    print(f"   containers:  {containers}")
    print(f"   energy:      {energy}")
    print()
    r = mcp.call_tool(
        "taodb_memorize",
        {
            "text": text,
            "containers": containers,
            "energy_floor": energy,
        },
    )
    payload = json.loads(r["result"]["content"][0]["text"])
    mid = payload.get("memory_id", "?")
    print(f"{GREEN}✓ stored memory_id={mid}{RESET}")
    if payload.get("warnings"):
        for w in payload["warnings"]:
            print(f"{YELLOW}  ⚠ {w}{RESET}")


def recall(mcp, query, containers, days):
    header(f'🔍  AGENT  →  taodb_recall  "{query}"')
    print(f"   containers:  {containers}")
    print(f"   last:        {days} days")
    print()
    r = mcp.call_tool(
        "taodb_recall",
        {
            "query": query,
            "containers": containers,
            "narrative_span_days": days,
            "top_k": 3,
        },
    )
    payload = json.loads(r["result"]["content"][0]["text"])
    memories = payload.get("memories", [])
    paths = payload.get("recall_paths", [])
    if not memories:
        print(f"{YELLOW}(no memories returned){RESET}")
        return
    print(f"{CYAN}taodb returned {len(memories)} memories:{RESET}")
    print()
    for i, m in enumerate(memories, 1):
        text = m.get("text", "")
        energy = m.get("energy", 0)
        floor = m.get("energy_floor", 0)
        mem_containers = m.get("containers", [])
        print(f"  {i}. {text}")
        print(
            f"     {DIM}energy={energy:.2f} (floor={floor:.2f}) · "
            f"containers={mem_containers}{RESET}"
        )
        print()
    if paths:
        print(f"{DIM}─── recall paths ───{RESET}")
        for p in paths:
            print(f"{DIM}  {p}{RESET}")
        print()


# ─── CLI ────────────────────────────────────────────────────────────────


def main():
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        sys.exit(1)

    cmd = sys.argv[1]
    if cmd not in ("memorize", "recall"):
        print(f"unknown command: {cmd}", file=sys.stderr)
        sys.exit(1)

    user, project = discover_identity()
    mcp = TaodbMcp()
    try:
        mcp.initialize()
        if cmd == "memorize":
            # demo_agent.py memorize "TEXT" --containers a,b --energy 0.3
            text = sys.argv[2]
            containers = ["default"]
            energy = 0.0
            i = 3
            while i < len(sys.argv):
                flag = sys.argv[i]
                if flag == "--containers" and i + 1 < len(sys.argv):
                    containers = [
                        c.strip()
                        for c in sys.argv[i + 1].split(",")
                        if c.strip()
                    ]
                    i += 2
                elif flag == "--energy" and i + 1 < len(sys.argv):
                    energy = float(sys.argv[i + 1])
                    i += 2
                else:
                    i += 1
            memorize(mcp, text, containers, energy)
        elif cmd == "recall":
            # demo_agent.py recall "QUERY" --containers a,b --days 14
            query = sys.argv[2]
            containers = ["default"]
            days = 14
            i = 3
            while i < len(sys.argv):
                flag = sys.argv[i]
                if flag == "--containers" and i + 1 < len(sys.argv):
                    containers = [
                        c.strip()
                        for c in sys.argv[i + 1].split(",")
                        if c.strip()
                    ]
                    i += 2
                elif flag == "--days" and i + 1 < len(sys.argv):
                    days = int(sys.argv[i + 1])
                    i += 2
                else:
                    i += 1
            recall(mcp, query, containers, days)
    finally:
        mcp.close()


if __name__ == "__main__":
    main()
