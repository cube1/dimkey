import i18n from "../i18n";

/** @deprecated v0.1 遗留类型，v0.2 使用 CenterView 替代 */
export type ViewType = "home" | "preview" | "result" | "history" | "restore";

// ============================================================
// 敏感信息相关
// ============================================================

/** 敏感信息类型（对齐 Rust SensitiveType 枚举） */
export type SensitiveType =
  // 通用
  | "Email"
  | "IpAddress"
  // 中文特有
  | "Phone"
  | "IdCard"
  | "BankCard"
  | "LandlinePhone"
  | "LicensePlate"
  | "CreditCode"
  // 英文特有
  | "Ssn"
  | "CreditCard"
  | "UsPhone"
  | "UkPhone"
  | "Passport"
  | "Iban"
  | "ZipCode"
  | "UkPostcode"
  | "DriversLicense"
  // NER
  | "PersonName"
  | "OrgName"
  | "Address"
  | "Title"
  // 自定义
  | { Custom: string };

/** 识别来源 */
export type DetectSource = "Regex" | "Ner" | "Dict" | "Manual";

/** PDF 手动框选涂黑区域（归一化屏幕坐标 0~1） */
export interface PdfBbox {
  page_index: number;
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** 单条敏感信息 */
export interface SensitiveItem {
  id: string;
  text: string;
  sensitive_type: SensitiveType;
  source: DetectSource;
  confidence: number;
  start: number;
  end: number;
  row: number;
  col: number;
  sheet_index: number;
  pdf_bbox?: PdfBbox;
}

// ============================================================
// 文件内容相关
// ============================================================

/** 文件类型 */
export type FileType = "Xlsx" | "Xls" | "Csv" | "Docx" | "Txt" | "Pdf";

/** 段落在表格中的位置信息 */
export interface TablePosition {
  table_index: number;
  row: number;
  col: number;
  col_count: number;
}

/** 段落（Word 文档） */
export interface Paragraph {
  index: number;
  text: string;
  style: string;
  table_position?: TablePosition;
}

/** 单元格类型（对齐 Rust CellType 枚举） */
export type CellType =
  | "Text"
  | "Integer"
  | "Float"
  | "Boolean"
  | { DateTime: { serial: number } }
  | "DateTimeIso"
  | "DurationIso"
  | "Empty";

/** 单元格值（文本 + 原始类型） */
export interface CellValue {
  text: string;
  cell_type: CellType;
}

/** 单个 Sheet 的数据 */
export interface SheetData {
  name: string;
  headers: string[];
  rows: CellValue[][];
  row_count: number;
  col_count: number;
}

/** 解析后的文件内容（tagged union，对齐 Rust FileContent 枚举） */
export type FileContent =
  | {
      type: "Spreadsheet";
      file_name: string;
      file_type: FileType;
      sheets: SheetData[];
    }
  | {
      type: "Document";
      file_name: string;
      file_type: FileType;
      paragraphs: Paragraph[];
      encoding?: string;
    };

// ============================================================
// 脱敏策略相关
// ============================================================

/** 替换风格（对齐 Rust ReplaceStyle 枚举） */
export type ReplaceStyle = "Fake" | "Mou" | "Ordinal";

/** 替换风格标签（通过 Proxy 动态获取 i18n 标签） */
export const REPLACE_STYLE_LABELS: Record<ReplaceStyle, string> = new Proxy(
  {} as Record<ReplaceStyle, string>,
  {
    get: (_target, key: string) => getReplaceStyleLabel(key as ReplaceStyle),
    ownKeys: () => ["Fake", "Mou", "Ordinal"],
    getOwnPropertyDescriptor: () => ({ configurable: true, enumerable: true }),
  }
);

/** 脱敏策略（对齐 Rust Strategy 枚举） */
export type Strategy =
  | { Mask: { keep_prefix: number; keep_suffix: number } }
  | { Replace: { style: ReplaceStyle } }
  | "Generalize";

/** 策略类型（轻量版，用于映射记录） */
export type StrategyType = "Mask" | "Replace" | "Generalize";

/** 策略配置（单类型，传给 Rust apply_desensitize） */
export interface StrategyConfig {
  sensitive_type: SensitiveType;
  strategy: Strategy;
  consistent: boolean;
}

/** 全局配置 */
export interface AppConfig {
  strategies: Record<string, Strategy>;
}

/** 自定义词典条目 */
export interface DictEntry {
  text: string;
  sensitive_type: SensitiveType;
  match_mode: "Exact" | "Fuzzy";
  replacement?: string;  // 模版替换时的替换值
  language?: string;     // 词条所属语言（不填则所有语言生效）
  builtin?: boolean;     // 内置词条（只读，不可删除）
}

/** 白名单排除条目 */
export interface WhitelistEntry {
  text: string;
  match_mode: "Exact" | "Fuzzy";
}

// ============================================================
// 脱敏结果相关
// ============================================================

/** 脱敏汇总 */
export interface DesensitizeSummary {
  total: number;
  by_type: Record<string, number>;
}

/** 映射关系条目 */
export interface MappingEntry {
  original_text: string;
  replaced_text: string;
  sensitive_type: SensitiveType;
  strategy: StrategyType;
  occurrences: number;
}

/** 脱敏执行结果 */
export interface DesensitizeResult {
  content: FileContent;
  mappings: MappingEntry[];
  summary: DesensitizeSummary;
}

/** 列类型推断结果（对齐 Rust ColumnInference） */
export interface ColumnInference {
  col: number;
  header: string;
  inferred_type: SensitiveType | null;
  confidence: number;
  sample_hits: number;
  sample_total: number;
  sheet_index: number;
}

/** 列级脱敏规则（对齐 Rust ColumnRule） */
export interface ColumnRule {
  col: number;
  sensitive_type: string;
  strategy: Strategy;
  reversible: boolean;
  sheet_index: number;
}

/** 列状态 */
export type ColumnStatus = "undetected" | "inferred" | "confirmed";

/** 还原位置信息 */
export interface RestoreItem {
  row: number;
  col: number;
  start: number;
  end: number;
  text: string;
  replaced_text: string;
  sensitive_type: SensitiveType;
  sheet_index: number;
}

/** 还原结果 */
export interface RestoreResult {
  original_content: FileContent;
  restored_content: FileContent;
  matched_count: number;
  restore_items: RestoreItem[];
  original_items: RestoreItem[];
  file_path: string;
}

// ============================================================
// 历史任务相关
// ============================================================

/** 脱敏任务记录 */
export interface TaskRecord {
  id: string;
  original_file_name: string;
  file_type: FileType;
  created_at: string;
  sensitive_count: number;
  replaced_count: number;
  mappings: MappingEntry[];
}

// ============================================================
// 工作区相关（v0.2）
// ============================================================

/** 工作区来源类型 */
export type WorkspaceSource = "File" | "Clipboard";

/** 工作区模式 */
export type WorkspaceMode = "Desensitize" | "TemplateReplace";

/** 处理状态 */
export type ProcessingStatus = "Completed" | "Restored";

/** 跨文件一致性替换映射条目 */
export interface ConsistencyMapping {
  original_text: string;
  sensitive_type_key: string;
  replaced_text: string;
  strategy: StrategyType;
  alias_group_id?: string;
}

/** 别名组：将同一实体的多个名称关联 */
export interface AliasGroup {
  id: string;
  primary: string;
  members: string[];
  sensitive_type_key: string;
  created_at: string;
}

/** 工作区 */
export interface Workspace {
  id: string;
  name: string;
  source?: WorkspaceSource;
  created_at: string;
  updated_at: string;
  strategies: Record<string, Strategy>;
  replace_style?: ReplaceStyle;
  dict_entries: DictEntry[];
  column_rules: Record<string, string>;
  output_dir: string | null;
  consistency_mappings: ConsistencyMapping[];
  enabled_types: string[];
  mode?: WorkspaceMode;  // 工作区模式，默认 Desensitize
  whitelist?: WhitelistEntry[];  // 白名单排除列表
  alias_groups?: AliasGroup[];  // 别名组列表
}

/** 处理记录 */
export interface ProcessingRecord {
  id: string;
  file_name: string;
  file_path: string;
  file_type: FileType;
  processed_at: string;
  mappings: MappingEntry[];
  sensitive_count: number;
  status: ProcessingStatus;
  codebook_path?: string;
}

/** 工作区完整数据（含历史记录） */
export interface WorkspaceData {
  workspace: Workspace;
  history: ProcessingRecord[];
}

/** 工作区列表项（轻量结构） */
export interface WorkspaceListItem {
  id: string;
  name: string;
  updated_at: string;
  history_count: number;
  source?: WorkspaceSource;
}

/** 中栏视图类型 */
export type CenterView = "empty" | "dropzone" | "processing" | "comparison" | "restore";

/** 自动脱敏处理步骤 */
export type AutoDesensitizeStep = "idle" | "parsing" | "detecting" | "desensitizing" | "saving" | "done";

// ============================================================
// 敏感类型配置（颜色、显示名）
// ============================================================

export interface SensitiveTypeInfo {
  label: string;
  bgClass: string;
  textClass: string;
}

/** 各敏感类型的颜色配置（静态） */
export const SENSITIVE_TYPE_COLORS: Record<string, { bgClass: string; textClass: string }> = {
  Phone:         { bgClass: "bg-blue-50",    textClass: "text-blue-700" },
  IdCard:        { bgClass: "bg-red-50",     textClass: "text-red-700" },
  BankCard:      { bgClass: "bg-orange-50",  textClass: "text-orange-700" },
  Email:         { bgClass: "bg-purple-50",  textClass: "text-purple-700" },
  IpAddress:     { bgClass: "bg-slate-100",  textClass: "text-slate-700" },
  LandlinePhone: { bgClass: "bg-cyan-50",    textClass: "text-cyan-700" },
  LicensePlate:  { bgClass: "bg-yellow-50",  textClass: "text-yellow-700" },
  CreditCode:    { bgClass: "bg-pink-50",    textClass: "text-pink-700" },
  PersonName:    { bgClass: "bg-green-50",   textClass: "text-green-700" },
  OrgName:       { bgClass: "bg-indigo-50",  textClass: "text-indigo-700" },
  Address:       { bgClass: "bg-amber-50",   textClass: "text-amber-700" },
  Title:         { bgClass: "bg-lime-50",    textClass: "text-lime-700" },
  Custom:        { bgClass: "bg-slate-50",   textClass: "text-slate-700" },
  // 英文类型
  Ssn:            { bgClass: "bg-red-50",     textClass: "text-red-700" },
  CreditCard:     { bgClass: "bg-orange-50",  textClass: "text-orange-700" },
  UsPhone:        { bgClass: "bg-blue-50",    textClass: "text-blue-700" },
  UkPhone:        { bgClass: "bg-cyan-50",    textClass: "text-cyan-700" },
  Passport:       { bgClass: "bg-pink-50",    textClass: "text-pink-700" },
  Iban:           { bgClass: "bg-indigo-50",  textClass: "text-indigo-700" },
  ZipCode:        { bgClass: "bg-amber-50",   textClass: "text-amber-700" },
  UkPostcode:     { bgClass: "bg-yellow-50",  textClass: "text-yellow-700" },
  DriversLicense: { bgClass: "bg-lime-50",    textClass: "text-lime-700" },
};

/** 获取敏感类型的显示配置（label 从 i18n 获取） */
export function getSensitiveTypeConfig(key: string): SensitiveTypeInfo {
  const colors = SENSITIVE_TYPE_COLORS[key] ?? SENSITIVE_TYPE_COLORS.Custom;
  const label = i18n.t(`sensitiveType.${key}`, { defaultValue: key });
  return { label, ...colors };
}

/** 获取策略标签 */
export function getStrategyLabel(type: StrategyType): string {
  return i18n.t(`strategy.${type}`);
}

/** 获取替换风格标签 */
export function getReplaceStyleLabel(style: ReplaceStyle): string {
  return i18n.t(`replaceStyle.${style}`);
}

/** 各敏感类型的显示配置（通过 Proxy 动态获取 i18n 标签） */
export const SENSITIVE_TYPE_CONFIG: Record<string, SensitiveTypeInfo> = new Proxy(
  {} as Record<string, SensitiveTypeInfo>,
  {
    get: (_target, key: string) => getSensitiveTypeConfig(key),
    ownKeys: () => Object.keys(SENSITIVE_TYPE_COLORS),
    getOwnPropertyDescriptor: () => ({ configurable: true, enumerable: true }),
  }
);

/** 策略类型标签（通过 Proxy 动态获取 i18n 标签） */
export const STRATEGY_LABELS: Record<StrategyType, string> = new Proxy(
  {} as Record<StrategyType, string>,
  {
    get: (_target, key: string) => getStrategyLabel(key as StrategyType),
    ownKeys: () => ["Mask", "Replace", "Generalize"],
    getOwnPropertyDescriptor: () => ({ configurable: true, enumerable: true }),
  }
);

/** 从 Strategy 联合类型中提取策略类型标识 */
export function getStrategyType(strategy: Strategy): StrategyType {
  if (typeof strategy === "string") return strategy as StrategyType;
  if ("Mask" in strategy) return "Mask";
  if ("Replace" in strategy) return "Replace";
  return "Mask";
}

/** 创建 Replace 策略对象 */
export function createReplaceStrategy(style: ReplaceStyle = "Fake"): Strategy {
  return { Replace: { style } };
}

/** 从 Strategy 中提取 ReplaceStyle（非 Replace 策略返回 null） */
export function getReplaceStyle(strategy: Strategy): ReplaceStyle | null {
  if (typeof strategy === "object" && "Replace" in strategy) {
    return strategy.Replace.style;
  }
  return null;
}

/** 密码弹窗状态 */
export interface PasswordModalState {
  visible: boolean;
  filePath: string;
  fileType: string;
  attemptsLeft: number;
  errorMessage: string | null;
}

/** 解析错误是否为加密文件标记 */
export function parseEncryptedError(err: unknown): string | null {
  if (typeof err === "string" && err.startsWith("ENCRYPTED:")) {
    return err.split(":")[1];
  }
  return null;
}

/** 判断是否为密码错误 */
export function isWrongPasswordError(err: unknown): boolean {
  return typeof err === "string" && err === "WRONG_PASSWORD";
}

/** 批量导入队列中的单个文件 */
export interface QueueFile {
  id: string;
  filePath: string;
  fileName: string;
  status: "pending" | "processing" | "confirmed" | "failed";
  errorMessage?: string;
}

/** 批量导入最大文件数 */
export const MAX_QUEUE_SIZE = 20;

/** 根据敏感类型获取可用策略列表 */
export function getAllowedStrategies(typeKey: string): StrategyType[] {
  switch (typeKey) {
    case "PersonName":
    case "OrgName":
    case "Title":
      return ["Replace", "Mask"];
    case "Address":
      return ["Mask", "Replace", "Generalize"];
    default:
      return ["Mask", "Replace"];
  }
}

/** 获取敏感类型的显示键名 */
export function getSensitiveTypeKey(st: SensitiveType): string {
  if (typeof st === "string") return st;
  return "Custom";
}

/** 获取敏感类型的显示信息 */
export function getSensitiveTypeInfo(st: SensitiveType): SensitiveTypeInfo {
  const key = getSensitiveTypeKey(st);
  return SENSITIVE_TYPE_CONFIG[key] ?? SENSITIVE_TYPE_CONFIG.Custom;
}
