/**
 * 通用 Promise 池：对 items 以固定并发数 concurrency 执行 worker。
 * - 任一 worker 完成后立刻启动下一个 pending 任务（滚动窗口）
 * - signal.aborted 为 true 时不再启动新任务；已启动的任务由 worker 自行决定是否短路
 * - worker 内部需 try/catch，不应 reject 到调度器（否则会中断整体）
 * - 全部任务 settle 后 resolve
 */
export async function runBatch<T>(
  items: T[],
  concurrency: number,
  worker: (item: T, index: number, signal: AbortSignal) => Promise<void>,
  signal: AbortSignal,
): Promise<void> {
  if (items.length === 0) return;
  const effectiveConcurrency = Math.max(1, Math.min(concurrency, items.length));
  let nextIndex = 0;

  const runOne = async (): Promise<void> => {
    while (true) {
      if (signal.aborted) return;
      const idx = nextIndex++;
      if (idx >= items.length) return;
      try {
        await worker(items[idx], idx, signal);
      } catch (err) {
        // 防御性：worker 理应自己 catch；若抛出也不中断整体
        console.error("[batchScheduler] worker threw:", err);
      }
    }
  };

  const runners: Promise<void>[] = [];
  for (let i = 0; i < effectiveConcurrency; i++) {
    runners.push(runOne());
  }
  await Promise.all(runners);
}
