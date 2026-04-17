import { invoke } from "@tauri-apps/api/core";

/** 拆分文件名为 base + ext（保留点号） */
export function splitExt(fileName: string): { base: string; ext: string } {
  const dot = fileName.lastIndexOf(".");
  if (dot <= 0) return { base: fileName, ext: "" };
  return { base: fileName.slice(0, dot), ext: fileName.slice(dot) };
}

/** 拼接路径（兼容 Unix/Windows） */
export function joinPath(dir: string, name: string): string {
  const sep = dir.includes("\\") ? "\\" : "/";
  const trimmed = dir.endsWith("/") || dir.endsWith("\\") ? dir.slice(0, -1) : dir;
  return `${trimmed}${sep}${name}`;
}

/**
 * 解析输出文件路径：`{base}_脱敏{ext}`；重名时追加 `_1`、`_2`...
 * 并发下存在理论竞态（两个 worker 同时探测到同名不存在），但批次内文件基本不重名，可接受。
 */
export async function resolveOutputPath(outputDir: string, originalName: string): Promise<string> {
  const { base, ext } = splitExt(originalName);
  let candidate = `${base}_脱敏${ext}`;
  let n = 1;
  // check_file_exists 是项目已有 Tauri 命令，返回 Result<bool, String>
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const full = joinPath(outputDir, candidate);
    const exists = await invoke<boolean>("check_file_exists", { filePath: full }).catch(() => false);
    if (!exists) return full;
    candidate = `${base}_脱敏_${n}${ext}`;
    n++;
    if (n > 999) return full; // 兜底，防死循环
  }
}
