import { useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import toast from "react-hot-toast";
import i18n from "../i18n";
import { useWorkspaceStore } from "../stores/workspaceStore";
import type {
  FileContent,
  SensitiveItem,
  DesensitizeResult,
  Strategy,
  StrategyConfig,
  ProcessingRecord,
  ColumnInference,
  ColumnRule,
  DictEntry,
  WhitelistEntry,
  ConsistencyMapping,
  AliasGroup,
  ReplaceStyle,
} from "../types";
import { parseEncryptedError, isWrongPasswordError } from "../types";

export type { AutoDesensitizeStep } from "../types";

// ============================================================
// 共享脱敏流水线 — 供单文件与批量全自动模式复用
// ============================================================

/**
 * 脱敏流水线输入参数（全部显式传入，不依赖 React store）
 */
export interface PipelineOptions {
  workspaceId: string;
  strategies: Record<string, Strategy>;
  dictEntries: DictEntry[];
  enabledTypes: string[];
  replaceStyle: ReplaceStyle;
  consistencyMappings: ConsistencyMapping[];
  language: "zh" | "en";
  aliasGroups: AliasGroup[];
  whitelist: WhitelistEntry[];
  password?: string;
  /** 工作区模式，默认 "Desensitize"。TemplateReplace 时仅做检测，不调 apply_desensitize 也不保存记录 */
  mode?: "Desensitize" | "TemplateReplace";
}

/**
 * 脱敏流水线返回结果
 *
 * 当 `sensitiveItems` 为空（未检测到任何敏感项）或后续为模版模式时，
 * `desensitizeResult`/`record` 可能为 null —— 调用方据此决定 UI 流程。
 *
 * 额外字段（可选）供调用方执行 UI/store 副作用：
 * - columnInferences: regex 列推断（仅表格类型）
 * - nerColumnInferences: NER 列推断合并后的结果（仅表格类型且有 NER 结果）
 * - nerOnlyCount: NER 独有发现数量（用于 toast 提示）
 */
export interface PipelineResult {
  content: FileContent;
  sensitiveItems: SensitiveItem[];
  rawSensitiveItems: SensitiveItem[];
  desensitizeResult: DesensitizeResult | null;
  record: ProcessingRecord | null;
  durationMs: number;
  columnInferences?: ColumnInference[] | null;
  nerColumnInferences?: ColumnInference[] | null;
  nerOnlyCount?: number;
}

/** 构建策略配置列表（根据 items 中出现的类型 + workspace 默认策略） */
function buildStrategyConfigs(
  items: SensitiveItem[],
  strategiesMap: Record<string, Strategy>,
): StrategyConfig[] {
  return items
    .reduce<string[]>((acc, item) => {
      const key = typeof item.sensitive_type === "string"
        ? item.sensitive_type
        : "Custom";
      if (!acc.includes(key)) acc.push(key);
      return acc;
    }, [])
    .map((key) => ({
      sensitive_type: key === "Custom"
        ? { Custom: "Custom" }
        : (key as SensitiveItem["sensitive_type"]),
      strategy: strategiesMap[key] || { Mask: { keep_prefix: 1, keep_suffix: 1 } },
      consistent: true,
    }));
}

/** 合并去重（regex 优先，dict/ner 不覆盖已有项） */
function mergeItemsNoOverlap(
  base: SensitiveItem[],
  extra: SensitiveItem[],
): SensitiveItem[] {
  const merged = [...base];
  for (const di of extra) {
    const overlap = merged.some(
      (ex) =>
        ex.sheet_index === di.sheet_index &&
        ex.row === di.row &&
        ex.col === di.col &&
        ex.start < di.end &&
        di.start < ex.end
    );
    if (!overlap) merged.push(di);
  }
  return merged;
}

/** 白名单过滤 */
function applyWhitelist(
  items: SensitiveItem[],
  whitelist: WhitelistEntry[],
): SensitiveItem[] {
  if (whitelist.length === 0) return items;
  return items.filter((item) =>
    !whitelist.some((w) =>
      w.match_mode === "Exact"
        ? item.text === w.text
        : item.text.toLowerCase() === w.text.toLowerCase()
    )
  );
}

/** 按 enabledTypes 过滤 */
function applyEnabledTypes(
  items: SensitiveItem[],
  enabledTypes: string[],
): SensitiveItem[] {
  return items.filter((item) => {
    const key = typeof item.sensitive_type === "string"
      ? item.sensitive_type
      : "Custom";
    return enabledTypes.includes(key);
  });
}

/**
 * 纯非 React 的共享脱敏流水线函数
 *
 * 步骤：import → detect(regex/ner/dict + 表格列推断) → merge/dedupe →
 *       whitelist 过滤 → enabledTypes 过滤 → apply_desensitize → 保存处理记录
 *
 * 不触碰 useWorkspaceStore 或 React 状态；所有输入显式传参，所有错误向上抛出。
 *
 * 注意：当检测到 0 条敏感项时，返回的 `desensitizeResult` 为空 Result、
 * `record` 为 null —— 调用方需据此决定 UI 流程（例如是否切到对比视图）。
 */
export async function runDesensitizePipeline(
  filePath: string,
  options: PipelineOptions,
): Promise<PipelineResult> {
  const {
    workspaceId,
    strategies,
    dictEntries,
    enabledTypes,
    whitelist,
    password,
  } = options;

  const started = Date.now();
  const name = filePath.split(/[/\\]/).pop() || filePath;

  // 1. 解析文件
  const content = password
    ? await invoke<FileContent>("import_file_with_password", { filePath, password })
    : await invoke<FileContent>("import_file", { filePath });

  // 2. 识别敏感信息（regex + dict + NER 三路并行，表格类型附带列推断）
  const isSpreadsheet = content.type === "Spreadsheet";

  const detectPromises: [
    Promise<PromiseSettledResult<SensitiveItem[]>[]>,
    Promise<ColumnInference[] | null>,
  ] = [
    Promise.allSettled([
      invoke<SensitiveItem[]>("detect_by_regex", { content, enabledTypes }),
      dictEntries.length > 0
        ? invoke<SensitiveItem[]>("detect_by_dict", { content, dictEntries })
        : Promise.resolve([] as SensitiveItem[]),
      invoke<SensitiveItem[]>("detect_by_ner", { content }),
    ]),
    isSpreadsheet
      ? invoke<ColumnInference[]>("detect_columns", { content, sampleSize: 100 })
          .catch(() => null)
      : Promise.resolve(null),
  ];

  const [scanResults, columnInferences] = await Promise.all(detectPromises);

  const [regexResult, dictResult, nerResult] = scanResults;

  // regex 失败视为致命错误
  if (regexResult.status === "rejected") {
    throw regexResult.reason;
  }
  const regexItems = regexResult.value;
  const dictItems = dictResult.status === "fulfilled" ? dictResult.value : [];
  const nerItems = nerResult.status === "fulfilled" ? nerResult.value : [];

  // NER 列推断（表格类型）
  let nerColumnInferences: ColumnInference[] | null = null;
  if (isSpreadsheet && nerItems.length > 0 && content.type === "Spreadsheet") {
    const nerColInferences = buildNerColumnInferences(nerItems, content);
    if (nerColInferences.length > 0) {
      const regexColInferences = columnInferences || [];
      nerColumnInferences = mergeColumnInferences(regexColInferences, nerColInferences);
    }
  }

  // 合并去重
  let mergedItems = mergeItemsNoOverlap(regexItems, [...dictItems, ...nerItems]);

  // 白名单过滤
  mergedItems = applyWhitelist(mergedItems, whitelist);

  // enabledTypes 过滤（dict/ner 亦需过滤；regex 已在后端过滤）
  mergedItems = applyEnabledTypes(mergedItems, enabledTypes);

  // 统计 NER 独有发现数量
  const nerOnlyCount = nerItems.filter(
    (ni) =>
      !regexItems.some(
        (ex) =>
          ex.sheet_index === ni.sheet_index &&
          ex.row === ni.row &&
          ex.col === ni.col &&
          ex.start < ni.end &&
          ni.start < ex.end
      ) &&
      !dictItems.some(
        (ex) =>
          ex.sheet_index === ni.sheet_index &&
          ex.row === ni.row &&
          ex.col === ni.col &&
          ex.start < ni.end &&
          ni.start < ex.end
      )
  ).length;

  // 空识别结果：返回空 result（caller 据此决定 UI）
  if (mergedItems.length === 0) {
    return {
      content,
      sensitiveItems: [],
      rawSensitiveItems: [],
      desensitizeResult: {
        content,
        mappings: [],
        summary: { total: 0, by_type: {} },
      },
      record: null,
      durationMs: Date.now() - started,
      columnInferences,
      nerColumnInferences,
      nerOnlyCount,
    };
  }

  // 模版模式：仅做检测，不执行 apply_desensitize，不写入处理记录
  if (options.mode === "TemplateReplace") {
    return {
      content,
      sensitiveItems: mergedItems,
      rawSensitiveItems: mergedItems,
      desensitizeResult: null,
      record: null,
      durationMs: Date.now() - started,
      columnInferences,
      nerColumnInferences,
      nerOnlyCount,
    };
  }

  // 3. 构建策略配置并执行脱敏
  const strategyConfigs = buildStrategyConfigs(mergedItems, strategies);
  const desensitizeResult = await invoke<DesensitizeResult>("apply_desensitize", {
    content,
    items: mergedItems,
    strategies: strategyConfigs,
    workspaceId,
  });

  // 4. 保存处理记录
  const record: ProcessingRecord = {
    id: generateRecordId(),
    file_name: name,
    file_path: filePath,
    file_type: content.file_type,
    processed_at: new Date().toISOString(),
    mappings: desensitizeResult.mappings,
    sensitive_count: desensitizeResult.summary.total,
    status: "Completed",
  };

  await invoke("add_processing_record", {
    workspaceId,
    record,
  });

  return {
    content,
    sensitiveItems: mergedItems,
    rawSensitiveItems: mergedItems,
    desensitizeResult,
    record,
    durationMs: Date.now() - started,
    columnInferences,
    nerColumnInferences,
    nerOnlyCount,
  };
}

/**
 * 从 NER 扫描结果中聚合列级类型推断
 * 按 (sheetIndex, col) 分组统计各类型命中数，取最高频类型作为该列推断
 */
function buildNerColumnInferences(
  nerItems: SensitiveItem[],
  content: FileContent & { type: "Spreadsheet" },
): ColumnInference[] {
  // 按 (sheetIndex, col) 分组
  type GroupKey = string;
  const colGroups = new Map<GroupKey, Map<string, number>>();
  const colHitRows = new Map<GroupKey, Set<number>>();

  for (const item of nerItems) {
    const typeKey = typeof item.sensitive_type === "string"
      ? item.sensitive_type
      : "Custom";

    const gk = `${item.sheet_index}:${item.col}`;
    if (!colGroups.has(gk)) {
      colGroups.set(gk, new Map());
      colHitRows.set(gk, new Set());
    }
    const typeCounts = colGroups.get(gk)!;
    typeCounts.set(typeKey, (typeCounts.get(typeKey) || 0) + 1);
    colHitRows.get(gk)!.add(item.row);
  }

  const inferences: ColumnInference[] = [];
  for (const [gk, typeCounts] of colGroups) {
    const [sheetIdxStr, colStr] = gk.split(":");
    const sheetIdx = Number(sheetIdxStr);
    const col = Number(colStr);
    const sheet = content.sheets[sheetIdx];
    if (!sheet) continue;

    // 取命中次数最多的类型
    let bestType = "";
    let bestCount = 0;
    for (const [t, c] of typeCounts) {
      if (c > bestCount) {
        bestType = t;
        bestCount = c;
      }
    }

    const hitRows = colHitRows.get(gk)!.size;
    const rowCount = sheet.row_count;
    const confidence = rowCount > 0 ? hitRows / rowCount : 0;

    // NER 阈值较低：>= 10% 命中即视为有效推断
    if (confidence >= 0.1) {
      inferences.push({
        col,
        header: sheet.headers[col] || `Col ${col}`,
        inferred_type: bestType as ColumnInference["inferred_type"],
        confidence,
        sample_hits: hitRows,
        sample_total: rowCount,
        sheet_index: sheetIdx,
      });
    }
  }

  return inferences;
}

/**
 * 合并正则列推断和 NER 列推断
 * NER 填补 regex 未检测到的列，两者都有时取置信度更高的
 */
function mergeColumnInferences(
  regexInferences: ColumnInference[],
  nerInferences: ColumnInference[],
): ColumnInference[] {
  const merged = new Map<string, ColumnInference>();

  // 先放入 regex 推断
  for (const ri of regexInferences) {
    merged.set(`${ri.sheet_index}:${ri.col}`, ri);
  }

  // NER 推断：填补空缺或置信度更高时覆盖
  for (const ni of nerInferences) {
    const key = `${ni.sheet_index}:${ni.col}`;
    const existing = merged.get(key);
    if (!existing || existing.inferred_type === null || ni.confidence > existing.confidence) {
      merged.set(key, ni);
    }
  }

  return Array.from(merged.values()).sort((a, b) =>
    a.sheet_index !== b.sheet_index ? a.sheet_index - b.sheet_index : a.col - b.col
  );
}

export function useAutoDesensitize() {
  const isProcessingRef = useRef(false);

  // 依赖为空：内部通过 useWorkspaceStore.getState() 读取最新 store，无闭包陈旧风险
  const processFile = useCallback(async (filePath: string, password?: string) => {
    // 并发保护：正在处理时拒绝新文件
    if (isProcessingRef.current) {
      toast.error(i18n.t("hook.processing"));
      return;
    }
    isProcessingRef.current = true;

    const store = useWorkspaceStore.getState();
    const wsData = store.activeWorkspaceData;
    if (!wsData) {
      toast.error(i18n.t("hook.selectWorkspace"));
      isProcessingRef.current = false;
      return;
    }

    const ws = wsData.workspace;
    const enabledTypes = ws.enabled_types;

    // 提取文件名
    const name = filePath.split(/[/\\]/).pop() || filePath;

    try {
      // 1. 解析
      store.setProcessingStep("parsing", name);
      store.setCenterView("processing");

      // 2. 调用共享流水线（纯函数，不触碰 store）
      store.setProcessingStep("detecting");
      const pipeline = await runDesensitizePipeline(filePath, {
        workspaceId: ws.id,
        strategies: ws.strategies,
        dictEntries: ws.dict_entries,
        enabledTypes,
        replaceStyle: ws.replace_style || "Fake",
        consistencyMappings: ws.consistency_mappings || [],
        language: "zh",
        aliasGroups: ws.alias_groups || [],
        whitelist: ws.whitelist || [],
        password,
        mode: ws.mode || "Desensitize",
      });

      // 解密成功时关闭密码弹窗
      if (password) {
        store.setPasswordModal(null);
      }

      const { content, sensitiveItems, columnInferences, nerColumnInferences, nerOnlyCount } = pipeline;
      store.setCurrentFileContent(content, filePath);

      // 保存列推断结果到 store（用于 ComparisonView 列头展示）
      if (columnInferences) {
        store.setColumnInferences(columnInferences);
        store.setIsColumnMode(true);
      }
      if (nerColumnInferences) {
        store.setColumnInferences(nerColumnInferences);
        store.setIsColumnMode(true);
      }

      if (nerOnlyCount && nerOnlyCount > 0) {
        toast.success(i18n.t("hook.nerFound", { count: nerOnlyCount }));
      }

      // 空识别结果：仍进入对比视图允许手动标记
      if (sensitiveItems.length === 0) {
        store.setCurrentSensitiveItems([]);
        if (pipeline.desensitizeResult) {
          store.setCurrentResult(pipeline.desensitizeResult);
        }
        store.setCenterView("comparison");
        store.setProcessingStep("done");
        toast(i18n.t("hook.noSensitiveManual"), { icon: "ℹ️" });
        return;
      }

      // 保存识别到的敏感项到 store
      store.setRawSensitiveItems(pipeline.rawSensitiveItems);
      store.setCurrentSensitiveItems(sensitiveItems);

      const isTemplateMode = ws.mode === "TemplateReplace";

      if (isTemplateMode) {
        // 模版模式：仅检测，前端根据词典映射实时渲染预览
        store.setCenterView("comparison");
        store.setProcessingStep("done");
        return;
      }

      // 脱敏模式：pipeline 已执行 apply_desensitize 并保存记录
      store.setProcessingStep("desensitizing");
      store.setProcessingStep("saving");

      if (pipeline.record && pipeline.desensitizeResult) {
        store.setCurrentRecordId(pipeline.record.id);
        store.setCurrentResult(pipeline.desensitizeResult);
        await store.refreshActiveWorkspace();
      }
      store.setCenterView("comparison");
      store.setProcessingStep("done");

      // 批量模式：当前文件已成功处理，等待用户确认导出
      // （status 保持 "processing"，导出后由 WorkspaceLayout 标记为 "confirmed"）

    } catch (err) {
      const encryptedType = parseEncryptedError(err);
      if (encryptedType) {
        // 批量模式：跳过加密文件，不弹密码框
        const wsStore = useWorkspaceStore.getState();
        if (wsStore.isBatchMode()) {
          const currentFile = wsStore.fileQueue[wsStore.activeQueueIndex];
          if (currentFile) {
            wsStore.updateQueueFileStatus(currentFile.id, "failed", i18n.t("hook.encryptedSkipped"));
          }
          toast.error(i18n.t("hook.encrypted", { name: filePath.split(/[/\\]/).pop() }));
          store.setProcessingStep("idle");

          const nextFile = wsStore.advanceToNextFile();
          if (nextFile) {
            isProcessingRef.current = false;
            setTimeout(() => processFile(nextFile.filePath), 0);
            return;
          } else {
            toast.success(i18n.t("fileQueue.allDone"));
            store.setCenterView("dropzone");
          }
        } else {
          // 单文件模式：保留原有密码弹窗逻辑
          store.setPasswordModal({
            visible: true,
            filePath,
            fileType: encryptedType,
            attemptsLeft: 3,
            errorMessage: null,
          });
          store.setProcessingStep("idle");
        }
      } else if (isWrongPasswordError(err)) {
        // 密码错误 → 更新弹窗状态
        const modal = store.passwordModal;
        const left = modal.attemptsLeft - 1;
        if (left > 0) {
          store.setPasswordModal({
            ...modal,
            visible: true,
            attemptsLeft: left,
            errorMessage: i18n.t("hook.passwordWrong", { left }),
          });
        } else {
          store.setPasswordModal(null);
          toast.error(i18n.t("hook.passwordTooMany"));
          store.setCenterView("dropzone");
        }
        store.setProcessingStep("idle");
      } else {
        const message =
          typeof err === "string" ? err :
          err instanceof Error ? err.message : i18n.t("hook.processFailed");

        // 批量模式：标记失败，自动跳到下一个
        const wsStore = useWorkspaceStore.getState();
        if (wsStore.isBatchMode()) {
          const currentFile = wsStore.fileQueue[wsStore.activeQueueIndex];
          if (currentFile) {
            wsStore.updateQueueFileStatus(currentFile.id, "failed", message);
          }
          toast.error(i18n.t("hook.batchFileFailed", { name: filePath.split(/[/\\]/).pop(), message }));
          store.setProcessingStep("idle");

          const nextFile = wsStore.advanceToNextFile();
          if (nextFile) {
            isProcessingRef.current = false;
            setTimeout(() => processFile(nextFile.filePath), 0);
            return;
          } else {
            toast.success(i18n.t("fileQueue.allDone"));
            store.setCenterView("dropzone");
          }
        } else {
          toast.error(message);
          store.setProcessingStep("idle");
          store.setCenterView("dropzone");
        }
      }
    } finally {
      isProcessingRef.current = false;
    }
  }, []);

  /** 对单列重新脱敏（ComparisonView 中用户微调后调用） */
  const reDesensitizeColumn = useCallback(async (rule: ColumnRule) => {
    const store = useWorkspaceStore.getState();
    const originalContent = store.currentFileContent;
    const currentResult = store.currentResult;

    if (!originalContent || !currentResult || originalContent.type !== "Spreadsheet" || currentResult.content.type !== "Spreadsheet") {
      return;
    }

    const wsData = store.activeWorkspaceData;
    if (!wsData) return;

    const sheetIdx = rule.sheet_index;

    try {
      // 用原始内容的该列 + 新规则重新脱敏，然后合并到现有结果中
      const result = await invoke<DesensitizeResult>("apply_desensitize_by_columns", {
        content: originalContent,
        columnRules: [rule],
        workspaceId: wsData.workspace.id,
        recordId: `temp_${Date.now()}`,
      });

      if (result.content.type !== "Spreadsheet") return;

      // 合并：把该列的脱敏结果替换到现有 result 中的对应 sheet
      const mergedSheets = currentResult.content.sheets.map((sheet, si) => {
        if (si !== sheetIdx) return sheet;
        const resultSheet = result.content.type === "Spreadsheet" ? result.content.sheets[si] : null;
        const mergedRows = sheet.rows.map((row, rowIdx) => {
          const newRow = [...row];
          newRow[rule.col] = resultSheet?.rows[rowIdx]?.[rule.col] ?? row[rule.col];
          return newRow;
        });
        return { ...sheet, rows: mergedRows };
      });

      // 合并 mappings：保留旧映射 + 附加新映射，然后去重
      const allMappings = [...currentResult.mappings, ...result.mappings];
      const seen = new Set<string>();
      const dedupedMappings = allMappings.filter((m) => {
        const key = `${m.original_text}:${m.replaced_text}`;
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      });

      const newResult: DesensitizeResult = {
        content: {
          ...currentResult.content,
          sheets: mergedSheets,
        },
        mappings: dedupedMappings,
        summary: {
          total: currentResult.summary.total + result.summary.total,
          by_type: { ...currentResult.summary.by_type },
        },
      };

      // 合并 by_type
      for (const [k, v] of Object.entries(result.summary.by_type)) {
        newResult.summary.by_type[k] = (newResult.summary.by_type[k] || 0) + v;
      }

      store.setCurrentResult(newResult);

      // 持久化更新后的映射到处理记录（确保还原时能找到新映射）
      const recordId = store.currentRecordId;
      if (recordId && wsData.workspace.id) {
        try {
          await invoke("update_processing_record_mappings", {
            workspaceId: wsData.workspace.id,
            recordId,
            mappings: dedupedMappings,
          });
        } catch (persistErr) {
          const msg = typeof persistErr === "string" ? persistErr : i18n.t("hook.mappingSaveFailed");
          toast.error(msg);
        }
      }

      // 重建该列的高亮 items（仅匹配同 sheet 和同列）
      const existingItems = store.currentSensitiveItems.filter(
        (i) => !(i.sheet_index === sheetIdx && i.col === rule.col)
      );
      const origSheet = originalContent.sheets[sheetIdx];
      const mergedSheet = mergedSheets[sheetIdx];
      const newItems: SensitiveItem[] = [];
      if (origSheet && mergedSheet) {
        for (let rowIdx = 0; rowIdx < origSheet.rows.length; rowIdx++) {
          const originalCell = origSheet.rows[rowIdx]?.[rule.col]?.text ?? "";
          const newCell = mergedSheet.rows[rowIdx]?.[rule.col]?.text ?? "";
          if (originalCell && originalCell !== newCell) {
            newItems.push({
              id: `col_${sheetIdx}_${rule.col}_row_${rowIdx}`,
              text: originalCell,
              sensitive_type: rule.sensitive_type as SensitiveItem["sensitive_type"],
              source: "Regex",
              confidence: 1.0,
              start: 0,
              end: originalCell.length,
              row: rowIdx + 1,
              col: rule.col,
              sheet_index: sheetIdx,
            });
          }
        }
      }
      store.setCurrentSensitiveItems([...existingItems, ...newItems]);

      const header = origSheet?.headers[rule.col] || String(rule.col);
      toast.success(i18n.t("hook.columnDesensitized", { header }));
    } catch (err) {
      const message = typeof err === "string" ? err : i18n.t("hook.columnDesensitizeFailed");
      toast.error(message);
    }
  }, []);

  /** 对手动标记的敏感项执行脱敏（跳过解析和识别步骤） */
  const desensitizeManualItems = useCallback(async () => {
    const store = useWorkspaceStore.getState();
    const content = store.currentFileContent;
    const items = store.currentSensitiveItems;
    const wsData = store.activeWorkspaceData;

    if (!content || items.length === 0 || !wsData) {
      toast.error(i18n.t("hook.noDesensitizeItems"));
      return;
    }

    const ws = wsData.workspace;
    const isTemplateMode = ws.mode === "TemplateReplace";

    try {
      store.setProcessingStep("desensitizing");

      if (isTemplateMode) {
        // 模版模式：手动项直接更新列表，前端实时渲染预览，不需要调后端
        store.setCurrentSensitiveItems(items);
        store.setProcessingStep("done");
        return;
      }

      // 脱敏模式：现有逻辑不变
      const strategies: StrategyConfig[] = items
        .reduce<string[]>((acc, item) => {
          const key = typeof item.sensitive_type === "string"
            ? item.sensitive_type
            : "Custom";
          if (!acc.includes(key)) acc.push(key);
          return acc;
        }, [])
        .map((key) => ({
          sensitive_type: key === "Custom"
            ? { Custom: "Custom" }
            : (key as SensitiveItem["sensitive_type"]),
          strategy: ws.strategies[key] || { Mask: { keep_prefix: 1, keep_suffix: 1 } },
          consistent: true,
        }));

      const result = await invoke<DesensitizeResult>("apply_desensitize", {
        content,
        items,
        strategies,
        workspaceId: ws.id,
      });

      // 保存处理记录
      const name = content.file_name;
      const record: ProcessingRecord = {
        id: generateRecordId(),
        file_name: name,
        file_path: store.currentFilePath || "",
        file_type: content.file_type,
        processed_at: new Date().toISOString(),
        mappings: result.mappings,
        sensitive_count: result.summary.total,
        status: "Completed",
      };

      await invoke("add_processing_record", {
        workspaceId: ws.id,
        record,
      });

      store.setCurrentRecordId(record.id);
      store.setCurrentResult(result);
      await store.refreshActiveWorkspace();
      store.setProcessingStep("done");
      toast.success(i18n.t("hook.desensitizedCount", { count: result.summary.total }));
    } catch (err) {
      const message =
        typeof err === "string" ? err :
        err instanceof Error ? err.message : i18n.t("hook.desensitizeFailed");
      toast.error(message);
      store.setProcessingStep("done");
    }
  }, []);

  /** 从历史记录重新处理文件 */
  const reprocessFromRecord = useCallback(async (record: ProcessingRecord) => {
    // 清除历史查看状态
    const store = useWorkspaceStore.getState();
    store.setCurrentResult(null);
    store.setCurrentSensitiveItems([]);
    set_activeRecordId_null();

    // 先检测文件是否存在（通过尝试 invoke import_file 快速失败）
    let filePath = record.file_path;
    try {
      await invoke("check_file_exists", { filePath });
    } catch {
      toast.error(i18n.t("hook.originalFileMissing"));
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        multiple: false,
        filters: [{ name: "支持的文件", extensions: ["xlsx", "xls", "csv", "docx", "txt", "pdf"] }],
      });
      if (selected) {
        filePath = selected as string;
      } else {
        store.setCenterView("dropzone");
        return;
      }
    }

    await processFile(filePath);
  }, [processFile]);

  /** 撤销某列的脱敏（"不脱敏此列"） */
  const undoColumnDesensitize = useCallback((col: number, sheetIndex: number = 0) => {
    const store = useWorkspaceStore.getState();
    const originalContent = store.currentFileContent;
    const currentResult = store.currentResult;

    if (!originalContent || !currentResult ||
        originalContent.type !== "Spreadsheet" ||
        currentResult.content.type !== "Spreadsheet") {
      return;
    }

    // 1. 从原始内容恢复该 sheet 该列数据
    const restoredSheets = currentResult.content.sheets.map((sheet, si) => {
      if (si !== sheetIndex) return sheet;
      const origSheet = originalContent.sheets[si];
      if (!origSheet) return sheet;
      const restoredRows = sheet.rows.map((row, rowIdx) => {
        const newRow = [...row];
        newRow[col] = origSheet.rows[rowIdx]?.[col] ?? row[col];
        return newRow;
      });
      return { ...sheet, rows: restoredRows };
    });

    // 2. 清除该 sheet 该列的 sensitiveItems
    const filteredItems = store.currentSensitiveItems.filter(
      (item) => !(item.sheet_index === sheetIndex && item.col === col)
    );

    // 3. 清除只属于该列的 mappings（保留其他列也使用的映射）
    const otherColTexts = new Set<string>();
    for (const item of filteredItems) {
      otherColTexts.add(item.text);
    }
    const origSheet = originalContent.sheets[sheetIndex];
    const colOriginalTexts = new Set<string>();
    if (origSheet) {
      for (let rowIdx = 0; rowIdx < origSheet.rows.length; rowIdx++) {
        const cellValue = origSheet.rows[rowIdx]?.[col];
        if (cellValue?.text) colOriginalTexts.add(cellValue.text);
      }
    }
    const filteredMappings = currentResult.mappings.filter((m) => {
      if (colOriginalTexts.has(m.original_text) && !otherColTexts.has(m.original_text)) {
        return false;
      }
      return true;
    });

    // 4. 重新统计 summary
    const byType: Record<string, number> = {};
    for (const item of filteredItems) {
      const key = typeof item.sensitive_type === "string" ? item.sensitive_type : "Custom";
      byType[key] = (byType[key] || 0) + 1;
    }

    const newResult: DesensitizeResult = {
      content: { ...currentResult.content, sheets: restoredSheets },
      mappings: filteredMappings,
      summary: { total: filteredItems.length, by_type: byType },
    };

    store.setCurrentResult(newResult);
    store.setCurrentSensitiveItems(filteredItems);

    const header = origSheet?.headers[col] || String(col);
    toast.success(i18n.t("hook.columnUndone", { header }));
  }, []);

  /** 处理粘贴板文本：调用后端解析，然后走正常脱敏流程 */
  const processClipboardText = useCallback(async (text: string) => {
    if (isProcessingRef.current) {
      toast.error(i18n.t("hook.processing"));
      return;
    }
    isProcessingRef.current = true;

    const store = useWorkspaceStore.getState();

    try {
      // 解析粘贴板文本
      store.setProcessingStep("parsing", i18n.t("hook.clipboardText"));
      store.setCenterView("processing");

      const content = await invoke<FileContent>("import_clipboard_text", { text });
      // 使用虚拟路径，粘贴板无文件路径
      store.setCurrentFileContent(content, "clipboard://text");

      // 后续流程与 processFile 相同：识别 + 脱敏 + 保存记录
      const wsData = store.activeWorkspaceData;
      if (!wsData) {
        toast.error(i18n.t("hook.selectWorkspace"));
        isProcessingRef.current = false;
        store.setProcessingStep("idle");
        return;
      }

      const ws = wsData.workspace;
      const enabledTypes = ws.enabled_types;

      store.setProcessingStep("detecting");

      const scanResults = await Promise.allSettled([
        invoke<SensitiveItem[]>("detect_by_regex", { content, enabledTypes }),
        ws.dict_entries.length > 0
          ? invoke<SensitiveItem[]>("detect_by_dict", { content, dictEntries: ws.dict_entries })
          : Promise.resolve([] as SensitiveItem[]),
        invoke<SensitiveItem[]>("detect_by_ner", { content }),
      ]);

      const [regexResult, dictResult, nerResult] = scanResults;

      if (regexResult.status === "rejected") {
        throw regexResult.reason;
      }
      const regexItems = regexResult.value;
      const dictItems = dictResult.status === "fulfilled" ? dictResult.value : [];
      const nerItems = nerResult.status === "fulfilled" ? nerResult.value : [];

      // 合并去重
      const mergedItems = [...regexItems];
      for (const di of [...dictItems, ...nerItems]) {
        const overlap = mergedItems.some(
          (ex) =>
            ex.sheet_index === di.sheet_index &&
            ex.row === di.row &&
            ex.col === di.col &&
            ex.start < di.end &&
            di.start < ex.end
        );
        if (!overlap) mergedItems.push(di);
      }

      // 白名单过滤
      const whitelist = ws.whitelist || [];
      if (whitelist.length > 0) {
        const afterWhitelist = mergedItems.filter((item) =>
          !whitelist.some((w) =>
            w.match_mode === "Exact"
              ? item.text === w.text
              : item.text.toLowerCase() === w.text.toLowerCase()
          )
        );
        mergedItems.length = 0;
        mergedItems.push(...afterWhitelist);
      }

      // enabledTypes 过滤
      const filteredItems = mergedItems.filter((item) => {
        const key = typeof item.sensitive_type === "string"
          ? item.sensitive_type
          : "Custom";
        return enabledTypes.includes(key);
      });

      mergedItems.length = 0;
      mergedItems.push(...filteredItems);

      if (mergedItems.length === 0) {
        store.setCurrentSensitiveItems([]);
        const emptyResult: DesensitizeResult = {
          content: content,
          mappings: [],
          summary: { total: 0, by_type: {} },
        };
        store.setCurrentResult(emptyResult);
        store.setCenterView("comparison");
        store.setProcessingStep("done");
        toast(i18n.t("hook.noSensitiveManual"), { icon: "ℹ️" });
        return;
      }

      store.setRawSensitiveItems(mergedItems);
      store.setCurrentSensitiveItems(mergedItems);

      const isTemplateMode = ws.mode === "TemplateReplace";

      if (isTemplateMode) {
        // 模版模式：只做检测，前端根据词典映射实时渲染预览，导出时才调后端
        store.setCurrentSensitiveItems(mergedItems);
        store.setCenterView("comparison");
        store.setProcessingStep("done");
        return;
      }

      // 脱敏模式：现有逻辑不变
      store.setProcessingStep("desensitizing");
      const strategies: StrategyConfig[] = mergedItems
        .reduce<string[]>((acc, item) => {
          const key = typeof item.sensitive_type === "string"
            ? item.sensitive_type
            : "Custom";
          if (!acc.includes(key)) acc.push(key);
          return acc;
        }, [])
        .map((key) => ({
          sensitive_type: key === "Custom"
            ? { Custom: "Custom" }
            : (key as SensitiveItem["sensitive_type"]),
          strategy: ws.strategies[key] || { Mask: { keep_prefix: 1, keep_suffix: 1 } },
          consistent: true,
        }));

      const result = await invoke<DesensitizeResult>("apply_desensitize", {
        content,
        items: mergedItems,
        strategies,
        workspaceId: ws.id,
      });

      // 保存处理记录
      store.setProcessingStep("saving");
      const record: ProcessingRecord = {
        id: generateRecordId(),
        file_name: i18n.t("hook.clipboardText"),
        file_path: "clipboard://text",
        file_type: "Txt",
        processed_at: new Date().toISOString(),
        mappings: result.mappings,
        sensitive_count: result.summary.total,
        status: "Completed",
      };

      await invoke("add_processing_record", {
        workspaceId: ws.id,
        record,
      });

      store.setCurrentRecordId(record.id);
      store.setCurrentResult(result);
      await store.refreshActiveWorkspace();
      store.setCenterView("comparison");
      store.setProcessingStep("done");
    } catch (err) {
      const message =
        typeof err === "string" ? err :
        err instanceof Error ? err.message : "处理失败";
      toast.error(message);
      store.setProcessingStep("idle");
      store.setCenterView("dropzone");
    } finally {
      isProcessingRef.current = false;
    }
  }, []);

  /** 根据当前 enabledTypes 重新过滤并脱敏（切换类型开关后调用） */
  const reDesensitizeWithFilteredItems = useCallback(async () => {
    const store = useWorkspaceStore.getState();
    const content = store.currentFileContent;
    const rawItems = store.rawSensitiveItems;
    const wsData = store.activeWorkspaceData;

    if (!content || !wsData) return;

    const ws = wsData.workspace;

    const isTemplateMode = ws.mode === "TemplateReplace";
    if (isTemplateMode) {
      // 模版替换模式下不需要 reDesensitize（TypeSelector/RulesSection 已隐藏）
      return;
    }

    const enabledTypes = ws.enabled_types;

    // 白名单过滤
    const whitelist = ws.whitelist || [];
    const afterWhitelist = whitelist.length > 0
      ? rawItems.filter((item) =>
          !whitelist.some((w) =>
            w.match_mode === "Exact"
              ? item.text === w.text
              : item.text.toLowerCase() === w.text.toLowerCase()
          )
        )
      : rawItems;

    // 按 enabledTypes 过滤
    const filtered = afterWhitelist.filter((item) => {
      const key = typeof item.sensitive_type === "string"
        ? item.sensitive_type
        : "Custom";
      return enabledTypes.includes(key);
    });

    if (filtered.length === 0) {
      // 无敏感项，result 的 content 就是原始内容
      store.setCurrentSensitiveItems([]);
      store.setCurrentResult({
        content: content,
        mappings: [],
        summary: { total: 0, by_type: {} },
      });
      return;
    }

    // 构建策略并执行脱敏
    const strategies: StrategyConfig[] = filtered
      .reduce<string[]>((acc, item) => {
        const key = typeof item.sensitive_type === "string"
          ? item.sensitive_type
          : "Custom";
        if (!acc.includes(key)) acc.push(key);
        return acc;
      }, [])
      .map((key) => ({
        sensitive_type: key === "Custom"
          ? { Custom: "Custom" }
          : (key as SensitiveItem["sensitive_type"]),
        strategy: ws.strategies[key] || { Mask: { keep_prefix: 1, keep_suffix: 1 } },
        consistent: true,
      }));

    try {
      const result = await invoke<DesensitizeResult>("apply_desensitize", {
        content,
        items: filtered,
        strategies,
        workspaceId: ws.id,
      });
      store.setCurrentSensitiveItems(filtered);
      store.setCurrentResult(result);
      toast.success(i18n.t("hook.strategyUpdated"));
    } catch (err) {
      const message = typeof err === "string" ? err : i18n.t("hook.reDesensitizeFailed");
      toast.error(message);
    }
  }, []);

  const reset = useCallback(() => {
    const store = useWorkspaceStore.getState();
    store.setProcessingStep("idle", "");
    store.resetColumnState();
  }, []);

  return { processFile, processClipboardText, reDesensitizeColumn, undoColumnDesensitize, desensitizeManualItems, reDesensitizeWithFilteredItems, reprocessFromRecord, reset };
}

/** 清除 activeRecordId */
function set_activeRecordId_null() {
  useWorkspaceStore.setState({ activeRecordId: null });
}

function generateRecordId(): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const date = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}`;
  const time = `${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  const rand = Math.random().toString(36).slice(2, 10);
  return `rec_${date}_${time}_${rand}`;
}

/**
 * 独立版 processFile — 不依赖 React hook，可在模块级直接调用。
 * 内部逻辑与 useAutoDesensitize 中的 processFile useCallback 完全一致。
 * 供 E2E 测试通过 window.__DIMKEY_PROCESS_FILE__ 调用。
 */
export async function processFileStandalone(filePath: string, password?: string): Promise<void> {
  const store = useWorkspaceStore.getState();
  const wsData = store.activeWorkspaceData;
  if (!wsData) {
    toast.error(i18n.t("hook.selectWorkspace"));
    return;
  }

  const ws = wsData.workspace;
  const enabledTypes = ws.enabled_types;
  const name = filePath.split(/[/\\]/).pop() || filePath;

  try {
    // 1. 解析文件
    store.setProcessingStep("parsing", name);
    store.setCenterView("processing");
    const content = password
      ? await invoke<FileContent>("import_file_with_password", { filePath, password })
      : await invoke<FileContent>("import_file", { filePath });
    if (password) {
      store.setPasswordModal(null);
    }
    store.setCurrentFileContent(content, filePath);

    // 2. 识别敏感信息（regex + dict + NER 三路并行）
    store.setProcessingStep("detecting");

    const isSpreadsheet = content.type === "Spreadsheet";

    const detectPromises: [
      Promise<PromiseSettledResult<SensitiveItem[]>[]>,
      Promise<ColumnInference[] | null>,
    ] = [
      Promise.allSettled([
        invoke<SensitiveItem[]>("detect_by_regex", { content, enabledTypes }),
        ws.dict_entries.length > 0
          ? invoke<SensitiveItem[]>("detect_by_dict", { content, dictEntries: ws.dict_entries })
          : Promise.resolve([] as SensitiveItem[]),
        invoke<SensitiveItem[]>("detect_by_ner", { content }),
      ]),
      isSpreadsheet
        ? invoke<ColumnInference[]>("detect_columns", { content, sampleSize: 100 })
            .catch(() => null)
        : Promise.resolve(null),
    ];

    const [scanResults, columnInferences] = await Promise.all(detectPromises);

    if (columnInferences) {
      store.setColumnInferences(columnInferences);
      store.setIsColumnMode(true);
    }

    const [regexResult, dictResult, nerResult] = scanResults;

    if (regexResult.status === "rejected") {
      throw regexResult.reason;
    }
    const regexItems = regexResult.value;
    const dictItems = dictResult.status === "fulfilled" ? dictResult.value : [];
    const nerItems = nerResult.status === "fulfilled" ? nerResult.value : [];

    if (isSpreadsheet && nerItems.length > 0 && content.type === "Spreadsheet") {
      const nerColInferences = buildNerColumnInferences(nerItems, content);
      if (nerColInferences.length > 0) {
        const regexColInferences = columnInferences || [];
        const merged = mergeColumnInferences(regexColInferences, nerColInferences);
        store.setColumnInferences(merged);
        store.setIsColumnMode(true);
      }
    }

    const mergedItems = [...regexItems];
    for (const di of [...dictItems, ...nerItems]) {
      const overlap = mergedItems.some(
        (ex) =>
          ex.sheet_index === di.sheet_index &&
          ex.row === di.row &&
          ex.col === di.col &&
          ex.start < di.end &&
          di.start < ex.end
      );
      if (!overlap) mergedItems.push(di);
    }

    const whitelist = ws.whitelist || [];
    if (whitelist.length > 0) {
      const afterWhitelist = mergedItems.filter((item) =>
        !whitelist.some((w) =>
          w.match_mode === "Exact"
            ? item.text === w.text
            : item.text.toLowerCase() === w.text.toLowerCase()
        )
      );
      mergedItems.length = 0;
      mergedItems.push(...afterWhitelist);
    }

    const filteredItems = mergedItems.filter((item) => {
      const key = typeof item.sensitive_type === "string" ? item.sensitive_type : "Custom";
      return enabledTypes.includes(key);
    });

    mergedItems.length = 0;
    mergedItems.push(...filteredItems);

    const nerOnlyCount = nerItems.filter(
      (ni) =>
        !regexItems.some(
          (ex) =>
            ex.sheet_index === ni.sheet_index &&
            ex.row === ni.row &&
            ex.col === ni.col &&
            ex.start < ni.end &&
            ni.start < ex.end
        ) &&
        !dictItems.some(
          (ex) =>
            ex.sheet_index === ni.sheet_index &&
            ex.row === ni.row &&
            ex.col === ni.col &&
            ex.start < ni.end &&
            ni.start < ex.end
        )
    ).length;
    if (nerOnlyCount > 0) {
      toast.success(i18n.t("hook.nerFound", { count: nerOnlyCount }));
    }

    if (mergedItems.length === 0) {
      store.setCurrentSensitiveItems([]);
      const emptyResult: DesensitizeResult = {
        content: content,
        mappings: [],
        summary: { total: 0, by_type: {} },
      };
      store.setCurrentResult(emptyResult);
      store.setCenterView("comparison");
      store.setProcessingStep("done");
      toast(i18n.t("hook.noSensitiveManual"), { icon: "ℹ️" });
      return;
    }

    store.setRawSensitiveItems(mergedItems);
    store.setCurrentSensitiveItems(mergedItems);

    const isTemplateMode = ws.mode === "TemplateReplace";

    if (isTemplateMode) {
      store.setCurrentSensitiveItems(mergedItems);
      store.setCenterView("comparison");
      store.setProcessingStep("done");
      return;
    }

    // 3. 构建策略配置并执行脱敏
    store.setProcessingStep("desensitizing");
    const strategies: StrategyConfig[] = mergedItems
      .reduce<string[]>((acc, item) => {
        const key = typeof item.sensitive_type === "string" ? item.sensitive_type : "Custom";
        if (!acc.includes(key)) acc.push(key);
        return acc;
      }, [])
      .map((key) => ({
        sensitive_type: key === "Custom"
          ? { Custom: "Custom" }
          : (key as SensitiveItem["sensitive_type"]),
        strategy: ws.strategies[key] || { Mask: { keep_prefix: 1, keep_suffix: 1 } },
        consistent: true,
      }));

    const result = await invoke<DesensitizeResult>("apply_desensitize", {
      content,
      items: mergedItems,
      strategies,
      workspaceId: ws.id,
    });

    // 4. 保存处理记录
    store.setProcessingStep("saving");
    const record: ProcessingRecord = {
      id: generateRecordId(),
      file_name: name,
      file_path: filePath,
      file_type: content.file_type,
      processed_at: new Date().toISOString(),
      mappings: result.mappings,
      sensitive_count: result.summary.total,
      status: "Completed",
    };

    await invoke("add_processing_record", {
      workspaceId: ws.id,
      record,
    });

    // 5. 更新 store 并切换到对比视图
    store.setCurrentRecordId(record.id);
    store.setCurrentResult(result);
    await store.refreshActiveWorkspace();
    store.setCenterView("comparison");
    store.setProcessingStep("done");
  } catch (err) {
    const message =
      typeof err === "string" ? err :
      err instanceof Error ? err.message : i18n.t("hook.processFailed");
    toast.error(message);
    store.setProcessingStep("idle");
    store.setCenterView("dropzone");
  }
}

// E2E 测试支持：DEV 模式或 E2E 标志下，将 processFileStandalone 暴露到 window
if ((window as any).__DIMKEY_E2E__ || import.meta.env.DEV) {
  (window as any).__DIMKEY_PROCESS_FILE__ = processFileStandalone;
}
