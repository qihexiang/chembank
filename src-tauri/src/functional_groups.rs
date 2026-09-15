//! 官能团预置词表，以及结构-官能团关联的维护。
//!
//! SMARTS 在结构写入时匹配一次并落表（`structure_functional_groups`），检索直接走索引，
//! 因此官能团词表变化时必须对既有结构重算：新增时回填该官能团，删除时清理其关联。

use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect,
};

use entities::*;

use crate::chemistry;

/// 预置官能团：名称与 SMARTS。
pub const DEFAULT_FUNCTIONAL_GROUPS: [(&str, &str); 35] = [
    ("硝基", "[#6][$([NX3](=O)=O),$([NX3+](=O)[O-])]"),
    ("硝酸酯", "[OX2][NX3+](=O)[O-]"),
    ("硝胺", "[NX3][NX3+](=O)[O-]"),
    ("硝酸根离子", "[$([N+](=O)([O-])[O-])]"),
    ("铵离子", "[NH4+]"),
    ("叠氮", "[$([NX2]=[NX2+]=[NX1-]),$([NX1-][NX2+]#[NX1])]"),
    ("亚硝基", "[#6][NX2]=[OX1]"),
    ("肟", "[#6]=[N][OX2H1]"),
    ("偶氮", "[$([NX2;+0]=[NX2;+0])]"),
    ("氰基", "*-[C;D2]#[N;D1]"),
    ("异氰酸酯", "*-[N;D2]=[C;D2]=[O;D1]"),
    ("异硫氰酸酯", "*-[N;D2]=[C;D2]=[S;D1]"),
    ("羧基", "*-C(=O)[O;D1]"),
    ("酯基", "[$([CX3](=[OX1])[OX2][#6])]"),
    ("酰胺", "[$([CX3](=[OX1])[NX3])]"),
    ("醛基", "[$([CX3H1]=[OX1])]"),
    ("羰基", "[#6]=[OX1]"),
    ("羟基", "[#6][OX2H1]"),
    ("醚键", "[$([OD2]([#6])[#6]);!$([OX2][CX3]=[OX1])]"),
    ("过氧键", "[$([OX2][OX2])]"),
    ("伯胺", "*-[N;D1]"),
    ("芳香胺", "[NX3;+0;!$(N=O);!$(NC=O)][c]"),
    ("磺酸基", "*-[S;D4](=O)(=O)-[O;D1]"),
    ("磺酰胺", "[$([SX4](=[OX1])(=[OX1])[NX3])]"),
    ("磺酰氯", "*-[S;D4](=O)(=O)-[Cl]"),
    ("硫醇", "*-[S;D1]"),
    ("硫醚", "[$([SD2]([#6])[#6])]"),
    ("卤素", "*-[#9,#17,#35,#53]"),
    ("三氟甲基", "*-[C;D4](F)(F)F"),
    ("叔丁基", "*-[C;D4]([C;D1])([C;D1])-[C;D1]"),
    ("环丙基", "*-[C;D3]1-[C;D2]-[C;D2]1"),
    ("端炔", "*-[C;D2]#[C;D1;H]"),
    ("甲氧基", "*-[O;D2]-[C;D1;H3]"),
    ("乙氧基", "*-[O;D2]-[C;D2]-[C;D1;H3]"),
    ("苯环", "c1ccccc1"),
];

/// 词表为空时写入预置官能团，返回本次是否写入。
pub async fn seed<C: ConnectionTrait>(db: &C) -> Result<bool, String> {
    if functional_group::Entity::find()
        .count(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?
        > 0
    {
        return Ok(false);
    }
    let models = DEFAULT_FUNCTIONAL_GROUPS
        .iter()
        .map(|(name, smarts)| functional_group::ActiveModel {
            id: ActiveValue::not_set(),
            name: ActiveValue::set(name.to_string()),
            smarts: ActiveValue::set(smarts.to_string()),
        })
        .collect::<Vec<_>>();
    functional_group::Entity::insert_many(models)
        .exec(db)
        .await
        .map_err(|e| format!("无法写入预置官能团，详细信息\n{:#?}", e))?;
    Ok(true)
}

/// 重算 `entries`（结构 ID 与其 SMILES）的官能团关联，SMILES 为空或无法解析时清空其关联。
pub async fn match_structures<C: ConnectionTrait>(
    db: &C,
    entries: &[(u32, Option<String>)],
) -> Result<(), String> {
    if entries.is_empty() {
        return Ok(());
    }
    let groups = vocabulary(db).await?;
    let targets = entries
        .iter()
        .filter_map(|(id, smiles)| {
            smiles
                .clone()
                .filter(|smiles| !smiles.trim().is_empty())
                .map(|smiles| (*id, smiles))
        })
        .collect::<Vec<_>>();
    let hits = if groups.is_empty() || targets.is_empty() {
        vec![Vec::new(); targets.len()]
    } else {
        let smiles = targets
            .iter()
            .map(|(_, smiles)| smiles.clone())
            .collect::<Vec<String>>();
        tokio::task::spawn_blocking(move || chemistry::matching_functional_groups(&smiles, &groups))
            .await
            .map_err(|e| format!("官能团匹配任务异常结束：{e}"))??
    };
    structure_functional_group::Entity::delete_many()
        .filter(structure_functional_group::Column::StructureId.is_in(entries.iter().map(|(id, _)| *id).collect::<Vec<_>>()))
        .exec(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    insert_associations(db, association_rows(&targets, hits)).await
}

/// 对这些官能团重算全部结构的命中关系。
pub async fn rematch_groups<C: ConnectionTrait>(db: &C, group_ids: &[u32]) -> Result<(), String> {
    if group_ids.is_empty() {
        return Ok(());
    }
    let groups = functional_group::Entity::find()
        .select_only()
        .column(functional_group::Column::Id)
        .column(functional_group::Column::Smarts)
        .filter(functional_group::Column::Id.is_in(group_ids.to_vec()))
        .into_tuple::<(u32, String)>()
        .all(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    structure_functional_group::Entity::delete_many()
        .filter(structure_functional_group::Column::FunctionalGroupId.is_in(group_ids.to_vec()))
        .exec(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    let targets = structure_smiles(db).await?;
    if groups.is_empty() || targets.is_empty() {
        return Ok(());
    }
    let smiles = targets
        .iter()
        .map(|(_, smiles)| smiles.clone())
        .collect::<Vec<String>>();
    let hits = tokio::task::spawn_blocking(move || chemistry::matching_functional_groups(&smiles, &groups))
        .await
        .map_err(|e| format!("官能团匹配任务异常结束：{e}"))??;
    insert_associations(db, association_rows(&targets, hits)).await
}

/// 用库中全部官能团重算全部结构的命中关系。
pub async fn rematch_all<C: ConnectionTrait>(db: &C) -> Result<(), String> {
    let group_ids = functional_group::Entity::find()
        .select_only()
        .column(functional_group::Column::Id)
        .into_tuple::<(u32,)>()
        .all(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?
        .into_iter()
        .map(|(id,)| id)
        .collect::<Vec<_>>();
    rematch_groups(db, &group_ids).await
}

/// 官能团词表（ID 与 SMARTS）。
async fn vocabulary<C: ConnectionTrait>(db: &C) -> Result<Vec<(u32, String)>, String> {
    functional_group::Entity::find()
        .select_only()
        .column(functional_group::Column::Id)
        .column(functional_group::Column::Smarts)
        .into_tuple::<(u32, String)>()
        .all(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))
}

/// 全部结构的（ID，SMILES），跳过没有 SMILES 的记录。
async fn structure_smiles<C: ConnectionTrait>(db: &C) -> Result<Vec<(u32, String)>, String> {
    let stored = structure::Entity::find()
        .select_only()
        .column(structure::Column::Id)
        .column(structure::Column::Smiles)
        .filter(structure::Column::Smiles.is_not_null())
        .into_tuple::<(u32, Option<String>)>()
        .all(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    Ok(stored
        .into_iter()
        .filter_map(|(id, smiles)| {
            smiles
                .filter(|smiles| !smiles.trim().is_empty())
                .map(|smiles| (id, smiles))
        })
        .collect())
}

/// 把 `targets` 与命中的官能团 ID 展平成关联行。
fn association_rows(
    targets: &[(u32, String)],
    hits: Vec<Vec<u32>>,
) -> Vec<structure_functional_group::ActiveModel> {
    targets
        .iter()
        .zip(hits)
        .flat_map(|((structure_id, _), functional_group_ids)| {
            functional_group_ids.into_iter().map(move |functional_group_id| {
                structure_functional_group::ActiveModel {
                    structure_id: ActiveValue::set(*structure_id),
                    functional_group_id: ActiveValue::set(functional_group_id),
                }
            })
        })
        .collect()
}

/// 写入关联行，空集合时什么也不做。
async fn insert_associations<C: ConnectionTrait>(
    db: &C,
    rows: Vec<structure_functional_group::ActiveModel>,
) -> Result<(), String> {
    if rows.is_empty() {
        return Ok(());
    }
    structure_functional_group::Entity::insert_many(rows)
        .exec(db)
        .await
        .map_err(|e| format!("数据库故障，详细信息\n{:#?}", e))?;
    Ok(())
}
