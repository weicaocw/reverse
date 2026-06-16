# Step 08：把裸数字翻译成可读枚举(`Arch` / `FileType`)

> 模块：B 解析文件头 ｜ 对应提交：`<待回填>` ｜ 测试：✅ 通过 ｜ 上一步：[Step 07](step-07-macho-header.md)

## 0. 一句话目标
把 Mach-O 头里的 `cputype`(如 `0x01000007`)和 `filetype`(如 `2`)映射成**可读枚举** `Arch::X86_64`、`FileType::Executable`,让工具输出从"魔法数字"变成"人话"。

## 1. 前置回顾
Step 07 我们解析出了 `MachHeader`,但 `cputype: 0x01000007`、`filetype: 2` 这种输出要查手册才懂。本步在不改解析逻辑的前提下,加一层"翻译",把数字变名字。这是逆向工具可读性的关键一招,也顺带复习 Step 01/03 的 `enum` 与 `match`。

## 2. 先写测试(TDD·红)
```rust
#[test]
fn arch_maps_known_cputypes() {
    assert_eq!(Arch::from_cputype(0x0100_0007), Arch::X86_64);
    assert_eq!(Arch::from_cputype(0x0100_000C), Arch::Arm64);
    assert_eq!(Arch::from_cputype(0x1234), Arch::Other(0x1234)); // 不认识的保留原值
}
#[test]
fn header_exposes_readable_arch_and_filetype() {
    let h = parse_macho_header(&MACHO64_HEADER).unwrap();
    assert_eq!(h.arch(), Arch::X86_64);
    assert_eq!(h.file_type(), FileType::Executable);
}
```
`Arch`/`FileType` 还不存在,`cargo test` → **红**:
```
error[E0433]: cannot find type `Arch` in this scope
error[E0599]: no method named `arch` found for struct `MachHeader`
```

## 3. 实现到通过(TDD·绿)
### 3.1 带"兜底变体"的枚举
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86,
    X86_64,
    Arm,
    Arm64,
    Other(u32),   // 不认识的 cputype,把原值装起来
}
```
重点是最后的 **`Other(u32)`**:一个**带数据的元组变体**。前面几个变体是光秃秃的名字,`Other` 却能装一个 `u32`。这样遇到我们没枚举的 cputype 时,**不丢失原始信息**(把那个数字原样保留),而不是粗暴地归为"未知"或报错。

### 3.2 用 `match` 做映射
```rust
impl Arch {
    pub fn from_cputype(cputype: u32) -> Arch {
        match cputype {
            0x0000_0007 => Arch::X86,
            0x0100_0007 => Arch::X86_64,
            0x0000_000C => Arch::Arm,
            0x0100_000C => Arch::Arm64,
            other => Arch::Other(other),   // 兜底:捕获其余所有值
        }
    }
}
```
- `match cputype { ... }`:按值分支。
- 最后一行 **`other => Arch::Other(other)`**:`other` 是一个"绑定模式",匹配前面没列到的**所有**值,并把那个值绑定到名字 `other` 上,装进 `Other`。这让 `match` **穷尽**了所有 u32(编译器要求 `match` 必须覆盖所有可能)。

`FileType` 同理(`1→Object`、`2→Executable`、`6→Dylib`、`8→Bundle`、其余 `Other(v)`)。

### 3.3 给 `MachHeader` 加便捷方法
```rust
impl MachHeader {
    pub fn arch(&self) -> Arch { Arch::from_cputype(self.cputype) }
    pub fn file_type(&self) -> FileType { FileType::from_u32(self.filetype) }
}
```
让调用方直接 `h.arch()` 拿到可读架构,不必自己记得调 `Arch::from_cputype`。

`cargo test` → **✅ 绿(19 passed)**。`main` 输出:
```
格式: MachO64
  架构      : X86_64
  文件类型  : Executable
  加载命令数: 15
```

## 4. 改了哪些文件 / 加了什么
- `src/lib.rs`:① 新增 `enum Arch` + `Arch::from_cputype`;② 新增 `enum FileType` + `FileType::from_u32`;③ 给 `MachHeader` 加 `arch()`/`file_type()` 方法;④ 新增 3 个测试。
- `src/main.rs`:打印 `h.arch()` / `h.file_type()` 替代裸数字。

## 5. 学到的语法 / 技巧
- **元组变体 `Other(u32)`**:枚举变体可以像 `Some(x)` 那样携带数据(这次是我们自己定义的)。
- **`match` 的绑定/兜底模式 `other => ...`**:用一个变量名捕获"其余所有情况",保证 `match` 穷尽。
- **关联函数 `Arch::from_cputype`**:写在 `impl` 里、不带 `self` 的函数,像"工厂"一样从输入造出枚举值(类似 `Arch` 的构造器)。
- **`impl` 给已有结构体加方法**:`MachHeader` 的 `arch()`/`file_type()`。
- **`{:?}` 打印枚举**:靠 `#[derive(Debug)]`,直接打印出 `X86_64`、`Executable`。

## 6. 语言设计巧思
**`match` 必须穷尽 + 兜底变体 = 既安全又不丢信息**。Rust 强制 `match` 覆盖所有可能值(漏了编译不过),这逼你认真想"还有什么情况没处理"。而 `Other(u32)` 兜底变体是一种优雅的折中:**已知的给好名字,未知的不假装认识、但也不丢弃**——把原始数字留在 `Other` 里,日后排查时仍能看到它到底是多少。

对比两种糟糕做法:① 用一堆 `const` 数字 + `if` 判断(没人逼你处理所有情况,容易漏);② 把未知值直接映射成一个无信息的 `Unknown`(丢了原始数字,排查时抓瞎)。Rust 的"穷尽 match + 带数据变体"两者皆避。这也呼应了 Step 04 错误枚举携带 `offset` 的同一思想:**让类型既分类、又保留细节**。

## 7. 领域知识:cputype 与 filetype 的含义
- **cputype(CPU 架构)**:低位是基础类型(`7`=x86 系,`12`=ARM 系),高位 `0x01000000`(CPU_ARCH_ABI64)表示 64 位。于是 `0x01000007`=x86_64、`0x0100000C`=arm64。Apple Silicon 的二进制就是 arm64。
- **filetype(文件用途)**:`1` MH_OBJECT(编译产生的 `.o` 目标文件)、`2` MH_EXECUTE(可执行程序)、`6` MH_DYLIB(动态库 `.dylib`)、`8` MH_BUNDLE(可加载插件 bundle)。逆向时,先看 filetype 就知道"这是个程序还是个库",分析策略不同。
- 把这些数字翻译成名字,正是 `otool -h`、`readelf -h` 等专业工具做的事——它们的 `Type: EXEC`、`cputype X86_64` 就是这层映射的产物。

## 8. 软件设计理念
**关注点分离:解析层只管"取出字节",解释层负责"赋予意义"**。`parse_macho_header` 只忠实读出 `cputype` 这个 `u32`(不评判);`Arch::from_cputype` 才把它解释成架构。两层分开的好处:解析逻辑保持简单稳定;"数字→名字"的映射表可以独立扩展(以后多支持一种架构,只加一行 `match` 分支,不碰解析代码)。这种"先忠实记录原始数据,再在上层解释"的分层,让系统更容易演进和测试。

## 9. 小测验(自测)
1. `Arch::Other(u32)` 这个变体为什么要携带一个 `u32`?换成没有数据的 `Unknown` 会损失什么?
2. `match` 里最后的 `other => Arch::Other(other)` 起什么作用?如果删掉它,会发生什么?
3. `Arch::from_cputype` 是方法(带 `self`)还是关联函数(不带 `self`)?它更像什么角色?
4. `cputype = 0x0100000C` 是什么架构?怎么从数字看出"64 位 + ARM"?
5. 为什么把"数字→名字"的映射放在单独的 `from_cputype`,而不是直接写进 `parse_macho_header`?

## 10. 参考答案
1. 携带 `u32` 是为了**保留我们没认出的原始 cputype 值**,排查时仍能看到它具体是多少。换成无数据的 `Unknown` 会把"`0x1234`"和"`0x9999`"都抹成同一个"未知",丢失了区分和追溯的能力。
2. 它是**兜底分支**:捕获前面没列出的所有 `u32` 值并装进 `Other`。Rust 要求 `match` 穷尽所有可能,删掉它(又没列全所有 u32)会编译报错"non-exhaustive patterns"(没覆盖所有情况)。
3. 它是**关联函数**(签名是 `from_cputype(cputype: u32)`,不带 `self`)。它更像一个"工厂/构造器":从一个输入值造出一个 `Arch`,用 `Arch::from_cputype(x)` 调用。
4. arm64。低位 `0x0C`(12)是 CPU_TYPE_ARM 的基础类型,高位 `0x01000000` 是 64 位标志,二者相或表示"64 位的 ARM",即 arm64(Apple Silicon)。
5. 分层:`parse_macho_header` 只负责忠实地从字节取出原始数字,保持简单稳定;`from_cputype` 负责"解释"。分开后,映射表能独立扩展(多支持一种架构只改 `match`),且各自更好测试。这是"解析与解释分离"的关注点分离原则。

## 11. 下一步预告
Step 09:Mach-O 头里的 `ncmds`/`sizeofcmds` 告诉我们"后面有多少条加载命令"。下一步开始**遍历加载命令(load commands)**——头之后紧跟的一串变长记录,每条有自己的类型和长度。我们会读出每条命令的类型,为模块 C "列出段 / 节区 / 入口点"铺路。这会引入"变长记录的循环解析",是解析器进阶的核心技能。
