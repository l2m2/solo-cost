mod commands;
mod db;
mod domain;
mod error;
mod state;

use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
fn ping() -> AppResult<String> {
    Ok("pong".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            ping,
            commands::auth::is_initialized,
            commands::auth::setup,
            commands::auth::unlock,
            commands::auth::lock,
            commands::companies::list_companies,
            commands::companies::get_company,
            commands::companies::create_company,
            commands::companies::update_company,
            commands::companies::get_current_company_id,
            commands::companies::set_current_company,
            commands::categories::list_categories,
            commands::categories::create_category,
            commands::categories::update_category,
            commands::categories::delete_category,
            commands::categories::seed_preset_categories_if_empty,
            commands::projects::list_projects,
            commands::projects::get_project,
            commands::projects::create_project,
            commands::projects::update_project,
            commands::projects::set_project_status,
            commands::projects::delete_project,
            commands::costs::list_cost_entries,
            commands::costs::create_cost_entry,
            commands::costs::update_cost_entry,
            commands::costs::delete_cost_entry,
            commands::costs::get_project_cost_summary,
            commands::trash::list_trash,
            commands::trash::restore_trash_item,
            commands::trash::purge_trash_item,
            commands::members::list_members,
            commands::members::get_member,
            commands::members::create_member,
            commands::members::update_member,
            commands::members::set_member_active,
            commands::members::delete_member,
            commands::payments::list_payments,
            commands::payments::get_payment,
            commands::payments::create_payment,
            commands::payments::update_payment,
            commands::payments::mark_payment_received,
            commands::payments::delete_payment,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
