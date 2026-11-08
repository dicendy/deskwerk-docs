#!/usr/bin/env python3
"""Round-trip a corpus of real-world Word files through wordcraft-cli and report how it went.

    python3 tools/corpus/run.py --bin target/release/wordcraft-cli --corpus DIR --out DIR
                                [--timeout 30] [--jobs N] [--soffice] [--deadline EPOCH] [--limit N]

DIR (from fetch.sh) holds the files and manifest.tsv. For every file, each step in its own process
with a timeout:

  open     wordcraft-cli text <file>               open + plain text (apps/wordcraft-cli/src/main.rs `text`)
  save     wordcraft-cli convert <file> <rt.docx>  open + save as .docx (`convert`, file.save)
  reopen   wordcraft-cli text <rt.docx>            the saved file opens again, and its text

Then, in Python only: the saved file is a valid zip whose XML parts all parse; the body text and
a few structure counts (paragraphs, tables, pictures, fields, tracked changes, ...) of
word/document.xml before and after; and with --soffice, LibreOffice's text of both files.
Each step ends ok, err (non-zero exit), panic (Rust panic, exit 101), crash (killed by a signal)
or timeout. Writes corpus-report.json and corpus-report.md to --out and appends the summary to
$GITHUB_STEP_SUMMARY when set. Standard library only.
"""

from __future__ import annotations

import argparse
import collections
import concurrent.futures
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
import zipfile
import xml.etree.ElementTree as ET

TOOL = "wordcraft-cli"
EXT = ".docx"
W = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"
OFFICE_DOC = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
STRICT_OFFICE_DOC = "http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument"
# Structure counted in the main part: local name -> label.
FEATURES = {
    "p": "paragraphs",
    "tbl": "tables",
    "drawing": "drawings",
    "pict": "vml_pictures",
    "hyperlink": "hyperlinks",
    "fldSimple": "simple_fields",
    "fldChar": "complex_field_chars",
    "ins": "tracked_insertions",
    "del": "tracked_deletions",
    "commentRangeStart": "comment_ranges",
    "footnoteReference": "footnote_refs",
    "endnoteReference": "endnote_refs",
    "sectPr": "sections",
    "bookmarkStart": "bookmarks",
    "sdt": "content_controls",
    "oMath": "equations",
}
MAX_XML_BYTES = 200 * 1024 * 1024  # unpacked part size above which the part is not parsed
STAGE_RANK = {"open": 0, "save": 1, "reopen": 2, "package": 3}


# ---------------------------------------------------------------- processes


def run_proc(argv: list[str], timeout: float, env: dict | None = None, cwd: str | None = None) -> dict:
    """Run argv in its own process group; kill the whole group on timeout."""
    t0 = time.monotonic()
    try:
        p = subprocess.Popen(
            argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, stdin=subprocess.DEVNULL, env=env, cwd=cwd, start_new_session=True
        )
    except OSError as e:
        return {"status": "err", "code": None, "stdout": b"", "stderr": str(e), "seconds": 0.0}
    try:
        out, err = p.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(p.pid, signal.SIGKILL)
        except OSError:
            pass
        out, err = p.communicate()
        return {"status": "timeout", "code": None, "stdout": out, "stderr": err.decode("utf-8", "replace"), "seconds": round(time.monotonic() - t0, 2)}
    err_text = err.decode("utf-8", "replace")
    code = p.returncode
    if code == 0:
        status = "ok"
    elif code < 0:
        status = "crash"
    elif code == 101 or "panicked at" in err_text:
        status = "panic"
    else:
        status = "err"
    return {"status": status, "code": code, "stdout": out, "stderr": err_text, "seconds": round(time.monotonic() - t0, 2)}


def error_message(r: dict, paths: list[str]) -> str | None:
    """One line saying why a step failed, with file paths taken out."""
    if r["status"] == "ok":
        return None
    err = r["stderr"]
    for p in paths:
        if p:
            err = err.replace(p, "<file>")
    lines = [ln.strip() for ln in err.splitlines() if ln.strip()]
    if r["status"] == "timeout":
        return "timeout"
    if r["status"] == "crash":
        sig = -r["code"]
        try:
            name = signal.Signals(sig).name
        except ValueError:
            name = f"signal {sig}"
        tail = next((ln for ln in reversed(lines) if "overflow" in ln or "fatal" in ln.lower()), "")
        return f"killed by {name}" + (f": {tail}" if tail else "")
    if r["status"] == "panic":
        for i, ln in enumerate(lines):
            m = re.search(r"panicked at (\S+?):\d+:\d+:?\s*(.*)", ln)
            if m:
                msg = m.group(2) or (lines[i + 1] if i + 1 < len(lines) else "")
                return f"panic at {m.group(1)}: {msg}"
        return "panic: " + (lines[-1] if lines else "")
    ours = [ln for ln in lines if ln.startswith(f"{TOOL}: ")]
    other = [ln for ln in lines if not ln.startswith("Warning:")]
    msg = (ours or other or lines or [f"exit {r['code']}"])[0]
    return msg.removeprefix(f"{TOOL}: ")


def group_key(msg: str) -> str:
    """Error message with the variable bits (numbers, quoted names) folded, for grouping."""
    msg = re.sub(r"`[^`]*`", "`…`", msg)
    msg = re.sub(r"\"[^\"]*\"", '"…"', msg)
    msg = re.sub(r"\b0x[0-9a-fA-F]+\b", "N", msg)
    msg = re.sub(r"\d+", "N", msg)
    return msg[:200]


# ---------------------------------------------------------------- package checks


def main_part(z: zipfile.ZipFile) -> str | None:
    try:
        rels = ET.fromstring(z.read("_rels/.rels"))
    except (KeyError, ET.ParseError):
        return None
    for rel in rels:
        if rel.get("Type") in (OFFICE_DOC, STRICT_OFFICE_DOC):
            target = (rel.get("Target") or "").lstrip("/")
            return target
    return None


def check_package(path: str) -> dict:
    """Is `path` a zip whose XML parts all parse, with content types and a main document?"""
    out = {"zip": False, "content_types": False, "main_part": None, "xml_parts": 0, "xml_errors": [], "error": None}
    try:
        with zipfile.ZipFile(path) as z:
            bad = z.testzip()
            if bad is not None:
                out["error"] = f"bad CRC in {bad}"
                return out
            out["zip"] = True
            names = set(z.namelist())
            out["content_types"] = "[Content_Types].xml" in names
            out["main_part"] = main_part(z)
            for info in z.infolist():
                n = info.filename
                if not (n.endswith(".xml") or n.endswith(".rels")) or info.file_size > MAX_XML_BYTES:
                    continue
                out["xml_parts"] += 1
                try:
                    ET.fromstring(z.read(n))
                except ET.ParseError as e:
                    out["xml_errors"].append(f"{n}: {e}")
    except (zipfile.BadZipFile, OSError, RuntimeError, NotImplementedError, EOFError) as e:
        out["error"] = f"{type(e).__name__}: {e}"
        return out
    if out["main_part"] is None:
        out["error"] = "no officeDocument relationship"
    elif out["main_part"] not in names:
        out["error"] = f"main part {out['main_part']} missing"
    elif out["xml_errors"]:
        out["error"] = f"{len(out['xml_errors'])} XML part(s) do not parse"
    elif not out["content_types"]:
        out["error"] = "no [Content_Types].xml"
    return out


def docx_body(path: str) -> dict | None:
    """Text and structure counts of the main part, read with the standard library. None when the
    file is not a readable package (encrypted, binary .doc, broken zip)."""
    try:
        with zipfile.ZipFile(path) as z:
            part = main_part(z)
            if part is None:
                return None
            info = z.getinfo(part)
            if info.file_size > MAX_XML_BYTES:
                return None
            with z.open(part) as f:
                counts = collections.Counter()
                paras: list[str] = []
                cur: list[str] = []
                for event, el in ET.iterparse(f, events=("start", "end")):
                    tag = el.tag
                    local = tag.rsplit("}", 1)[-1]
                    if event == "start":
                        if local in FEATURES and (local != "fldChar" or el.get(W + "fldCharType") == "begin"):
                            counts[FEATURES[local]] += 1
                        continue
                    if tag == W + "t":
                        cur.append(el.text or "")
                    elif tag == W + "tab":
                        cur.append("\t")
                    elif tag in (W + "br", W + "cr"):
                        cur.append("\n")
                    elif tag == W + "p":
                        paras.append("".join(cur))
                        cur = []
                        el.clear()
                return {"text": "\n".join(paras), "counts": dict(counts)}
    except (zipfile.BadZipFile, KeyError, OSError, ET.ParseError, RuntimeError, NotImplementedError, EOFError):
        return None


# ---------------------------------------------------------------- text comparison

WORD = re.compile(r"\w+", re.UNICODE)


def words(text: str) -> collections.Counter:
    return collections.Counter(w.lower() for w in WORD.findall(text))


def similarity(a: str | None, b: str | None) -> float | None:
    """Share of words kept: the multiset overlap over the larger multiset (1.0 = same words)."""
    if a is None or b is None:
        return None
    wa, wb = words(a), words(b)
    na, nb = sum(wa.values()), sum(wb.values())
    if na == 0 and nb == 0:
        return 1.0
    return round(sum((wa & wb).values()) / max(na, nb), 4)


_local = threading.local()


def soffice_text(soffice: str, path: str, timeout: float) -> tuple[str | None, str | None]:
    """LibreOffice's plain text of `path` (each worker thread has its own LibreOffice profile)."""
    if not hasattr(_local, "profile"):
        _local.profile = tempfile.mkdtemp(prefix="lo-profile-")
    outdir = tempfile.mkdtemp(prefix="lo-out-")
    try:
        r = run_proc(
            [soffice, f"-env:UserInstallation=file://{_local.profile}", "--headless", "--norestore", "--nolockcheck",
             "--convert-to", "txt:Text (encoded):UTF8", "--outdir", outdir, path],
            timeout,
        )
        txts = [f for f in os.listdir(outdir) if f.endswith(".txt")]
        if r["status"] != "ok" or not txts:
            return None, r["status"] if r["status"] != "ok" else "no output"
        with open(os.path.join(outdir, txts[0]), encoding="utf-8", errors="replace") as f:
            return f.read(), None
    finally:
        shutil.rmtree(outdir, ignore_errors=True)


# ---------------------------------------------------------------- one file


def process(entry: dict, args, workdir: str, env: dict) -> dict:
    src = os.path.join(args.corpus, entry["file"])
    rec = {"file": entry["file"], "source": entry["source"], "url": entry["url"], "bytes": int(entry["bytes"])}
    rt = os.path.join(workdir, f"{entry['n']:05d}{EXT}")
    paths = [src, rt, args.corpus, workdir]

    def step(name: str, argv: list[str]) -> dict:
        r = run_proc(argv, args.timeout, env)
        rec[name] = r["status"]
        rec[f"{name}_seconds"] = r["seconds"]
        msg = error_message(r, paths)
        if msg:
            rec[f"{name}_error"] = msg
        return r

    r_open = step("open", [args.bin, "text", src])
    text_a = r_open["stdout"].decode("utf-8", "replace") if r_open["status"] == "ok" else None
    r_save = step("save", [args.bin, "convert", src, rt])
    saved = r_save["status"] == "ok" and os.path.isfile(rt) and os.path.getsize(rt) > 0
    if r_save["status"] == "ok" and not saved:
        rec["save"] = "err"
        rec["save_error"] = "convert exited 0 but wrote no file"
    text_b = None
    if saved:
        rec["rt_bytes"] = os.path.getsize(rt)
        r_re = step("reopen", [args.bin, "text", rt])
        text_b = r_re["stdout"].decode("utf-8", "replace") if r_re["status"] == "ok" else None
        pkg = check_package(rt)
        rec["package"] = "ok" if pkg["error"] is None else "err"
        if pkg["error"]:
            rec["package_error"] = pkg["error"]
            rec["package_xml_errors"] = pkg["xml_errors"][:5]
    else:
        rec["reopen"] = "skipped"
        rec["package"] = "skipped"

    # Fidelity: the app's own text before/after, the raw XML's text and structure before/after.
    if text_a is not None:
        rec["text_chars"] = len(text_a)
        rec["text_words"] = sum(words(text_a).values())
    rec["sim_app_text"] = similarity(text_a, text_b)
    body_a = docx_body(src)
    body_b = docx_body(rt) if saved else None
    if body_a and body_b:
        rec["sim_xml_text"] = similarity(body_a["text"], body_b["text"])
        lost = {}
        for label in FEATURES.values():
            a, b = body_a["counts"].get(label, 0), body_b["counts"].get(label, 0)
            if a != b:
                lost[label] = [a, b]
        rec["count_changes"] = lost
    if body_a and text_a is not None:
        # How much of the file's own text the app shows at all (no save involved).
        rec["sim_open_vs_xml"] = similarity(body_a["text"], text_a)
    if args.soffice and saved:
        lo_a, ea = soffice_text(args.soffice, src, args.timeout * 2)
        lo_b, eb = soffice_text(args.soffice, rt, args.timeout * 2)
        rec["sim_lo_text"] = similarity(lo_a, lo_b)
        if ea or eb:
            rec["lo_error"] = f"original: {ea or 'ok'}, round-trip: {eb or 'ok'}"
    if not args.keep:
        try:
            os.remove(rt)
        except OSError:
            pass
    rec["verdict"] = verdict(rec)
    return rec


def verdict(rec: dict) -> str:
    for stage in ("open", "save", "reopen", "package"):
        st = rec.get(stage)
        if st not in ("ok", "skipped", None):
            return f"{stage}:{st}"
    if rec.get("save") != "ok":
        return "save:skipped"
    sims = [rec.get(k) for k in ("sim_app_text", "sim_xml_text", "sim_lo_text") if rec.get(k) is not None]
    if sims and min(sims) < 0.99:
        return "text_changed"
    return "ok"


def badness(rec: dict) -> tuple:
    v = rec["verdict"]
    if ":" in v:
        stage, st = v.split(":", 1)
        return (0, STAGE_RANK.get(stage, 9), {"panic": 0, "crash": 0, "timeout": 1, "err": 2}.get(st, 3), rec["file"])
    sims = [rec.get(k) for k in ("sim_app_text", "sim_xml_text", "sim_lo_text") if rec.get(k) is not None]
    return (1, min(sims) if sims else 1.0, -len(rec.get("count_changes") or {}), rec["file"])


# ---------------------------------------------------------------- report


def pct(n: int, d: int) -> str:
    return f"{100.0 * n / d:.1f}%" if d else "-"


def summarize(recs: list[dict], not_run: int) -> dict:
    n = len(recs)
    s: dict = {"files": n + not_run, "processed": n, "not_run_deadline": not_run}
    for stage in ("open", "save", "reopen", "package"):
        s[stage] = dict(collections.Counter(r.get(stage, "skipped") for r in recs))
    s["verdicts"] = dict(collections.Counter(r["verdict"] for r in recs).most_common())
    s["full_round_trip_ok"] = sum(1 for r in recs if all(r.get(k) == "ok" for k in ("open", "save", "reopen", "package")))
    for key in ("sim_app_text", "sim_xml_text", "sim_lo_text", "sim_open_vs_xml"):
        vals = sorted(r[key] for r in recs if r.get(key) is not None)
        if vals:
            s[key] = {
                "files": len(vals),
                "identical": sum(1 for v in vals if v >= 0.9999),
                "ge_0.99": sum(1 for v in vals if v >= 0.99),
                "lt_0.90": sum(1 for v in vals if v < 0.90),
                "median": vals[len(vals) // 2],
                "mean": round(sum(vals) / len(vals), 4),
            }
    changes = collections.Counter()
    for r in recs:
        for label, (a, b) in (r.get("count_changes") or {}).items():
            changes[f"{label} {'lost' if b < a else 'gained'}"] += 1
    s["structure_changes"] = dict(changes.most_common())
    by_src = {}
    for src in sorted({r["source"] for r in recs}):
        rs = [r for r in recs if r["source"] == src]
        by_src[src] = {"files": len(rs), "open_ok": sum(r.get("open") == "ok" for r in rs), "round_trip_ok": sum(r["verdict"] == "ok" for r in rs)}
    s["by_source"] = by_src
    s["seconds_total"] = round(sum(r.get(f"{k}_seconds", 0) for r in recs for k in ("open", "save", "reopen")), 1)
    return s


def error_groups(recs: list[dict]) -> list[dict]:
    groups: dict = {}
    for r in recs:
        for stage in ("open", "save", "reopen", "package"):
            msg = r.get(f"{stage}_error")
            if not msg:
                continue
            key = (stage, r.get(stage), group_key(msg))
            g = groups.setdefault(key, {"stage": stage, "status": r.get(stage), "message": msg, "count": 0, "files": []})
            g["count"] += 1
            if len(g["files"]) < 5:
                g["files"].append(r["file"])
    return sorted(groups.values(), key=lambda g: (-g["count"], STAGE_RANK[g["stage"]], g["message"]))


def md_cell(s) -> str:
    return str(s).replace("|", "\\|").replace("\n", " ")[:220]


def render_md(report: dict, worst: int) -> tuple[str, str]:
    s = report["summary"]
    m = report["meta"]
    L = [f"# {TOOL} corpus round trip", ""]
    L.append(f"Commit `{m.get('commit') or '?'}` · {s['processed']} of {s['files']} files · timeout {m['timeout']} s per step · "
             f"LibreOffice comparison: {'on' if m['soffice'] else 'off'}")
    if s["not_run_deadline"]:
        L.append(f"\n**{s['not_run_deadline']} files were not run: the time budget ran out.**")
    L += ["", "| Stage | ok | err | panic | crash | timeout | skipped |", "|---|---|---|---|---|---|---|"]
    for stage in ("open", "save", "reopen", "package"):
        c = s[stage]
        L.append(f"| {stage} | {c.get('ok', 0)} ({pct(c.get('ok', 0), s['processed'])}) | {c.get('err', 0)} | {c.get('panic', 0)} | "
                 f"{c.get('crash', 0)} | {c.get('timeout', 0)} | {c.get('skipped', 0)} |")
    L += ["", f"Full round trip (open, save, reopen, valid package): **{s['full_round_trip_ok']}** "
          f"({pct(s['full_round_trip_ok'], s['processed'])}); verdict ok (and text kept ≥ 99%): **{s['verdicts'].get('ok', 0)}**.", ""]
    L += ["| Text kept (word overlap) | files | identical | ≥ 0.99 | < 0.90 | median | mean |", "|---|---|---|---|---|---|---|"]
    labels = {"sim_app_text": "app text, original vs saved", "sim_xml_text": "document.xml text, original vs saved",
              "sim_lo_text": "LibreOffice text, original vs saved", "sim_open_vs_xml": "app text vs document.xml (open only)"}
    for key, label in labels.items():
        if key in s:
            v = s[key]
            L.append(f"| {label} | {v['files']} | {v['identical']} | {v['ge_0.99']} | {v['lt_0.90']} | {v['median']} | {v['mean']} |")
    if s["structure_changes"]:
        L += ["", "Structure counts in document.xml that changed on the round trip (files):", ""]
        L.append(", ".join(f"{k}: {v}" for k, v in s["structure_changes"].items()))
    L += ["", "| Source | files | open ok | round trip ok |", "|---|---|---|---|"]
    for src, v in s["by_source"].items():
        L.append(f"| {src} | {v['files']} | {v['open_ok']} | {v['round_trip_ok']} |")
    L += ["", "## Top errors", "", "| # | stage | status | message | example |", "|---|---|---|---|---|"]
    for g in report["error_groups"][:25]:
        L.append(f"| {g['count']} | {g['stage']} | {g['status']} | {md_cell(g['message'])} | {md_cell(g['files'][0])} |")
    summary = "\n".join(L) + "\n"
    W2 = ["", f"## Worst {worst} files", "", "| verdict | app text | xml text | LO text | file | error |", "|---|---|---|---|---|---|"]
    for r in report["worst"][:worst]:
        err = next((r.get(f"{k}_error") for k in ("open", "save", "reopen", "package") if r.get(f"{k}_error")), "")
        W2.append(f"| {r['verdict']} | {r.get('sim_app_text', '')} | {r.get('sim_xml_text', '')} | {r.get('sim_lo_text', '')} | "
                  f"[{md_cell(r['file'])}]({r['url']}) | {md_cell(err)} |")
    return summary, summary + "\n".join(W2) + "\n"


def git_commit() -> str | None:
    if os.environ.get("GITHUB_SHA"):
        return os.environ["GITHUB_SHA"][:12]
    try:
        return subprocess.run(["git", "rev-parse", "--short=12", "HEAD"], capture_output=True, text=True, timeout=10).stdout.strip() or None
    except (OSError, subprocess.SubprocessError):
        return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", required=True, help=f"path to the release-built {TOOL}")
    ap.add_argument("--corpus", required=True, help="directory written by fetch.sh (with manifest.tsv)")
    ap.add_argument("--out", required=True, help="directory for corpus-report.json/.md")
    ap.add_argument("--timeout", type=float, default=30.0, help="seconds per step (default 30)")
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 2)
    ap.add_argument("--soffice", nargs="?", const="soffice", default=None, help="also compare LibreOffice's text (soffice on PATH or a path)")
    ap.add_argument("--deadline", type=float, default=0, help="Unix time after which no new file is started")
    ap.add_argument("--limit", type=int, default=0, help="only the first N files (0 = all)")
    ap.add_argument("--worst", type=int, default=40, help="files listed in the worst table")
    ap.add_argument("--keep", action="store_true", help="keep the saved round-trip files in OUT/work")
    args = ap.parse_args()
    args.bin = os.path.abspath(args.bin)
    args.corpus = os.path.abspath(args.corpus)
    if args.soffice:
        found = shutil.which(args.soffice) or shutil.which("libreoffice")
        if not found:
            print(f"soffice not found ({args.soffice}); LibreOffice comparison off", file=sys.stderr)
        args.soffice = found
    os.makedirs(args.out, exist_ok=True)
    workdir = os.path.join(os.path.abspath(args.out), "work")
    os.makedirs(workdir, exist_ok=True)

    with open(os.path.join(args.corpus, "manifest.tsv"), encoding="utf-8") as f:
        header = f.readline().rstrip("\n").split("\t")
        entries = [dict(zip(header, line.rstrip("\n").split("\t"))) for line in f if line.strip()]
    for i, e in enumerate(entries):
        e["n"] = i
    if args.limit:
        entries = entries[: args.limit]

    env = dict(os.environ, RUST_BACKTRACE="0", WORDCRAFT_REGION="en-US", LC_ALL="C.UTF-8")
    env.pop("WORDCRAFT_PASSWORD", None)
    recs: list[dict] = []
    not_run = 0
    lock = threading.Lock()
    t0 = time.monotonic()

    def job(e: dict):
        if args.deadline and time.time() > args.deadline:
            return None
        try:
            return process(e, args, workdir, env)
        except Exception as ex:  # a bug here must not end the run
            return {"file": e["file"], "source": e["source"], "url": e["url"], "bytes": int(e["bytes"]), "verdict": "runner:error",
                    "runner_error": f"{type(ex).__name__}: {ex}"}

    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        for i, rec in enumerate(pool.map(job, entries), 1):
            with lock:
                if rec is None:
                    not_run += 1
                    continue
                recs.append(rec)
                if i % 25 == 0 or ":" in rec["verdict"]:
                    print(f"[{i}/{len(entries)} {time.monotonic() - t0:.0f}s] {rec['verdict']:<16} {rec['file']}", flush=True)

    recs.sort(key=lambda r: r["file"])
    report = {
        "meta": {"tool": TOOL, "bin": args.bin, "commit": git_commit(), "generated": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                 "timeout": args.timeout, "jobs": args.jobs, "soffice": bool(args.soffice), "wall_seconds": round(time.monotonic() - t0, 1)},
        "summary": summarize(recs, not_run),
        "error_groups": error_groups(recs),
        "worst": [],
        "files": recs,
    }
    report["worst"] = [
        {k: r.get(k) for k in ("file", "url", "verdict", "sim_app_text", "sim_xml_text", "sim_lo_text", "open_error", "save_error", "reopen_error", "package_error", "count_changes") if r.get(k) not in (None, {}, [])}
        for r in sorted(recs, key=badness)
        if r["verdict"] != "ok"
    ][: args.worst]
    with open(os.path.join(args.out, "corpus-report.json"), "w", encoding="utf-8") as f:
        json.dump(report, f, indent=1, ensure_ascii=False)
    summary_md, full_md = render_md(report, args.worst)
    with open(os.path.join(args.out, "corpus-report.md"), "w", encoding="utf-8") as f:
        f.write(full_md)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as f:
            f.write(full_md if len(full_md) < 900_000 else summary_md)
    print(summary_md)
    if not args.keep:
        shutil.rmtree(workdir, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
