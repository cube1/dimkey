pub mod commands;
pub mod engine;
pub mod parser;
pub mod desensitizer;
pub mod models;
pub mod analytics;

use std::sync::{Arc, Mutex};
use commands::file::{import_file, export_file, check_file_exists, import_file_with_password, copy_file_to_clipboard, import_clipboard_text, export_pdf_redacted_cmd, compute_pdf_redact_preview, render_pdf_pages};
use commands::detect::{detect_by_regex, detect_by_ner, detect_by_dict, detect_columns, get_builtin_dict};
use commands::desensitize::{apply_desensitize, apply_desensitize_by_columns};
use commands::config::{load_config, save_config, load_dict, save_dict};
use commands::task::{save_task, list_tasks, delete_task, restore_file};
use commands::workspace::{
    create_workspace, create_clipboard_workspace, list_workspaces, get_workspace,
    update_workspace, delete_workspace, rename_workspace, add_processing_record,
    update_processing_record_mappings, delete_processing_record,
    restore_processing, restore_from_workspace, restore_ai_response,
    clear_consistency_mappings, clear_type_consistency_mappings,
};
use commands::alias_group::{
    create_alias_group, add_alias_member, remove_alias_member,
    delete_alias_group, list_alias_groups,
};
use commands::language::{AppLanguage, get_language};
use analytics::{get_analytics_enabled, set_analytics_enabled};
use engine::ner_engine::NerEngine;
use engine::backends::onnx_token_classifier::OnnxTokenClassifier;
use pdfium_render::prelude::*;

/// NER 引擎全局状态（Arc<Mutex> 包裹以支持 spawn_blocking）
pub struct NerEngineState(pub Arc<Mutex<NerEngine>>);

/// PDFium 库句柄全局状态（Mutex 包裹以满足 Tauri manage 的 Send + Sync 要求）
pub struct PdfiumState(pub Mutex<Option<Pdfium>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_aptabase::Builder::new("A-US-1575489135").build());

    // E2E 测试：仅 debug 模式加载 WebDriver plugin
    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_webdriver_automation::init());
    }

    builder
        .setup(|app| {
            use tauri::Manager;

            // v0.1 数据迁移（仅首次）
            match commands::migrate::migrate_v1_tasks(app.handle()) {
                Ok(Some(ws_id)) => println!("v0.1 数据已迁移到工作区: {}", ws_id),
                Ok(None) => {},
                Err(e) => eprintln!("v0.1 数据迁移警告: {}", e),
            }

            // 初始化 NER 引擎（从 resources/ner/ 加载，文件不存在则降级）
            // 优先从 Tauri resource_dir 加载，dev 模式下回退到 src-tauri/resources/
            let resource_dir = app.path().resource_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("resources"));
            let ner_dir = resource_dir.join("ner");
            let ner_dir = if ner_dir.join("model.onnx").exists() {
                ner_dir
            } else {
                // dev 模式回退：从 src-tauri/resources/ner/ 加载
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join("ner")
            };
            println!("NER 模型目录: {:?}", ner_dir);
            let ner_engine = match OnnxTokenClassifier::try_load(&ner_dir) {
                Ok(Some(backend)) => {
                    println!("NER 引擎已加载 (ONNX)");
                    NerEngine::from_backend(Box::new(backend))
                }
                Ok(None) => {
                    println!("NER 引擎未加载模型，降级运行");
                    NerEngine::degraded()
                }
                Err(e) => {
                    eprintln!("NER 引擎加载警告: {}", e);
                    NerEngine::degraded()
                }
            };
            app.manage(NerEngineState(Arc::new(Mutex::new(ner_engine))));

            // 初始化语言状态（由 Cargo feature `lang-zh` / `lang-en` 在编译期决定）
            app.manage(AppLanguage::from_build());

            // 初始化 PDFium（从 resources/pdfium/ 加载动态库）
            let pdfium_lib_name = if cfg!(target_os = "macos") {
                "libpdfium.dylib"
            } else if cfg!(target_os = "windows") {
                "pdfium.dll"
            } else {
                "libpdfium.so"
            };

            // 先尝试 resource_dir（生产环境），再回退到 CARGO_MANIFEST_DIR（dev 模式）
            let pdfium_lib = {
                let prod_path = resource_dir.join("pdfium").join(pdfium_lib_name);
                if prod_path.exists() {
                    prod_path
                } else {
                    // dev 模式回退
                    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("resources").join("pdfium").join(pdfium_lib_name)
                }
            };

            println!("PDFium 库路径: {:?}", pdfium_lib);
            let pdfium_inner = if pdfium_lib.exists() {
                match Pdfium::bind_to_library(&pdfium_lib) {
                    Ok(bindings) => {
                        println!("PDFium 已加载");
                        Some(Pdfium::new(bindings))
                    }
                    Err(e) => {
                        eprintln!("PDFium 加载失败，PDF 脱敏功能不可用: {}", e);
                        None
                    }
                }
            } else {
                println!("PDFium 动态库未找到，PDF 脱敏功能不可用");
                None
            };
            let pdfium_state = PdfiumState(Mutex::new(pdfium_inner));
            app.manage(pdfium_state);

            // 上报应用启动事件
            analytics::track(app.handle(), "app_launched", Some(serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
            })));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 文件操作
            import_file,
            import_file_with_password,
            import_clipboard_text,
            export_file,
            export_pdf_redacted_cmd,
            compute_pdf_redact_preview,
            render_pdf_pages,
            check_file_exists,
            copy_file_to_clipboard,
            // 识别引擎
            detect_by_regex,
            detect_by_ner,
            detect_by_dict,
            detect_columns,
            get_builtin_dict,
            // 脱敏
            apply_desensitize,
            apply_desensitize_by_columns,
            // v0.1 遗留命令（保留兼容性，Phase 6 清理）
            load_config,
            save_config,
            load_dict,
            save_dict,
            save_task,
            list_tasks,
            delete_task,
            restore_file,
            // 统计配置
            get_analytics_enabled,
            set_analytics_enabled,
            // v0.2 工作区命令
            create_workspace,
            create_clipboard_workspace,
            list_workspaces,
            get_workspace,
            update_workspace,
            delete_workspace,
            rename_workspace,
            add_processing_record,
            update_processing_record_mappings,
            delete_processing_record,
            restore_processing,
            restore_from_workspace,
            restore_ai_response,
            clear_consistency_mappings,
            clear_type_consistency_mappings,
            // 别名组
            create_alias_group,
            add_alias_member,
            remove_alias_member,
            delete_alias_group,
            list_alias_groups,
            // 语言（运行时只读，编译期由 Cargo feature 决定）
            get_language,
        ])
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
