#!/usr/bin/env python3
"""Tauri E2E 测试运行器：启动 cargo tauri dev → 等待 :1420 就绪 → 执行测试 → 关闭"""

import argparse
import os
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent
DEV_PORT = 1420


def is_port_open(port: int) -> bool:
    """检查端口是否可连接"""
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=1):
            return True
    except (ConnectionRefusedError, TimeoutError, OSError):
        return False


def wait_for_port(port: int, proc: subprocess.Popen, timeout: int = 120):
    """轮询端口直到可用"""
    print(f"[with_tauri] 等待端口 {port} 就绪...")
    start = time.time()
    while time.time() - start < timeout:
        if proc.poll() is not None:
            raise RuntimeError(f"进程意外退出，退出码: {proc.returncode}")
        if is_port_open(port):
            print(f"[with_tauri] 端口 {port} 已就绪 ({time.time() - start:.1f}s)")
            return
        time.sleep(1)
    raise TimeoutError(f"端口 {port} 在 {timeout} 秒内未就绪")


def main():
    parser = argparse.ArgumentParser(description="Tauri E2E 测试运行器")
    parser.add_argument("--keep-alive", action="store_true", help="测试后不关闭应用")
    parser.add_argument("--timeout", type=int, default=120, help="启动超时秒数")
    parser.add_argument("--port", type=int, default=DEV_PORT, help="Vite dev server 端口")
    parser.add_argument("command", nargs=argparse.REMAINDER, help="测试命令（-- 之后）")
    args = parser.parse_args()

    cmd = args.command
    if cmd and cmd[0] == "--":
        cmd = cmd[1:]
    if not cmd:
        print("用法: python with_tauri.py [options] -- <test command>")
        print("示例: python with_tauri.py -- pytest e2e/tests/ -v")
        sys.exit(1)

    env = os.environ.copy()
    env["DIMKEY_E2E"] = "1"

    # 检查端口是否已被占用（可能已有 dev server 在运行）
    if is_port_open(args.port):
        print(f"[with_tauri] 端口 {args.port} 已在使用，跳过启动")
        env["DIMKEY_TEST_URL"] = f"http://localhost:{args.port}"
        result = subprocess.run(cmd, cwd=PROJECT_ROOT, env=env)
        sys.exit(result.returncode)

    # 启动 cargo tauri dev
    print("[with_tauri] 启动 cargo tauri dev...")
    proc = subprocess.Popen(
        ["cargo", "tauri", "dev"],
        cwd=PROJECT_ROOT,
        env=env,
        # 使用进程组以便一次性关闭所有子进程
        preexec_fn=os.setsid if hasattr(os, "setsid") else None,
    )

    try:
        wait_for_port(args.port, proc, args.timeout)

        # 额外等待 WebView 初始化
        time.sleep(3)

        env["DIMKEY_TEST_URL"] = f"http://localhost:{args.port}"
        print(f"[with_tauri] 执行测试: {' '.join(cmd)}")
        result = subprocess.run(cmd, cwd=PROJECT_ROOT, env=env)

        if args.keep_alive:
            print("[with_tauri] --keep-alive: 应用保持运行，按 Ctrl+C 退出")
            proc.wait()

        sys.exit(result.returncode)

    except KeyboardInterrupt:
        print("\n[with_tauri] 收到中断信号")
    finally:
        if proc.poll() is None:
            print("[with_tauri] 关闭应用...")
            # 关闭整个进程组（cargo tauri dev 会启动子进程）
            if hasattr(os, "killpg"):
                try:
                    os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
                except ProcessLookupError:
                    pass
            else:
                proc.send_signal(signal.SIGTERM)
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                if hasattr(os, "killpg"):
                    try:
                        os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                else:
                    proc.kill()
        print("[with_tauri] 完成")


if __name__ == "__main__":
    main()
