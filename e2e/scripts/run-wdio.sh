#!/bin/bash
# Dimkey E2E 测试运行器 — 启动 tauri-wd → 执行 WebDriverIO 测试 → 清理
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
E2E_DIR="$PROJECT_ROOT/e2e"
WD_PORT=4444

# 确保 output 目录存在
mkdir -p "$E2E_DIR/output"

# 检查 tauri-wd 是否安装
if ! command -v tauri-wd &>/dev/null; then
  echo "[run-wdio] 错误: tauri-wd 未安装。请运行: cargo install tauri-webdriver-automation"
  exit 1
fi

# 检查 debug binary 是否存在
BINARY="$PROJECT_ROOT/src-tauri/target/debug/dimkey"
if [ ! -f "$BINARY" ]; then
  echo "[run-wdio] debug binary 不存在，开始编译..."
  (cd "$PROJECT_ROOT/src-tauri" && cargo build)
fi

# 启动 tauri-wd
echo "[run-wdio] 启动 tauri-wd (端口 $WD_PORT)..."
tauri-wd --port $WD_PORT &
WD_PID=$!

# 等待 tauri-wd 就绪
for i in $(seq 1 15); do
  if curl -s "http://localhost:$WD_PORT/status" >/dev/null 2>&1; then
    echo "[run-wdio] tauri-wd 已就绪"
    break
  fi
  sleep 1
done

# 执行测试
echo "[run-wdio] 执行 WebDriverIO 测试..."
cd "$PROJECT_ROOT"
npx wdio run "$E2E_DIR/wdio.conf.mjs" "$@"
TEST_EXIT=$?

# 清理
echo "[run-wdio] 清理..."
kill $WD_PID 2>/dev/null || true
wait $WD_PID 2>/dev/null || true

echo "[run-wdio] 完成 (exit: $TEST_EXIT)"
exit $TEST_EXIT
