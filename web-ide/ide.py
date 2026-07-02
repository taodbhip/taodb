#!/usr/bin/env python3
"""
taodb-ziran 网文创作 IDE —— 给计算机小白用

功能：
  1. 创建项目（故事名 + 大纲）
  2. 写下一章（自动调 taodb recall + M3 生成）
  3. 列出项目

设计原则：
  - 3 个按钮搞定一切
  - 大字、清晰、无干扰
  - 不需要懂"API"、"token"、"embedding"
"""

"""taodb-ziran auxiliary: Web IDE for novel writing.

This is an OPTIONAL web frontend for taodb-ziran. The core product is the
Rust binary (MCP + HTTP server). This IDE is a convenience tool for
non-technical authors who want a 3-button writing interface.

Requires:
  - TAODB_BASE: taodb HTTP server URL (default http://127.0.0.1:8766)
  - TAODB_ADMIN_TOKEN: admin token matching the server's --admin-token
  - TAODB_LLM_CMD: command to invoke your LLM, e.g.
      "python3 /path/to/call_llm.py"
"""

import json
import urllib.request
import urllib.error
import subprocess
import os
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse

# Configuration
TAODB_BASE = os.environ.get("TAODB_BASE", "http://127.0.0.1:8766")
TAODB_TOKEN = os.environ.get("TAODB_TOKEN", "")
TAODB_ADMIN_TOKEN = os.environ.get("TAODB_ADMIN_TOKEN", "tk_admin")
TAODB_LLM_CMD = os.environ.get("TAODB_LLM_CMD", "")
IDE_PORT = int(os.environ.get("IDE_PORT", "8767"))
DATA_DIR = os.path.expanduser(os.environ.get("IDE_DATA_DIR", "~/taodb-ide-data"))

# 全局（简化）：单用户，不需要登录
STATE = {"taodb_token": TAODB_TOKEN, "taodb_user": None, "taodb_project": None}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass  # 静默

    def do_GET(self):
        url = urlparse(self.path)
        if url.path == "/" or url.path == "/index.html":
            self.send_html(INDEX_HTML)
        elif url.path == "/style.css":
            self.send_css()
        elif url.path == "/app.js":
            self.send_js()
        elif url.path == "/api/projects":
            self.handle_list_projects()
        elif url.path == "/api/health":
            self.send_json({"ok": True})
        else:
            self.send_error(404)

    def do_POST(self):
        url = urlparse(self.path)
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        try:
            data = json.loads(body.decode()) if body else {}
        except Exception:
            data = {}
        if url.path == "/api/login":
            self.handle_login(data)
        elif url.path == "/api/projects":
            self.handle_create_project(data)
        elif url.path == "/api/chapters/write":
            self.handle_write_chapter(data)
        elif url.path == "/api/chapters/list":
            self.handle_list_chapters(data)
        elif url.path == "/api/recall":
            self.handle_recall(data)
        else:
            self.send_error(404)

    # ===== HTML/CSS/JS =====
    def send_html(self, html):
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.end_headers()
        self.wfile.write(html.encode())

    def send_css(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/css")
        self.end_headers()
        self.wfile.write(STYLE_CSS.encode())

    def send_js(self):
        self.send_response(200)
        self.send_header("Content-Type", "application/javascript")
        self.end_headers()
        self.wfile.write(APP_JS.encode())

    def send_json(self, data, status=200):
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(data, ensure_ascii=False).encode())

    # ===== API =====
    def call_taodb_admin(self, method, path, data=None):
        """Call taodb with admin token (create users/projects)."""
        url = TAODB_BASE + path
        body = json.dumps(data).encode() if data else None
        req = urllib.request.Request(url, data=body, method=method)
        req.add_header("Authorization", f"Bearer {TAODB_ADMIN_TOKEN}")
        req.add_header("Content-Type", "application/json")
        try:
            with urllib.request.urlopen(req, timeout=10) as resp:
                return resp.status, json.loads(resp.read())
        except urllib.error.HTTPError as e:
            try:
                return e.code, json.loads(e.read())
            except Exception:
                return e.code, {"error": str(e)}

    def call_taodb_user(self, method, path, data=None):
        """用用户 token 调 taodb"""
        url = TAODB_BASE + path
        body = json.dumps(data).encode() if data else None
        req = urllib.request.Request(url, data=body, method=method)
        req.add_header("Authorization", f"Bearer {STATE['taodb_token']}")
        req.add_header("Content-Type", "application/json")
        if STATE.get("taodb_project"):
            req.add_header("x-project-id", STATE["taodb_project"])
        try:
            with urllib.request.urlopen(req, timeout=10) as resp:
                return resp.status, json.loads(resp.read())
        except urllib.error.HTTPError as e:
            try:
                return e.code, json.loads(e.read())
            except Exception:
                return e.code, {"error": str(e)}

    def handle_login(self, data):
        """简化：自动创建/复用 'demo' 用户"""
        email = data.get("email", "demo@local")
        status, user = self.call_taodb_admin("POST", "/v1/users", {
            "user_id": "demo",
            "email": email,
        })
        if status == 409:
            # 已存在，从 list 找
            status2, users_data = self.call_taodb_admin("GET", "/v1/users")
            for u in users_data.get("users", []):
                if u["user_id"] == "demo":
                    user = u
                    break
        if isinstance(user, dict) and "api_token" in user:
            STATE["taodb_token"] = user["api_token"]
            STATE["taodb_user"] = user["user_id"]
            self.send_json({"ok": True, "user": user["user_id"], "token": user["api_token"]})
        else:
            self.send_json({"ok": False, "error": str(user)}, 400)

    def handle_create_project(self, data):
        project_id = data.get("project_id", "").strip()
        name = data.get("name", "").strip()
        if not project_id or not name:
            self.send_json({"ok": False, "error": "需要项目 ID 和名称"}, 400)
            return
        status, proj = self.call_taodb_user("POST", "/v1/projects", {
            "project_id": project_id,
            "name": name,
        })
        if status in (200, 201):
            STATE["taodb_project"] = project_id
            self.send_json({"ok": True, "project": proj})
        else:
            self.send_json({"ok": False, "error": str(proj)}, status)

    def handle_list_projects(self):
        if not STATE["taodb_token"]:
            self.send_json({"ok": False, "error": "请先登录"}, 401)
            return
        status, data = self.call_taodb_user("GET", "/v1/projects")
        self.send_json(data, status if status < 400 else 200)

    def handle_list_chapters(self, data):
        # 简化：从 recall 获取所有记忆，按 ID 排序
        status, data = self.call_taodb_user("POST", "/v1/recall", {
            "query": "",
            "top_k": 100,
            "field_marker": [],
        })
        if status == 200:
            chapters = []
            for m in data.get("memories", []):
                what = m["events"][0]["what"] if m.get("events") else ""
                title = what.split("\n")[0] if what else ""
                chapters.append({
                    "id": m["id"],
                    "title": title,
                    "preview": what[:200],
                })
            chapters.sort(key=lambda c: c["title"])
            self.send_json({"ok": True, "chapters": chapters, "count": len(chapters)})
        else:
            self.send_json({"ok": False, "error": str(data)}, 200)

    def handle_recall(self, data):
        query = data.get("query", "").strip()
        if not query:
            self.send_json({"ok": False, "error": "需要输入查询"})
            return
        status, data = self.call_taodb_user("POST", "/v1/recall", {
            "query": query,
            "top_k": data.get("top_k", 5),
            "field_marker": [],
        })
        self.send_json(data, status if status < 400 else 200)

    def handle_write_chapter(self, data):
        """Core: call LLM to write next chapter, using taodb for context."""
        chapter_num = data.get("chapter_num", "")
        chapter_title = data.get("title", "")
        outline = data.get("outline", "")
        if not chapter_title:
            self.send_json({"ok": False, "error": "Chapter title required"}, 400)
            return

        if not TAODB_LLM_CMD:
            self.send_json({"ok": False, "error": "TAODB_LLM_CMD not set. Set it to your LLM invocation command."})
            return

        # 1. Call taodb recall for context
        query = f"{chapter_title} {outline}".strip()
        status, recall = self.call_taodb_user("POST", "/v1/recall", {
            "query": query,
            "top_k": 5,
        })
        context = ""
        if status == 200:
            for m in recall.get("memories", [])[:5]:
                for ev in m.get("events", []):
                    context += ev.get("what", "") + "\n\n"
        context = context[:3000] if context else "(No prior chapters)"

        # 2. Build prompt
        prompt = f"""You are a novel-writing AI assistant. Write the next chapter as specified.

Style (most important — follow strictly):
- Use concrete bodily sensations (fingers tingling, toes curling, stomach rumbling) instead of abstract descriptions
- One paragraph = one action + one sense + one object
- End on an image, not a summary
- No modern abstract words (data, system, pattern, frequency, etc.)

Previous chapters (from taodb temporal-spatial window):
{context}

Chapter requirements:
Chapter {chapter_num}: {chapter_title}
{outline}

500-800 words. Write the chapter directly, no explanations.
"""

        # 3. Call LLM
        try:
            cmd = TAODB_LLM_CMD.split() + [
                "--model", "MiniMax-M3",
                "--max-tokens", "3500",
                "--temperature", "0.7",
                prompt,
            ]
            result = subprocess.run(
                cmd,
                capture_output=True, text=True, timeout=120,
            )
            if result.returncode != 0:
                self.send_json({"ok": False, "error": f"LLM failed: {result.stderr[:200]}"})
                return
            text = result.stdout.strip()
        except Exception as e:
            self.send_json({"ok": False, "error": f"LLM error: {e}"})
            return

        # 4. Save to taodb
        full_text = f"Chapter {chapter_num}: {chapter_title}\n\n{text}"
        status, ingest = self.call_taodb_user("POST", "/v1/memories", {
            "text": full_text,
        })

        # 5. Save to local file if save_dir specified
        save_dir = data.get("save_dir", "")
        local_path = ""
        if save_dir:
            os.makedirs(save_dir, exist_ok=True)
            local_path = os.path.join(save_dir, f"ch{chapter_num}-{chapter_title}.md")
            with open(local_path, "w") as f:
                f.write(full_text)

        self.send_json({
            "ok": True,
            "chapter_num": chapter_num,
            "title": chapter_title,
            "content": text,
            "word_count": len(text),
            "taodb_ingest": ingest,
            "local_path": local_path,
            "recall_count": len(recall.get("memories", [])) if status == 200 else 0,
        })


# ===== HTML / CSS / JS =====

INDEX_HTML = """<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="UTF-8">
<title>taodb 网文创作 IDE</title>
<link rel="stylesheet" href="/style.css">
</head>
<body>
<div id="app">
    <h1>📖 taodb 网文创作 IDE</h1>
    <p class="subtitle">用你的 taodb 时空记忆 + AI 帮你写下一章</p>

    <!-- 步骤 1：登录 -->
    <section id="step-login">
        <h2>第 1 步：登录</h2>
        <input id="email" type="email" placeholder="邮箱（任意即可）" value="demo@local">
        <button onclick="login()">登录</button>
        <p id="login-status" class="status"></p>
    </section>

    <!-- 步骤 2：建/选项目 -->
    <section id="step-project" style="display:none">
        <h2>第 2 步：项目</h2>
        <div id="projects-list"></div>
        <h3>或新建项目</h3>
        <input id="new-project-id" placeholder="项目 ID（英文/数字）">
        <input id="new-project-name" placeholder="项目名（你的书名）">
        <input id="save-dir" placeholder="保存到本地目录（可选，如 ~/my-novel）">
        <button onclick="createProject()">创建项目</button>
        <p id="project-status" class="status"></p>
    </section>

    <!-- 步骤 3：写章节 -->
    <section id="step-write" style="display:none">
        <h2>第 3 步：写下一章</h2>
        <p class="hint">taodb 会自动检索你之前写过的章节，AI 会参考风格写新章。</p>
        <input id="chapter-num" placeholder="章节号（如 70）">
        <input id="chapter-title" placeholder="章节标题（如 鹿骨之约）">
        <textarea id="chapter-outline" placeholder="本章大纲：谁、做什么、发生什么"></textarea>
        <button onclick="writeChapter()">✨ 写这一章</button>
        <div id="chapter-result"></div>
    </section>

    <!-- 章节列表 -->
    <section id="step-chapters" style="display:none">
        <h2>已写章节（taodb 记忆库）</h2>
        <button onclick="listChapters()">刷新列表</button>
        <div id="chapters-list"></div>
    </section>
</div>
<script src="/app.js"></script>
</body>
</html>
"""

STYLE_CSS = """
* { box-sizing: border-box; }
body {
    font-family: -apple-system, "PingFang SC", "Microsoft YaHei", sans-serif;
    background: #f5f5f5;
    margin: 0;
    padding: 20px;
    color: #333;
}
#app {
    max-width: 900px;
    margin: 0 auto;
    background: white;
    padding: 40px;
    border-radius: 8px;
    box-shadow: 0 2px 8px rgba(0,0,0,0.08);
}
h1 { color: #2c3e50; margin-bottom: 8px; font-size: 28px; }
.subtitle { color: #7f8c8d; margin-bottom: 30px; }
h2 { color: #34495e; margin-top: 30px; border-bottom: 2px solid #ecf0f1; padding-bottom: 8px; }
section { margin-bottom: 30px; }
input, textarea, button {
    font-size: 16px;
    padding: 10px 14px;
    margin: 6px 0;
    border: 1px solid #ddd;
    border-radius: 4px;
    width: 100%;
    font-family: inherit;
}
textarea { min-height: 80px; }
button {
    background: #3498db;
    color: white;
    border: none;
    cursor: pointer;
    width: auto;
    padding: 12px 24px;
    margin-top: 8px;
}
button:hover { background: #2980b9; }
button:disabled { background: #bdc3c7; cursor: not-allowed; }
.status { color: #7f8c8d; font-size: 14px; margin-top: 8px; }
.status.ok { color: #27ae60; }
.status.err { color: #c0392b; }
.hint { color: #95a5a6; font-size: 14px; font-style: italic; }
.chapter {
    background: #fafafa;
    padding: 16px;
    margin: 8px 0;
    border-radius: 4px;
    border-left: 3px solid #3498db;
}
.chapter-title { font-weight: bold; color: #2c3e50; }
.chapter-meta { color: #7f8c8d; font-size: 13px; margin-top: 4px; }
.chapter-preview { margin-top: 8px; color: #555; font-size: 14px; line-height: 1.6; }
.content-box {
    background: #fdfdfd;
    border: 1px solid #ecf0f1;
    padding: 20px;
    margin-top: 16px;
    border-radius: 4px;
    white-space: pre-wrap;
    line-height: 1.8;
    font-size: 16px;
    max-height: 500px;
    overflow-y: auto;
}
.loading {
    display: inline-block;
    padding: 8px 16px;
    background: #f39c12;
    color: white;
    border-radius: 4px;
}
"""

APP_JS = """
let currentToken = '';
let currentProject = '';

async function api(path, method='GET', data=null) {
    const opts = { method, headers: {} };
    if (data) {
        opts.headers['Content-Type'] = 'application/json';
        opts.body = JSON.stringify(data);
    }
    const r = await fetch(path, opts);
    return await r.json();
}

async function login() {
    const email = document.getElementById('email').value;
    const status = document.getElementById('login-status');
    status.textContent = '登录中...';
    const r = await api('/api/login', 'POST', { email });
    if (r.ok) {
        currentToken = r.token;
        status.className = 'status ok';
        status.textContent = '✓ 已登录：' + r.user;
        document.getElementById('step-project').style.display = 'block';
        loadProjects();
    } else {
        status.className = 'status err';
        status.textContent = '✗ 失败：' + JSON.stringify(r);
    }
}

async function loadProjects() {
    const r = await api('/api/projects');
    const div = document.getElementById('projects-list');
    if (r.projects && r.projects.length > 0) {
        div.innerHTML = '<h3>你的项目</h3>' + r.projects.map(p =>
            `<div class="chapter">
                <div class="chapter-title">${p.name}</div>
                <div class="chapter-meta">ID: ${p.project_id}</div>
                <button onclick="selectProject('${p.project_id}')">打开</button>
            </div>`
        ).join('');
    } else {
        div.innerHTML = '<p class="hint">还没有项目，创建你的第一个项目：</p>';
    }
}

async function selectProject(projectId) {
    currentProject = projectId;
    document.getElementById('project-status').textContent = '✓ 已选：' + projectId;
    document.getElementById('step-write').style.display = 'block';
    document.getElementById('step-chapters').style.display = 'block';
    listChapters();
}

async function createProject() {
    const projectId = document.getElementById('new-project-id').value.trim();
    const name = document.getElementById('new-project-name').value.trim();
    const saveDir = document.getElementById('save-dir').value.trim();
    const status = document.getElementById('project-status');
    if (!projectId || !name) {
        status.textContent = '请填项目 ID 和名称';
        return;
    }
    const r = await api('/api/projects', 'POST', { project_id: projectId, name, save_dir: saveDir });
    if (r.ok) {
        status.className = 'status ok';
        status.textContent = '✓ 项目创建成功';
        selectProject(projectId);
        loadProjects();
    } else {
        status.className = 'status err';
        status.textContent = '✗ ' + JSON.stringify(r);
    }
}

async function writeChapter() {
    const num = document.getElementById('chapter-num').value;
    const title = document.getElementById('chapter-title').value.trim();
    const outline = document.getElementById('chapter-outline').value.trim();
    const saveDir = document.getElementById('save-dir').value.trim();
    const result = document.getElementById('chapter-result');
    if (!title) {
        result.innerHTML = '<p class="status err">请填章节标题</p>';
        return;
    }
    result.innerHTML = '<p class="loading">taodb 检索 + AI 生成中...</p>';

    const r = await api('/api/chapters/write', 'POST', {
        chapter_num: num,
        title,
        outline,
        save_dir: saveDir,
    });
    if (r.ok) {
        result.innerHTML = `
            <h3>第 ${r.chapter_num} 回 · ${r.title}（${r.word_count} 字）</h3>
            <p class="hint">taodb 检索到 ${r.recall_count} 条相关记忆</p>
            <div class="content-box">${escapeHtml(r.content)}</div>
            ${r.local_path ? '<p class="status ok">已保存到：' + r.local_path + '</p>' : ''}
            <p class="status ok">已存入 taodb 时空记忆库</p>
            <button onclick="listChapters()">刷新章节列表</button>
        `;
        listChapters();
    } else {
        result.innerHTML = '<p class="status err">✗ ' + JSON.stringify(r) + '</p>';
    }
}

async function listChapters() {
    if (!currentProject) return;
    const r = await api('/api/chapters/list', 'POST', {});
    const div = document.getElementById('chapters-list');
    if (r.chapters && r.chapters.length > 0) {
        div.innerHTML = '<p class="status ok">共 ' + r.count + ' 章</p>' + r.chapters.map(c =>
            `<div class="chapter">
                <div class="chapter-title">${escapeHtml(c.title)}</div>
                <div class="chapter-preview">${escapeHtml(c.preview)}</div>
            </div>`
        ).join('');
    } else {
        div.innerHTML = '<p class="hint">还没有章节。先用第 3 步写第一章。</p>';
    }
}

function escapeHtml(s) {
    const div = document.createElement('div');
    div.textContent = s || '';
    return div.innerHTML;
}
"""


def main():
    # 确保 taodb daemon 跑了
    print(f"taodb-ziran 网文创作 IDE")
    print(f"  taodb: {TAODB_BASE}")
    print(f"  listen: http://127.0.0.1:{IDE_PORT}")
    print(f"  data:  {DATA_DIR}")
    os.makedirs(DATA_DIR, exist_ok=True)

    server = HTTPServer(("127.0.0.1", IDE_PORT), Handler)
    print(f"\n✓ Ready. Open: http://127.0.0.1:{IDE_PORT}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopped")


if __name__ == "__main__":
    main()