# 课程地图 · 用 Rust 造一个逆向工程工具 `revx`

> 这是一张**活地图**:每完成一小步就更新进度标记。读者随时能看到"我在哪、还剩多少"。

## 我们最终要造什么

`revx` —— 一个**工业级、多功能的逆向工程命令行工具**(对标精简版 `readelf` + `objdump` + `binwalk`)。
给它一个可执行文件,它能:

- 识别文件格式(ELF / Mach-O / PE)
- 解析文件头(架构、位数、字节序、入口地址)
- 列出节区 / 段、符号表
- 十六进制转储(hex dump)、提取可见字符串
- 计算熵值,辅助判断是否加壳 / 加密
- 反汇编 `.text` 机器码 → 汇编指令
- 友好的子命令式 CLI、完善的错误处理、CI 保绿

## 方法论:自顶向下理解,自底向上建造

- **理解**靠自顶向下:先有上面的全貌,拆成下面 7 个模块。
- **建造**靠自底向上:先做最底层、最纯粹、最好测的零件(无依赖、能单元测试),再一层层往上,CLI 与集成放最后。
- 每一步:**先写测试(红)→ 最小实现(绿)→ 写一篇中文课 → 一个聚焦提交**。每个模块完成开一条中英双语 PR。

## 模块与小步

### 模块 A — 基础:字节与格式识别(纯逻辑,无第三方依赖)
- [x] Step 01 — 拆出库 crate;`Format` 枚举 + `detect()` 按魔数识别格式(enum / match / 单元测试)
- [x] Step 02 — `ByteReader` 游标:带边界检查地读 u8(切片、Option、错误处理引子)
- [x] Step 03 — 读 u16 / u32 / u64,处理大小端(字节序、位运算 / from_le_bytes)
- [x] Step 04 — 自定义错误类型 `ParseError`(enum 承载错误、Result、`?` 运算符)

### 模块 CI — 工程化(测试套件就绪后提前引入,之后滚动加层)
- [x] Step 05 — 加 CI:每次 push/PR 自动 build + test
- [x] Step 06 — CI 加 fmt 检查 + clippy 静态检查(`-D warnings` 零警告)
- [ ] (后期)release 构建与产物上传

### 模块 B — 解析文件头(以 Mach-O 为主,你机器的原生格式)
- [x] Step 07 — 解析 Mach-O header:magic / cputype / 文件类型
- [x] Step 08 — 架构与位数映射成可读枚举(x86_64 / arm64 …)
- [ ] (拓展)识别并拆解 fat/通用二进制(0xCAFEBABE);ELF header

### 模块 C — 加载命令 / 段 / 节区 / 符号
- [x] Step 09 — 遍历加载命令(load commands),读出每条的类型与长度
- [x] Step 10 — 解析段(LC_SEGMENT_64),列出段名、虚拟地址、大小
- [x] Step 11 — 列出段内节区(.text/.data…)
- [x] Step 12 — 找到入口地址(LC_MAIN entryoff)
- [x] Step 13 — 解析符号表(LC_SYMTAB),列出函数名

### 模块 D — 分析工具(逆向常用瑞士军刀)
- [x] Step 14 — hex dump(地址 + 十六进制 + ASCII 三栏)
- [x] Step 15 — 提取可见字符串(strings)
- [x] Step 16 — 计算字节熵值,标记疑似加壳区段

### 模块 E — 反汇编(引入第三方 crate)
- [x] Step 17 — 接入反汇编库(iced-x86 / capstone),反汇编一段机器码
- [x] Step 18 — 反汇编 `.text`,输出 `地址: 机器码  汇编`

### 模块 F — CLI 产品化
- [x] Step 19 — 用 clap 做子命令(`info` / `sections` / `strings` / `disasm` …)
- [x] Step 20 — 统一错误处理与退出码、彩色输出
- [ ] Step 21 — release 构建与产物

## 进度

当前:**模块 A ✅ · CI ✅ · 模块 B ✅,模块 C ✅,模块 D ✅,模块 E ✅,进入模块 F(Step 19:clap 子命令 CLI)**。
