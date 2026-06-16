# Step 20：统一错误处理与退出码

> 模块：F CLI 产品化 ｜ 对应提交：`<待回填>` ｜ 测试：✅ 通过 ｜ 上一步：[Step 19](step-19-clap-cli.md)

## 0. 一句话目标
让 `revx` 出错时给出**友好的中文提示**(而非难看的调试信息)、写到**标准错误流**、并以**非 0 退出码**结束,使工具既好用又可脚本化。

## 1. 前置回顾
Step 19 我们有了子命令。但错误体验还粗糙:文件不存在时,旧的 `fn main() -> Result<...>` 会打印 `Error: Os { code: 2, kind: NotFound, ... }`——这是给开发者看的 `Debug` 形式,对用户不友好。本步统一错误处理,让 `revx` 像成熟工具一样报错。

## 2. 先写测试(TDD·红)
```rust
#[test]
fn read_file_missing_gives_friendly_error() {
    let e = read_file("/no/such/revx/file/here").unwrap_err();
    assert!(e.contains("无法读取"));   // 错误信息友好且含路径
}
```
`read_file` 不存在 → **红**。(退出码 / stderr 行为靠真实运行验证。)

## 3. 实现到通过(TDD·绿)
### 3.1 顶层 main:跑 run(),统一报错 + 退出码
```rust
fn main() {
    if let Err(e) = run() {
        eprintln!("revx: 错误: {e}");   // 写到 stderr,用 Display(友好)而非 Debug
        std::process::exit(1);          // 非 0 退出码
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.cmd { /* 分派,各处理函数用 ? 向上抛错 */ }
    Ok(())
}
```
- **`main` 不再返回 `Result`**,而是手动处理 `run()` 的错误。这样我们能控制**怎么打印**(`{e}` 用 `Display`,友好)和**退出码**,而不是用默认的 `Debug` 输出。
- **`eprintln!`**:打印到**标准错误流(stderr)**,而非标准输出(stdout)。这样错误信息不会污染正常输出——脚本里 `revx strings foo > out.txt` 时,错误仍显示在终端、不混进 `out.txt`。
- **`std::process::exit(1)`**:非 0 退出码。脚本可用 `if revx info x; then ...` 判断成功与否。

### 3.2 友好的文件读取
```rust
fn read_file(path: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("无法读取 {path}:{e}"))
}
```
- **`.map_err(|e| ...)`**:把底层 io 错误**转换**成我们自己的、带上下文(路径)的友好字符串。原始错误 `No such file or directory` 被包进 `无法读取 /path:...`。
- 返回 `Result<_, String>`,而处理函数返回 `Result<_, Box<dyn Error>>`——`?` 能自动把 `String` 转成 `Box<dyn Error>`(标准库为此提供了转换),所以 `read_file(path)?` 直接可用。

`cargo test` → **✅ 绿(44 passed)**。真实运行:
```
$ revx info /no/such/file
revx: 错误: 无法读取 /no/such/file:No such file or directory (os error 2)
$ echo $?
1
$ revx sections src/main.rs        # 不是 Mach-O
revx: 错误: 无法识别的文件格式(未知魔数)
$ echo $?
1
```
错误友好、有上下文、走 stderr、退出码 1——可脚本化的产品质感。

## 4. 改了哪些文件 / 加了什么
- `src/main.rs`:`main` 拆成 `main`(报错 + 退出码)与 `run`(主逻辑);新增 `read_file` 友好封装;7 处文件读取改用它;新增 1 个测试。

## 5. 学到的语法 / 技巧
- **`fn main()` 手动处理错误**:`if let Err(e) = run() { ... exit(1) }`,自定义报错与退出码。
- **`eprintln!`**:输出到 stderr(对比 `println!` 输出到 stdout)。
- **`std::process::exit(code)`**:以指定退出码结束进程。
- **`.map_err(闭包)`**:转换 `Result` 的错误类型 / 内容,常用来加上下文。
- **`?` 跨错误类型**:`String` / 具体错误 → `Box<dyn Error>` 的自动转换。

## 6. 语言设计巧思
**`Display` vs `Debug`:给人看 vs 给开发者看**。Rust 的两套格式化早在 Step 04 就埋下:`{}`(`Display`)是面向最终用户的友好呈现,`{:?}`(`Debug`)是面向开发者的结构化细节。默认的 `fn main() -> Result` 用 `Debug` 打印错误(`Error: Os {...}`),适合开发但不适合用户。我们改成手动 `eprintln!("{e}")` 用 `Display`,就得到了干净的中文。**同一个错误值,按受众选不同的格式化** ——这正是当初给 `ParseError` 实现 `Display`(Step 04)的回报:那时种的因,这里结的果。

**stdout/stderr 分离是 Unix 哲学的基石**。正常结果走 stdout、诊断信息走 stderr,使工具能被**管道和重定向**正确组合(`revx strings x | grep http` 只过滤真正的字符串输出,错误仍现于终端)。`eprintln!` 与 `println!` 的区分让我们零成本地遵循了这一惯例。

**退出码是程序对外的"返回值"**。`exit(0)` = 成功、非 0 = 失败,是进程间最朴素的契约(`&&`、`||`、CI、脚本全靠它)。我们的工具因此能被自动化编排——这是"可脚本化"的硬性前提。

## 7. 领域知识:命令行工具的错误工程
- **三件套**:友好信息 + stderr + 退出码,是所有专业 CLI(grep、curl、git…)的标配。逆向工具常被串进自动化流水线(批量扫一批样本),这三件套缺一不可:批处理脚本靠退出码判断每个样本是否分析成功,靠 stderr 单独收集错误日志。
- **错误要带上下文**:`无法读取 /path/to/x` 比裸的 `No such file or directory` 有用得多——用户一眼知道是哪个文件出了问题。逆向时一次处理很多文件,定位"哪个文件、哪一步失败"至关重要。我们的 `parse` 系列错误还带 `offset`(Step 04),也是同一思路。
- **不要 panic**:对用户输入的错误(文件不存在、格式不对)应返回 `Result` 并友好报告,而非 `unwrap()` 崩溃(那会打印吓人的 panic 栈)。我们整条链路用 `Result`/`?`,把 panic 留给"真正不该发生的程序 bug"。

## 8. 软件设计理念
**关注点分离:`main`(策略)与 `run`(机制)**。`main` 现在只管"出错了怎么对外表现"(打印格式、退出码)——这是**错误处理策略**;`run` 及各 `cmd_*` 只管"做事,出错就用 `?` 往上抛"——这是**业务机制**。把"如何报告错误"集中到唯一的顶层出口,下面所有代码只需老实地 `?` 传播,无需各自操心打印和退出。**错误产生于各处、报告集中于一处**,是健壮 CLI 的标准骨架。`read_file` 则把"给错误加上下文"这件小事也收拢到一个函数,避免 7 处重复。

## 9. 小测验(自测)
1. 为什么把错误信息用 `eprintln!` 打到 stderr,而不是 `println!` 到 stdout?
2. 旧的 `fn main() -> Result` 打印 `Error: Os {...}`,我们改成 `eprintln!("{e}")` 后变友好了,根本原因是什么(提示:两种格式化)?
3. 退出码非 0 有什么实际用途?谁会去读它?
4. `read_file` 用 `.map_err(...)` 做了什么?为什么要带上 `path`?
5. `read_file` 返回 `Result<_, String>`,而调用它的函数返回 `Result<_, Box<dyn Error>>`,为什么 `read_file(path)?` 能直接用?

## 10. 参考答案
1. 因为 stdout 是程序的**正常输出**,常被重定向 / 管道(`> file`、`| grep`)。错误信息走 stderr 才不会混进正常输出里污染数据,同时仍能显示在终端。这是 Unix"正常结果走 stdout、诊断走 stderr"的惯例,让工具能被正确地组合。
2. 根本原因是用了不同的**格式化 trait**:默认 `main` 返回 `Result` 时用 `Debug`(`{:?}`)打印错误,呈现结构化的开发者细节(`Os { code: 2, ... }`);`{e}` 用 `Display`,呈现我们为错误精心写的友好文本。同一错误值,`Display` 给人看、`Debug` 给开发者看。
3. 退出码是进程成功 / 失败的标准信号:`0` 成功、非 `0` 失败。**脚本、CI、shell 的 `&&`/`||`、批处理编排**都靠读它来决定后续流程(如"分析成功才继续下一步")。这是工具可自动化的前提。
4. `.map_err` 把底层 io 错误**转换并包装**成带路径上下文的友好字符串(`无法读取 {path}:...`)。带上 `path` 是因为用户(尤其批量处理多个文件时)需要知道**具体哪个文件**出了问题,裸的"文件不存在"无法定位。
5. 因为标准库为 `Box<dyn Error>` 实现了 `From<String>`(以及从各种具体错误类型的转换)。`?` 运算符在传播错误时会自动调用这个 `From` 转换,把 `String` 错误转成 `Box<dyn Error>`。所以不同错误类型能经由 `?` 顺畅地向上汇聚。

## 11. 下一步预告
Step 21:**release 构建与 CI 产物**——给 CI 增加一个发布作业:用 `cargo build --release` 产出优化过的可执行文件,并作为构建产物上传。我们会顺带认识 debug 与 release 构建的区别(优化、体积、符号),让 `revx` 具备"可分发"的最后一块拼图。这也是整个课程的收官步。
