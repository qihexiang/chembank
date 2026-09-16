#!/usr/bin/env node
// 组装随安装包分发的 Python 运行时：python-build-standalone 的 CPython + 固定版本的 rdkit、numpy。
//
// 产物：
//   src-tauri/python-runtime/   自包含解释器（Lib、DLLs、python312.dll 等），运行时作为 PYTHONHOME
//   src-tauri/python312.dll     同一份解释器 DLL 的副本，安装后与 chembank.exe 同目录，供 Windows 加载器解析
//
// 两者都会被 tauri.conf.json 的 bundle.resources 打进安装包，用户无需自行安装 Python。
// 已按戳记跳过重复组装；用 --force 强制重建。

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
    copyFileSync,
    existsSync,
    mkdirSync,
    readFileSync,
    readdirSync,
    renameSync,
    rmSync,
    statSync,
    writeFileSync,
} from "node:fs";
import { createReadStream, createWriteStream } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";

const REPO_DIR = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const TAURI_DIR = join(REPO_DIR, "src-tauri");

/** 解释器 DLL 的文件名，必须与 tauri.conf.json 里声明的资源名一致。 */
const DLL_NAME = "python312.dll";
/**
 * 需要与 chembank.exe 同目录的文件：`python312.dll` 由导入表直接引用（加载器只在 exe 目录、
 * 系统目录与 PATH 中查找），`vcruntime140*.dll` 是它的依赖，未装 VC++ 运行库的机器上系统目录里没有。
 */
const EXE_COMPANIONS = [DLL_NAME, "vcruntime140.dll", "vcruntime140_1.dll"];
const RUNTIME_DIR = join(TAURI_DIR, "python-runtime");
const EXE_DLL_PATH = join(TAURI_DIR, DLL_NAME);
const CACHE_DIR = join(REPO_DIR, ".cache", "python-runtime");
const STAMP_PATH = join(RUNTIME_DIR, ".staged.json");
const PBS_TAG = "20260901";
const PBS_ASSET = `cpython-3.12.14+${PBS_TAG}-x86_64-pc-windows-msvc-install_only_stripped.tar.gz`;
const PBS_SHA256 = "7c45c9622400d578709a9b2cddbe8124cc21d382409d9f13406d706d28e31b14";
const PBS_URL = `https://github.com/astral-sh/python-build-standalone/releases/download/${PBS_TAG}/${PBS_ASSET}`;
const RDKIT_VERSION = "2026.3.6";
const NUMPY_VERSION = "2.5.3";

/** 与运行时无关、可以安全删除的目录与文件（相对运行时根目录）。 */
const PRUNE_PATHS = [
    "pythonw.exe",
    "Scripts",
    "include",
    "libs",
    "tcl",
    "Lib/test",
    "Lib/idlelib",
    "Lib/lib2to3",
    "Lib/tkinter",
    "Lib/turtledemo",
    "Lib/ensurepip",
    "Lib/site-packages/pip",
    "Lib/site-packages/pip-25.0.1.dist-info",
    "Lib/site-packages/rdkit-stubs",
];

const STAMP = {
    pbsTag: PBS_TAG,
    asset: PBS_ASSET,
    sha256: PBS_SHA256,
    rdkit: RDKIT_VERSION,
    numpy: NUMPY_VERSION,
};

function log(message) {
    console.log(`[python-runtime] ${message}`);
}

function fail(message) {
    console.error(`[python-runtime] 错误：${message}`);
    process.exit(1);
}

function directorySize(dir) {
    let total = 0;
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
        const path = join(dir, entry.name);
        total += entry.isDirectory() ? directorySize(path) : statSync(path).size;
    }
    return total;
}

function megabytes(bytes) {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function sha256(path) {
    return new Promise((resolvePromise, rejectPromise) => {
        const hash = createHash("sha256");
        const input = createReadStream(path);
        input.on("error", rejectPromise);
        input.on("data", chunk => hash.update(chunk));
        input.on("end", () => resolvePromise(hash.digest("hex")));
    });
}

function isStaged() {
    if (
        !existsSync(STAMP_PATH) ||
        !existsSync(join(RUNTIME_DIR, "Lib", "os.py")) ||
        !EXE_COMPANIONS.every(name => existsSync(join(TAURI_DIR, name)))
    ) {
        return false;
    }
    try {
        return readFileSync(STAMP_PATH, "utf8").trim() === JSON.stringify(STAMP);
    } catch {
        return false;
    }
}

async function download(archive) {
    if (existsSync(archive)) {
        log(`复用缓存归档 ${archive}`);
    } else {
        log(`下载 ${PBS_URL}`);
        const response = await fetch(PBS_URL);
        if (!response.ok || !response.body) {
            fail(`下载失败：HTTP ${response.status} ${response.statusText}`);
        }
        mkdirSync(CACHE_DIR, { recursive: true });
        await pipeline(Readable.fromWeb(response.body), createWriteStream(archive));
    }
    const digest = await sha256(archive);
    if (digest !== PBS_SHA256) {
        rmSync(archive, { force: true });
        fail(`归档校验和不匹配，已删除：期望 ${PBS_SHA256}，实际 ${digest}`);
    }
    log(`校验和通过：${PBS_SHA256.slice(0, 16)}…`);
}

function extract(archive, destination) {
    log("解压归档");
    mkdirSync(destination, { recursive: true });
    // Windows 10 起自带 bsdtar，可解 .tar.gz；git-bash 的 GNU tar 亦可。
    execFileSync("tar", ["-xzf", archive, "-C", destination], { stdio: "inherit" });
}

function installPackages(runtime) {
    log(`安装 rdkit==${RDKIT_VERSION}、numpy==${NUMPY_VERSION}`);
    const python = join(runtime, "python.exe");
    execFileSync(
        python,
        [
            "-m",
            "pip",
            "install",
            "--no-deps",
            "--no-compile",
            "--disable-pip-version-check",
            "--no-warn-script-location",
            `rdkit==${RDKIT_VERSION}`,
            `numpy==${NUMPY_VERSION}`,
        ],
        {
            stdio: "inherit",
            // 宿主环境里的 PYTHON* 变量会干扰这个解释器自身，安装期间一律清空。
            env: { ...process.env, PYTHONHOME: undefined, PYTHONPATH: undefined, PYTHONNOUSERSITE: "1" },
        },
    );
}

function prune(runtime) {
    log("裁剪用不到的目录");
    for (const relative of PRUNE_PATHS) {
        rmSync(join(runtime, relative), { recursive: true, force: true });
    }
    // pip 只用于本次组装，装完即删；版本号随 wheel 变化，按前缀清理。
    const sitePackages = join(runtime, "Lib", "site-packages");
    for (const entry of readdirSync(sitePackages)) {
        if (/^pip-\d/.test(entry)) {
            rmSync(join(sitePackages, entry), { recursive: true, force: true });
        }
    }
}

function compile(runtime) {
    log("预编译字节码");
    execFileSync(join(runtime, "python.exe"), ["-m", "compileall", "-q", "-j", "0", "Lib"], {
        stdio: "inherit",
        cwd: runtime,
        env: { ...process.env, PYTHONHOME: runtime, PYTHONPATH: undefined, PYTHONNOUSERSITE: "1" },
    });
}

function verifyLayout(runtime) {
    const dlls = readdirSync(runtime).filter(name => /^python3\d+\.dll$/.test(name));
    if (dlls.length !== 1 || dlls[0] !== DLL_NAME) {
        fail(`运行时根目录期望只有一个 ${DLL_NAME}，实际为 ${dlls.join("、") || "无"}；请同步更新 tauri.conf.json 与 DLL_NAME`);
    }
    for (const relative of [
        ["Lib", "os.py"],
        ["Lib", "site-packages", "rdkit", "__init__.py"],
        ["Lib", "site-packages", "numpy", "__init__.py"],
        ...EXE_COMPANIONS.map(name => [name]),
    ]) {
        if (!existsSync(join(runtime, ...relative))) {
            fail(`运行时缺少 ${relative.join("/")}，组装不完整`);
        }
    }
}

async function main() {
    const force = process.argv.includes("--force");
    if (!force && isStaged()) {
        log(`已是最新（rdkit ${RDKIT_VERSION} / numpy ${NUMPY_VERSION}），跳过；如需重建请加 --force`);
        return;
    }

    const archive = join(CACHE_DIR, PBS_ASSET);
    const staging = join(TAURI_DIR, "python-runtime.staging");
    rmSync(staging, { recursive: true, force: true });

    await download(archive);
    extract(archive, staging);

    // 归档内的根目录为 python/，其内容即用户级解释器目录。
    const extracted = join(staging, "python");
    if (!existsSync(extracted)) {
        fail(`归档结构异常：找不到 ${extracted}`);
    }
    rmSync(RUNTIME_DIR, { recursive: true, force: true });
    renameSync(extracted, RUNTIME_DIR);
    rmSync(staging, { recursive: true, force: true });

    installPackages(RUNTIME_DIR);
    prune(RUNTIME_DIR);
    compile(RUNTIME_DIR);
    verifyLayout(RUNTIME_DIR);

    copyFileSync(join(RUNTIME_DIR, DLL_NAME), EXE_DLL_PATH);
    for (const name of EXE_COMPANIONS) {
        copyFileSync(join(RUNTIME_DIR, name), join(TAURI_DIR, name));
    }
    writeFileSync(STAMP_PATH, `${JSON.stringify(STAMP)}\n`);

    log(`完成：运行时 ${megabytes(directorySize(RUNTIME_DIR))}（${RUNTIME_DIR}）`);
    for (const name of EXE_COMPANIONS) {
        log(`完成：${name} ${megabytes(statSync(join(TAURI_DIR, name)).size)}（与 chembank.exe 同级）`);
    }
}

await main();
