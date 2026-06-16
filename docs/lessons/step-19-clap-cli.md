# Step 19：用 clap 做子命令式 CLI

> 模块：F CLI 产品化 ｜ 对应提交：`2d14fb0` ｜ 测试：✅ 通过 ｜ 上一步：[Step 18](step-18-disasm-view.md)

## 0. 一句话目标
用 `clap`(Rust 主流命令行库)把 `main` 重构成**子命令**:`revx info`、`sections`、`symbols`、`strings`、`hexdump`、`entropy`、`disasm`,各带参数,并自动生成帮助。

## 1. 前置回顾
模块 A–E 我们攒了一库逆向能力,但 `main` 一直是"读文件 → 把所有信息一股脑全打出来"。真正的工具应该让用户**按需调用**——想看反汇编就 `disasm`,想提字符串就 `strings`。本步用 `clap` 的派生宏把命令行接口声明出来,把工具产品化。

## 2. 先写测试(TDD)——CLI 的"红绿"用结构自检
命令行解析不适合传统单元测试,但 clap 提供了一个**定义自检**,能在测试里验证我们的 CLI 结构没有冲突 / 错误:
```rust
#[test]
fn cli_definition_is_valid() {
    use clap::CommandFactory;
    Cli::command().debug_assert();   // 有重复参数名 / 非法配置会 panic
}
```
"红绿"在这里体现为:CLI 定义写错(如两个参数同名)→ 这个测试 panic;写对 → 通过。再辅以真实运行 `revx --help` 看输出。

## 3. 实现到通过(TDD·绿)
### 3.1 加依赖
```
$ cargo add clap --features derive
```
`derive` 特性开启"用派生宏定义 CLI"的能力。

### 3.2 用派生宏声明命令树
```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "revx", version, about = "一个学习用的逆向工程工具")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 显示文件格式与 Mach-O 头
    Info { path: String },
    /// 提取可见字符串
    Strings {
        path: String,
        #[arg(long, default_value_t = 6)]   // --min,默认 6
        min: usize,
    },
    /// 反汇编 __text
    Disasm {
        path: String,
        #[arg(long, default_value_t = 20)]  // --count,默认 20
        count: usize,
    },
    // ... sections / symbols / hexdump / entropy ...
}
```
- **`#[derive(Parser)]`**:让 `Cli` 结构体自动获得"从命令行参数解析自己"的能力。
- **`#[derive(Subcommand)] enum Cmd`**:**每个枚举变体就是一个子命令**!变体的字段就是该子命令的参数。这是 clap 派生宏的精髓——用类型描述命令行结构。
- **变体上的 `///` 文档注释**:自动变成 `--help` 里该子命令的说明。
- **`#[arg(long, default_value_t = 6)]`**:把字段变成可选参数 `--min`,带默认值。`path: String` 没有 `#[arg]` 则是位置参数。

### 3.3 解析并分派
```rust
fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();              // 解析命令行(失败自动打印错误并退出)
    match cli.cmd {
        Cmd::Info { path } => cmd_info(&path)?,
        Cmd::Strings { path, min } => cmd_strings(&path, min)?,
        Cmd::Disasm { path, count } => cmd_disasm(&path, count)?,
        // ...
    }
    Ok(())
}
```
`Cli::parse()` 一行搞定解析;`match cli.cmd` 按子命令分派到各自的处理函数(每个函数读文件、调用对应的库能力、打印)。

`cargo test` → **✅ 绿(43 passed)**。真实运行:
```
$ revx --help
Commands:
  info      显示文件格式与 Mach-O 头
  sections  列出段与节区
  strings   提取可见字符串
  disasm    反汇编 __text 节区
  ...
$ revx info target/debug/reverse
格式: MachO64
  架构    : X86_64
  入口点  : entryoff=0xe100
$ revx disasm target/debug/reverse --count 4
0x0100000820  55              push rbp
0x0100000821  48 89 e5        mov rbp,rsp
...
```
帮助信息、子命令、参数、默认值——全部由那几个派生宏自动生成。

## 4. 改了哪些文件 / 加了什么
- `Cargo.toml`:`clap`(开 `derive` 特性)。
- `src/main.rs`:重写为 `Cli` + `Cmd` 子命令枚举 + 每个子命令一个 `cmd_*` 处理函数;新增 CLI 自检测试。

## 5. 学到的语法 / 技巧
- **`#[derive(Parser)]` / `#[derive(Subcommand)]`**:派生宏自动生成命令行解析代码。
- **枚举变体即子命令、字段即参数**:用类型结构表达 CLI 结构。
- **`#[command(...)]` / `#[arg(...)]` 属性**:配置命令名、版本、参数(`long`、`default_value_t`)。
- **文档注释 `///` 复用为帮助文本**:一处书写,既是代码文档又是用户帮助。
- **`Cli::parse()`**:入口解析;**`Cli::command().debug_assert()`**:结构自检。

## 6. 语言设计巧思
**派生宏:用类型声明"是什么",让宏生成"怎么做"**。我们没有手写任何参数解析代码(没有 `if args[1] == "info"`、没有手动校验、没有手写帮助文本)。我们只是用 `enum`/`struct` **声明**了命令行"长什么样",`#[derive(Parser)]` 在编译期**生成**了全部解析、校验、帮助、报错逻辑。这是 Rust **宏(尤其派生宏)** 的威力:把"样板代码"自动化,让你专注表达意图。还记得最早 Step 01 的 `#[derive(Debug)]` 吗?那是同一机制的小试牛刀——这里它撑起了整个命令行接口。

**类型驱动的 CLI 还顺带保证了正确性**:`Cmd::Disasm { count: usize }` 里 `count` 是 `usize`,clap 会自动把字符串参数解析成数字、非数字就报错;你在 `match` 里拿到的 `count` 已经是类型安全的 `usize`,无需自己转换和校验。**让类型承载约束**的思想从库内部一路贯穿到了命令行边界。

## 7. 领域知识:逆向工具的命令行设计
- **子命令模式**:成熟工具(`git`、`cargo`、`radare2` 的 `rabin2`、`llvm-objdump` 的各种 flag)普遍用"一个主命令 + 多个子命令/动作"组织功能。逆向工具要做的事很多(看头、列节区、提字符串、反汇编、查符号…),子命令让每件事有独立入口和参数,清晰且可发现(`--help` 自带目录)。
- **对照专业工具**:我们的 `revx info` ≈ `otool -h` + `rabin2 -I`;`sections` ≈ `otool -l`/`size`;`symbols` ≈ `nm`;`strings` ≈ `strings`;`disasm` ≈ `objdump -d`;`entropy` ≈ Detect-It-Easy 的熵图。我们用一个统一 CLI 把这些零散工具的核心功能聚合了起来——这正是"多功能逆向工具"的产品形态。
- **可发现性**:好的 CLI 让用户不看文档也能上手——`--help` 列出所有子命令,每个子命令 `--help` 列出它的参数。clap 自动提供了这一切。

## 8. 软件设计理念
**门面(Facade)+ 分派:统一入口,各司其职**。`main` 成了一个轻薄的**门面**:解析命令、分派给对应的 `cmd_*` 函数,自己不含业务逻辑。每个 `cmd_*` 只负责"读文件 + 调用库 + 打印",而真正的能力全在库里(`parse_segments`、`disassemble_view`…)。这形成清晰的三层:**clap 解析层 → main 分派层 → 库逻辑层**。新增一个子命令,只需加一个枚举变体 + 一个 `cmd_*` 函数,完全不动既有命令——又一次"对扩展开放、对修改封闭"。整个项目"库提供纯逻辑、main 只做 IO 与编排"的原则,在这里收获了最大回报:CLI 重构没有碰任何一行核心逻辑。

## 9. 小测验(自测)
1. `#[derive(Subcommand)] enum Cmd` 里,枚举的"变体"和"字段"分别对应命令行里的什么?
2. 我们没有手写任何参数解析 / 帮助文本代码,这些是从哪来的?这体现了什么机制?
3. `#[arg(long, default_value_t = 6)]` 让 `min` 字段变成命令行里的什么?不加 `#[arg]` 的 `path: String` 又是什么?
4. `Cli::command().debug_assert()` 测试在验证什么?它怎么体现 TDD 的"红"?
5. 新增一个 `revx checksec` 子命令,大致需要改哪些地方?既有子命令会受影响吗?

## 10. 参考答案
1. 每个**枚举变体**对应一个**子命令**(如 `Info` → `revx info`);变体里的**字段**对应该子命令的**参数**(位置参数或带 `#[arg]` 的选项)。用类型结构直接表达命令行结构。
2. 它们由 `#[derive(Parser)]` / `#[derive(Subcommand)]` **派生宏在编译期自动生成**。这体现 Rust 宏(尤其派生宏)的能力:你用类型声明"CLI 长什么样",宏生成解析、校验、帮助、报错的全部样板代码。
3. 让 `min` 成为一个**可选命名参数 `--min`**,带默认值 6(不传就是 6)。而没有 `#[arg]` 的 `path: String` 是**必填的位置参数**(直接跟在子命令后,如 `revx strings 文件路径`)。
4. 它验证我们的 **CLI 定义本身合法**(没有重复参数名、冲突配置等)。如果定义写错,`debug_assert()` 会 panic、测试失败(红);定义正确则通过(绿)。这把"CLI 结构正确性"纳入了自动化测试。
5. 大致需要:① 在 `Cmd` 枚举里加一个 `Checksec { path: String }` 变体(自动获得子命令和帮助);② 在 `main` 的 `match` 里加一条分派;③ 写一个 `cmd_checksec` 处理函数。**既有子命令完全不受影响**——这正是子命令 + 分派结构"对扩展开放、对修改封闭"的好处。

## 11. 下一步预告
Step 20:**打磨 CLI 的健壮性与体验**——统一错误处理(让"文件不存在""不是 Mach-O"等给出友好提示而非难看的调试信息)、规范退出码(成功 0 / 失败非 0,便于脚本判断)、并给关键输出加一点结构。让 `revx` 从"能跑"走向"好用、可脚本化"的产品质感。
