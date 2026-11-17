#!/usr/bin/env python3
"""Persistent Ollama task runner.

Usage:
  python3 scripts/task-runner.py --batch-size 3
  python3 scripts/task-runner.py --dry-run --batch-size 3
"""

from __future__ import annotations

import argparse
import json
import re
import os
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
from pathlib import Path

MODEL = "beam:cloud"
DEFAULT_TASKS = "scripts/tasks.txt"
DEFAULT_OUTDIR = "scratchpad/drv/task-results"
RETRY_COUNT = 3
BACKOFF_BASE = 2.0
POLL_INTERVAL = 2.0


def find_ollama() -> str:
    candidates = []
    for env_name in ("OLLAMA_BIN", "OLLAMA_PATH"):
        value = os.environ.get(env_name)
        if value:
            candidates.append(value)
    candidates.append(shutil.which("ollama"))
    home = Path.home()
    candidates.extend(
        [
            "/usr/local/bin/ollama",
            "/var/usrlocal/bin/ollama",
            str(home / ".local" / "bin" / "ollama"),
        ]
    )
    for candidate in candidates:
        if not candidate:
            continue
        path = Path(candidate).expanduser()
        if path.is_file() and (path.stat().st_mode & 0o111):
            return str(path.resolve())
    raise FileNotFoundError("ollama executable not found; set OLLAMA_BIN")


def resolve_repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def load_tasks(path: Path) -> list[str]:
    tasks = []
    if not path.exists():
        return tasks
    with path.open("r", encoding="utf-8") as handle:
        for raw in handle:
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            tasks.append(line)
    return tasks


def completed_indices(out_dir: Path) -> set[int]:
    indices = set()
    if not out_dir.exists():
        return indices
    for path in out_dir.glob("*.md"):
        try:
            indices.add(int(path.stem))
        except ValueError:
            continue
    return indices


def parse_continue(value, out_dir):
    completed = completed_indices(out_dir)
    start = 1
    if value is None:
        return completed, start
    if value.isdigit():
        start = int(value) + 1
        return completed, start
    path = Path(value).expanduser()
    if path.is_dir():
        return completed_indices(path), start
    if path.is_file():
        text = path.read_text(encoding="utf-8").strip()
        try:
            data = json.loads(text)
        except json.JSONDecodeError:
            for token in text.split():
                if token.isdigit():
                    completed.add(int(token))
            return completed, start
        if isinstance(data, list):
            for item in data:
                if isinstance(item, int):
                    completed.add(item)
                elif isinstance(item, str) and item.isdigit():
                    completed.add(int(item))
                elif isinstance(item, dict):
                    for key in ("index", "n", "task"):
                        v = item.get(key)
                        if isinstance(v, int):
                            completed.add(v)
        elif isinstance(data, dict):
            for key in ("completed", "done", "indices"):
                values = data.get(key)
                if isinstance(values, list):
                    for item in values:
                        if isinstance(item, int):
                            completed.add(item)
            if isinstance(data.get("start"), int):
                start = int(data["start"]) + 1
    return completed, start


def write_result(out_dir, index, task, reply, exit_code, attempts, elapsed, error, dry_run):
    out_dir.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now(timezone.utc).isoformat()
    md_path = out_dir / f"{index}.md"
    md_path.write_text(f"# Task {index}\n\n{task}\n\n---\n\n{reply}\n", encoding="utf-8")
    exit_path = out_dir / f"{index}.exit"
    exit_path.write_text(
        "\n".join(
            [
                f"task_index={index}",
                f"exit_code={exit_code}",
                f"attempts={attempts}",
                f"duration_seconds={elapsed:.3f}",
                f"timestamp={stamp}",
                f"model={'dry-run' if dry_run else MODEL}",
                f"error={error}",
            ]
        )
        + "\n",
        encoding="utf-8",
    )


def _retryable_text(text: str) -> bool:
    low = text.lower()
    if "queue" in low or "concurrency" in low or "too many requests" in low or "rate limit" in low or "429" in low or "5xx" in low:
        return True
    if re.search(r"\b429\b", text):
        return True
    if re.search(r"\b5\d{2}\b", text):
        return True
    return False


def run_one_task(ollama, index, task, out_dir, dry_run, timeout, max_retries=0, max_backoff=120.0):
    if dry_run:
        write_result(out_dir, index, task, "DRY RUN: model was not invoked.\n", 0, 1, 0.0, "", True)
        print(f"[dry-run] task {index}: {task[:60]}")
        return index, 0

    last_error = ""
    reply = ""
    exit_code = 1
    attempts_used = 0
    started = time.monotonic()
    attempt = 0
    while True:
        if max_retries and max_retries > 0 and attempt >= max_retries:
            break
        attempt += 1
        attempts_used = attempt
        retryable = False
        try:
            proc = subprocess.run(
                [ollama, "run", MODEL],
                input=task,
                capture_output=True,
                text=True,
                timeout=timeout,
            )
            combined = (proc.stdout or "") + "\n" + (proc.stderr or "")
            if proc.returncode == 0 and not _retryable_text(combined):
                reply = proc.stdout
                exit_code = 0
                if proc.stderr.strip():
                    last_error = proc.stderr.strip()
                break
            retryable = True
            last_error = proc.stderr.strip() or f"ollama exited with code {proc.returncode}"
            reply = combined
            if proc.returncode == 0 and _retryable_text(combined):
                last_error = "retryable output detected (429/5xx/concurrency/queue)"
        except subprocess.TimeoutExpired as exc:
            last_error = f"timeout after {timeout}s"
            reply = (exc.stdout or "") + "\n" + last_error
            retryable = True
        except Exception as exc:
            last_error = f"{type(exc).__name__}: {exc}"
            reply = last_error
            retryable = True

        if max_retries and max_retries > 0 and attempt >= max_retries:
            break
        if not retryable and not (max_retries and max_retries > 0):
            # default: still retry on any command failure
            pass
        delay = min(float(max_backoff), BACKOFF_BASE ** max(0, attempt - 1))
        if delay > 0:
            time.sleep(delay)

    elapsed = time.monotonic() - started
    if exit_code == 0:
        write_result(out_dir, index, task, reply.strip(), exit_code, attempts_used, elapsed, last_error, False)
    status = "ok" if exit_code == 0 else "FAIL"
    print(f"[{status}] task {index} (attempts={attempts_used}, {elapsed:.1f}s)")
    return index, exit_code


def main():
    parser = argparse.ArgumentParser(description="Persistent Ollama task runner")
    parser.add_argument("--tasks-file", default=DEFAULT_TASKS)
    parser.add_argument("--output-dir", default=DEFAULT_OUTDIR)
    parser.add_argument("--batch-size", type=int, default=3)
    parser.add_argument("--continue-from", default=None)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--timeout", type=int, default=300)
    parser.add_argument("--idle-timeout", type=int, default=6)
    parser.add_argument("--max-retries", type=int, default=0)
    parser.add_argument("--max-backoff", type=float, default=120.0)
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    repo_root = resolve_repo_root()
    tasks_path = (repo_root / args.tasks_file).resolve()
    out_dir = (repo_root / args.output_dir).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)

    ollama = None
    if not args.dry_run:
        try:
            ollama = find_ollama()
        except FileNotFoundError as exc:
            print(f"error: {exc}", file=sys.stderr)
            return 2

    completed, start_index = parse_continue(args.continue_from, out_dir)
    if args.force:
        completed = set()

    tasks = load_tasks(tasks_path)
    if start_index > 1:
        for idx in range(1, start_index):
            completed.add(idx)

    processed = set(completed)
    idle_elapsed = 0.0

    while True:
        tasks = load_tasks(tasks_path)
        todo = [i for i in range(len(tasks)) if (i + 1) not in processed]
        if not todo:
            time.sleep(POLL_INTERVAL)
            idle_elapsed += POLL_INTERVAL
            fresh = load_tasks(tasks_path)
            if len(fresh) > len(tasks):
                tasks = fresh
                todo = [i for i in range(len(tasks)) if (i + 1) not in processed]
            if todo:
                idle_elapsed = 0.0
                continue
            if idle_elapsed >= args.idle_timeout:
                break
            continue

        idle_elapsed = 0.0
        batch_size = max(1, args.batch_size)
        batch = todo[:batch_size]
        print(f"Dispatching batch: tasks {[i + 1 for i in batch]} (batch_size={batch_size})")

        with ThreadPoolExecutor(max_workers=batch_size) as executor:
            futures = {
                executor.submit(run_one_task, ollama, i + 1, tasks[i], out_dir, args.dry_run, args.timeout, args.max_retries, args.max_backoff): i
                for i in batch
            }
            for future in as_completed(futures):
                idx, code = future.result()
                processed.add(idx)

        fresh = load_tasks(tasks_path)
        if len(fresh) > len(tasks):
            print(f"Tailed {len(fresh) - len(tasks)} new task(s) from {tasks_path}")

    print(f"Runner finished. Processed tasks: {sorted(processed)}")
    print(f"Results: {out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
