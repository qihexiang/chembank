//! 通过 PyO3 内嵌 CPython 调用 RDKit，由 SMILES 解析结构信息。

use std::{
    path::{Path, PathBuf},
    sync::Once,
};

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use serde::Serialize;

/// 内嵌解释器只初始化一次，且必须在 CPython 启动前确定解释器位置。
static PYTHON_INIT: Once = Once::new();

/// 结构式 SVG 的边长（像素）。
const SVG_SIZE: u32 = 300;
/// 氮、氧的原子序数，顺序与 [`element_mass_fractions`] 的返回值一致。
const NITROGEN: u8 = 7;
const OXYGEN: u8 = 8;

/// SMILES 中一个互不连接的片段。
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct Fragment {
    /// 标准化 SMILES，同时作为数据库中的匹配依据。
    pub smiles: String,
    /// 含氢分子式。
    pub formula: String,
    /// 形式电荷数。
    pub formal_charge: i32,
    /// 该片段在分子中出现的数目。
    pub count: u32,
}

/// 由 SMILES 解析得到的结构信息。
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct SmilesInfo {
    /// 含氢分子式，例如 `c1ccccc1` → `C6H6`。
    pub formula: String,
    /// 形式电荷数，例如 `[Na+]` → 1。
    pub formal_charge: i32,
    /// 氮元素质量分数，0–1，无量纲。
    pub n_mass_fraction: f64,
    /// 氧元素质量分数，0–1，无量纲。
    pub o_mass_fraction: f64,
    /// 结构式 SVG。
    pub svg: String,
    /// 分子中互不连接的部分，按出现顺序排列；分子本身只有一个部分时为空。
    pub fragments: Vec<Fragment>,
}

/// 由 SMILES 解析分子式、形式电荷、氮/氧质量分数、结构式 SVG 与片段组成。
pub fn analyze_smiles(smiles: &str) -> Result<SmilesInfo, String> {
    if smiles.trim().is_empty() {
        return Err("SMILES 为空".to_string());
    }
    init_python();
    Python::attach(|py| analyze(py, smiles))
}

fn analyze(py: Python<'_>, smiles: &str) -> Result<SmilesInfo, String> {
    let chem = py
        .import("rdkit.Chem")
        .map_err(|e| format!("无法导入 RDKit，请确认程序使用的 Python 中已安装 rdkit：{e}"))?;
    let mol = parse(py, &chem, smiles)?;
    let (n_mass_fraction, o_mass_fraction) = element_mass_fractions(py, &chem, &mol, smiles)?;
    Ok(SmilesInfo {
        formula: molecular_formula(py, &mol, smiles)?,
        formal_charge: formal_charge(&chem, &mol, smiles)?,
        n_mass_fraction,
        o_mass_fraction,
        svg: structure_svg(py, &mol, smiles)?,
        fragments: fragments(py, &chem, &mol, smiles)?,
    })
}

/// 解析 SMILES；语法错误与结构校验失败分别给出提示。
fn parse<'py>(
    py: Python<'py>,
    chem: &Bound<'py, PyModule>,
    smiles: &str,
) -> Result<Bound<'py, PyAny>, String> {
    let mol = chem
        .getattr("MolFromSmiles")
        .and_then(|parse| parse.call1((smiles,)))
        .map_err(|e| format!("RDKit 解析 SMILES `{smiles}` 时出错：{e}"))?;
    if mol.is_none() {
        return Err(if unsanitized_parse_succeeds(py, chem, smiles).unwrap_or(false) {
            format!("SMILES `{smiles}` 无法通过结构校验（价键、芳香性或电荷不合理）")
        } else {
            format!("SMILES `{smiles}` 语法错误，无法解析")
        });
    }
    Ok(mol)
}

/// 含氢分子式，例如 `c1ccccc1` → `C6H6`。
fn molecular_formula(py: Python<'_>, mol: &Bound<'_, PyAny>, smiles: &str) -> Result<String, String> {
    py.import("rdkit.Chem.rdMolDescriptors")
        .and_then(|descriptors| descriptors.getattr("CalcMolFormula"))
        .and_then(|calc| calc.call1((mol,)))
        .and_then(|formula| formula.extract::<String>())
        .map_err(|e| format!("RDKit 计算 SMILES `{smiles}` 的分子式时出错：{e}"))
}

/// 氮、氧元素的质量分数，按含氢相对分子质量归一。
fn element_mass_fractions(
    py: Python<'_>,
    chem: &Bound<'_, PyModule>,
    mol: &Bound<'_, PyAny>,
    smiles: &str,
) -> Result<(f64, f64), String> {
    let context = |e: PyErr| format!("RDKit 计算 SMILES `{smiles}` 的元素质量分数时出错：{e}");
    let molecular_weight = py
        .import("rdkit.Chem.Descriptors")
        .and_then(|descriptors| descriptors.getattr("MolWt"))
        .and_then(|weight| weight.call1((mol,)))
        .and_then(|weight| weight.extract::<f64>())
        .map_err(context)?;
    let table = chem
        .getattr("GetPeriodicTable")
        .and_then(|table| table.call0())
        .map_err(context)?;
    let atoms = mol.call_method0("GetAtoms").map_err(context)?;
    let mut counts = [0.0f64; 2];
    for atom in atoms.try_iter().map_err(context)? {
        let atomic_number = atom
            .map_err(context)?
            .call_method0("GetAtomicNum")
            .and_then(|number| number.extract::<u8>())
            .map_err(context)?;
        match atomic_number {
            NITROGEN => counts[0] += 1.0,
            OXYGEN => counts[1] += 1.0,
            _ => {}
        }
    }
    let mut fractions = [0.0f64; 2];
    for (index, (count, atomic_number)) in counts.iter().zip([NITROGEN, OXYGEN]).enumerate() {
        let atomic_mass = table
            .getattr("GetAtomicWeight")
            .and_then(|weight| weight.call1((atomic_number,)))
            .and_then(|weight| weight.extract::<f64>())
            .map_err(context)?;
        fractions[index] = count * atomic_mass / molecular_weight;
    }
    Ok((fractions[0], fractions[1]))
}

/// 形式电荷数，例如 `[Na+]` → 1。
fn formal_charge(
    chem: &Bound<'_, PyModule>,
    mol: &Bound<'_, PyAny>,
    smiles: &str,
) -> Result<i32, String> {
    chem.getattr("GetFormalCharge")
        .and_then(|charge| charge.call1((mol,)))
        .and_then(|charge| charge.extract::<i32>())
        .map_err(|e| format!("RDKit 计算 SMILES `{smiles}` 的形式电荷时出错：{e}"))
}

/// 拆分互不连接的片段，按标准化 SMILES 归并统计；只有一个片段时返回空列表。
fn fragments(
    py: Python<'_>,
    chem: &Bound<'_, PyModule>,
    mol: &Bound<'_, PyAny>,
    smiles: &str,
) -> Result<Vec<Fragment>, String> {
    let context = |e: PyErr| format!("RDKit 拆分 SMILES `{smiles}` 的片段时出错：{e}");
    let kwargs = PyDict::new(py);
    kwargs.set_item("asMols", true).map_err(context)?;
    let parts = chem
        .getattr("GetMolFrags")
        .and_then(|frags| frags.call((mol,), Some(&kwargs)))
        .map_err(context)?;
    let parts = parts
        .try_iter()
        .map_err(context)?
        .collect::<PyResult<Vec<_>>>()
        .map_err(context)?;
    if parts.len() < 2 {
        return Ok(Vec::new());
    }
    let canonical = chem.getattr("MolToSmiles").map_err(context)?;
    let mut fragments: Vec<Fragment> = Vec::new();
    for part in parts {
        let part_smiles = canonical
            .call1((&part,))
            .and_then(|smiles| smiles.extract::<String>())
            .map_err(context)?;
        match fragments
            .iter_mut()
            .find(|fragment| fragment.smiles == part_smiles)
        {
            Some(fragment) => fragment.count += 1,
            None => fragments.push(Fragment {
                formula: molecular_formula(py, &part, &part_smiles)?,
                formal_charge: formal_charge(chem, &part, &part_smiles)?,
                smiles: part_smiles,
                count: 1,
            }),
        }
    }
    Ok(fragments)
}

/// 批量把 SMILES 转为标准化形式，无法解析的记为 `None`。
pub fn canonical_smiles_batch(smiles: &[String]) -> Result<Vec<Option<String>>, String> {
    if smiles.is_empty() {
        return Ok(Vec::new());
    }
    init_python();
    Python::attach(|py| {
        let chem = py
            .import("rdkit.Chem")
            .map_err(|e| format!("无法导入 RDKit，请确认程序使用的 Python 中已安装 rdkit：{e}"))?;
        let parse = chem
            .getattr("MolFromSmiles")
            .map_err(|e| format!("RDKit 缺少 MolFromSmiles：{e}"))?;
        let canonical = chem
            .getattr("MolToSmiles")
            .map_err(|e| format!("RDKit 缺少 MolToSmiles：{e}"))?;
        smiles
            .iter()
            .map(|smiles| {
                let mol = parse.call1((smiles.as_str(),)).map_err(|e| {
                    format!("RDKit 解析 SMILES `{smiles}` 时出错：{e}")
                })?;
                if mol.is_none() {
                    return Ok(None);
                }
                canonical
                    .call1((mol,))
                    .and_then(|canonical| canonical.extract::<Option<String>>())
                    .map_err(|e| format!("RDKit 标准化 SMILES `{smiles}` 时出错：{e}"))
            })
            .collect()
    })
}

/// 逐个 SMILES 匹配官能团（ID 与 SMARTS），返回各自命中的官能团 ID。
pub fn matching_functional_groups(
    smiles: &[String],
    groups: &[(u32, String)],
) -> Result<Vec<Vec<u32>>, String> {
    if smiles.is_empty() || groups.is_empty() {
        return Ok(vec![Vec::new(); smiles.len()]);
    }
    init_python();
    Python::attach(|py| {
        let chem = py
            .import("rdkit.Chem")
            .map_err(|e| format!("无法导入 RDKit，请确认程序使用的 Python 中已安装 rdkit：{e}"))?;
        let parse_smarts = chem
            .getattr("MolFromSmarts")
            .map_err(|e| format!("RDKit 缺少 MolFromSmarts：{e}"))?;
        let patterns = groups
            .iter()
            .map(|(id, smarts)| match parse_smarts.call1((smarts.as_str(),)) {
                Ok(pattern) if !pattern.is_none() => Ok((*id, pattern)),
                Ok(_) => Err(format!("官能团 SMARTS `{smarts}` 无法解析，请检查语法")),
                Err(e) => Err(format!("RDKit 解析官能团 SMARTS `{smarts}` 时出错：{e}")),
            })
            .collect::<Result<Vec<_>, String>>()?;
        let parse = chem
            .getattr("MolFromSmiles")
            .map_err(|e| format!("RDKit 缺少 MolFromSmiles：{e}"))?;
        smiles
            .iter()
            .map(|smiles| {
                let mol = parse
                    .call1((smiles.as_str(),))
                    .map_err(|e| format!("RDKit 解析 SMILES `{smiles}` 时出错：{e}"))?;
                // 无法解析的 SMILES 视为不含任何官能团，而不是让整批匹配失败
                if mol.is_none() {
                    return Ok(Vec::new());
                }
                let mut matched = Vec::new();
                for (id, pattern) in &patterns {
                    let hit = mol
                        .call_method1("HasSubstructMatch", (pattern,))
                        .and_then(|hit| hit.extract::<bool>())
                        .map_err(|e| format!("RDKit 匹配 SMILES `{smiles}` 的官能团时出错：{e}"))?;
                    if hit {
                        matched.push(*id);
                    }
                }
                Ok(matched)
            })
            .collect()
    })
}

/// 校验 SMARTS 是否可被 RDKit 编译。
pub fn validate_smarts(smarts: &str) -> Result<(), String> {
    if smarts.trim().is_empty() {
        return Err("SMARTS 不能为空".to_string());
    }
    init_python();
    Python::attach(|py| {
        let pattern = py
            .import("rdkit.Chem")
            .and_then(|chem| chem.getattr("MolFromSmarts"))
            .and_then(|parse| parse.call1((smarts,)))
            .map_err(|e| format!("RDKit 解析 SMARTS `{smarts}` 时出错：{e}"))?;
        if pattern.is_none() {
            return Err(format!("SMARTS `{smarts}` 无法解析，请检查语法"));
        }
        Ok(())
    })
}

/// 二维结构式 SVG，由 RDKit 直接绘制。
fn structure_svg(
    py: Python<'_>,
    mol: &Bound<'_, PyAny>,
    smiles: &str,
) -> Result<String, String> {
    let context = |e: PyErr| format!("RDKit 绘制 SMILES `{smiles}` 的结构式时出错：{e}");
    let drawer = py
        .import("rdkit.Chem.Draw.rdMolDraw2D")
        .and_then(|module| module.getattr("MolDraw2DSVG"))
        .and_then(|svg| svg.call1((SVG_SIZE, SVG_SIZE)))
        .map_err(context)?;
    drawer.call_method1("DrawMolecule", (mol,)).map_err(context)?;
    drawer.call_method0("FinishDrawing").map_err(context)?;
    drawer
        .call_method0("GetDrawingText")
        .and_then(|svg| svg.extract::<String>())
        .map_err(context)
}

/// 跳过结构校验重新解析，用于区分 SMILES 语法错误与结构校验失败。
fn unsanitized_parse_succeeds(
    py: Python<'_>,
    chem: &Bound<'_, PyModule>,
    smiles: &str,
) -> PyResult<bool> {
    let kwargs = PyDict::new(py);
    kwargs.set_item("sanitize", false)?;
    let mol = chem
        .getattr("MolFromSmiles")?
        .call((smiles,), Some(&kwargs))?;
    Ok(!mol.is_none())
}

fn init_python() {
    PYTHON_INIT.call_once(|| {
        if let Some(env) = python_env() {
            // 调用方已显式指定时不做覆盖。
            if std::env::var_os("PYTHONHOME").is_none() {
                std::env::set_var("PYTHONHOME", &env.home);
            }
            if let Some(site_packages) = env.site_packages {
                if std::env::var_os("PYTHONPATH").is_none() {
                    std::env::set_var("PYTHONPATH", site_packages);
                }
            }
        }
        Python::initialize();
    });
}

struct PythonEnv {
    home: PathBuf,
    site_packages: Option<PathBuf>,
}

/// 解释器位置：运行时环境变量优先，其次编译期的 `PYO3_PYTHON`；均不可用时不干预 CPython 的默认查找。
fn python_env() -> Option<PythonEnv> {
    let interpreter = interpreter_path()?;
    let bin_dir = interpreter.parent()?;
    let env_root = bin_dir.parent()?;
    let pyvenv_cfg = env_root.join("pyvenv.cfg");
    // 虚拟环境：`home` 指向基础解释器目录，包目录则在虚拟环境内。
    if pyvenv_cfg.is_file() {
        return Some(PythonEnv {
            home: venv_base_home(&pyvenv_cfg)?,
            site_packages: Some(env_root.join("Lib").join("site-packages")),
        });
    }
    Some(PythonEnv {
        home: bin_dir.to_path_buf(),
        site_packages: None,
    })
}

fn interpreter_path() -> Option<PathBuf> {
    ["CHEMBANK_PYTHON", "PYO3_PYTHON"]
        .into_iter()
        .filter_map(|key| std::env::var_os(key).map(PathBuf::from))
        .chain(option_env!("PYO3_PYTHON").map(PathBuf::from))
        .find(|path| path.is_file())
}

/// 读取 `pyvenv.cfg` 的 `home` 项，即虚拟环境所用基础解释器的安装目录。
fn venv_base_home(pyvenv_cfg: &Path) -> Option<PathBuf> {
    let content = std::fs::read_to_string(pyvenv_cfg).ok()?;
    content
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == "home").then(|| PathBuf::from(value.trim()))
        })
        .filter(|home| home.is_dir())
}

#[cfg(test)]
mod tests {
    use super::{
        analyze_smiles, canonical_smiles_batch, matching_functional_groups, validate_smarts,
        Fragment,
    };

    fn formula(smiles: &str) -> String {
        analyze_smiles(smiles).unwrap().formula
    }

    #[test]
    fn formulas_include_hydrogens() {
        assert_eq!(formula("c1ccccc1"), "C6H6");
        assert_eq!(formula("C"), "CH4");
        assert_eq!(formula("CC(=O)O"), "C2H4O2");
    }

    #[test]
    fn formulas_are_canonical() {
        assert_eq!(formula("c1ccccc1"), formula("C1=CC=CC=C1"));
    }

    #[test]
    fn rejects_invalid_smiles() {
        assert!(analyze_smiles("C1CC").is_err());
        assert!(analyze_smiles("  ").is_err());
    }

    #[test]
    fn reports_formal_charge() {
        assert_eq!(analyze_smiles("[Na+]").unwrap().formal_charge, 1);
        assert_eq!(analyze_smiles("C[N+](C)(C)C").unwrap().formal_charge, 1);
        assert_eq!(analyze_smiles("O=C([O-])[O-]").unwrap().formal_charge, -2);
        assert_eq!(analyze_smiles("C[N+](C)(C)C.[Cl-]").unwrap().formal_charge, 0);
    }

    #[test]
    fn reports_element_mass_fractions() {
        let nitrobenzene = analyze_smiles("O=[N+]([O-])c1ccccc1").unwrap();
        assert_eq!(nitrobenzene.formula, "C6H5NO2");
        assert!((nitrobenzene.n_mass_fraction - 14.007 / 123.111).abs() < 1e-6);
        assert!((nitrobenzene.o_mass_fraction - 2.0 * 15.999 / 123.111).abs() < 1e-6);

        let benzene = analyze_smiles("c1ccccc1").unwrap();
        assert_eq!(benzene.n_mass_fraction, 0.0);
        assert_eq!(benzene.o_mass_fraction, 0.0);
    }

    #[test]
    fn renders_structure_svg() {
        let svg = analyze_smiles("O=[N+]([O-])c1ccccc1").unwrap().svg;
        assert!(svg.contains("<svg"), "未生成 SVG：{svg}");
        assert!(svg.contains("</svg>"), "SVG 未闭合：{svg}");
    }

    #[test]
    fn reports_disconnected_fragments() {
        let fragments = analyze_smiles("[Na+].[Cl-].[Cl-]").unwrap().fragments;
        assert_eq!(
            fragments,
            vec![
                Fragment {
                    smiles: "[Na+]".to_string(),
                    formula: "Na+".to_string(),
                    formal_charge: 1,
                    count: 1,
                },
                Fragment {
                    smiles: "[Cl-]".to_string(),
                    formula: "Cl-".to_string(),
                    formal_charge: -1,
                    count: 2,
                },
            ]
        );
    }

    #[test]
    fn single_part_molecule_has_no_fragments() {
        assert!(analyze_smiles("c1ccccc1").unwrap().fragments.is_empty());
    }

    #[test]
    fn canonicalizes_smiles_batch() {
        let canonical = canonical_smiles_batch(&[
            "C1=CC=CC=C1".to_string(),
            "C1CC".to_string(),
            "O=C([O-])[O-]".to_string(),
        ])
        .unwrap();
        assert_eq!(
            canonical,
            vec![
                Some("c1ccccc1".to_string()),
                None,
                Some("O=C([O-])[O-]".to_string()),
            ]
        );
    }

    fn patterns() -> Vec<(u32, String)> {
        vec![
            (1, "[#6][$([NX3](=O)=O),$([NX3+](=O)[O-])]".to_string()),
            (2, "[OX2][NX3+](=O)[O-]".to_string()),
            (3, "[$([NX2]=[NX2+]=[NX1-]),$([NX1-][NX2+]#[NX1])]".to_string()),
            (4, "c1ccccc1".to_string()),
        ]
    }

    #[test]
    fn matches_functional_groups() {
        let matched = matching_functional_groups(
            &[
                "O=[N+]([O-])c1ccccc1".to_string(),
                "CO[N+](=O)[O-]".to_string(),
                "CN=[N+]=[N-]".to_string(),
                "CCO".to_string(),
            ],
            &patterns(),
        )
        .unwrap();
        assert_eq!(
            matched,
            vec![vec![1, 4], vec![2], vec![3], Vec::<u32>::new()]
        );
    }

    #[test]
    fn matching_functional_groups_skips_unparsable_smiles() {
        assert_eq!(
            matching_functional_groups(
                &["C1CC".to_string(), "c1ccccc1".to_string()],
                &[(4, "c1ccccc1".to_string())],
            )
            .unwrap(),
            vec![Vec::new(), vec![4]]
        );
    }

    #[test]
    fn matching_functional_groups_without_patterns() {
        assert_eq!(
            matching_functional_groups(&["c1ccccc1".to_string()], &[]).unwrap(),
            vec![Vec::<u32>::new()]
        );
    }

    #[test]
    fn matching_functional_groups_rejects_invalid_smarts() {
        assert!(matching_functional_groups(&["C".to_string()], &[(1, "[".to_string())]).is_err());
    }

    #[test]
    fn validate_smarts_rejects_invalid_patterns() {
        assert!(validate_smarts("[N+](=O)[O-]").is_ok());
        assert!(validate_smarts("[").is_err());
        assert!(validate_smarts("   ").is_err());
    }
}
