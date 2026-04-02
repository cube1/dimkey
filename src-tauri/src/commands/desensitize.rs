use std::collections::HashMap;
use crate::models::sensitive::{
    FileContent, SensitiveItem, SensitiveType, DesensitizeResult, DesensitizeSummary,
    ColumnRule, Codebook, CodebookColumn,
};
use crate::models::task::{MappingEntry, StrategyType};
use crate::models::strategy::{Strategy, StrategyConfig};
use crate::models::workspace::{ConsistencyMapping, WorkspaceMode};
use crate::desensitizer::{mask, replace, generalize};
use crate::desensitizer::replace::ReplaceState;
use crate::commands::workspace::{
    get_workspaces_dir, read_workspace_data, write_workspace_data, chrono_now,
};
use crate::commands::language::AppLanguage;
use crate::models::language::Language;

/// 将字符串键名转回 SensitiveType 枚举
pub fn string_to_sensitive_type(key: &str) -> SensitiveType {
    match key {
        "Phone" => SensitiveType::Phone,
        "IdCard" => SensitiveType::IdCard,
        "BankCard" => SensitiveType::BankCard,
        "Email" => SensitiveType::Email,
        "IpAddress" => SensitiveType::IpAddress,
        "LandlinePhone" => SensitiveType::LandlinePhone,
        "LicensePlate" => SensitiveType::LicensePlate,
        "CreditCode" => SensitiveType::CreditCode,
        "Ssn" => SensitiveType::Ssn,
        "CreditCard" => SensitiveType::CreditCard,
        "UsPhone" => SensitiveType::UsPhone,
        "UkPhone" => SensitiveType::UkPhone,
        "Passport" => SensitiveType::Passport,
        "Iban" => SensitiveType::Iban,
        "ZipCode" => SensitiveType::ZipCode,
        "UkPostcode" => SensitiveType::UkPostcode,
        "DriversLicense" => SensitiveType::DriversLicense,
        "PersonName" => SensitiveType::PersonName,
        "OrgName" => SensitiveType::OrgName,
        "Address" => SensitiveType::Address,
        "Title" => SensitiveType::Title,
        other => {
            let custom_text = other.strip_prefix("Custom:").unwrap_or(other);
            SensitiveType::Custom(custom_text.to_string())
        }
    }
}

/// 执行脱敏处理
#[tauri::command]
pub async fn apply_desensitize(
    content: FileContent,
    items: Vec<SensitiveItem>,
    strategies: Vec<StrategyConfig>,
    workspace_id: Option<String>,
    app_handle: tauri::AppHandle,
    language_state: tauri::State<'_, AppLanguage>,
) -> Result<DesensitizeResult, String> {
    let lang = *language_state.0.read().map_err(|e| format!("语言状态锁失败: {}", e))?;

    if items.is_empty() {
        return Err("没有需要脱敏的敏感项".to_string());
    }

    // 1. 构建策略查找表：SensitiveType -> (Strategy, consistent)
    let strategy_map: HashMap<SensitiveType, (Strategy, bool)> = strategies
        .into_iter()
        .map(|s| (s.sensitive_type, (s.strategy, s.consistent)))
        .collect();

    // 2. 一致性替换映射：(原文, 敏感类型) → 脱敏结果
    // 当 consistent=true 时复用已有结果，否则每次独立生成
    let mut consistency_map: HashMap<(String, SensitiveType), (String, StrategyType)> = HashMap::new();

    // 2a. 若有 workspace_id，从工作区加载已有的一致性映射和替换状态
    let (ws_path, replace_seed, replace_counters, ws_mode, ws_dict_entries, member_to_group) = if let Some(ref ws_id) = workspace_id {
        let dir = get_workspaces_dir(&app_handle)?;
        let path = dir.join(format!("{}.json", ws_id));
        if path.exists() {
            let ws_data = read_workspace_data(&path)?;
            for m in &ws_data.workspace.consistency_mappings {
                let st = string_to_sensitive_type(&m.sensitive_type_key);
                let key = (m.original_text.clone(), st);
                consistency_map.insert(key, (m.replaced_text.clone(), m.strategy.clone()));
            }
            // 构建别名组反查索引：(member_text, sensitive_type_key) → (group_id, primary)
            let mut m2g: HashMap<(String, String), (String, String)> = HashMap::new();
            for group in &ws_data.workspace.alias_groups {
                for member in &group.members {
                    m2g.insert(
                        (member.clone(), group.sensitive_type_key.clone()),
                        (group.id.clone(), group.primary.clone()),
                    );
                }
            }
            let seed = ws_data.workspace.replace_seed;
            let counters = ws_data.workspace.replace_counters.clone();
            let mode = ws_data.workspace.mode.clone();
            let dict_entries = ws_data.workspace.dict_entries.clone();
            (Some(path), seed, counters, mode, dict_entries, m2g)
        } else {
            (None, 0, HashMap::new(), WorkspaceMode::Desensitize, Vec::new(), HashMap::new())
        }
    } else {
        (None, 0, HashMap::new(), WorkspaceMode::Desensitize, Vec::new(), HashMap::new())
    };
    let mut replace_state = ReplaceState::new(replace_seed, replace_counters);

    let consistency_map_before_count = consistency_map.len();

    match ws_mode {
        WorkspaceMode::TemplateReplace => {
            // 模版替换模式：用字典的 replacement 值填充一致性映射
            let dict_replacement_map: HashMap<String, String> = ws_dict_entries
                .iter()
                .filter_map(|e| {
                    e.replacement.as_ref().map(|r| (e.text.clone(), r.clone()))
                })
                .collect();

            for item in &items {
                let key = (item.text.clone(), item.sensitive_type.clone());
                if let Some(replacement) = dict_replacement_map.get(&item.text) {
                    consistency_map.insert(key, (replacement.clone(), StrategyType::Replace));
                }
                // 无替换值 → 不插入，后续替换时跳过
            }
        }
        WorkspaceMode::Desensitize => {
            // 脱敏模式：走标准策略生成
            for item in &items {
                let key = (item.text.clone(), item.sensitive_type.clone());
                let (strategy, consistent) = strategy_map
                    .get(&item.sensitive_type)
                    .cloned()
                    .unwrap_or((Strategy::Mask {
                        keep_prefix: 1,
                        keep_suffix: 1,
                    }, true));

                // 一致性模式下，已有映射则跳过
                if consistent {
                    if consistency_map.contains_key(&key) {
                        continue;
                    }
                    // 检查是否属于某个别名组
                    let type_key = sensitive_type_to_key(&item.sensitive_type);
                    if let Some((_group_id, primary)) = member_to_group.get(&(item.text.clone(), type_key)) {
                        let primary_key = (primary.clone(), item.sensitive_type.clone());
                        if let Some((replaced, st_type)) = consistency_map.get(&primary_key) {
                            // 复用主名的替换值
                            consistency_map.insert(key, (replaced.clone(), st_type.clone()));
                            continue;
                        }
                        // 主名也没有映射 → 后面正常生成，生成后也写入主名映射
                    }
                }

                let (replaced, st_type) = match &strategy {
                    Strategy::Mask {
                        keep_prefix,
                        keep_suffix,
                    } => {
                        let r = mask::apply_mask(
                            &item.text,
                            &item.sensitive_type,
                            *keep_prefix,
                            *keep_suffix,
                        );
                        (r, StrategyType::Mask)
                    }
                    Strategy::Replace { ref style } => {
                        let r = match lang {
                            Language::En => replace::apply_replace_en(&item.text, &item.sensitive_type, &mut replace_state, style),
                            Language::Zh => replace::apply_replace(&item.text, &item.sensitive_type, &mut replace_state, style),
                        };
                        (r, StrategyType::Replace)
                    }
                    Strategy::Generalize => {
                        let r = generalize::apply_generalize_for_language(&item.text, &item.sensitive_type, lang);
                        (r, StrategyType::Generalize)
                    }
                };

                consistency_map.insert(key.clone(), (replaced, st_type));

                // 如果此项属于别名组，同时为主名写入映射
                let type_key_for_group = sensitive_type_to_key(&item.sensitive_type);
                if let Some((_group_id, primary)) = member_to_group.get(&(item.text.clone(), type_key_for_group)) {
                    let primary_key = (primary.clone(), item.sensitive_type.clone());
                    if !consistency_map.contains_key(&primary_key) {
                        if let Some((replaced, st_type)) = consistency_map.get(&key) {
                            consistency_map.insert(primary_key, (replaced.clone(), st_type.clone()));
                        }
                    }
                }
            }
        }
    }

    // 3. 克隆内容并替换文本
    let mut new_content = content.clone();

    match &mut new_content {
        FileContent::Spreadsheet {
            sheets, ..
        } => {
            // 按 (sheet_index, row, col) 分组
            let mut cell_items: HashMap<(usize, usize, usize), Vec<&SensitiveItem>> = HashMap::new();
            for item in &items {
                cell_items
                    .entry((item.sheet_index, item.row, item.col))
                    .or_default()
                    .push(item);
            }

            for ((sheet_idx, row, col), ref cell_items) in &cell_items {
                if let Some(sheet) = sheets.get_mut(*sheet_idx) {
                    if *row == 0 {
                        // 表头仍为 String
                        if let Some(header) = sheet.headers.get_mut(*col) {
                            *header = replace_in_text_v2(header, cell_items, &consistency_map);
                        }
                    } else {
                        // 数据行为 CellValue
                        if let Some(cell_value) = sheet.rows.get_mut(row - 1).and_then(|r| r.get_mut(*col)) {
                            cell_value.text = replace_in_text_v2(&cell_value.text, cell_items, &consistency_map);
                            cell_value.cell_type = crate::models::sensitive::CellType::Text;
                        }
                    }
                }
            }
        }
        FileContent::Document { paragraphs, .. } => {
            let mut para_items: HashMap<usize, Vec<&SensitiveItem>> = HashMap::new();
            for item in &items {
                para_items.entry(item.row).or_default().push(item);
            }

            for (para_idx, ref p_items) in &para_items {
                if let Some(para) = paragraphs.iter_mut().find(|p| p.index == *para_idx) {
                    para.text = replace_in_text_v2(&para.text, p_items, &consistency_map);
                }
            }
        }
    }

    // 4. 构建去重后的映射记录
    let mut mapping_map: HashMap<(String, SensitiveType), MappingEntry> = HashMap::new();
    for item in &items {
        let key = (item.text.clone(), item.sensitive_type.clone());
        if let Some((replaced, st_type)) = consistency_map.get(&key) {
            let entry = mapping_map
                .entry(key)
                .or_insert(MappingEntry {
                    original_text: item.text.clone(),
                    replaced_text: replaced.clone(),
                    sensitive_type: item.sensitive_type.clone(),
                    strategy: st_type.clone(),
                    occurrences: 0,
                });
            entry.occurrences += 1;
        }
    }
    let mappings: Vec<MappingEntry> = mapping_map.into_values().collect();

    // 5. 构建统计摘要（by_type 用 String key 避免 JSON 序列化问题）
    let mut by_type: HashMap<String, usize> = HashMap::new();
    for item in &items {
        let key = sensitive_type_to_key(&item.sensitive_type);
        *by_type.entry(key).or_default() += 1;
    }
    let total = items.len();
    let summary = DesensitizeSummary { total, by_type };

    // 6. 将新增的一致性映射和替换计数器写回工作区 JSON
    if let Some(ref path) = ws_path {
        if ws_mode == WorkspaceMode::Desensitize {
            // 脱敏模式：写回一致性映射和替换计数器
            let counters_changed = !replace_state.export_counters().is_empty();
            if consistency_map.len() > consistency_map_before_count || counters_changed {
                let mut ws_data = read_workspace_data(path)?;

                // 写回替换计数器
                ws_data.workspace.replace_counters = replace_state.export_counters();

                // 收集新增的一致性映射条目（跳过已存在的）
                if consistency_map.len() > consistency_map_before_count {
                    let existing_keys: std::collections::HashSet<(String, String)> = ws_data
                        .workspace
                        .consistency_mappings
                        .iter()
                        .map(|m| (m.original_text.clone(), m.sensitive_type_key.clone()))
                        .collect();

                    for ((original, st), (replaced, strategy)) in &consistency_map {
                        let st_key = sensitive_type_to_key(st);
                        if !existing_keys.contains(&(original.clone(), st_key.clone())) {
                            // 检查该成员是否属于某个别名组
                            let group_id = member_to_group
                                .get(&(original.clone(), st_key.clone()))
                                .map(|(gid, _)| gid.clone());
                            ws_data.workspace.consistency_mappings.push(ConsistencyMapping {
                                original_text: original.clone(),
                                sensitive_type_key: st_key,
                                replaced_text: replaced.clone(),
                                strategy: strategy.clone(),
                                alias_group_id: group_id,
                            });
                        }
                    }
                }

                ws_data.workspace.updated_at = chrono_now();
                write_workspace_data(path, &ws_data)?;
            }
        }
        // 模版替换模式不写回一致性映射（字典本身就是映射源）
    }

    // 上报脱敏事件：统计使用最多的策略
    let mut strategy_counts: HashMap<String, usize> = HashMap::new();
    for m in &mappings {
        let s = match m.strategy {
            StrategyType::Mask => "mask",
            StrategyType::Replace => "replace",
            StrategyType::Generalize => "generalize",
        };
        *strategy_counts.entry(s.to_string()).or_default() += m.occurrences;
    }
    let top_strategy = strategy_counts.into_iter()
        .max_by_key(|(_, v)| *v)
        .map(|(k, _)| k)
        .unwrap_or_else(|| "mask".to_string());
    crate::analytics::track(&app_handle, "desensitize_applied", Some(serde_json::json!({
        "strategy": top_strategy,
        "cell_count": summary.total,
    })));

    Ok(DesensitizeResult {
        content: new_content,
        mappings,
        summary,
    })
}

/// SensitiveType → 字符串 key（与前端 getSensitiveTypeKey 一致）
pub fn sensitive_type_to_key(st: &SensitiveType) -> String {
    match st {
        SensitiveType::Phone => "Phone".to_string(),
        SensitiveType::IdCard => "IdCard".to_string(),
        SensitiveType::BankCard => "BankCard".to_string(),
        SensitiveType::Email => "Email".to_string(),
        SensitiveType::IpAddress => "IpAddress".to_string(),
        SensitiveType::LandlinePhone => "LandlinePhone".to_string(),
        SensitiveType::LicensePlate => "LicensePlate".to_string(),
        SensitiveType::CreditCode => "CreditCode".to_string(),
        SensitiveType::Ssn => "Ssn".to_string(),
        SensitiveType::CreditCard => "CreditCard".to_string(),
        SensitiveType::UsPhone => "UsPhone".to_string(),
        SensitiveType::UkPhone => "UkPhone".to_string(),
        SensitiveType::Passport => "Passport".to_string(),
        SensitiveType::Iban => "Iban".to_string(),
        SensitiveType::ZipCode => "ZipCode".to_string(),
        SensitiveType::UkPostcode => "UkPostcode".to_string(),
        SensitiveType::DriversLicense => "DriversLicense".to_string(),
        SensitiveType::PersonName => "PersonName".to_string(),
        SensitiveType::OrgName => "OrgName".to_string(),
        SensitiveType::Address => "Address".to_string(),
        SensitiveType::Title => "Title".to_string(),
        SensitiveType::Custom(s) => format!("Custom:{}", s),
    }
}

/// 在文本中替换敏感项（从后往前替换，避免偏移问题）
fn replace_in_text_v2(
    text: &str,
    items: &[&SensitiveItem],
    consistency_map: &HashMap<(String, SensitiveType), (String, StrategyType)>,
) -> String {
    // 去除重叠项：按 start 升序，跳过与前一项重叠的
    let mut sorted: Vec<&SensitiveItem> = items.to_vec();
    sorted.sort_by_key(|i| i.start);

    let mut non_overlapping: Vec<&SensitiveItem> = Vec::new();
    let mut last_end = 0usize;
    for item in &sorted {
        if item.start >= last_end {
            non_overlapping.push(item);
            last_end = item.end;
        }
    }

    // 按 start 降序，从后往前替换
    non_overlapping.sort_by(|a, b| b.start.cmp(&a.start));

    let mut chars: Vec<char> = text.chars().collect();

    for item in non_overlapping {
        let key = (item.text.clone(), item.sensitive_type.clone());
        if let Some((replacement, _)) = consistency_map.get(&key) {
            let end = item.end.min(chars.len());
            let start = item.start.min(end);
            let replacement_chars: Vec<char> = replacement.chars().collect();
            chars.splice(start..end, replacement_chars);
        }
    }

    chars.into_iter().collect()
}

/// 按列批量脱敏（列级模式专用）
#[tauri::command]
pub async fn apply_desensitize_by_columns(
    content: FileContent,
    column_rules: Vec<ColumnRule>,
    workspace_id: String,
    record_id: String,
    app_handle: tauri::AppHandle,
    language_state: tauri::State<'_, AppLanguage>,
) -> Result<DesensitizeResult, String> {
    let lang = *language_state.0.read().map_err(|e| format!("语言状态锁失败: {}", e))?;

    let (sheets, file_name, file_type) = match &content {
        FileContent::Spreadsheet { sheets, file_name, file_type, .. } => {
            (sheets.clone(), file_name.clone(), file_type.clone())
        }
        _ => return Err("列级脱敏仅支持表格类型文件".to_string()),
    };

    if column_rules.is_empty() {
        return Err("请至少选择一列进行脱敏".to_string());
    }

    // 加载工作区状态（一次性读取，避免多次 IO）
    let ws_dir = get_workspaces_dir(&app_handle)?;
    let ws_json_path = ws_dir.join(format!("{}.json", workspace_id));
    let mut global_consistency: HashMap<(String, SensitiveType), String> = HashMap::new();
    let mut member_to_group: HashMap<(String, String), (String, String)> = HashMap::new();
    let (replace_seed, replace_counters) = if ws_json_path.exists() {
        let ws_data = read_workspace_data(&ws_json_path)?;
        // 加载一致性映射
        for m in &ws_data.workspace.consistency_mappings {
            let st = string_to_sensitive_type(&m.sensitive_type_key);
            global_consistency.insert((m.original_text.clone(), st), m.replaced_text.clone());
        }
        // 构建别名组反查索引：(member_text, sensitive_type_key) → (group_id, primary)
        for group in &ws_data.workspace.alias_groups {
            for member in &group.members {
                member_to_group.insert(
                    (member.clone(), group.sensitive_type_key.clone()),
                    (group.id.clone(), group.primary.clone()),
                );
            }
        }
        (ws_data.workspace.replace_seed, ws_data.workspace.replace_counters.clone())
    } else {
        (0, HashMap::new())
    };
    let mut replace_state = ReplaceState::new(replace_seed, replace_counters);

    let mut new_sheets = sheets.clone();
    let mut all_mappings: Vec<MappingEntry> = Vec::new();
    let mut by_type: HashMap<String, usize> = HashMap::new();
    let mut total_count: usize = 0;
    let mut codebook_columns: HashMap<String, CodebookColumn> = HashMap::new();

    for rule in &column_rules {
        let col = rule.col;
        let sheet_idx = rule.sheet_index;
        let st = string_to_sensitive_type(&rule.sensitive_type);
        let st_key = sensitive_type_to_key(&st);

        let sheet = match new_sheets.get_mut(sheet_idx) {
            Some(s) => s,
            None => continue,
        };
        let orig_sheet = &sheets[sheet_idx];

        // 列内去重映射：原文 → 脱敏后
        let mut unique_map: HashMap<String, String> = HashMap::new();
        let mut col_count: usize = 0;

        for row_idx in 0..sheet.rows.len() {
            let cell_text = match sheet.rows[row_idx].get(col) {
                Some(cv) if !cv.text.is_empty() => cv.text.clone(),
                _ => continue,
            };

            let replaced = if let Some(existing) = unique_map.get(&cell_text) {
                existing.clone()
            } else if let Some(existing) = global_consistency.get(&(cell_text.clone(), st.clone())) {
                // 跨列/跨 sheet 一致性：复用已有映射
                let r = existing.clone();
                unique_map.insert(cell_text.clone(), r.clone());
                r
            } else if let Some((_gid, primary)) = member_to_group.get(&(cell_text.clone(), st_key.clone())) {
                if let Some(existing) = global_consistency.get(&(primary.clone(), st.clone())) {
                    let r = existing.clone();
                    unique_map.insert(cell_text.clone(), r.clone());
                    global_consistency.insert((cell_text.clone(), st.clone()), r.clone());
                    r
                } else {
                    // 主名也没有映射，走正常生成
                    let result = match &rule.strategy {
                        Strategy::Mask { keep_prefix, keep_suffix } => {
                            mask::apply_mask(&cell_text, &st, *keep_prefix, *keep_suffix)
                        }
                        Strategy::Replace { ref style } => {
                            match lang {
                                Language::En => replace::apply_replace_en(&cell_text, &st, &mut replace_state, style),
                                Language::Zh => replace::apply_replace(&cell_text, &st, &mut replace_state, style),
                            }
                        }
                        Strategy::Generalize => {
                            generalize::apply_generalize_for_language(&cell_text, &st, lang)
                        }
                    };
                    unique_map.insert(cell_text.clone(), result.clone());
                    global_consistency.insert((cell_text.clone(), st.clone()), result.clone());
                    global_consistency.insert((primary.clone(), st.clone()), result.clone());
                    result
                }
            } else {
                let result = match &rule.strategy {
                    Strategy::Mask { keep_prefix, keep_suffix } => {
                        mask::apply_mask(&cell_text, &st, *keep_prefix, *keep_suffix)
                    }
                    Strategy::Replace { ref style } => {
                        match lang {
                            Language::En => replace::apply_replace_en(&cell_text, &st, &mut replace_state, style),
                            Language::Zh => replace::apply_replace(&cell_text, &st, &mut replace_state, style),
                        }
                    }
                    Strategy::Generalize => {
                        generalize::apply_generalize_for_language(&cell_text, &st, lang)
                    }
                };
                unique_map.insert(cell_text.clone(), result.clone());
                global_consistency.insert((cell_text.clone(), st.clone()), result.clone());
                result
            };

            if replaced != cell_text {
                sheet.rows[row_idx][col].text = replaced;
                sheet.rows[row_idx][col].cell_type = crate::models::sensitive::CellType::Text;
                col_count += 1;
            }
        }

        if col_count > 0 {
            total_count += col_count;
            *by_type.entry(st_key.clone()).or_default() += col_count;

            // 构建映射记录
            let strategy_type = match &rule.strategy {
                Strategy::Mask { .. } => StrategyType::Mask,
                Strategy::Replace { .. } => StrategyType::Replace,
                Strategy::Generalize => StrategyType::Generalize,
            };

            for (original, replaced) in &unique_map {
                if original != replaced {
                    all_mappings.push(MappingEntry {
                        original_text: original.clone(),
                        replaced_text: replaced.clone(),
                        sensitive_type: st.clone(),
                        strategy: strategy_type.clone(),
                        occurrences: orig_sheet.rows.iter().filter(|r| r.get(col).map_or(false, |cv| cv.text == *original)).count(),
                    });
                }
            }

            // 可还原列写入码本
            let is_mask = matches!(&rule.strategy, Strategy::Mask { .. });
            if rule.reversible && !is_mask {
                let header_name = sheet.headers.get(col).cloned().unwrap_or_else(|| format!("列{}", col));
                let strategy_name = match &rule.strategy {
                    Strategy::Mask { .. } => "Mask".to_string(),
                    Strategy::Replace { .. } => "Replace".to_string(),
                    Strategy::Generalize => "Generalize".to_string(),
                };
                let mappings: HashMap<String, String> = unique_map
                    .iter()
                    .filter(|(k, v)| k != v)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                if !mappings.is_empty() {
                    codebook_columns.insert(header_name, CodebookColumn {
                        sensitive_type: st_key.clone(),
                        strategy: strategy_name,
                        mappings,
                    });
                }
            }
        }
    }

    // 写入码本文件
    if !codebook_columns.is_empty() {
        let codebook = Codebook {
            version: 1,
            columns: codebook_columns,
        };
        let ws_codebook_dir = ws_dir.join(&workspace_id);
        std::fs::create_dir_all(&ws_codebook_dir)
            .map_err(|e| format!("创建工作区目录失败: {}", e))?;
        let codebook_path = ws_codebook_dir.join(format!("{}.codebook.json", record_id));
        let codebook_json = serde_json::to_string_pretty(&codebook)
            .map_err(|e| format!("序列化码本失败: {}", e))?;
        std::fs::write(&codebook_path, codebook_json)
            .map_err(|e| format!("写入码本文件失败: {}", e))?;
    }

    // 写回替换计数器和新增一致性映射到工作区
    if ws_json_path.exists() {
        let counters = replace_state.export_counters();
        let has_new_counters = !counters.is_empty();
        let has_new_mappings = !global_consistency.is_empty();

        if has_new_counters || has_new_mappings {
            let mut ws_data = read_workspace_data(&ws_json_path)?;

            if has_new_counters {
                ws_data.workspace.replace_counters = counters;
            }

            // 将列级脱敏产生的新一致性映射写回工作区
            if has_new_mappings {
                let existing_keys: std::collections::HashSet<(String, String)> = ws_data
                    .workspace
                    .consistency_mappings
                    .iter()
                    .map(|m| (m.original_text.clone(), m.sensitive_type_key.clone()))
                    .collect();

                for ((original, st), replaced) in &global_consistency {
                    let st_key = sensitive_type_to_key(st);
                    if !existing_keys.contains(&(original.clone(), st_key.clone())) {
                        let strategy_type = column_rules.iter()
                            .find(|r| sensitive_type_to_key(&string_to_sensitive_type(&r.sensitive_type)) == st_key)
                            .map(|r| match &r.strategy {
                                Strategy::Mask { .. } => StrategyType::Mask,
                                Strategy::Replace { .. } => StrategyType::Replace,
                                Strategy::Generalize => StrategyType::Generalize,
                            })
                            .unwrap_or(StrategyType::Mask);
                        let group_id = member_to_group
                            .get(&(original.clone(), st_key.clone()))
                            .map(|(gid, _)| gid.clone());
                        ws_data.workspace.consistency_mappings.push(ConsistencyMapping {
                            original_text: original.clone(),
                            sensitive_type_key: st_key,
                            replaced_text: replaced.clone(),
                            strategy: strategy_type,
                            alias_group_id: group_id,
                        });
                    }
                }
            }

            ws_data.workspace.updated_at = chrono_now();
            write_workspace_data(&ws_json_path, &ws_data)?;
        }
    }

    let new_content = FileContent::Spreadsheet {
        file_name,
        file_type,
        sheets: new_sheets,
    };

    let summary = DesensitizeSummary { total: total_count, by_type };

    // 上报列级脱敏事件
    let mut strategy_counts: HashMap<String, usize> = HashMap::new();
    for rule in &column_rules {
        let s = match &rule.strategy {
            Strategy::Mask { .. } => "mask",
            Strategy::Replace { .. } => "replace",
            Strategy::Generalize => "generalize",
        };
        *strategy_counts.entry(s.to_string()).or_default() += 1;
    }
    let top_strategy = strategy_counts.into_iter()
        .max_by_key(|(_, v)| *v)
        .map(|(k, _)| k)
        .unwrap_or_else(|| "mask".to_string());
    crate::analytics::track(&app_handle, "desensitize_applied", Some(serde_json::json!({
        "strategy": top_strategy,
        "cell_count": total_count,
    })));

    Ok(DesensitizeResult {
        content: new_content,
        mappings: all_mappings,
        summary,
    })
}
