// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, ModelTrait, PaginatorTrait, QueryFilter, Schema,
};
use tauri::State;
use tokio::sync::Mutex;

use entities::*;

struct AppState {
    db: Mutex<Option<DatabaseConnection>>,
}

#[tokio::main]
async fn main() {
    let db = Database::connect("sqlite:chembank.db?mode=rwc")
        .await
        .unwrap();
    let _ = init_db(&db).await;
    tauri::Builder::default()
        .manage(AppState {
            db: Mutex::new(Some(db)),
        })
        .invoke_handler(tauri::generate_handler![
            structure_count,
            reset_database,
            create_structure,
            update_structure,
            remove_structure,
            set_component,
            delete_component,
            set_image,
            set_property,
            search_structure,
            get_structure_detail,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
#[specta::specta]
async fn reset_database(state: State<'_, AppState>) -> Result<(), String> {
    let mut db = state.db.lock().await;
    if let Some(db) = db.take() {
        db.close().await.map_err(|e| format!("无法关闭当前数据库，请确认磁盘空间充足，或强制重启程序，但可能导致最近的部分操作丢失, 详细信息：{:#?}", e))?;
    };
    tokio::fs::remove_file("chembank.db")
        .await
        .map_err(|e| format!("无法删除旧的数据库，原因如下：\n{:#?}", e))?;
    let new_db = Database::connect("sqlite:chembank.db?mode=rwc")
        .await
        .map_err(|e| {
            format!(
                "无法创建目标数据库，这可能是由于权限问题或文件损坏导致的，详细信息：{:#?}",
                e
            )
        })?;
    init_db(&new_db).await?;
    *db = Some(new_db);
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn structure_count(state: State<'_, AppState>) -> Result<u32, String> {
    let db = state.db.lock().await;
    let db = db
        .as_ref()
        .ok_or("无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员".to_string())?;
    structure::Entity::find()
        .count(db)
        .await
        .map(|value| value as u32)
        .map_err(|e| format!("数据库故障，原因：{:#?}", e))
}

#[tauri::command]
#[specta::specta]
async fn create_structure(
    state: State<'_, AppState>,
    name: Option<String>,
    formula: String,
    smiles: Option<String>,
    charge: i8,
) -> Result<u32, String> {
    let db = state.db.lock().await;
    let db = db
        .as_ref()
        .ok_or("无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员".to_string())?;
    let model = structure::ActiveModel {
        id: ActiveValue::not_set(),
        name: ActiveValue::set(name),
        formula: ActiveValue::set(formula),
        smiles: ActiveValue::set(smiles),
        charge: ActiveValue::set(charge),
    };
    let model = model.insert(db).await.map_err(|e| {
        format!(
            "无法添加结构，请检查是否有重复的名称或SMILES，详细信息\n{:#?}",
            e
        )
    })?;
    Ok(model.id)
}

#[tauri::command]
#[specta::specta]
async fn update_structure(
    state: State<'_, AppState>,
    id: u32,
    name: Option<String>,
    formula: String,
    smiles: Option<String>,
    charge: i8,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db
        .as_ref()
        .ok_or("无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员".to_string())?;

    let model = structure::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| format!("数据库故障 ，详细信息\n{:#?}", e))?
        .ok_or("没有找到对应的结构记录，可能已经删除或未添加")?;
    let mut model: structure::ActiveModel = model.into();
    model.name = ActiveValue::set(name);
    model.formula = ActiveValue::set(formula);
    model.smiles = ActiveValue::set(smiles);
    model.charge = ActiveValue::set(charge);
    model.update(db).await.map_err(|e| {
        format!(
            "无法更新结构，请检查是否有重复的名称或SMILES，详细信息\n{:#?}",
            e
        )
    })?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn remove_structure(state: State<'_, AppState>, id: u32) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db
        .as_ref()
        .ok_or("无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员".to_string())?;

    let component_of = component::Entity::find()
        .filter(component::Column::ComponentId.eq(id))
        .count(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    if component_of > 0 {
        Err(format!(
            "该结构仍被作为其他结构的组成部分存在，请检查删除相应结构后再删除此结构"
        ))?;
    };
    structure::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?
        .ok_or("未找到对应结构，可能已经删除或未添加".to_string())?
        .delete(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    component::Entity::delete_many()
        .filter(component::Column::StructureId.eq(id))
        .exec(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    image::Entity::delete_many()
        .filter(image::Column::StructureId.eq(id))
        .exec(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    property::Entity::delete_many()
        .filter(property::Column::StructureId.eq(id))
        .exec(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn set_component(
    state: State<'_, AppState>,
    structure_id: u32,
    component_id: u32,
    count: u32,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or(format!(
        "无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员"
    ))?;
    let model = component::ActiveModel {
        structure_id: ActiveValue::set(structure_id),
        component_id: ActiveValue::set(component_id),
        count: ActiveValue::Set(count),
    };
    model
        .save(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn delete_component(
    state: State<'_, AppState>,
    structure_id: u32,
    component_id: u32,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or(format!(
        "无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员"
    ))?;
    let model = component::Entity::find_by_id((structure_id, component_id))
        .one(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?
        .ok_or("未找到对应组件信息，可能尚未创建？".to_string())?;
    model
        .delete(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn set_image(
    state: State<'_, AppState>,
    structure_id: u32,
    image: Vec<u8>,
    filename: String,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or(format!(
        "无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员"
    ))?;
    let model = image::ActiveModel {
        structure_id: ActiveValue::set(structure_id),
        image: ActiveValue::set(image),
        filename: ActiveValue::set(filename),
    };
    if image::Entity::find_by_id(structure_id)
        .one(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?
        .is_some()
    {
        model
            .update(db)
            .await
            .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    } else {
        model
            .insert(db)
            .await
            .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    };

    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn set_property(state: State<'_, AppState>, model: property::Model) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or(format!(
        "无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员"
    ))?;
    let structure_id = model.structure_id;
    let model: property::ActiveModel = model.into();
    let model = model.reset_all();
    if property::Entity::find_by_id(structure_id)
        .one(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?
        .is_some()
    {
        model
            .update(db)
            .await
            .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    } else {
        model
            .insert(db)
            .await
            .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn get_structure_detail(
    state: State<'_, AppState>,
    id: u32,
) -> Result<
    (
        structure::Model,
        Option<property::Model>,
        Option<image::Model>,
        Vec<(component::Model, Option<structure::Model>)>,
    ),
    String,
> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or(format!(
        "无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员"
    ))?;
    let model = structure::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))?
        .ok_or("没有找到对应记录")?;
    let property_model = model
        .find_related(property::Entity)
        .one(db)
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))?;
    let image_model = model
        .find_related(image::Entity)
        .one(db)
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))?;
    let components = component::Entity::find()
        .filter(component::Column::StructureId.eq(model.id))
        .find_also_linked(links::ComponentStructure)
        .all(db)
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))
        .unwrap();
    Ok((model, property_model, image_model, components))
}

#[tauri::command]
#[specta::specta]
async fn search_structure(
    state: State<'_, AppState>,
    page_size: u32,
    page_number: u32,
    keyword: Option<String>,
    max_charge: i8,
    min_charge: i8,
) -> Result<Vec<structure::Model>, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or(format!(
        "无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员"
    ))?;
    let mut models = structure::Entity::find();
    if let Some(keyword) = keyword {
        let keyword = format!("%{}%", keyword);
        models = models.filter(
            Condition::any()
                .add(structure::Column::Formula.like(&keyword))
                .add(structure::Column::Smiles.like(&keyword))
                .add(structure::Column::Name.like(&keyword)),
        )
    }
    let models = models
        .filter(structure::Column::Charge.gte(min_charge))
        .filter(structure::Column::Charge.lte(max_charge))
        .cursor_by(structure::Column::Id)
        .after(page_size * page_number)
        .before(page_size * (page_number + 1))
        .all(db)
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))?;
    Ok(models)
}

#[test]
fn export_bindings() {
    use specta::collect_types;
    use tauri_specta::ts;

    ts::export(
        collect_types![
            structure_count,
            reset_database,
            create_structure,
            update_structure,
            remove_structure,
            set_component,
            delete_component,
            set_image,
            set_property,
            search_structure,
            get_structure_detail,
        ],
        "../src/bindings.ts",
    )
    .unwrap();
}

async fn init_db(db: &DatabaseConnection) -> Result<(), String> {
    let builder = db.get_database_backend();
    let structure_stmt = Schema::new(builder).create_table_from_entity(structure::Entity);
    let component_stmt = Schema::new(builder).create_table_from_entity(component::Entity);
    let property_stmt = Schema::new(builder).create_table_from_entity(property::Entity);
    let image_stmt = Schema::new(builder).create_table_from_entity(image::Entity);
    for stmt in vec![structure_stmt, component_stmt, property_stmt, image_stmt] {
        let stmt = builder.build(&stmt);
        db.execute(stmt)
            .await
            .map_err(|e| format!("未能完成初始化，详细信息：\n{:#?}", e))?;
    }
    Ok(())
}

#[tokio::test]
async fn test_create_db() {
    let db = Database::connect("sqlite://./chembank.db?mode=rwc")
        .await
        .unwrap();
    init_db(&db).await.unwrap();
    db.close().await.unwrap();
}
