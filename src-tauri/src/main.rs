// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
};

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, Schema,
};
use skip_bom::{BomType, SkipEncodingBom};
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
            export_to_folder,
            import_from_folder,
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

fn write_bom<T: std::io::Write>(w: &mut T) -> std::io::Result<()> {
    w.write_all(&[0xEF, 0xBB, 0xBF])
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
            export_to_folder,
            import_from_folder,
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

#[tokio::test]
async fn write_to_csv() {
    use std::fs::{create_dir, File};
    use std::path::Path;

    let db = Database::connect("sqlite:chembank.db").await.unwrap();
    let base_path = Path::new("./export");
    let _ = create_dir(base_path);
    let structures_path = base_path.join("structures.csv");
    let mut csv_file = File::create(structures_path).unwrap();
    csv_file.write_all(&[0xEF, 0xBB, 0xBF]).unwrap();
    let structures = structure::Entity::find().all(&db).await.unwrap();
    let mut writer = csv::Writer::from_writer(csv_file);
    for record in structures {
        writer.serialize(record).unwrap();
    }
}
