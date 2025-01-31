# Chembank

用于存储化合物的属性信息

## 准备项目

### 环境要求：

运行时和编译环境：

- Node.js >= 22.13.1
- Rust >= 1.84.0

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

### 待办列表

- [x] 构建Rust一侧用于数据库交互的函数
  - [x] 创建并初始化数据库
  - [x] 获得总结构数目
  - [x] 检查数据库连接状态
  - [x] 添加/更新/删除/搜索结构以及获取结构详细信息
  - [x] 设置和删除组件关系
  - [x] 设置结构图片
  - [x] 设置结构对应属性
  - [ ] 数据导入
  - [ ] 数据导出
  - [x] 数据库重置
- [ ] 页面功能
  - [ ] 结构检索
    - [x] 基于名称/分子式/SMILES/电荷量检索
    - [ ] 基于属性数值检索
  - [x] 添加/删除结构
  - [x] 编辑结构信息
  - [x] 编辑结构属性信息
  - [x] 修改和编辑结构图片
    - [x] 基于SMILES生成
    - [x] 文件上传
  - [x] 编辑结构组成信息
  - [ ] 数据导入
  - [ ] 数据导出
  - [x] 数据库重置
