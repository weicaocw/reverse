# Step 23:符号反修饰(demangle)—— 让符号名变人话

> 模块：G 扩展 ｜ 对应提交：`<待回填>` ｜ 测试：✅ 通过 ｜ 上一步：[Step 22](step-22-fat-binary.md)

## 0. 一句话目标
把符号表里编译器修饰过的名字(`__ZN7reverse4main17h..E`)还原成人类可读形式(`reverse::main`),让 `symbols` 命令真正好用;同时保留 `--raw` 选项看原始名。

## 1. 前置回顾
Step 13 我们读出了符号表,但名字是 `__ZN7reverse4main17h18c33da12f55fe7eE` 这种"天书"——编译器为支持泛型 / 重载 / 避免重名,把模块路径、类型、哈希**编码(mangle)**进了一个唯一字符串。逆向时要快速看懂"有哪些函数",必须**反修饰(demangle)**还原。本步用成熟的 `rustc-demangle` 库完成它。

## 2. 先写测试(TDD·红)
```rust
#[test]
fn demangles_rust_symbol() {
    let d = demangle_symbol("__ZN7reverse4main17h18c33da12f55fe7eE");
    assert_eq!(d, "reverse::main");
}
#[test]
fn demangle_leaves_plain_name() {
    assert_eq!(demangle_symbol("_main"), "main");   // 非修饰名基本原样
}
```
`demangle_symbol` 不存在 → **红**。

## 3. 实现到通过(TDD·绿)
```rust
pub fn demangle_symbol(name: &str) -> String {
    let stripped = name.strip_prefix('_').unwrap_or(name);   // 去掉 Mach-O 的前导 _
    format!("{:#}", rustc_demangle::demangle(stripped))      // {:#} 省略哈希后缀
}
```
两个关键细节:
- **`strip_prefix('_')`**:Mach-O 给所有符号加了一个前导下划线(C 语言历史惯例),所以文件里是 `__ZN..`,而真正的修饰名是 `_ZN..`。去掉一个 `_` 才能喂给 demangle 库。`strip_prefix` 返回 `Option`(有前缀才去),`.unwrap_or(name)` 表示"没有就用原值"。
- **`format!("{:#}", ...)`**:`rustc_demangle::demangle` 返回一个实现了 `Display` 的类型。普通 `{}` 会带上哈希(`reverse::main::h18c3..`),用 `{:#}`(**alternate** 格式)则省略哈希,得到干净的 `reverse::main`。又一次"同一个值,按需选 `Display` 的不同形态"(对照 Step 20 的 Display vs Debug)。

CLI 里给 `symbols` 加 `--raw` 开关(默认反修饰):
```rust
let name = if raw { s.name.clone() } else { demangle_symbol(&s.name) };
```

`cargo test` → **✅ 绿(48 passed)**。真实文件:
```
$ revx symbols target/debug/reverse --limit 3
  0x...0820  clap_builder::parser::matches::arg_matches::ArgMatches::verify_arg
  0x...08d0  core::option::Option<T>::ok_or_else
$ revx symbols target/debug/reverse --limit 1 --raw
  0x...0820  __ZN12clap_builder6parser7matches11arg_matches10ArgMatches10verify_arg17h..E
```
从天书到人话——`symbols` 命令现在能一眼看出"这个程序里有哪些函数"。

## 4. 改了哪些文件 / 加了什么
- `Cargo.toml`:`rustc-demangle`。
- `src/lib.rs`:`demangle_symbol`;2 个测试。
- `src/main.rs`:`symbols` 默认反修饰,加 `--raw` 开关看原始名。

## 5. 学到的语法 / 技巧
- **`.strip_prefix('_')`**:去掉前缀(返回 `Option`,没前缀返回 `None`)。
- **`.unwrap_or(default)`**:`Option` 为 `None` 时取默认值。
- **`{:#}`(alternate 格式)**:让实现了 `Display` 的类型走"另一种"输出(这里省略哈希)。
- **`bool` 命令行开关**:`#[arg(long)] raw: bool`——`--raw` 出现即 `true`。

## 6. 语言设计巧思
**`{}` vs `{:#}`:同一类型的两种呈现**。Rust 的格式化支持 **alternate** 标志(`#`),让一个类型对"普通"和"详细/另类"两种输出各给一套。`rustc_demangle` 用它区分"带哈希"(`{}`,唯一可区分同名泛型实例)和"不带哈希"(`{:#}`,人读友好)。这和 Step 20 的 `Display` vs `Debug`、Step 14 的 `{:02x}` vs `{:#x}` 一脉相承:Rust 的格式化系统是一套**富表达力的呈现层协议**,同一数据按场景选不同形态,无需写多个函数。

**又一次"防腐层 + 复用成熟库"**。符号修饰规则复杂且多版本(legacy `_ZN..E`、v0 `_R..`),还要处理各种转义(`$LT$`→`<`、`..`→`::`)。我们不自己解析,而用权威的 `rustc-demangle`,并把它包在自己的 `demangle_symbol` 后面(对外只暴露 `&str → String`)。和 Step 17 用 iced-x86 反汇编同理:复杂、有权威实现、自研无收益的部分,用库 + 薄封装。

## 7. 领域知识:名字修饰与反修饰
- **为什么修饰**:链接器的符号表是"扁平"的——同名符号会冲突。但高级语言有命名空间、泛型、重载(`Vec<i32>::push` 和 `Vec<String>::push` 是不同函数)。编译器把这些信息**编码进一个唯一的修饰名**,既避免冲突又保留了还原所需的全部信息。
- **修饰格式**:C++ 和 Rust(legacy)用 Itanium ABI 的 `_ZN<len><name>..E` 结构;Rust 还有更规整的 v0 格式(`_R..`)。`$LT$`/`$GT$`/`$u20$` 等是对 `<`/`>`/空格 等字符的转义。
- **哈希后缀**:Rust legacy 名字末尾的 `17h..E` 是一段哈希,用来区分**单态化**出的不同泛型实例(同一个 `ok_or_else` 被不同类型实例化多份——你在真实输出里看到好几行 `Option<T>::ok_or_else` 正是如此)。给人看时通常省略(`{:#}`)。
- **逆向价值**:demangle 后,符号名直接告诉你**模块结构、用了哪些库、调了哪些泛型**——是快速建立程序全貌的捷径。`c++filt`、`rustfilt`、IDA/Ghidra 的 demangler 都干这事。被 strip 的程序没有符号表,就享受不到这个便利(Step 13 讲过)。

## 8. 软件设计理念
**给"原始 vs 加工"提供开关,默认友好、保留逃生通道**。我们让 `symbols` **默认反修饰**(对绝大多数用户更有用),但提供 `--raw` 让需要原始修饰名的高级用户(比如要精确匹配、或反修饰失败时核对)拿到未加工数据。**默认值服务最常见需求,选项服务专家需求**——是好 CLI / API 的常见取舍。同时反修饰是**纯函数 `&str → String`**,既可单测,也不耦合到符号解析逻辑里(`parse_symbols` 仍返回原始名,反修饰是上层可选加工)。原始数据与呈现加工分离,让两端都灵活。

## 9. 小测验(自测)
1. 为什么要先 `strip_prefix('_')` 再 demangle?Mach-O 的符号名和"真正的修饰名"差在哪?
2. `{:#}` 和 `{}` 对 demangle 结果有什么区别?为什么默认想要 `{:#}`?
3. 符号名末尾的哈希(如 `17h18c3..E`)是干什么用的?为什么同一个 `Option::ok_or_else` 会出现好几次?
4. 我们为什么不自己写 demangle,而用 `rustc-demangle` 库?这和 Step 17 用 iced-x86 是同一种判断吗?
5. `symbols` 默认反修饰、提供 `--raw`,这种"默认友好 + 逃生通道"的设计好在哪?

## 10. 参考答案
1. 因为 Mach-O 按 C 惯例给每个符号加了一个**前导下划线**,所以文件里存的是 `__ZN..`,而 demangle 库期望的真正修饰名是 `_ZN..`。去掉一个 `_` 才能正确反修饰。`strip_prefix` 在有前缀时去掉、没有时(用 `unwrap_or`)保持原样。
2. `{}` 会带上区分泛型实例的**哈希后缀**(`reverse::main::h18c3..`),`{:#}`(alternate)**省略哈希**,得到干净的 `reverse::main`。给人读时默认想要不带哈希的简洁形式,所以用 `{:#}`。
3. 哈希用来**区分单态化(monomorphization)产生的不同泛型实例**。`Option<T>::ok_or_else` 对每种具体 `T` 会被实例化成一份独立机器码,它们修饰名主体相同、靠哈希区分——所以符号表里出现多行同名、哈希不同的条目。
4. 因为修饰规则复杂(多版本、大量转义、边界情况),`rustc-demangle` 是权威且测试充分的实现,自己写既费力又易错、无学习/掌控收益。这与 Step 17 用 iced-x86 反汇编是**同一种工程判断**:复杂且有权威实现的基础设施,用库 + 薄封装,把精力留给核心。
5. 它**默认满足最常见需求**(绝大多数人想看可读名),又**给专家留了逃生通道**(`--raw` 看原始名,用于精确匹配 / 核对 / 反修饰失败时)。既不牺牲易用性,又不丢失能力;且反修饰作为可选的上层加工,不污染底层符号解析。

## 11. 收尾与后续
`revx` 现已是一个相当完整的**多功能逆向工具**:格式识别 / Mach-O 全结构解析 / 胖二进制 / hex / strings / 熵 / 反汇编 / 反修饰符号,配子命令 CLI、友好错误、完整 CI 与发布。剩余可继续的方向(都在现有干净架构上加一块即可):
- **FAT_MAGIC_64**(0xCAFEBABF,offset/size 用 u64);
- **ELF / PE** 多格式解析(我们 `detect` 已能识别,补上各自的头/节区解析);
- **arm64 反汇编**(`/bin/ls` 的 arm64 切片,换 iced-x86 的 ARM 支持或接 capstone);
- **UTF-16 字符串**扫描(Windows 程序);
- **CI 多平台矩阵**(Linux/macOS/Windows 三套产物)。

每一个都是独立的一小步,沿用本课程"红→绿→教学→提交"的同一节奏即可推进。
