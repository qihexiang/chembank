# Chembank

用于存储化合物的属性信息

## 准备项目

### 环境要求：

运行时和编译环境：

- Node.js >= 22.13.1
- Rust >= 1.84.0
- Python 3.12（仅编译期需要，用于链接内嵌解释器；最终用户无需安装）

VSCode插件：

- Tauri
- rust-analyzer

### 安装依赖

项目使用pnpm管理时，可以使用仓库的`pnpm-lock.yaml`减少解析时间和避免问题。`pnpm install`

### 生成绑定

在`src-tauri/src/main.rs`末尾，找到测试函数`export_bindings`，点击`Run Test`执行该函数后，会生成`src/bindings.ts`文件，包含了所有的调用函数和签名。

> 在Rust一侧添加新的Tauri函数后，需要在`main`函数的`generate_handler!`和`export_bindings`的`collect_types!`中分别添加新的函数名称，然后重新生成绑定。

### 启动项目

`pnpm tauri dev`

开发时若已组装内置运行时（见下节）则直接使用，否则回退到系统 Python，需要系统 Python 3.12 中安装有 rdkit。

### 构建发布

`pnpm tauri build` 会先执行 `pnpm stage-runtime`，由`scripts/stage-python-runtime.mjs`从 python-build-standalone 下载 CPython 3.12，再安装固定版本的 rdkit 与 numpy，组装成`src-tauri/python-runtime/`；随后`tauri.conf.json`中`bundle.resources`会把该目录与解释器 DLL 打进 NSIS 安装包。安装后二者与`chembank.exe`同级，程序通过`PYTHONHOME`指向该目录，最终用户无需安装 Python 或 rdkit。

- 组装结果由`.staged.json`戳记判定，重复构建直接跳过；需要重建时执行`pnpm stage-runtime -- --force`。
- 编译链接的 CPython 由构建机的`PYO3_PYTHON`或 PATH 中的`python`决定，`build.rs`会校验其与随包运行时同系列，不一致时中止构建并提示。
- 运行时目录与解释器 DLL 已加入`.gitignore`，不随仓库分发。

## 示例文件

- `example/ionics`：简单离子化合物库，可以通过软件的导入功能从该目录导入数据库

## 导入导出格式

导出目录中包含四个CSV格式的文件和一个图片目录：

- `structures.csv`：分子基本信息，ID、名称、分子式、SMILES和电荷数
- `properties.csv`：分子属性信息，包含于结构表对应的ID以及其他详细信息
- `components.csv`：分子的子结构关系表
- `functional_groups.csv`：官能团词表（名称与 SMARTS），导入时按此重建词表并重新匹配全部结构
- `images`：图片目录，目录下的子目录名称与结构ID对应，每个子目录内包含对应的图片文件

CSV文件均编码为UTF-8 BOM格式，可以在Excel中打开，如果使用其他仅支持UTF-8编码的程序处理时，应先跳过文件头的BOM标记。

## 待办列表

- [x] 构建Rust一侧用于数据库交互的函数
  - [x] 创建并初始化数据库
  - [x] 获得总结构数目
  - [x] 检查数据库连接状态
  - [x] 添加/更新/删除/搜索结构以及获取结构详细信息
  - [x] 设置和删除组件关系
  - [x] 设置结构图片
  - [x] 设置结构对应属性
  - [x] 数据导入
  - [x] 数据导出
  - [x] 数据库重置
- [ ] 页面功能
  - [ ] 错误信息优化
  - [ ] 结构检索
    - [x] 基于名称/分子式/SMILES/电荷量检索
    - [x] 基于官能团检索（多选，需同时含有全部所选）
    - [ ] 基于属性数值检索
  - [x] 添加/删除结构
  - [x] 编辑结构信息
  - [x] 编辑结构属性信息
  - [x] 修改和编辑结构图片
    - [x] 基于SMILES生成
    - [x] 文件上传
  - [x] 编辑结构组成信息
    - [x] 由SMILES生成时自动拆分并登记片段子结构
    - [x] 一键按电荷数配平阴、阳离子子结构的数目
  - [x] 官能团词表管理（增删 SMARTS、新增后回填、全量重匹配）
  - [x] 数据导入
    - [ ] 进度显示优化
  - [x] 数据导出
    - [ ] 合并导出的结构表和属性表
    - [ ] 进度显示优化
  - [x] 数据库重置
- [x] 发布
  - [x] 安装包内置 Python 与 RDKit 运行时，用户无需配置
  - [x] 数据库改存用户数据目录，与安装目录、启动方式无关
