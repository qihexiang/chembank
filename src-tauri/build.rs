//! 构建期准备：让随包分发的 CPython 在开发运行与安装后都能被 Windows 加载器找到。
//!
//! `chembank.exe` 通过导入表隐式依赖 `python312.dll`，而 `python312.dll` 又依赖 `vcruntime140*.dll`。
//! 加载器只在 exe 所在目录、系统目录与 PATH 中查找这些 DLL，因此它们必须与 exe 同目录：
//! 安装包由 `tauri.conf.json` 的 `bundle.resources` 保证，开发时由这里复制到 target 目录，
//! 使 `cargo run`、`cargo test` 与安装后的加载路径一致。

use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// 需要与 chembank.exe 同目录的文件，与 `tauri.conf.json` 的 resources 保持一致。
const EXE_COMPANIONS: [&str; 3] = ["python312.dll", "vcruntime140.dll", "vcruntime140_1.dll"];
/// 随包分发的运行时目录。
const RUNTIME_DIR: &str = "python-runtime";

fn main() {
    copy_exe_companions();
    verify_runtime_matches_linked_python();
    tauri_build::build();
}

fn copy_exe_companions() {
    let Some(target) = profile_dir() else {
        println!("cargo:warning=无法定位 target 目录，未复制随包运行时 DLL");
        return;
    };
    for name in EXE_COMPANIONS {
        println!("cargo:rerun-if-changed={name}");
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(name);
        let Ok(source_meta) = fs::metadata(&source) else {
            continue; // 未组装运行时：此时按系统解释器回退，不复制。
        };
        let destination = target.join(name);
        let up_to_date = fs::metadata(&destination)
            .map(|meta| meta.len() == source_meta.len() && modified(&meta) >= modified(&source_meta))
            .unwrap_or(false);
        if up_to_date {
            continue;
        }
        if let Err(error) = fs::copy(&source, &destination) {
            println!(
                "cargo:warning=复制 {name} 到 {} 失败：{error}",
                destination.display()
            );
        }
    }
}

/// 链接进 exe 的 CPython 必须与随包分发的运行时同系列。
/// 否则安装后进程会因为找不到 `python3xy.dll` 而无法启动，且窗口程序没有任何报错，只剩“打不开”。
fn verify_runtime_matches_linked_python() {
    if !Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(RUNTIME_DIR)
        .join(EXE_COMPANIONS[0])
        .is_file()
    {
        return; // 未组装运行时，不做约束。
    }
    let linked = pyo3_build_config::get().version();
    let expected = format!("python{}{}.dll", linked.major, linked.minor);
    if expected == EXE_COMPANIONS[0] {
        return;
    }
    eprintln!(
        "构建中止：Cargo 链接的是 CPython {}.{}（{expected}），而随包分发的运行时是 {}。\n\
         请安装同系列的 Python（例如 3.12）并用 PYO3_PYTHON 指向它，或改用 \
         scripts/stage-python-runtime.mjs 里对应版本的运行时。",
        linked.major,
        linked.minor,
        EXE_COMPANIONS[0]
    );
    std::process::exit(1);
}

/// 当前构建的产物目录（`<target>/<profile>`），来自 `OUT_DIR` = `<target>/<profile>/build/<pkg>-<hash>/out`。
fn profile_dir() -> Option<PathBuf> {
    let out_dir = env::var_os("OUT_DIR")?;
    Path::new(&out_dir).ancestors().nth(3).map(Path::to_path_buf)
}

fn modified(meta: &fs::Metadata) -> SystemTime {
    meta.modified().unwrap_or(UNIX_EPOCH)
}
