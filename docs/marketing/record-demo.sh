#!/usr/bin/env bash
# record-demo.sh — record the taodb 30-second demo via macOS screencapture.
#
# This is the **screencapture fallback** for users whose network blocks
# vhs (which depends on tesseract, a heavy C build). The script is the
# source of truth; the markdown version in demo-script.md is reference.
#
# Usage:
#   1. Open System Settings → Privacy & Security → Screen Recording
#      and grant access to your terminal app (Terminal.app, iTerm2, …).
#   2. Open a fresh terminal, sized to ~120×32 cols, Tokyo Night or
#      any dark theme.
#   3. Run this script in *one* terminal:
#           bash record-demo.sh
#      In *another* terminal (or via a hotkey), start the recorder:
#           screencapture -V 35 -k -C ~/demo/auth-service/demo.mp4
#      Give screencapture ~1 second to start, then watch.
#
# The 30-second timing in demo-runner.sh accounts for the 1s setup
# buffer; total screencapture is 35s to capture the trailing
# `# github.com/taodbhip/taodb` line for ≥ 2 seconds.

set -euo pipefail

DEMO_DIR="${TAODB_DEMO_DIR:-$HOME/demo/auth-service}"
TRASH="${MAVIS_TRASH:-mavis-trash}"  # set to "rm -rf" if unavailable

echo ">>> step 1/2 — fresh pre-flight at $DEMO_DIR"
$TRASH "$DEMO_DIR" 2>/dev/null || true
mkdir -p "$DEMO_DIR/src"

cat > "$DEMO_DIR/Cargo.toml" <<'EOF'
[package]
name = "auth-service"
version = "0.1.0"
edition = "2021"
EOF

cat > "$DEMO_DIR/src/lib.rs" <<'EOF'
// Minimal auth stub — the real project has more
pub fn rotate_token(_t: u64) -> u64 { 0 }
EOF

# demo-runner.sh — what gets recorded. Init runs *inside* the
# recording so the audience sees the "one command" magic. The
# `--containers feature:auth --days 14` flags on the recall
# command are what makes the recall_paths block show
# "地: container_overlap → 1 hits" — the punchline.
cat > "$DEMO_DIR/demo-runner.sh" <<BASH_EOF
#!/usr/bin/env bash
set -e
cd "\$(dirname "\$0")"

echo "# taodb — memory for LLM agents"
sleep 2

taodb init --user demo --project auth-service 2>&1 | tail -3
sleep 2

echo "# Session 1 — agent finishes a debugging session"
sleep 1
python3 \$(dirname "\$0")/demo_agent.py memorize \\
    "Fixed token-rotation race; switched mutex to atomic swap. Mutex deadlocked under contention." \\
    --containers feature:auth,bug:race-condition --energy 0.3
sleep 1

echo "# ... two weeks pass ..."
sleep 3

echo "# Session 2 — new agent, same project"
sleep 1
python3 \$(dirname "\$0")/demo_agent.py ask \\
    "what did we decide about auth last week?" \\
    --containers feature:auth --days 14
sleep 4

echo ""
echo "# github.com/taodbhip/taodb"
sleep 3
BASH_EOF
chmod +x "$DEMO_DIR/demo-runner.sh"

# Copy the latest demo_agent.py into the demo project dir so the
# runner doesn't have to reach into the taodb checkout.
cp "$(dirname "$0")/demo_agent.py" "$DEMO_DIR/demo_agent.py"

echo ">>> step 2/2 — run demo (start screencapture now in another terminal)"
echo ">>>"
bash "$DEMO_DIR/demo-runner.sh"
