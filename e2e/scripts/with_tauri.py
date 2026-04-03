#!/usr/bin/env python3
"""Tauri 应用进程管理器：启动 debug build → 等待就绪 → 执行测试命令 → 关闭"""

import argparse
import os
import platform
import signal
import subprocess
import sys
import time
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent
APP_NAME = "Dimkey"


def find_debug_binary() -> Path:
    """查找 Tauri debug build 产物"""
    system = platform.system()
    if system == "Darwin":
        binary = PROJECT_ROOT / "src-tauri" / "target" / "debug" / APP_NAME
        if not binary.exists():
            bundle = PROJECT_ROOT / "src-tauri" / "target" / "debug" / "bundle" / "macos" / f"{APP_NAME}.app" / "Contents" / "MacOS" / APP_NAME
            if bundle.exists():
                return bundle
        return binary
    elif system == "Windows":
        return PROJECT_ROOT / "src-tauri" / "target" / "debug" / f"{APP_NAME}.exe"
    else:
        return PROJECT_ROOT / "src-tauri" / "target" / "debug" / APP_NAME.lower()


def build_debug():
    """构建 debug 版本"""
    print("[with_tauri] 构建 debug 版本...")
    subprocess.run(["npm", "run", "build"], cwd=PROJECT_ROOT, check=True)
    subprocess.run(
        ["cargo", "build"],
        cwd=PROJECT_ROOT / "src-tauri",
        check=True,
    )
    print("[with_tauri] 构建完成")


def wait_for_ready(proc: subprocess.Popen, timeout: int = 60):
    """等待应用启动就绪"""
    start = time.time()
    while time.time() - start < timeout:
        if proc.poll() is not None:
            raise RuntimeError(f"应用启动失败，退出码: {proc.returncode}")
        time.sleep(2)
        print("[with_tauri] 应用已启动")
        return
    raise TimeoutError(f"应用在 {timeout} 秒内未启动")


def main():
    parser = argparse.ArgumentParser(description="Tauri E2E 测试运行器")
    parser.add_argument("--no-build", action="store_true", help="跳过构建步骤")
    parser.add_argument("--keep-alive", action="store_true", help="测试后不关闭应用")
    parser.add_argument("--timeout", type=int, default=60, help="启动超时秒数")
    parser.add_argument("command", nargs=argparse.REMAINDER, help="测试命令（-- 之后）")
    args = parser.parse_args()

    cmd = args.command
    if cmd and cmd[0] == "--":
        cmd = cmd[1:]
    if not cmd:
        print("用法: python with_tauri.py [options] -- <test command>")
        print("示例: python with_tauri.py -- pytest e2e/tests/ -v")
        sys.exit(1)

    binary = find_debug_binary()

    if not args.no_build and not binary.exists():
        build_debug()

    if not binary.exists():
        print(f"[with_tauri] 未找到 debug binary: {binary}")
        print("[with_tauri] 请先运行: cargo build (在 src-tauri 目录)")
        sys.exit(1)

    env = os.environ.copy()
    env["DIMKEY_E2E"] = "1"
    env["DIMKEY_TEST_BINARY"] = str(binary)

    print(f"[with_tauri] 启动应用: {binary}")
    proc = subprocess.Popen([str(binary)], env=env)

    try:
        wait_for_ready(proc, args.timeout)

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
            proc.send_signal(signal.SIGTERM)
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
        print("[with_tauri] 完成")


if __name__ == "__main__":
    main()
