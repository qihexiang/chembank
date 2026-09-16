// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use sea_orm::{
    prelude::Expr, sea_query::Func, ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, Database, DatabaseConnection, EntityTrait, JoinType, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Schema, Select, TransactionTrait
};
use skip_bom::{BomType, SkipEncodingBom};
use tauri::State;
use tokio::sync::Mutex;

use entities::*;

mod chemistry;
mod functional_groups;

struct AppState {
    db: Mutex<Option<DatabaseConnection>>,
    /// 数据库文件位置，与启动方式（工作目录）无关。
    database_path: PathBuf,
}

/// 数据库文件名，存放于用户本地数据目录。
const DATABASE_FILE: &str = "chembank.db";

/// SQLite 连接串；Windows 路径统一用正斜杠，避免反斜杠被当作转义。
fn database_url(path: &Path) -> String {
    format!("sqlite:{}?mode=rwc", path.to_string_lossy().replace('\\', "/"))
}

#[tokio::main]
async fn main() {
    let context = tauri::generate_context!();
    // 数据库放在用户数据目录而不是可执行文件旁：安装到 Program Files 后目录不可写，
    // 且工作目录会随启动方式（快捷方式、文件关联）变化，数据位置必须与两者无关。
    let data_dir = tauri::api::path::app_local_data_dir(context.config())
        .expect("无法确定用户数据目录，请确认当前用户配置文件正常");
    fs::create_dir_all(&data_dir)
        .unwrap_or_else(|error| panic!("无法创建用户数据目录 {}：{error}", data_dir.display()));
    let database_path = data_dir.join(DATABASE_FILE);
    let db = Database::connect(database_url(&database_path)).await.unwrap();
    let _ = init_db(&db).await;
    tauri::Builder::default()
        .manage(AppState {
            db: Mutex::new(Some(db)),
            database_path,
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
            generate_structure_from_smiles,
            list_functional_groups,
            create_functional_group,
            remove_functional_group,
            rematch_functional_groups,
            export_to_folder,
            import_from_folder,
        ])
        .run(context)
        .expect("error while running tauri application");
}

#[tauri::command]
#[specta::specta]
async fn reset_database(state: State<'_, AppState>) -> Result<(), String> {
    let mut db = state.db.lock().await;
    if let Some(db) = db.take() {
        db.close().await.map_err(|e| format!("无法关闭当前数据库，请确认磁盘空间充足，或强制重启程序，但可能导致最近的部分操作丢失, 详细信息：{:#?}", e))?;
    };
    let path = state.database_path.as_path();
    tokio::fs::remove_file(path)
        .await
        .map_err(|e| format!("无法删除旧的数据库，原因如下：\n{:#?}", e))?;
    let new_db = Database::connect(database_url(path)).await.map_err(|e| {
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
        smiles: ActiveValue::set(smiles.clone()),
        charge: ActiveValue::set(charge),
    };
    let model = model.insert(db).await.map_err(|e| {
        format!(
            "无法添加结构，请检查是否有重复的名称或SMILES，详细信息\n{:#?}",
            e
        )
    })?;
    functional_groups::match_structures(db, &[(model.id, smiles)]).await?;
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
    model.smiles = ActiveValue::set(smiles.clone());
    model.charge = ActiveValue::set(charge);
    model.update(db).await.map_err(|e| {
        format!(
            "无法更新结构，请检查是否有重复的名称或SMILES，详细信息\n{:#?}",
            e
        )
    })?;
    functional_groups::match_structures(db, &[(id, smiles)]).await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn remove_structure(state: State<'_, AppState>, id: u32) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db
        .as_ref()
        .ok_or("无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员".to_string())?;
    let txn = db.begin().await.map_err(|e| {
        format!(
            "无法开始事务，可能是由于数据库损坏或权限问题，详细信息\n{:#?}",
            e
        )
    })?;
    let component_of = component::Entity::find()
        .filter(component::Column::ComponentId.eq(id))
        .count(&txn)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    if component_of > 0 {
        Err(format!(
            "该结构仍被作为其他结构的组成部分存在，请检查删除相应结构后再删除此结构"
        ))?;
    };
    component::Entity::delete_many()
        .filter(component::Column::StructureId.eq(id))
        .exec(&txn)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    structure_functional_group::Entity::delete_many()
        .filter(structure_functional_group::Column::StructureId.eq(id))
        .exec(&txn)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    image::Entity::delete_many()
        .filter(image::Column::StructureId.eq(id))
        .exec(&txn)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    property::Entity::delete_many()
        .filter(property::Column::StructureId.eq(id))
        .exec(&txn)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    structure::Entity::find_by_id(id)
        .one(&txn)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?
        .ok_or("未找到对应结构，可能已经删除或未添加".to_string())?
        .delete(&txn)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    txn.commit().await.map_err(|e| {
        format!(
            "无法提交事务，可能是由于数据库损坏或权限问题，详细信息\n{:#?}",
            e
        )
    })?;
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
    upsert_component(db, structure_id, component_id, count).await
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
    let relateds = component::Entity::find()
        .filter(component::Column::ComponentId.eq(model.id))
        .find_also_linked(links::StructureComponent)
        .all(db)
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))?;
    Ok((model, property_model, image_model, components, relateds))
}

/// 只保留同时含有 `functional_group_ids` 全部官能团的结构。
fn filter_by_functional_groups(
    models: Select<structure::Entity>,
    functional_group_ids: &[u32],
) -> Select<structure::Entity> {
    if functional_group_ids.is_empty() {
        return models;
    }
    models
        .join(
            JoinType::InnerJoin,
            structure_functional_group::Relation::Structure.def().rev(),
        )
        .filter(
            structure_functional_group::Column::FunctionalGroupId
                .is_in(functional_group_ids.to_vec()),
        )
        .group_by(structure::Column::Id)
        .having(
            Expr::expr(Func::count_distinct(Expr::col(
                structure_functional_group::Column::FunctionalGroupId,
            )))
            .eq(functional_group_ids.len() as i32),
        )
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
    functional_group_ids: Vec<u32>,
) -> Result<(Vec<structure::Model>, u32), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or(format!(
        "无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员"
    ))?;
    let mut models = structure::Entity::find().left_join(property::Entity);
    if let Some(keyword) = keyword {
        let keyword = format!("%{}%", keyword);
        models = models.filter(
            Expr::col((structure::Entity, structure::Column::Name))
            .like(&keyword)
            .or(Expr::col((structure::Entity, structure::Column::Formula)).like(&keyword))
            .or(Expr::col((structure::Entity, structure::Column::Smiles)).like(&keyword))
            .or(Expr::col((property::Entity, property::Column::Remarks)).like(&keyword))
            .or(Expr::col((property::Entity, property::Column::References)).like(&keyword))
            .or(Expr::col((property::Entity, property::Column::DecompTemp)).like(&keyword))
            .or(Expr::col((property::Entity, property::Column::DetPressure)).like(&keyword))
            .or(Expr::col((property::Entity, property::Column::DetVelocity)).like(&keyword))
            .or(Expr::col((property::Entity, property::Column::DissTemp)).like(&keyword))
            .or(Expr::col((property::Entity, property::Column::Density)).like(&keyword))
            .or(Expr::col((property::Entity, property::Column::FormationEnthalpy)).like(&keyword))
            .or(Expr::col((property::Entity, property::Column::FrictionSensitivity)).like(&keyword))
            .or(Expr::col((property::Entity, property::Column::ImpactSensitive)).like(&keyword))
            .or(Expr::col((property::Entity, property::Column::NContent)).like(&keyword))
            .or(Expr::col((property::Entity, property::Column::NoContent)).like(&keyword))
            .or(Expr::col((property::Entity, property::Column::OContent)).like(&keyword))
        );
    }
    let models = filter_by_functional_groups(models, &functional_group_ids);
    let models = models
        .filter(structure::Column::Charge.gte(min_charge))
        .filter(structure::Column::Charge.lte(max_charge))
        .order_by_desc(structure::Column::Id)
        .paginate(db, page_size as u64);
    let pages = models.num_pages().await.map_err(|e| {
        format!(
            "查询错误，可能是由于数据库损坏或权限问题，详细信息\n{:#?}",
            e
        )
    })?;
    let models = models.fetch_page(page_number as u64).await.map_err(|e| {
        format!(
            "查询错误，可能是由于数据库损坏或权限问题，详细信息\n{:#?}",
            e
        )
    })?;
    Ok((models, pages as u32))
}

/// 由 SMILES 生成结构信息，并把其中互不连接的片段登记为子结构。
#[tauri::command]
#[specta::specta]
async fn generate_structure_from_smiles(
    state: State<'_, AppState>,
    id: u32,
    smiles: String,
) -> Result<chemistry::SmilesInfo, String> {
    let info = tokio::task::spawn_blocking(move || chemistry::analyze_smiles(&smiles))
        .await
        .map_err(|e| format!("SMILES 解析任务异常结束：{e}"))??;
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员".to_string())?;
    link_fragments(db, id, &info.fragments).await?;
    Ok(info)
}

/// 把片段登记为 `structure_id` 的子结构：库中已有的直接引用，缺失的先新建结构。
async fn link_fragments(
    db: &DatabaseConnection,
    structure_id: u32,
    fragments: &[chemistry::Fragment],
) -> Result<(), String> {
    if fragments.is_empty() {
        return Ok(());
    }
    // `smiles` 是唯一列，先按标准化 SMILES 精确匹配；未命中的片段再与库中所有 SMILES 的
    // 标准化结果比对一次，以兼容导入数据中未标准化的写法。
    let mut component_ids = Vec::with_capacity(fragments.len());
    for fragment in fragments {
        component_ids.push(find_structure_by_smiles(db, &fragment.smiles).await?);
    }
    if component_ids.iter().any(Option::is_none) {
        let canonical = canonical_smiles_index(db).await?;
        for (fragment, component_id) in fragments.iter().zip(component_ids.iter_mut()) {
            if component_id.is_none() {
                *component_id = canonical.get(&fragment.smiles).copied();
            }
        }
    }
    let txn = db
        .begin()
        .await
        .map_err(|e| format!("无法开启事务，详细信息\n{:#?}", e))?;
    let mut created = Vec::new();
    for (fragment, component_id) in fragments.iter().zip(component_ids) {
        let component_id = match component_id {
            // 片段即当前结构本身，跳过以免自引用
            Some(component_id) if component_id == structure_id => continue,
            Some(component_id) => component_id,
            None => {
                let charge = i8::try_from(fragment.formal_charge).map_err(|_| {
                    format!("片段 `{}` 的形式电荷超出可存储范围", fragment.smiles)
                })?;
                let model = structure::ActiveModel {
                    id: ActiveValue::not_set(),
                    name: ActiveValue::set(None),
                    formula: ActiveValue::set(fragment.formula.clone()),
                    smiles: ActiveValue::set(Some(fragment.smiles.clone())),
                    charge: ActiveValue::set(charge),
                };
                let created_id = model
                    .insert(&txn)
                    .await
                    .map_err(|e| format!("无法新建片段 `{}` 的结构，详细信息\n{:#?}", fragment.smiles, e))?
                    .id;
                created.push((created_id, Some(fragment.smiles.clone())));
                created_id
            }
        };
        upsert_component(&txn, structure_id, component_id, fragment.count).await?;
    }
    functional_groups::match_structures(&txn, &created).await?;
    txn.commit().await.map_err(|e| {
        format!(
            "无法提交事务，可能是由于数据库损坏或权限问题，详细信息\n{:#?}",
            e
        )
    })
}

/// 按标准化 SMILES 精确查找结构。
async fn find_structure_by_smiles<C: ConnectionTrait>(
    db: &C,
    smiles: &str,
) -> Result<Option<u32>, String> {
    Ok(structure::Entity::find()
        .filter(structure::Column::Smiles.eq(smiles))
        .one(db)
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))?
        .map(|model| model.id))
}

/// 库中全部结构的 SMILES 标准化后的索引，同一标准化 SMILES 保留首次出现的结构。
async fn canonical_smiles_index<C: ConnectionTrait>(
    db: &C,
) -> Result<HashMap<String, u32>, String> {
    let stored: Vec<(u32, Option<String>)> = structure::Entity::find()
        .select_only()
        .column(structure::Column::Id)
        .column(structure::Column::Smiles)
        .filter(structure::Column::Smiles.is_not_null())
        .into_tuple()
        .all(db)
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))?;
    let smiles = stored
        .iter()
        .filter_map(|(_, smiles)| smiles.clone())
        .collect::<Vec<_>>();
    let canonical = tokio::task::spawn_blocking(move || chemistry::canonical_smiles_batch(&smiles))
        .await
        .map_err(|e| format!("SMILES 标准化任务异常结束：{e}"))??;
    let mut index = HashMap::new();
    for ((id, _), canonical) in stored.iter().zip(canonical) {
        if let Some(canonical) = canonical {
            index.entry(canonical).or_insert(*id);
        }
    }
    Ok(index)
}

/// 写入或更新子结构数量。
async fn upsert_component<C: ConnectionTrait>(
    db: &C,
    structure_id: u32,
    component_id: u32,
    count: u32,
) -> Result<(), String> {
    let model = component::ActiveModel {
        structure_id: ActiveValue::set(structure_id),
        component_id: ActiveValue::set(component_id),
        count: ActiveValue::set(count),
    };
    if component::Entity::find_by_id((structure_id, component_id))
        .one(db)
        .await
        .map_err(|e| format!("数据库错误，详细信息：\n{:#?}", e))?
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

fn write_bom<T: std::io::Write>(w: &mut T) -> std::io::Result<()> {
    w.write_all(&[0xEF, 0xBB, 0xBF])
}

/// 列出全部官能团。
#[tauri::command]
#[specta::specta]
async fn list_functional_groups(
    state: State<'_, AppState>,
) -> Result<Vec<functional_group::Model>, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员".to_string())?;
    functional_group::Entity::find()
        .order_by_asc(functional_group::Column::Id)
        .all(db)
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))
}

/// 新增官能团，并对全部既有结构回填该官能团的匹配结果。
#[tauri::command]
#[specta::specta]
async fn create_functional_group(
    state: State<'_, AppState>,
    name: String,
    smarts: String,
) -> Result<u32, String> {
    let name = name.trim().to_string();
    let smarts = smarts.trim().to_string();
    if name.is_empty() {
        return Err("官能团名称不能为空".to_string());
    }
    tokio::task::spawn_blocking({
        let smarts = smarts.clone();
        move || chemistry::validate_smarts(&smarts)
    })
    .await
    .map_err(|e| format!("SMARTS 校验任务异常结束：{e}"))??;
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员".to_string())?;
    let model = functional_group::ActiveModel {
        id: ActiveValue::not_set(),
        name: ActiveValue::set(name),
        smarts: ActiveValue::set(smarts),
    };
    let model = model.insert(db).await.map_err(|e| {
        format!(
            "无法添加官能团，请检查是否与已有官能团重名，详细信息\n{:#?}",
            e
        )
    })?;
    functional_groups::rematch_groups(db, &[model.id]).await?;
    Ok(model.id)
}

/// 删除官能团及其全部关联。
#[tauri::command]
#[specta::specta]
async fn remove_functional_group(state: State<'_, AppState>, id: u32) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员".to_string())?;
    delete_functional_group(db, id).await
}

async fn delete_functional_group(db: &DatabaseConnection, id: u32) -> Result<(), String> {
    let txn = db
        .begin()
        .await
        .map_err(|e| format!("无法开启事务，详细信息\n{:#?}", e))?;
    structure_functional_group::Entity::delete_many()
        .filter(structure_functional_group::Column::FunctionalGroupId.eq(id))
        .exec(&txn)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    functional_group::Entity::find_by_id(id)
        .one(&txn)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?
        .ok_or("未找到对应官能团，可能已经删除")?
        .delete(&txn)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    txn.commit().await.map_err(|e| {
        format!(
            "无法提交事务，可能是由于数据库损坏或权限问题，详细信息\n{:#?}",
            e
        )
    })
}

/// 按当前词表重算全部结构的官能团关联。
#[tauri::command]
#[specta::specta]
async fn rematch_functional_groups(state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员".to_string())?;
    functional_groups::rematch_all(db).await
}

#[tauri::command]
#[specta::specta]
async fn import_from_folder(
    state: State<'_, AppState>,
    folder_path: PathBuf,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or(format!(
        "无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员"
    ))?;
    // 词表随数据导入：先清掉 reset_database 后自动写入的预置项，再读导出目录中的词表
    functional_group::Entity::delete_many()
        .exec(db)
        .await
        .map_err(|e| format!("写入失败，原因：\n{:#?}", e))?;
    let functional_group_csv = folder_path.join("functional_groups.csv");
    if functional_group_csv.is_file() {
        let functional_group_csv =
            File::open(&functional_group_csv).map_err(|e| format!("无法打开表格，{:#?}", e))?;
        let functional_group_csv = SkipEncodingBom::new(&[BomType::UTF8], functional_group_csv);
        let mut functional_group_csv = csv::Reader::from_reader(functional_group_csv);
        for model in functional_group_csv.deserialize() {
            let model: functional_group::Model =
                model.map_err(|e| format!("functional_group表格式不正确：\n{:#?}", e))?;
            let model: functional_group::ActiveModel = model.into();
            let model = model.reset_all();
            model
                .insert(db)
                .await
                .map_err(|e| format!("写入失败，原因：\n{:#?}", e))?;
        }
    }
    let structure_csv = folder_path.join("structures.csv");
    let structure_csv = File::open(structure_csv).map_err(|e| format!("无法打开表格，{:#?}", e))?;
    let structure_csv = SkipEncodingBom::new(&[BomType::UTF8], structure_csv);
    let mut structure_csv = csv::Reader::from_reader(structure_csv);
    for model in structure_csv.deserialize() {
        let model: structure::Model =
            model.map_err(|e| format!("structure表格式不正确：\n{:#?}", e))?;
        let model: structure::ActiveModel = model.into();
        let model = model.reset_all();
        model
            .insert(db)
            .await
            .map_err(|e| format!("写入失败，原因：\n{:#?}", e))?;
    }
    // 结构入库后按词表补齐关联（关联是派生数据，不随表格导出）
    functional_groups::rematch_all(db).await?;
    let property_csv = folder_path.join("properties.csv");
    let property_csv = File::open(property_csv).map_err(|e| format!("无法打开表格，{:#?}", e))?;
    let property_csv = SkipEncodingBom::new(&[BomType::UTF8], property_csv);
    let mut property_csv = csv::Reader::from_reader(property_csv);
    for model in property_csv.deserialize() {
        let model: property::Model =
            model.map_err(|e| format!("property表格式不正确：\n{:#?}", e))?;
        let model: property::ActiveModel = model.into();
        let model = model.reset_all();
        model
            .insert(db)
            .await
            .map_err(|e| format!("写入失败，原因：\n{:#?}", e))?;
    }
    let component_csv = folder_path.join("components.csv");
    let component_csv = File::open(component_csv).map_err(|e| format!("无法打开表格，{:#?}", e))?;
    let component_csv = SkipEncodingBom::new(&[BomType::UTF8], component_csv);
    let mut component_csv = csv::Reader::from_reader(component_csv);
    for model in component_csv.deserialize() {
        let model: component::Model =
            model.map_err(|e| format!("component表格式不正确：\n{:#?}", e))?;
        let model: component::ActiveModel = model.into();
        let model = model.reset_all();
        model
            .insert(db)
            .await
            .map_err(|e| format!("写入失败，原因：\n{:#?}", e))?;
    }
    let image_folder = folder_path.join("images");
    let image_folders =
        fs::read_dir(&image_folder).map_err(|e| format!("无法读取图片目录\n{:#?}", e))?;
    for item in image_folders {
        let item = item.map_err(|e| format!("无法读取的路径\n{:#?}", e))?;
        let structure_id: u32 = item
            .file_name()
            .into_string()
            .map_err(|e| format!("无法识别的路径\n{:?}", e))?
            .parse()
            .map_err(|e| format!("无法将名称解析为结构ID：{:#?}", e))?;
        let filename = fs::read_dir(image_folder.join(structure_id.to_string()))
            .map_err(|e| format!("无法读取图片文件夹\n{:#?}", e))?
            .next()
            .ok_or("发现了空的图片文件夹")?
            .map_err(|e| format!("无法读取的路径\n{:#?}", e))?
            .file_name()
            .into_string()
            .map_err(|e| format!("无法识别的路径\n{:?}", e))?;
        let full_image_path = image_folder.join(structure_id.to_string()).join(&filename);
        let mut image_content = vec![];
        File::open(full_image_path)
            .map_err(|e| format!("无法打开文件\n{:#?}", e))?
            .read_to_end(&mut image_content)
            .map_err(|e| format!("无法读取文件\n{:#?}", e))?;
        let model = image::ActiveModel {
            structure_id: ActiveValue::set(structure_id),
            filename: ActiveValue::set(filename),
            image: ActiveValue::set(image_content),
        };
        model
            .insert(db)
            .await
            .map_err(|e| format!("未能添加记录，原因：\n{:#?}", e))?;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn export_to_folder(state: State<'_, AppState>, folder_path: PathBuf) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or(format!(
        "无法连接到数据库，请重启程序，如果该问题仍然发生，请联系管理员"
    ))?;
    let _ = fs::create_dir(&folder_path);
    let structures_csv = folder_path.join("structures.csv");
    let mut structures_csv =
        File::create(structures_csv).map_err(|e| format!("无法创建表格：\n{:#?}", e))?;
    write_bom(&mut structures_csv).map_err(|e| format!("无法写入文件：\n{:#?}", e))?;
    let mut structure_csv = csv::Writer::from_writer(structures_csv);
    let structures = structure::Entity::find()
        .all(db)
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))?;
    for structure in structures {
        structure_csv
            .serialize(structure)
            .map_err(|e| format!("写入错误，原因为：{:#?}", e))?;
    }
    let properties_csv = folder_path.join("properties.csv");
    let mut properties_csv =
        File::create(properties_csv).map_err(|e| format!("无法创建表格：\n{:#?}", e))?;
    write_bom(&mut properties_csv).map_err(|e| format!("无法写入文件：\n{:#?}", e))?;
    let mut property_csv = csv::Writer::from_writer(properties_csv);
    let properties = property::Entity::find()
        .all(db)
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))?;
    for property in properties {
        property_csv
            .serialize(property)
            .map_err(|e| format!("写入错误，原因为：{:#?}", e))?;
    }
    let components_csv = folder_path.join("components.csv");
    let mut components_csv =
        File::create(components_csv).map_err(|e| format!("无法创建表格：\n{:#?}", e))?;
    write_bom(&mut components_csv).map_err(|e| format!("无法写入文件：\n{:#?}", e))?;
    let mut component_csv = csv::Writer::from_writer(components_csv);
    let components = component::Entity::find()
        .all(db)
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))?;
    for component in components {
        component_csv
            .serialize(component)
            .map_err(|e| format!("写入错误，原因为：\n{:#?}", e))?;
    }
    let functional_groups_csv = folder_path.join("functional_groups.csv");
    let mut functional_groups_csv =
        File::create(functional_groups_csv).map_err(|e| format!("无法创建表格：\n{:#?}", e))?;
    write_bom(&mut functional_groups_csv).map_err(|e| format!("无法写入文件：\n{:#?}", e))?;
    let mut functional_group_csv = csv::Writer::from_writer(functional_groups_csv);
    let functional_groups = functional_group::Entity::find()
        .order_by_asc(functional_group::Column::Id)
        .all(db)
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))?;
    for functional_group in functional_groups {
        functional_group_csv
            .serialize(functional_group)
            .map_err(|e| format!("写入错误，原因为：\n{:#?}", e))?;
    }
    let image_folder = folder_path.join("images");
    let _ = fs::create_dir(&image_folder);
    let mut image_pages = image::Entity::find()
        .order_by_asc(image::Column::StructureId)
        .paginate(db, 10);
    while let Some(images) = image_pages
        .fetch_and_next()
        .await
        .map_err(|e| format!("查询错误，详细信息\n{:#?}", e))?
    {
        for image in images {
            let image_folder = image_folder.join(image.structure_id.to_string());
            let _ = fs::create_dir(&image_folder);
            let write_path = image_folder.join(image.filename);
            let mut write_file =
                File::create(write_path).map_err(|e| format!("无法创建图片：\n{:#?}", e))?;
            write_file
                .write_all(&image.image)
                .map_err(|e| format!("无法写入图片：\n{:#?}", e))?;
        }
    }
    Ok(())
}

#[test]
fn export_bindings() {
    use specta::collect_types;
    use tauri_specta::ts;

    ts::export(
        collect_types![
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
            generate_structure_from_smiles,
            list_functional_groups,
            create_functional_group,
            remove_functional_group,
            rematch_functional_groups,
            export_to_folder,
            import_from_folder,
        ],
        "../src/bindings.ts",
    )
    .unwrap();
}

async fn init_db(db: &DatabaseConnection) -> Result<(), String> {
    let builder = db.get_database_backend();
    let statements = [
        Schema::new(builder).create_table_from_entity(structure::Entity),
        Schema::new(builder).create_table_from_entity(component::Entity),
        Schema::new(builder).create_table_from_entity(property::Entity),
        Schema::new(builder).create_table_from_entity(image::Entity),
        Schema::new(builder).create_table_from_entity(functional_group::Entity),
        Schema::new(builder).create_table_from_entity(structure_functional_group::Entity),
    ];
    for mut stmt in statements {
        // 增量建表：老库已存在的表不能中断后续建表
        stmt.if_not_exists();
        let stmt = builder.build(&stmt);
        db.execute(stmt)
            .await
            .map_err(|e| format!("未能完成初始化，详细信息：\n{:#?}", e))?;
    }
    // 词表为空说明是新库或老库首次升级：写入预置官能团，并补齐既有结构的关联
    if functional_groups::seed(db).await? {
        functional_groups::rematch_all(db).await?;
    }
    Ok(())
}

/// 首次启动（空数据库）应当建好表结构并写入预置官能团词表。
#[tokio::test]
async fn empty_database_is_initialized_with_vocabulary() {
    let (db, path) = temp_db("init").await;
    let groups = functional_group::Entity::find().count(&db).await.unwrap();
    assert_eq!(
        groups as usize,
        functional_groups::DEFAULT_FUNCTIONAL_GROUPS.len()
    );
    assert_eq!(structure::Entity::find().count(&db).await.unwrap(), 0);
    close_temp_db(db, path).await;
}

/// 新建一份独立的临时数据库，避免影响真实数据。
#[cfg(test)]
async fn temp_db(name: &str) -> (DatabaseConnection, PathBuf) {
    let path = std::env::temp_dir().join(format!("chembank_{name}_{}.db", std::process::id()));
    let _ = fs::remove_file(&path);
    let url = format!(
        "sqlite:{}?mode=rwc",
        path.to_string_lossy().replace('\\', "/")
    );
    let db = Database::connect(url).await.unwrap();
    init_db(&db).await.unwrap();
    (db, path)
}

#[cfg(test)]
async fn close_temp_db(db: DatabaseConnection, path: PathBuf) {
    db.close().await.unwrap();
    let _ = fs::remove_file(path);
}

#[cfg(test)]
async fn insert_structure(
    db: &DatabaseConnection,
    name: &str,
    formula: &str,
    smiles: &str,
    charge: i8,
) -> u32 {
    structure::ActiveModel {
        id: ActiveValue::not_set(),
        name: ActiveValue::set(Some(name.to_string())),
        formula: ActiveValue::set(formula.to_string()),
        smiles: ActiveValue::set(Some(smiles.to_string())),
        charge: ActiveValue::set(charge),
    }
    .insert(db)
    .await
    .unwrap()
    .id
}

/// 库中全部结构的（分子式，SMILES，电荷）。
#[cfg(test)]
async fn stored_structures(db: &DatabaseConnection) -> Vec<(String, Option<String>, i8)> {
    let mut structures = structure::Entity::find()
        .select_only()
        .column(structure::Column::Formula)
        .column(structure::Column::Smiles)
        .column(structure::Column::Charge)
        .into_tuple()
        .all(db)
        .await
        .unwrap();
    structures.sort();
    structures
}

/// 指定结构的（子结构ID，数目）。
#[cfg(test)]
async fn stored_components(db: &DatabaseConnection, structure_id: u32) -> Vec<(u32, u32)> {
    let mut components = component::Entity::find()
        .select_only()
        .column(component::Column::ComponentId)
        .column(component::Column::Count)
        .filter(component::Column::StructureId.eq(structure_id))
        .into_tuple()
        .all(db)
        .await
        .unwrap();
    components.sort();
    components
}

#[tokio::test]
async fn link_fragments_reuses_existing_and_creates_missing() {
    let (db, path) = temp_db("link_fragments").await;
    let sodium = insert_structure(&db, "钠离子", "Na+", "[Na+]", 1).await;
    let salt = insert_structure(&db, "氯化钠", "ClNa", "[Na+].[Cl-].[Cl-]", 0).await;
    let fragments = chemistry::analyze_smiles("[Na+].[Cl-].[Cl-]").unwrap().fragments;

    link_fragments(&db, salt, &fragments).await.unwrap();
    // 重复执行应保持同一结果
    link_fragments(&db, salt, &fragments).await.unwrap();

    let chloride = find_structure_by_smiles(&db, "[Cl-]").await.unwrap().unwrap();
    assert_ne!(chloride, sodium);
    assert_eq!(
        stored_structures(&db).await,
        vec![
            ("Cl-".to_string(), Some("[Cl-]".to_string()), -1),
            ("ClNa".to_string(), Some("[Na+].[Cl-].[Cl-]".to_string()), 0),
            ("Na+".to_string(), Some("[Na+]".to_string()), 1),
        ]
    );
    assert_eq!(
        stored_components(&db, salt).await,
        vec![(sodium, 1), (chloride, 2)]
    );
    close_temp_db(db, path).await;
}

#[tokio::test]
async fn link_fragments_skips_the_structure_itself() {
    let (db, path) = temp_db("link_fragments_self").await;
    let sodium = insert_structure(&db, "钠离子", "Na+", "[Na+]", 1).await;
    let fragments = chemistry::analyze_smiles("[Na+].[Cl-]").unwrap().fragments;

    link_fragments(&db, sodium, &fragments).await.unwrap();

    let chloride = find_structure_by_smiles(&db, "[Cl-]").await.unwrap().unwrap();
    assert_eq!(stored_components(&db, sodium).await, vec![(chloride, 1)]);
    // 钠离子不被重复创建
    assert_eq!(stored_structures(&db).await.len(), 2);
    close_temp_db(db, path).await;
}

#[tokio::test]
async fn link_fragments_matches_non_canonical_stored_smiles() {
    let (db, path) = temp_db("link_fragments_canonical").await;
    let benzene = insert_structure(&db, "苯", "C6H6", "C1=CC=CC=C1", 0).await;
    let mixture = insert_structure(&db, "苯与水", "C6H6O", "C1=CC=CC=C1.O", 0).await;
    let fragments = chemistry::analyze_smiles("C1=CC=CC=C1.O").unwrap().fragments;

    link_fragments(&db, mixture, &fragments).await.unwrap();

    let water = find_structure_by_smiles(&db, "O").await.unwrap().unwrap();
    assert_eq!(
        stored_components(&db, mixture).await,
        vec![(benzene, 1), (water, 1)]
    );
    assert_eq!(
        stored_structures(&db).await,
        vec![
            ("C6H6".to_string(), Some("C1=CC=CC=C1".to_string()), 0),
            ("C6H6O".to_string(), Some("C1=CC=CC=C1.O".to_string()), 0),
            ("H2O".to_string(), Some("O".to_string()), 0),
        ]
    );
    close_temp_db(db, path).await;
}

/// 某结构命中的官能团名称，按词表顺序。
#[cfg(test)]
async fn structure_functional_group_names(
    db: &DatabaseConnection,
    structure_id: u32,
) -> Vec<String> {
    let names = structure_functional_group::Entity::find()
        .select_only()
        .column(functional_group::Column::Name)
        .join(
            JoinType::InnerJoin,
            structure_functional_group::Relation::FunctionalGroup.def(),
        )
        .filter(structure_functional_group::Column::StructureId.eq(structure_id))
        .order_by_asc(functional_group::Column::Id)
        .into_tuple::<(String,)>()
        .all(db)
        .await
        .unwrap();
    names.into_iter().map(|(name,)| name).collect()
}

/// 官能团检索命中的结构 ID，按 ID 升序。
#[cfg(test)]
async fn search_ids_by_functional_groups(
    db: &DatabaseConnection,
    functional_group_ids: &[u32],
) -> Vec<u32> {
    let mut ids = filter_by_functional_groups(structure::Entity::find(), functional_group_ids)
        .all(db)
        .await
        .unwrap()
        .into_iter()
        .map(|model| model.id)
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

/// 名称到官能团 ID。
#[cfg(test)]
async fn functional_group_id(db: &DatabaseConnection, name: &str) -> u32 {
    functional_group::Entity::find()
        .filter(functional_group::Column::Name.eq(name))
        .one(db)
        .await
        .unwrap()
        .unwrap()
        .id
}

#[test]
fn functional_group_vocabulary_classifies_reference_molecules() {
    let groups = functional_groups::DEFAULT_FUNCTIONAL_GROUPS
        .iter()
        .enumerate()
        .map(|(index, (_, smarts))| (index as u32, smarts.to_string()))
        .collect::<Vec<_>>();
    let names = |smiles: &str| {
        let matched = chemistry::matching_functional_groups(&[smiles.to_string()], &groups)
            .unwrap()
            .remove(0);
        matched
            .into_iter()
            .map(|id| functional_groups::DEFAULT_FUNCTIONAL_GROUPS[id as usize].0.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(names("O=[N+]([O-])c1ccccc1"), vec!["硝基", "苯环"]);
    // 硝酸酯不应被误判为硝基或羟基
    assert_eq!(
        names("O=[N+]([O-])OCC(CO[N+](=O)[O-])O[N+](=O)[O-]"),
        vec!["硝酸酯"]
    );
    // 酯的氧不应被误判为醚键
    assert_eq!(names("CCOC(=O)C"), vec!["酯基", "羰基", "乙氧基"]);
    assert_eq!(names("Nc1ccccc1"), vec!["伯胺", "芳香胺", "苯环"]);
    assert_eq!(names("O=[N+]([O-])[O-].[K+]"), vec!["硝酸根离子"]);
}

#[tokio::test]
async fn functional_groups_follow_structure_smiles() {
    let (db, path) = temp_db("functional_groups_smiles").await;
    assert_eq!(
        functional_group::Entity::find().count(&db).await.unwrap(),
        35
    );

    let nitrobenzene = insert_structure(&db, "硝基苯", "C6H5NO2", "O=[N+]([O-])c1ccccc1", 0).await;
    functional_groups::match_structures(&db, &[(nitrobenzene, Some("O=[N+]([O-])c1ccccc1".to_string()))])
        .await
        .unwrap();
    assert_eq!(
        structure_functional_group_names(&db, nitrobenzene).await,
        vec!["硝基", "苯环"]
    );

    // SMILES 改变后关联随之重建
    functional_groups::match_structures(&db, &[(nitrobenzene, Some("Nc1ccccc1".to_string()))])
        .await
        .unwrap();
    assert_eq!(
        structure_functional_group_names(&db, nitrobenzene).await,
        vec!["伯胺", "芳香胺", "苯环"]
    );

    // SMILES 被清空后不再命中任何官能团
    functional_groups::match_structures(&db, &[(nitrobenzene, None)]).await.unwrap();
    assert!(structure_functional_group_names(&db, nitrobenzene).await.is_empty());
    close_temp_db(db, path).await;
}

#[tokio::test]
async fn functional_group_backfill_and_removal() {
    let (db, path) = temp_db("functional_groups_backfill").await;
    let benzene = insert_structure(&db, "苯", "C6H6", "c1ccccc1", 0).await;
    let ester = insert_structure(&db, "乙酸乙酯", "C4H8O2", "CCOC(=O)C", 0).await;
    functional_groups::rematch_all(&db).await.unwrap();

    let ethyl_ester = functional_group::ActiveModel {
        id: ActiveValue::not_set(),
        name: ActiveValue::set("乙酯基".to_string()),
        smarts: ActiveValue::set("[CX3](=O)OCC".to_string()),
    }
    .insert(&db)
    .await
    .unwrap()
    .id;
    functional_groups::rematch_groups(&db, &[ethyl_ester]).await.unwrap();

    assert_eq!(
        structure_functional_group_names(&db, ester).await,
        vec!["酯基", "羰基", "乙氧基", "乙酯基"]
    );
    assert_eq!(structure_functional_group_names(&db, benzene).await, vec!["苯环"]);

    delete_functional_group(&db, ethyl_ester).await.unwrap();
    assert_eq!(
        structure_functional_group_names(&db, ester).await,
        vec!["酯基", "羰基", "乙氧基"]
    );
    assert!(
        functional_group::Entity::find_by_id(ethyl_ester)
            .one(&db)
            .await
            .unwrap()
            .is_none()
    );
    close_temp_db(db, path).await;
}

#[tokio::test]
async fn functional_group_search_requires_every_selected_group() {
    let (db, path) = temp_db("functional_groups_search").await;
    let nitrobenzene = insert_structure(&db, "硝基苯", "C6H5NO2", "O=[N+]([O-])c1ccccc1", 0).await;
    let aniline = insert_structure(&db, "苯胺", "C6H7N", "Nc1ccccc1", 0).await;
    let ester = insert_structure(&db, "乙酸乙酯", "C4H8O2", "CCOC(=O)C", 0).await;
    functional_groups::rematch_all(&db).await.unwrap();

    let nitro = functional_group_id(&db, "硝基").await;
    let benzene_ring = functional_group_id(&db, "苯环").await;
    let primary_amine = functional_group_id(&db, "伯胺").await;

    assert!(search_ids_by_functional_groups(&db, &[]).await == vec![nitrobenzene, aniline, ester]);
    assert_eq!(
        search_ids_by_functional_groups(&db, &[nitro]).await,
        vec![nitrobenzene]
    );
    assert_eq!(
        search_ids_by_functional_groups(&db, &[nitro, benzene_ring]).await,
        vec![nitrobenzene]
    );
    assert_eq!(
        search_ids_by_functional_groups(&db, &[benzene_ring]).await,
        vec![nitrobenzene, aniline]
    );
    assert!(search_ids_by_functional_groups(&db, &[nitro, primary_amine])
        .await
        .is_empty());
    // 分组查询也要能正确计数（分页总数来自包一层的 COUNT(*)）
    let paginator = filter_by_functional_groups(structure::Entity::find(), &[benzene_ring]).paginate(&db, 100);
    assert_eq!(paginator.num_items().await.unwrap(), 2);
    close_temp_db(db, path).await;
}
