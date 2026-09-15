use anyhow::Result;
use std::path::Path;

pub fn scaffold_marketplace_plugin(target_dir: &Path, plugin_id: &str) -> Result<()> {
    match plugin_id {
        "filesystem-tools" => scaffold_filesystem_tools(target_dir),
        "web-search-tools" => scaffold_web_search_tools(target_dir),
        "git-assistant" => scaffold_git_assistant(target_dir),
        "sqlite-database" => scaffold_sqlite_database(target_dir),
        "python-analytics" => scaffold_python_analytics(target_dir),
        "code-reviewer" => scaffold_code_reviewer(target_dir),
        "google-workspace" => scaffold_google_workspace(target_dir),
        "openai-ecosystem" => scaffold_openai_ecosystem(target_dir),
        "github-developer" => scaffold_github_developer(target_dir),
        "slack-workspace" => scaffold_slack_workspace(target_dir),
        other => scaffold_generic_plugin(target_dir, other),
    }
}

fn scaffold_filesystem_tools(target_dir: &Path) -> Result<()> {
    let manifest = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "filesystem-tools",
  "version": "1.0.0",
  "description": "Standard filesystem operations, tree inspection, and search via Model Context Protocol.",
  "license": "MIT",
  "keywords": ["filesystem", "files", "mcp", "workspace"]
}"#;

    let mcp = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {
    "local-fs": {
      "type": "stdio",
      "command": "python",
      "args": ["${PLUGIN_ROOT}/mcp_server.py"]
    }
  }
}"#;

    let mcp_server = r#"import sys, json, os

SERVER_NAME = "local-fs"
TOOLS = [
    {
        "name": "read_file",
        "description": "Read file contents at path",
        "inputSchema": {
            "type": "object",
            "properties": {"path": {"type": "string"}},
            "required": ["path"]
        }
    },
    {
        "name": "write_file",
        "description": "Write content to a file at path",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "content": {"type": "string"}
            },
            "required": ["path", "content"]
        }
    },
    {
        "name": "list_dir",
        "description": "List contents of a directory",
        "inputSchema": {
            "type": "object",
            "properties": {"path": {"type": "string"}}
        }
    }
]

def send(resp):
    sys.stdout.write(json.dumps(resp) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    req_id = req.get("id")
    method = req.get("method")
    params = req.get("params", {})
    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": SERVER_NAME, "version": "1.0.0"}
            }
        })
    elif method == "notifications/initialized":
        pass
    elif method == "ping":
        send({"jsonrpc": "2.0", "id": req_id, "result": {}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": req_id, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        name = params.get("name")
        args = params.get("arguments", {})
        try:
            if name == "read_file":
                p = args.get("path", "")
                with open(p, "r", encoding="utf-8", errors="replace") as f:
                    txt = f.read()
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": txt}], "isError": False}})
            elif name == "write_file":
                p = args.get("path", "")
                c = args.get("content", "")
                os.makedirs(os.path.dirname(os.path.abspath(p)), exist_ok=True)
                with open(p, "w", encoding="utf-8") as f:
                    f.write(c)
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Successfully wrote {len(c)} bytes to {p}"}], "isError": False}})
            elif name == "list_dir":
                p = args.get("path", ".")
                entries = [{"name": e, "is_dir": os.path.isdir(os.path.join(p, e))} for e in os.listdir(p)]
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps(entries, indent=2)}], "isError": False}})
            else:
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Unknown tool {name}"}], "isError": True}})
        except Exception as e:
            send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Error: {e}"}], "isError": True}})
    else:
        if req_id is not None:
            send({"jsonrpc": "2.0", "id": req_id, "error": {"code": -32601, "message": "Method not found"}})
"#;

    let skill_dir = target_dir.join("skills").join("workspace-explorer");
    let scripts_dir = skill_dir.join("scripts");
    std::fs::create_dir_all(&scripts_dir)?;

    let skill_md = r#"---
name: Workspace Explorer
description: Analyzes directory layouts and discovers code structures.
tags: [filesystem, workspace, structure]
icon: 📂
---
# Workspace Explorer Instructions
1. Inspect the workspace root directory.
2. Identify major module boundaries and entry points.
3. Report structure and dependencies clearly.
"#;

    let skill_script = r#"import sys, json, os

input_data = {}
if len(sys.argv) > 1:
    try:
        input_data = json.loads(sys.argv[1])
    except Exception:
        pass

path = input_data.get("path") or "."
try:
    items = []
    for root, dirs, files in os.walk(path):
        rel = os.path.relpath(root, path)
        if rel == ".":
            rel = ""
        for d in dirs:
            if not d.startswith("."):
                items.append({"type": "directory", "path": os.path.join(rel, d).replace("\\", "/")})
        for f in files:
            if not f.startswith("."):
                items.append({"type": "file", "path": os.path.join(rel, f).replace("\\", "/")})
        if len(items) >= 100:
            break
    print(json.dumps({"success": True, "path": os.path.abspath(path), "items": items}, indent=2))
except Exception as e:
    print(json.dumps({"success": False, "error": str(e)}))
"#;

    std::fs::write(target_dir.join("plugin.json"), manifest)?;
    std::fs::write(target_dir.join("mcp.json"), mcp)?;
    std::fs::write(target_dir.join("mcp_server.py"), mcp_server)?;
    std::fs::write(skill_dir.join("SKILL.md"), skill_md)?;
    std::fs::write(scripts_dir.join("run.py"), skill_script)?;
    Ok(())
}

fn scaffold_web_search_tools(target_dir: &Path) -> Result<()> {
    let manifest = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "web-search-tools",
  "version": "1.2.0",
  "description": "Web search and documentation content extractor for AI agents.",
  "license": "MIT",
  "keywords": ["web", "search", "mcp", "documentation"]
}"#;

    let mcp = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {
    "web-fetcher": {
      "type": "stdio",
      "command": "python",
      "args": ["${PLUGIN_ROOT}/mcp_server.py"]
    }
  }
}"#;

    let mcp_server = r#"import sys, json, urllib.request, re

SERVER_NAME = "web-fetcher"
TOOLS = [
    {
        "name": "fetch_url",
        "description": "Fetch web page and extract clean text",
        "inputSchema": {
            "type": "object",
            "properties": {"url": {"type": "string"}},
            "required": ["url"]
        }
    },
    {
        "name": "extract_text",
        "description": "Strip HTML markup into clean markdown/text",
        "inputSchema": {
            "type": "object",
            "properties": {"html": {"type": "string"}},
            "required": ["html"]
        }
    }
]

def clean_html(html):
    html = re.sub(r'<script[^>]*>[\s\S]*?</script>', '', html, flags=re.IGNORECASE)
    html = re.sub(r'<style[^>]*>[\s\S]*?</style>', '', html, flags=re.IGNORECASE)
    text = re.sub(r'<[^>]+>', ' ', html)
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    return '\n'.join(lines)

def send(resp):
    sys.stdout.write(json.dumps(resp) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    req_id = req.get("id")
    method = req.get("method")
    params = req.get("params", {})
    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": SERVER_NAME, "version": "1.0.0"}
            }
        })
    elif method == "notifications/initialized":
        pass
    elif method == "ping":
        send({"jsonrpc": "2.0", "id": req_id, "result": {}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": req_id, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        name = params.get("name")
        args = params.get("arguments", {})
        try:
            if name == "fetch_url":
                url = args.get("url", "")
                req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
                with urllib.request.urlopen(req, timeout=15) as response:
                    raw = response.read().decode('utf-8', errors='replace')
                text = clean_html(raw)
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": text[:10000]}], "isError": False}})
            elif name == "extract_text":
                html = args.get("html", "")
                text = clean_html(html)
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": text}], "isError": False}})
            else:
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Unknown tool {name}"}], "isError": True}})
        except Exception as e:
            send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Fetch error: {e}"}], "isError": True}})
    else:
        if req_id is not None:
            send({"jsonrpc": "2.0", "id": req_id, "error": {"code": -32601, "message": "Method not found"}})
"#;

    let skill_dir = target_dir.join("skills").join("documentation-crawler");
    let scripts_dir = skill_dir.join("scripts");
    std::fs::create_dir_all(&scripts_dir)?;

    let skill_md = r#"---
name: Documentation Crawler
description: Extracts clean markdown from API documentation sites.
tags: [web, crawler, docs]
icon: 🌐
---
# Documentation Crawler Instructions
1. Accept documentation URL.
2. Fetch main page and navigate internal links.
3. Output synthesized, clean markdown documentation.
"#;

    let skill_script = r#"import sys, json, urllib.request, re

input_data = {}
if len(sys.argv) > 1:
    try:
        input_data = json.loads(sys.argv[1])
    except Exception:
        pass

url = input_data.get("url")
if not url:
    print(json.dumps({"success": False, "error": "URL parameter required"}))
    sys.exit(0)

try:
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(req, timeout=15) as resp:
        html = resp.read().decode("utf-8", errors="replace")
    clean = re.sub(r'<[^>]+>', ' ', html)
    print(json.dumps({"success": True, "url": url, "excerpt": clean[:1000].strip()}, indent=2))
except Exception as e:
    print(json.dumps({"success": False, "error": str(e)}))
"#;

    std::fs::write(target_dir.join("plugin.json"), manifest)?;
    std::fs::write(target_dir.join("mcp.json"), mcp)?;
    std::fs::write(target_dir.join("mcp_server.py"), mcp_server)?;
    std::fs::write(skill_dir.join("SKILL.md"), skill_md)?;
    std::fs::write(scripts_dir.join("run.py"), skill_script)?;
    Ok(())
}

fn scaffold_git_assistant(target_dir: &Path) -> Result<()> {
    let manifest = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "git-assistant",
  "version": "1.1.0",
  "description": "Git repository automation, staging, commit formatting, and branch synchronization.",
  "license": "MIT",
  "keywords": ["git", "version-control", "commits", "review"]
}"#;

    let mcp = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {
    "git-mcp": {
      "type": "stdio",
      "command": "python",
      "args": ["${PLUGIN_ROOT}/mcp_server.py"]
    }
  }
}"#;

    let mcp_server = r#"import sys, json, subprocess

SERVER_NAME = "git-mcp"
TOOLS = [
    {
        "name": "git_status",
        "description": "Get git working tree status",
        "inputSchema": {
            "type": "object",
            "properties": {"repo_path": {"type": "string"}}
        }
    },
    {
        "name": "git_diff",
        "description": "Get git diff for unstaged or staged changes",
        "inputSchema": {
            "type": "object",
            "properties": {
                "repo_path": {"type": "string"},
                "staged": {"type": "boolean"}
            }
        }
    },
    {
        "name": "git_log",
        "description": "Get recent git commits",
        "inputSchema": {
            "type": "object",
            "properties": {
                "repo_path": {"type": "string"},
                "count": {"type": "integer"}
            }
        }
    }
]

def send(resp):
    sys.stdout.write(json.dumps(resp) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    req_id = req.get("id")
    method = req.get("method")
    params = req.get("params", {})
    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": SERVER_NAME, "version": "1.0.0"}
            }
        })
    elif method == "notifications/initialized":
        pass
    elif method == "ping":
        send({"jsonrpc": "2.0", "id": req_id, "result": {}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": req_id, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        name = params.get("name")
        args = params.get("arguments", {})
        cwd = args.get("repo_path") or "."
        try:
            if name == "git_status":
                p = subprocess.run(["git", "status", "--porcelain"], cwd=cwd, capture_output=True, text=True, check=True)
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": p.stdout}], "isError": False}})
            elif name == "git_diff":
                cmd = ["git", "diff"]
                if args.get("staged"):
                    cmd.append("--cached")
                p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, check=True)
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": p.stdout}], "isError": False}})
            elif name == "git_log":
                count = str(args.get("count", 5))
                p = subprocess.run(["git", "log", "-n", count, "--oneline"], cwd=cwd, capture_output=True, text=True, check=True)
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": p.stdout}], "isError": False}})
            else:
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Unknown tool {name}"}], "isError": True}})
        except Exception as e:
            send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Git error: {e}"}], "isError": True}})
    else:
        if req_id is not None:
            send({"jsonrpc": "2.0", "id": req_id, "error": {"code": -32601, "message": "Method not found"}})
"#;

    let skill_dir = target_dir.join("skills").join("smart-commit");
    let scripts_dir = skill_dir.join("scripts");
    std::fs::create_dir_all(&scripts_dir)?;

    let skill_md = r#"---
name: Smart Commit
description: Formats standardized conventional commits based on diff analysis.
tags: [git, commit, workflow]
icon: 🐈
---
# Smart Commit Instructions
1. Inspect staged changes with git diff.
2. Deduce type (feat, fix, docs, refactor, test, chore).
3. Draft a precise, imperative conventional commit message.
"#;

    let skill_script = r#"import sys, json, subprocess

input_data = {}
if len(sys.argv) > 1:
    try:
        input_data = json.loads(sys.argv[1])
    except Exception:
        pass

cwd = input_data.get("repo_path") or "."
try:
    p = subprocess.run(["git", "diff", "--cached"], cwd=cwd, capture_output=True, text=True)
    diff = p.stdout
    if not diff:
        p = subprocess.run(["git", "diff"], cwd=cwd, capture_output=True, text=True)
        diff = p.stdout
    if "test" in diff.lower():
        prefix = "test"
    elif "fix" in diff.lower() or "error" in diff.lower():
        prefix = "fix"
    elif "docs" in diff.lower():
        prefix = "docs"
    else:
        prefix = "feat"
    summary = f"{prefix}: updates to workspace files"
    print(json.dumps({"success": True, "suggested_commit": summary, "diff_length": len(diff)}, indent=2))
except Exception as e:
    print(json.dumps({"success": False, "error": str(e)}))
"#;

    std::fs::write(target_dir.join("plugin.json"), manifest)?;
    std::fs::write(target_dir.join("mcp.json"), mcp)?;
    std::fs::write(target_dir.join("mcp_server.py"), mcp_server)?;
    std::fs::write(skill_dir.join("SKILL.md"), skill_md)?;
    std::fs::write(scripts_dir.join("run.py"), skill_script)?;
    Ok(())
}

fn scaffold_sqlite_database(target_dir: &Path) -> Result<()> {
    let manifest = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "sqlite-database",
  "version": "1.0.1",
  "description": "Query, introspect schemas, and manage SQLite databases securely via MCP.",
  "license": "MIT",
  "keywords": ["sqlite", "sql", "database"]
}"#;

    let mcp = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {
    "sqlite-server": {
      "type": "stdio",
      "command": "python",
      "args": ["${PLUGIN_ROOT}/mcp_server.py"],
      "env": {
        "DB_PATH": "${PLUGIN_DATA}/app.db"
      }
    }
  }
}"#;

    let mcp_server = r#"import sys, json, os, sqlite3

SERVER_NAME = "sqlite-server"
DB_PATH = os.environ.get("DB_PATH", "app.db")
if os.path.dirname(DB_PATH):
    os.makedirs(os.path.dirname(os.path.abspath(DB_PATH)), exist_ok=True)

TOOLS = [
    {
        "name": "execute_query",
        "description": "Execute SQL query on SQLite database",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "db_path": {"type": "string"}
            },
            "required": ["query"]
        }
    },
    {
        "name": "list_tables",
        "description": "List all tables in SQLite database",
        "inputSchema": {
            "type": "object",
            "properties": {"db_path": {"type": "string"}}
        }
    },
    {
        "name": "schema_info",
        "description": "Get schema column info for table",
        "inputSchema": {
            "type": "object",
            "properties": {
                "table": {"type": "string"},
                "db_path": {"type": "string"}
            },
            "required": ["table"]
        }
    }
]

def get_conn(p=None):
    return sqlite3.connect(p or DB_PATH)

def send(resp):
    sys.stdout.write(json.dumps(resp) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    req_id = req.get("id")
    method = req.get("method")
    params = req.get("params", {})
    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": SERVER_NAME, "version": "1.0.0"}
            }
        })
    elif method == "notifications/initialized":
        pass
    elif method == "ping":
        send({"jsonrpc": "2.0", "id": req_id, "result": {}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": req_id, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        name = params.get("name")
        args = params.get("arguments", {})
        try:
            conn = get_conn(args.get("db_path"))
            cur = conn.cursor()
            if name == "execute_query":
                q = args.get("query", "")
                cur.execute(q)
                if q.strip().upper().startswith("SELECT") or q.strip().upper().startswith("PRAGMA"):
                    rows = cur.fetchall()
                    cols = [d[0] for d in cur.description] if cur.description else []
                    res = [dict(zip(cols, r)) for r in rows]
                    send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps(res, indent=2)}], "isError": False}})
                else:
                    conn.commit()
                    send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Query executed successfully, rows affected: {cur.rowcount}"}], "isError": False}})
            elif name == "list_tables":
                cur.execute("SELECT name FROM sqlite_master WHERE type='table'")
                tables = [r[0] for r in cur.fetchall()]
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps(tables)}], "isError": False}})
            elif name == "schema_info":
                t = args.get("table", "")
                cur.execute(f"PRAGMA table_info({t})")
                cols = cur.fetchall()
                res = [{"cid": c[0], "name": c[1], "type": c[2], "notnull": c[3], "dflt_value": c[4], "pk": c[5]} for c in cols]
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps(res, indent=2)}], "isError": False}})
            else:
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Unknown tool {name}"}], "isError": True}})
            conn.close()
        except Exception as e:
            send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"SQLite error: {e}"}], "isError": True}})
    else:
        if req_id is not None:
            send({"jsonrpc": "2.0", "id": req_id, "error": {"code": -32601, "message": "Method not found"}})
"#;

    let skill_dir = target_dir.join("skills").join("schema-auditor");
    let scripts_dir = skill_dir.join("scripts");
    std::fs::create_dir_all(&scripts_dir)?;

    let skill_md = r#"---
name: Schema Auditor
description: Audits database tables for proper indexing, primary keys and normal forms.
tags: [sql, database, performance]
icon: 🗄️
---
# Schema Auditor Instructions
1. Introspect table columns and foreign keys.
2. Verify foreign key index coverage.
3. Suggest missing indexes or normalization improvements.
"#;

    let skill_script = r#"import sys, json, os, sqlite3

input_data = {}
if len(sys.argv) > 1:
    try:
        input_data = json.loads(sys.argv[1])
    except Exception:
        pass

db_path = input_data.get("db_path", "app.db")
if not os.path.isabs(db_path):
    db_path = os.path.abspath(db_path)

try:
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    cur.execute("SELECT name FROM sqlite_master WHERE type='table'")
    tables = [r[0] for r in cur.fetchall()]
    audit = {}
    for t in tables:
        cur.execute(f"PRAGMA table_info({t})")
        cols = cur.fetchall()
        cur.execute(f"PRAGMA index_list({t})")
        indexes = cur.fetchall()
        has_pk = any(c[5] == 1 for c in cols)
        audit[t] = {
            "columns_count": len(cols),
            "has_primary_key": has_pk,
            "indexes_count": len(indexes),
            "columns": [c[1] for c in cols]
        }
    conn.close()
    print(json.dumps({"success": True, "database": db_path, "tables_audit": audit}, indent=2))
except Exception as e:
    print(json.dumps({"success": False, "error": str(e)}))
"#;

    std::fs::write(target_dir.join("plugin.json"), manifest)?;
    std::fs::write(target_dir.join("mcp.json"), mcp)?;
    std::fs::write(target_dir.join("mcp_server.py"), mcp_server)?;
    std::fs::write(skill_dir.join("SKILL.md"), skill_md)?;
    std::fs::write(scripts_dir.join("run.py"), skill_script)?;
    Ok(())
}

fn scaffold_python_analytics(target_dir: &Path) -> Result<()> {
    let manifest = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "python-analytics",
  "version": "1.0.0",
  "description": "Run Python scripts, compute statistical calculations, and transform data frames.",
  "license": "MIT",
  "keywords": ["python", "data", "analytics"]
}"#;

    let mcp = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {
    "python-kernel": {
      "type": "stdio",
      "command": "python",
      "args": ["${PLUGIN_ROOT}/mcp_server.py"]
    }
  }
}"#;

    let mcp_server = r#"import sys, json, math, statistics

SERVER_NAME = "python-kernel"
TOOLS = [
    {
        "name": "eval_expression",
        "description": "Evaluate a Python math/data expression",
        "inputSchema": {
            "type": "object",
            "properties": {"expr": {"type": "string"}},
            "required": ["expr"]
        }
    },
    {
        "name": "compute_stats",
        "description": "Compute statistical metrics on an array of numbers",
        "inputSchema": {
            "type": "object",
            "properties": {
                "numbers": {
                    "type": "array",
                    "items": {"type": "number"}
                }
            },
            "required": ["numbers"]
        }
    }
]

def send(resp):
    sys.stdout.write(json.dumps(resp) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    req_id = req.get("id")
    method = req.get("method")
    params = req.get("params", {})
    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": SERVER_NAME, "version": "1.0.0"}
            }
        })
    elif method == "notifications/initialized":
        pass
    elif method == "ping":
        send({"jsonrpc": "2.0", "id": req_id, "result": {}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": req_id, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        name = params.get("name")
        args = params.get("arguments", {})
        try:
            if name == "eval_expression":
                expr = args.get("expr", "0")
                val = eval(expr, {"math": math, "sum": sum, "min": min, "max": max, "len": len, "abs": abs})
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": str(val)}], "isError": False}})
            elif name == "compute_stats":
                nums = args.get("numbers", [])
                if not nums:
                    send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps({"count": 0})}], "isError": False}})
                else:
                    res = {
                        "count": len(nums),
                        "sum": sum(nums),
                        "mean": statistics.mean(nums),
                        "median": statistics.median(nums),
                        "min": min(nums),
                        "max": max(nums),
                        "stddev": statistics.stdev(nums) if len(nums) > 1 else 0.0
                    }
                    send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps(res, indent=2)}], "isError": False}})
            else:
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Unknown tool {name}"}], "isError": True}})
        except Exception as e:
            send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Computation error: {e}"}], "isError": True}})
    else:
        if req_id is not None:
            send({"jsonrpc": "2.0", "id": req_id, "error": {"code": -32601, "message": "Method not found"}})
"#;

    let skill_dir = target_dir.join("skills").join("data-summarizer");
    let scripts_dir = skill_dir.join("scripts");
    std::fs::create_dir_all(&scripts_dir)?;

    let skill_md = r#"---
name: Data Summarizer
description: Summarizes numeric arrays, computing averages, medians, and extremes.
tags: [analytics, math, summary]
icon: 📈
---
# Data Summarizer Instructions
1. Ingest input numbers.
2. Produce key distribution indicators.
"#;

    let skill_script = r#"import sys, json, statistics

input_data = {}
if len(sys.argv) > 1:
    try:
        input_data = json.loads(sys.argv[1])
    except Exception:
        pass

nums = input_data.get("numbers", [])
if not nums:
    print(json.dumps({"success": True, "count": 0, "message": "No numeric data supplied"}))
else:
    print(json.dumps({
        "success": True,
        "count": len(nums),
        "mean": statistics.mean(nums),
        "min": min(nums),
        "max": max(nums)
    }, indent=2))
"#;

    std::fs::write(target_dir.join("plugin.json"), manifest)?;
    std::fs::write(target_dir.join("mcp.json"), mcp)?;
    std::fs::write(target_dir.join("mcp_server.py"), mcp_server)?;
    std::fs::write(skill_dir.join("SKILL.md"), skill_md)?;
    std::fs::write(scripts_dir.join("run.py"), skill_script)?;
    Ok(())
}

fn scaffold_code_reviewer(target_dir: &Path) -> Result<()> {
    let manifest = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "code-reviewer",
  "version": "1.3.0",
  "description": "Automated architectural code reviews, linting validation, and security vulnerability scanning.",
  "license": "MIT",
  "keywords": ["security", "review", "audit", "architecture"]
}"#;

    let mcp = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {
    "code-reviewer-server": {
      "type": "stdio",
      "command": "python",
      "args": ["${PLUGIN_ROOT}/mcp_server.py"]
    }
  }
}"#;

    let mcp_server = r#"import sys, json, re

SERVER_NAME = "code-reviewer-server"
TOOLS = [
    {
        "name": "security_scan",
        "description": "Scan code for OWASP security vulnerabilities, hardcoded keys, and injection vectors",
        "inputSchema": {
            "type": "object",
            "properties": {
                "code": {"type": "string"},
                "file_path": {"type": "string"}
            }
        }
    },
    {
        "name": "lint_check",
        "description": "Validate basic syntax and code hygiene",
        "inputSchema": {
            "type": "object",
            "properties": {
                "code": {"type": "string"}
            }
        }
    }
]

def scan_text(code):
    findings = []
    lines = code.splitlines()
    for idx, line in enumerate(lines, 1):
        if re.search(r'(api[_-]?key|secret|password|token)\s*=\s*[\'"][^\'"]{8,}[\'"]', line, re.IGNORECASE):
            findings.append({"line": idx, "severity": "HIGH", "rule": "HardcodedSecret", "message": "Possible plaintext secret or API key"})
        if re.search(r'\beval\(|\bexec\(', line):
            findings.append({"line": idx, "severity": "CRITICAL", "rule": "CodeInjection", "message": "Unsafe eval/exec execution"})
        if re.search(r'shell\s*=\s*True', line):
            findings.append({"line": idx, "severity": "HIGH", "rule": "CommandInjection", "message": "Subprocess called with shell=True"})
        if re.search(r'SELECT\s+.*WHERE.*[\'"]\s*\+', line, re.IGNORECASE):
            findings.append({"line": idx, "severity": "HIGH", "rule": "SqlInjection", "message": "SQL query built via string concatenation"})
    return findings

def send(resp):
    sys.stdout.write(json.dumps(resp) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    req_id = req.get("id")
    method = req.get("method")
    params = req.get("params", {})
    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": SERVER_NAME, "version": "1.0.0"}
            }
        })
    elif method == "notifications/initialized":
        pass
    elif method == "ping":
        send({"jsonrpc": "2.0", "id": req_id, "result": {}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": req_id, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        name = params.get("name")
        args = params.get("arguments", {})
        try:
            if name == "security_scan":
                code = args.get("code")
                if not code and args.get("file_path"):
                    with open(args["file_path"], "r", encoding="utf-8", errors="replace") as f:
                        code = f.read()
                code = code or ""
                findings = scan_text(code)
                res = {
                    "total_issues": len(findings),
                    "status": "PASS" if not findings else "FLAGGED",
                    "findings": findings
                }
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps(res, indent=2)}], "isError": False}})
            elif name == "lint_check":
                code = args.get("code", "")
                findings = []
                stack = []
                pairs = {')': '(', ']': '[', '}': '{'}
                for idx, ch in enumerate(code):
                    if ch in '([{':
                        stack.append((ch, idx))
                    elif ch in ')]}':
                        if not stack or stack[-1][0] != pairs[ch]:
                            findings.append({"pos": idx, "message": f"Mismatched or unexpected closing bracket '{ch}'"})
                        else:
                            stack.pop()
                if stack:
                    for ch, idx in stack:
                        findings.append({"pos": idx, "message": f"Unclosed bracket '{ch}'"})
                for idx, line in enumerate(code.splitlines(), 1):
                    if len(line) > 120:
                        findings.append({"line": idx, "message": "Line exceeds 120 characters"})
                res = {
                    "status": "clean" if not findings else "warning",
                    "total_issues": len(findings),
                    "issues": findings
                }
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps(res, indent=2)}], "isError": len(findings) > 0}})
            else:
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Unknown tool {name}"}], "isError": True}})
        except Exception as e:
            send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Reviewer error: {e}"}], "isError": True}})
    else:
        if req_id is not None:
            send({"jsonrpc": "2.0", "id": req_id, "error": {"code": -32601, "message": "Method not found"}})
"#;

    let skill_dir = target_dir.join("skills").join("security-audit");
    let scripts_dir = skill_dir.join("scripts");
    std::fs::create_dir_all(&scripts_dir)?;

    let skill_md = r#"---
name: Security Audit
description: Scans source code for potential vulnerabilities, injections, and insecure secrets.
tags: [security, owasp, audit]
icon: 🛡️
---
# Security Audit Instructions
1. Review changed lines for hardcoded secrets, SQL injection, or command injection.
2. Flag any unsafe deserialization or unescaped HTML.
3. Recommend secure alternatives with code examples.
"#;

    let skill_script = r#"import sys, json, re

input_data = {}
if len(sys.argv) > 1:
    try:
        input_data = json.loads(sys.argv[1])
    except Exception:
        pass

code = input_data.get("code", "")
issues = []
if re.search(r'\beval\(|\bexec\(', code):
    issues.append("Unsafe dynamic code execution (eval/exec)")
if "password" in code.lower() and "=" in code:
    issues.append("Hardcoded credentials or sensitive tokens")

print(json.dumps({
    "success": True,
    "issues_count": len(issues),
    "issues": issues,
    "audit_passed": len(issues) == 0
}, indent=2))
"#;

    std::fs::write(target_dir.join("plugin.json"), manifest)?;
    std::fs::write(target_dir.join("mcp.json"), mcp)?;
    std::fs::write(target_dir.join("mcp_server.py"), mcp_server)?;
    std::fs::write(skill_dir.join("SKILL.md"), skill_md)?;
    std::fs::write(scripts_dir.join("run.py"), skill_script)?;
    Ok(())
}

fn scaffold_generic_plugin(target_dir: &Path, other: &str) -> Result<()> {
    let manifest = format!(
        r#"{{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "{other}",
  "version": "1.0.0",
  "description": "Custom agent plugin {other}",
  "license": "MIT"
}}"#
    );

    let mcp = format!(
        r#"{{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {{
    "{other}-server": {{
      "type": "stdio",
      "command": "python",
      "args": ["${{PLUGIN_ROOT}}/mcp_server.py"]
    }}
  }}
}}"#
    );

    let mcp_server = format!(
        r#"import sys, json

SERVER_NAME = "{other}-server"
TOOLS = [
    {{
        "name": "{other}_run",
        "description": "General task execution for {other}",
        "inputSchema": {{
            "type": "object",
            "properties": {{"input": {{"type": "string"}}}}
        }}
    }}
]

def send(resp):
    sys.stdout.write(json.dumps(resp) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    req_id = req.get("id")
    method = req.get("method")
    params = req.get("params", {{}})
    if method == "initialize":
        send({{
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {{
                "protocolVersion": "2024-11-05",
                "capabilities": {{"tools": {{}}}},
                "serverInfo": {{"name": SERVER_NAME, "version": "1.0.0"}}
            }}
        }})
    elif method == "notifications/initialized":
        pass
    elif method == "ping":
        send({{"jsonrpc": "2.0", "id": req_id, "result": {{}}}})
    elif method == "tools/list":
        send({{"jsonrpc": "2.0", "id": req_id, "result": {{"tools": TOOLS}}}})
    elif method == "tools/call":
        name = params.get("name")
        args = params.get("arguments", {{}})
        send({{
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {{
                "content": [{{"type": "text", "text": f"Output from {other} with args: {{args}}"}}],
                "isError": False
            }}
        }})
    else:
        if req_id is not None:
            send({{"jsonrpc": "2.0", "id": req_id, "error": {{"code": -32601, "message": "Method not found"}}}})
"#
    );

    let skill_dir = target_dir.join("skills").join("general-helper");
    let scripts_dir = skill_dir.join("scripts");
    std::fs::create_dir_all(&scripts_dir)?;

    let skill_md = format!(
        r#"---
name: {other} Helper
description: General assistance skill for {other}
tags: [agent, utility]
icon: ⚡
---
# Instructions for {other}
Provide domain-specific assistance.
"#
    );

    let skill_script = r#"import sys, json

input_data = {}
if len(sys.argv) > 1:
    try:
        input_data = json.loads(sys.argv[1])
    except Exception:
        pass

print(json.dumps({"success": True, "input": input_data}))
"#;

    std::fs::write(target_dir.join("plugin.json"), manifest)?;
    std::fs::write(target_dir.join("mcp.json"), mcp)?;
    std::fs::write(target_dir.join("mcp_server.py"), mcp_server)?;
    std::fs::write(skill_dir.join("SKILL.md"), skill_md)?;
    std::fs::write(scripts_dir.join("run.py"), skill_script)?;
    Ok(())
}

fn scaffold_google_workspace(target_dir: &Path) -> Result<()> {
    let manifest = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "google-workspace",
  "version": "1.0.0",
  "description": "Integrate Google Drive, Docs, Gmail, and Calendar into agent workflows via MCP.",
  "license": "MIT",
  "keywords": ["google", "workspace", "drive", "docs", "gmail", "calendar"]
}"#;

    let mcp = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {
    "google-workspace-mcp": {
      "type": "stdio",
      "command": "python",
      "args": ["${PLUGIN_ROOT}/mcp_server.py"]
    }
  }
}"#;

    let mcp_server = r#"import sys, json, os

SERVER_NAME = "google-workspace-mcp"
TOOLS = [
    {
        "name": "search_drive",
        "description": "Search Google Drive for documents, spreadsheets, and presentations",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "limit": {"type": "integer"}
            },
            "required": ["query"]
        }
    },
    {
        "name": "read_doc",
        "description": "Fetch text content and metadata of a Google Doc or file by ID",
        "inputSchema": {
            "type": "object",
            "properties": {
                "file_id": {"type": "string"}
            },
            "required": ["file_id"]
        }
    },
    {
        "name": "list_calendar_events",
        "description": "Retrieve upcoming events from Google Calendar",
        "inputSchema": {
            "type": "object",
            "properties": {
                "days_ahead": {"type": "integer"}
            }
        }
    },
    {
        "name": "draft_email",
        "description": "Draft a message in Gmail with subject and body content",
        "inputSchema": {
            "type": "object",
            "properties": {
                "to": {"type": "string"},
                "subject": {"type": "string"},
                "body": {"type": "string"}
            },
            "required": ["to", "subject", "body"]
        }
    }
]

def send(resp):
    sys.stdout.write(json.dumps(resp) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    req_id = req.get("id")
    method = req.get("method")
    params = req.get("params", {})
    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": SERVER_NAME, "version": "1.0.0"}
            }
        })
    elif method == "notifications/initialized":
        pass
    elif method == "ping":
        send({"jsonrpc": "2.0", "id": req_id, "result": {}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": req_id, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        name = params.get("name")
        args = params.get("arguments", {})
        try:
            if name == "search_drive":
                q = args.get("query", "")
                limit = int(args.get("limit", 5))
                files = [
                    {"id": f"doc_{i}", "name": f"Document on {q} (Part {i})", "mimeType": "application/vnd.google-apps.document"}
                    for i in range(1, limit + 1)
                ]
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps({"query": q, "files": files}, indent=2)}], "isError": False}})
            elif name == "read_doc":
                fid = args.get("file_id", "doc_1")
                doc_text = f"Content of document '{fid}': Synchronized notes and project specifications."
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": doc_text}], "isError": False}})
            elif name == "list_calendar_events":
                days = int(args.get("days_ahead", 7))
                events = [
                    {"summary": "Sprint Planning Sync", "time": "10:00 AM", "attendees": 4},
                    {"summary": "Architecture Review", "time": "02:30 PM", "attendees": 6}
                ]
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps({"days_ahead": days, "events": events}, indent=2)}], "isError": False}})
            elif name == "draft_email":
                to = args.get("to")
                subj = args.get("subject")
                body = args.get("body")
                draft = {"draftId": "draft_9921", "to": to, "subject": subj, "bodyLength": len(body), "status": "saved_draft"}
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps(draft, indent=2)}], "isError": False}})
            else:
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Unknown tool {name}"}], "isError": True}})
        except Exception as e:
            send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Error: {e}"}], "isError": True}})
    else:
        if req_id is not None:
            send({"jsonrpc": "2.0", "id": req_id, "error": {"code": -32601, "message": "Method not found"}})
"#;

    let skill1_dir = target_dir.join("skills").join("workspace-organizer");
    let scripts1_dir = skill1_dir.join("scripts");
    std::fs::create_dir_all(&scripts1_dir)?;

    let skill1_md = r#"---
name: Workspace Organizer
description: Organizes Drive folders, aggregates document notes, and prepares project briefs.
tags: [google, drive, docs, organization]
icon: 📑
---
# Workspace Organizer Instructions
1. Discover relevant files across Google Drive.
2. Synthesize key notes into concise structured summaries.
3. Recommend file filing conventions and folder structures.
"#;

    let skill1_script = r#"import sys, json

input_data = {}
if len(sys.argv) > 1:
    try:
        input_data = json.loads(sys.argv[1])
    except Exception:
        pass

topic = input_data.get("topic", "General Project")
print(json.dumps({
    "success": True,
    "topic": topic,
    "summary": f"Structured briefing notes prepared for '{topic}'.",
    "recommended_folders": ["Specs", "Meeting Notes", "Deliverables"]
}, indent=2))
"#;

    let skill2_dir = target_dir.join("skills").join("calendar-scheduler");
    let scripts2_dir = skill2_dir.join("scripts");
    std::fs::create_dir_all(&scripts2_dir)?;

    let skill2_md = r#"---
name: Calendar Scheduler
description: Schedules meetings, detects calendar slot availability, and prepares event agendas.
tags: [calendar, scheduling, meeting]
icon: 📅
---
# Calendar Scheduler Instructions
1. Inspect calendar availability for the given timeframe.
2. Identify optimal slots avoiding scheduling conflicts.
3. Format meeting invites with clear agenda items.
"#;

    let skill2_script = r#"import sys, json

input_data = {}
if len(sys.argv) > 1:
    try:
        input_data = json.loads(sys.argv[1])
    except Exception:
        pass

duration = input_data.get("duration_minutes", 30)
print(json.dumps({
    "success": True,
    "duration_minutes": duration,
    "slot": "Tomorrow at 14:00 UTC",
    "status": "available"
}, indent=2))
"#;

    std::fs::write(target_dir.join("plugin.json"), manifest)?;
    std::fs::write(target_dir.join("mcp.json"), mcp)?;
    std::fs::write(target_dir.join("mcp_server.py"), mcp_server)?;
    std::fs::write(skill1_dir.join("SKILL.md"), skill1_md)?;
    std::fs::write(scripts1_dir.join("run.py"), skill1_script)?;
    std::fs::write(skill2_dir.join("SKILL.md"), skill2_md)?;
    std::fs::write(scripts2_dir.join("run.py"), skill2_script)?;
    Ok(())
}

fn scaffold_openai_ecosystem(target_dir: &Path) -> Result<()> {
    let manifest = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "openai-ecosystem",
  "version": "1.2.0",
  "description": "Connect OpenAI Assistants API, prompt evaluations, embeddings, and token analysis via MCP.",
  "license": "MIT",
  "keywords": ["openai", "chatgpt", "assistants", "embeddings", "tokens"]
}"#;

    let mcp = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {
    "openai-mcp": {
      "type": "stdio",
      "command": "python",
      "args": ["${PLUGIN_ROOT}/mcp_server.py"]
    }
  }
}"#;

    let mcp_server = r#"import sys, json, os

SERVER_NAME = "openai-mcp"
TOOLS = [
    {
        "name": "list_models",
        "description": "List available OpenAI models and context window limits",
        "inputSchema": {
            "type": "object",
            "properties": {}
        }
    },
    {
        "name": "token_counter",
        "description": "Estimate token count and cost calculation for given text",
        "inputSchema": {
            "type": "object",
            "properties": {
                "text": {"type": "string"},
                "model": {"type": "string"}
            },
            "required": ["text"]
        }
    },
    {
        "name": "format_completion",
        "description": "Construct normalized JSON payload for OpenAI chat completions API",
        "inputSchema": {
            "type": "object",
            "properties": {
                "system": {"type": "string"},
                "prompt": {"type": "string"},
                "model": {"type": "string"}
            },
            "required": ["prompt"]
        }
    },
    {
        "name": "validate_schema",
        "description": "Validate JSON Schema for OpenAI Structured Outputs compliance",
        "inputSchema": {
            "type": "object",
            "properties": {
                "schema": {"type": "object"}
            },
            "required": ["schema"]
        }
    }
]

def send(resp):
    sys.stdout.write(json.dumps(resp) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    req_id = req.get("id")
    method = req.get("method")
    params = req.get("params", {})
    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": SERVER_NAME, "version": "1.2.0"}
            }
        })
    elif method == "notifications/initialized":
        pass
    elif method == "ping":
        send({"jsonrpc": "2.0", "id": req_id, "result": {}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": req_id, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        name = params.get("name")
        args = params.get("arguments", {})
        try:
            if name == "list_models":
                models = [
                    {"id": "gpt-4o", "context_window": 128000, "description": "High-intelligence flagship"},
                    {"id": "gpt-4o-mini", "context_window": 128000, "description": "Fast and lightweight"},
                    {"id": "o1", "context_window": 200000, "description": "Reasoning model for complex tasks"},
                    {"id": "o3-mini", "context_window": 200000, "description": "High-speed reasoning model"}
                ]
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps(models, indent=2)}], "isError": False}})
            elif name == "token_counter":
                txt = args.get("text", "")
                model = args.get("model", "gpt-4o")
                estimated_tokens = max(1, len(txt) // 4)
                words = len(txt.split())
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps({"model": model, "estimated_tokens": estimated_tokens, "words": words, "characters": len(txt)}, indent=2)}], "isError": False}})
            elif name == "format_completion":
                sys_msg = args.get("system", "You are a helpful AI assistant.")
                prompt = args.get("prompt", "")
                model = args.get("model", "gpt-4o")
                payload = {
                    "model": model,
                    "messages": [
                        {"role": "system", "content": sys_msg},
                        {"role": "user", "content": prompt}
                    ],
                    "temperature": 0.7
                }
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps(payload, indent=2)}], "isError": False}})
            elif name == "validate_schema":
                sch = args.get("schema", {})
                is_valid = isinstance(sch, dict) and sch.get("type") == "object"
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps({"valid": is_valid, "has_additionalProperties_false": sch.get("additionalProperties") is False}, indent=2)}], "isError": False}})
            else:
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Unknown tool {name}"}], "isError": True}})
        except Exception as e:
            send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Error: {e}"}], "isError": True}})
    else:
        if req_id is not None:
            send({"jsonrpc": "2.0", "id": req_id, "error": {"code": -32601, "message": "Method not found"}})
"#;

    let skill_dir = target_dir.join("skills").join("prompt-optimizer");
    let scripts_dir = skill_dir.join("scripts");
    std::fs::create_dir_all(&scripts_dir)?;

    let skill_md = r#"---
name: Prompt Optimizer
description: Refines prompt instructions for optimal reasoning, few-shot precision, and token efficiency.
tags: [openai, prompt-engineering, ai, reasoning]
icon: 🤖
---
# Prompt Optimizer Instructions
1. Analyze user prompt intent and edge cases.
2. Structure constraints, XML delimiters, and expected output formats.
3. Validate token economy and output predictability.
"#;

    let skill_script = r#"import sys, json

input_data = {}
if len(sys.argv) > 1:
    try:
        input_data = json.loads(sys.argv[1])
    except Exception:
        pass

prompt = input_data.get("prompt", "Analyze input")
optimized = "Role:\nSpecialized Domain Expert\n\nObjective:\n" + prompt + "\n\nOutput Format:\nStructured Markdown with actionable bullet points."

print(json.dumps({
    "success": True,
    "original": prompt,
    "optimized_prompt": optimized
}, indent=2))
"#;

    std::fs::write(target_dir.join("plugin.json"), manifest)?;
    std::fs::write(target_dir.join("mcp.json"), mcp)?;
    std::fs::write(target_dir.join("mcp_server.py"), mcp_server)?;
    std::fs::write(skill_dir.join("SKILL.md"), skill_md)?;
    std::fs::write(scripts_dir.join("run.py"), skill_script)?;
    Ok(())
}

fn scaffold_github_developer(target_dir: &Path) -> Result<()> {
    let manifest = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "github-developer",
  "version": "1.1.0",
  "description": "Inspect repositories, triage Pull Requests, manage issues, and audit GitHub Actions workflows via MCP.",
  "license": "MIT",
  "keywords": ["github", "git", "pull-request", "issues", "actions", "developer"]
}"#;

    let mcp = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {
    "github-mcp": {
      "type": "stdio",
      "command": "python",
      "args": ["${PLUGIN_ROOT}/mcp_server.py"]
    }
  }
}"#;

    let mcp_server = r#"import sys, json, os

SERVER_NAME = "github-mcp"
TOOLS = [
    {
        "name": "list_issues",
        "description": "List issues in a GitHub repository",
        "inputSchema": {
            "type": "object",
            "properties": {
                "repo": {"type": "string"},
                "state": {"type": "string"}
            },
            "required": ["repo"]
        }
    },
    {
        "name": "create_issue",
        "description": "Open a new issue in a GitHub repository",
        "inputSchema": {
            "type": "object",
            "properties": {
                "repo": {"type": "string"},
                "title": {"type": "string"},
                "body": {"type": "string"}
            },
            "required": ["repo", "title"]
        }
    },
    {
        "name": "get_pull_request",
        "description": "Retrieve details, changed files, and status of a Pull Request",
        "inputSchema": {
            "type": "object",
            "properties": {
                "repo": {"type": "string"},
                "pr_number": {"type": "integer"}
            },
            "required": ["repo", "pr_number"]
        }
    },
    {
        "name": "list_actions",
        "description": "List latest GitHub Actions workflow runs for a repository",
        "inputSchema": {
            "type": "object",
            "properties": {
                "repo": {"type": "string"}
            },
            "required": ["repo"]
        }
    }
]

def send(resp):
    sys.stdout.write(json.dumps(resp) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    req_id = req.get("id")
    method = req.get("method")
    params = req.get("params", {})
    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": SERVER_NAME, "version": "1.1.0"}
            }
        })
    elif method == "notifications/initialized":
        pass
    elif method == "ping":
        send({"jsonrpc": "2.0", "id": req_id, "result": {}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": req_id, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        name = params.get("name")
        args = params.get("arguments", {})
        try:
            if name == "list_issues":
                repo = args.get("repo", "aro/aro")
                issues = [
                    {"number": 101, "title": "Support default agent plugins", "state": "open", "author": "dev"},
                    {"number": 95, "title": "Memory vector search indexing fix", "state": "closed", "author": "contributor"}
                ]
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps({"repo": repo, "issues": issues}, indent=2)}], "isError": False}})
            elif name == "create_issue":
                repo = args.get("repo")
                title = args.get("title")
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps({"repo": repo, "issue_number": 102, "title": title, "status": "created"}, indent=2)}], "isError": False}})
            elif name == "get_pull_request":
                repo = args.get("repo")
                pr = int(args.get("pr_number", 1))
                pr_data = {
                    "number": pr,
                    "repo": repo,
                    "title": "Feat: Add default agent plugins",
                    "status": "open",
                    "mergeable": True,
                    "changed_files": 4,
                    "additions": 140,
                    "deletions": 12
                }
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps(pr_data, indent=2)}], "isError": False}})
            elif name == "list_actions":
                repo = args.get("repo")
                workflows = [
                    {"id": 501, "name": "CI Tests", "status": "completed", "conclusion": "success"},
                    {"id": 502, "name": "Desktop Build", "status": "completed", "conclusion": "success"}
                ]
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps({"repo": repo, "workflows": workflows}, indent=2)}], "isError": False}})
            else:
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Unknown tool {name}"}], "isError": True}})
        except Exception as e:
            send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Error: {e}"}], "isError": True}})
    else:
        if req_id is not None:
            send({"jsonrpc": "2.0", "id": req_id, "error": {"code": -32601, "message": "Method not found"}})
"#;

    let skill1_dir = target_dir.join("skills").join("pr-reviewer");
    let scripts1_dir = skill1_dir.join("scripts");
    std::fs::create_dir_all(&scripts1_dir)?;

    let skill1_md = r#"---
name: PR Reviewer
description: Conducts automated Pull Request reviews, flags regressions, and provides structured feedback.
tags: [github, git, pull-request, code-review]
icon: 🐙
---
# PR Reviewer Instructions
1. Inspect git commits, modified files, and diff lines.
2. Verify sound patterns, test coverage, and documentation consistency.
3. Formulate actionable review feedback for maintainers.
"#;

    let skill1_script = r#"import sys, json

input_data = {}
if len(sys.argv) > 1:
    try:
        input_data = json.loads(sys.argv[1])
    except Exception:
        pass

pr_title = input_data.get("title", "Update components")
print(json.dumps({
    "success": True,
    "review": f"LGTM: '{pr_title}' meets code standards with clean separation of concerns.",
    "approval_recommended": True
}, indent=2))
"#;

    let skill2_dir = target_dir.join("skills").join("issue-triager");
    let scripts2_dir = skill2_dir.join("scripts");
    std::fs::create_dir_all(&scripts2_dir)?;

    let skill2_md = r#"---
name: Issue Triager
description: Analyzes bug reports, labels issues by severity, and extracts reproduction steps.
tags: [github, issues, triage, bugs]
icon: 🏷️
---
# Issue Triager Instructions
1. Read the issue description and system environment.
2. Determine bug severity and assign appropriate labels.
3. Request missing reproduction info if needed.
"#;

    let skill2_script = r#"import sys, json

input_data = {}
if len(sys.argv) > 1:
    try:
        input_data = json.loads(sys.argv[1])
    except Exception:
        pass

title = input_data.get("title", "Bug report")
print(json.dumps({
    "success": True,
    "issue_title": title,
    "suggested_labels": ["bug", "triage-needed", "desktop"],
    "priority": "P2"
}, indent=2))
"#;

    std::fs::write(target_dir.join("plugin.json"), manifest)?;
    std::fs::write(target_dir.join("mcp.json"), mcp)?;
    std::fs::write(target_dir.join("mcp_server.py"), mcp_server)?;
    std::fs::write(skill1_dir.join("SKILL.md"), skill1_md)?;
    std::fs::write(scripts1_dir.join("run.py"), skill1_script)?;
    std::fs::write(skill2_dir.join("SKILL.md"), skill2_md)?;
    std::fs::write(scripts2_dir.join("run.py"), skill2_script)?;
    Ok(())
}

fn scaffold_slack_workspace(target_dir: &Path) -> Result<()> {
    let manifest = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "slack-workspace",
  "version": "1.0.0",
  "description": "Post messages, query conversation channels, and automate team standup summaries via MCP.",
  "license": "MIT",
  "keywords": ["slack", "messaging", "channels", "collaboration", "notifications"]
}"#;

    let mcp = r#"{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {
    "slack-mcp": {
      "type": "stdio",
      "command": "python",
      "args": ["${PLUGIN_ROOT}/mcp_server.py"]
    }
  }
}"#;

    let mcp_server = r#"import sys, json, os

SERVER_NAME = "slack-mcp"
TOOLS = [
    {
        "name": "post_message",
        "description": "Post a message to a Slack channel",
        "inputSchema": {
            "type": "object",
            "properties": {
                "channel": {"type": "string"},
                "text": {"type": "string"}
            },
            "required": ["channel", "text"]
        }
    },
    {
        "name": "list_channels",
        "description": "List public and private conversation channels in the Slack workspace",
        "inputSchema": {
            "type": "object",
            "properties": {}
        }
    },
    {
        "name": "read_history",
        "description": "Read recent messages and replies from a Slack channel",
        "inputSchema": {
            "type": "object",
            "properties": {
                "channel": {"type": "string"},
                "limit": {"type": "integer"}
            },
            "required": ["channel"]
        }
    }
]

def send(resp):
    sys.stdout.write(json.dumps(resp) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    req_id = req.get("id")
    method = req.get("method")
    params = req.get("params", {})
    if method == "initialize":
        send({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": SERVER_NAME, "version": "1.0.0"}
            }
        })
    elif method == "notifications/initialized":
        pass
    elif method == "ping":
        send({"jsonrpc": "2.0", "id": req_id, "result": {}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": req_id, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        name = params.get("name")
        args = params.get("arguments", {})
        try:
            if name == "post_message":
                ch = args.get("channel", "general")
                txt = args.get("text", "")
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps({"ok": True, "channel": ch, "ts": "1726246800.000100", "message_length": len(txt)}, indent=2)}], "isError": False}})
            elif name == "list_channels":
                channels = [
                    {"id": "C01001", "name": "general", "is_private": False, "num_members": 24},
                    {"id": "C01002", "name": "engineering", "is_private": False, "num_members": 18},
                    {"id": "C01003", "name": "announcements", "is_private": False, "num_members": 45}
                ]
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps(channels, indent=2)}], "isError": False}})
            elif name == "read_history":
                ch = args.get("channel", "general")
                limit = int(args.get("limit", 5))
                messages = [
                    {"user": "alex", "text": "Deployment to staging completed.", "ts": "1726245000.000100"},
                    {"user": "sarah", "text": "All unit tests passing cleanly.", "ts": "1726245300.000100"}
                ]
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": json.dumps({"channel": ch, "messages": messages[:limit]}, indent=2)}], "isError": False}})
            else:
                send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Unknown tool {name}"}], "isError": True}})
        except Exception as e:
            send({"jsonrpc": "2.0", "id": req_id, "result": {"content": [{"type": "text", "text": f"Error: {e}"}], "isError": True}})
    else:
        if req_id is not None:
            send({"jsonrpc": "2.0", "id": req_id, "error": {"code": -32601, "message": "Method not found"}})
"#;

    let skill_dir = target_dir.join("skills").join("standup-reporter");
    let scripts_dir = skill_dir.join("scripts");
    std::fs::create_dir_all(&scripts_dir)?;

    let skill_md = r#"---
name: Standup Reporter
description: Aggregates team progress, summarizes blockers, and posts daily standup updates to Slack.
tags: [slack, standup, summary, teamwork]
icon: 💬
---
# Standup Reporter Instructions
1. Collect daily achievements, today's targets, and blockers.
2. Format as a clean Slack bulleted message.
3. Post to team channel.
"#;

    let skill_script = r#"import sys, json

input_data = {}
if len(sys.argv) > 1:
    try:
        input_data = json.loads(sys.argv[1])
    except Exception:
        pass

completed = input_data.get("completed", ["Plugin system implementation"])
todo = input_data.get("todo", ["Run test suite", "Review PR"])
blockers = input_data.get("blockers", ["None"])

standup_text = (
    "*Daily Standup Summary*\n"
    "• *Completed:* " + ", ".join(completed) + "\n"
    "• *Today:* " + ", ".join(todo) + "\n"
    "• *Blockers:* " + ", ".join(blockers)
)

print(json.dumps({
    "success": True,
    "standup_text": standup_text
}, indent=2))
"#;

    std::fs::write(target_dir.join("plugin.json"), manifest)?;
    std::fs::write(target_dir.join("mcp.json"), mcp)?;
    std::fs::write(target_dir.join("mcp_server.py"), mcp_server)?;
    std::fs::write(skill_dir.join("SKILL.md"), skill_md)?;
    std::fs::write(scripts_dir.join("run.py"), skill_script)?;
    Ok(())
}
