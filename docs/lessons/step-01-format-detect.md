# Step 01：用魔数识别可执行文件格式

> 模块：A 基础 ｜ 对应提交：`<填提交后的短哈希>` ｜ 测试：✅ 通过 ｜ 上一步：无(这是第一步)

## 0. 一句话目标
把项目拆成"库 + 程序"两部分,并在库里写一个 `detect()` 函数:看文件开头几个字节(魔数),判断它是 ELF / Mach-O / PE 还是认不出来。

## 1. 前置回顾
在正式开始前,我们只搭了个能 `cargo run` 的空项目,并手动读过一个文件的前 16 字节,看到了 `cf fa ed fe`。本步把那次"手动观察"变成**库里一个可测试的函数**,这是整个 `revx` 工具的第一块积木——所有后续解析,都要先知道"这是什么格式"。

## 2. 先写测试(TDD·红)
我们想要的行为:给一段以某魔数开头的字节,`detect` 返回对应的 `Format`。先写测试:

```rust
#[test]
fn detects_macho64() {
    let bytes = [0xcf, 0xfa, 0xed, 0xfe, 0x07, 0x00];
    assert_eq!(detect(&bytes), Format::MachO64);
}
```

此时 `detect` 还没实现。运行 `cargo test` → **编译失败(红)**:

```
error[E0425]: cannot find function `detect` in this scope
  --> src/lib.rs:30:20
   |
30 |         assert_eq!(detect(&bytes), Format::MachO64);
   |                    ^^^^^^ not found in this scope
```

编译器替我们确认:功能确实还不存在。这正是要补的。

## 3. 实现到通过(TDD·绿)
在 `src/lib.rs` 里实现 `detect`:

```rust
pub fn detect(bytes: &[u8]) -> Format {
    if bytes.starts_with(&[0x7f, b'E', b'L', b'F']) {
        Format::Elf
    } else if bytes.starts_with(&[0xcf, 0xfa, 0xed, 0xfe]) {
        Format::MachO64
    } else if bytes.starts_with(&[0xce, 0xfa, 0xed, 0xfe]) {
        Format::MachO32
    } else if bytes.starts_with(&[0x4d, 0x5a]) {
        Format::Pe
    } else {
        Format::Unknown
    }
}
```

逐块看:
- `pub fn detect(bytes: &[u8]) -> Format`:`pub` = 公开(库外可调用);`bytes: &[u8]` = 参数是"一段字节的借用";`-> Format` = 返回一个 `Format`。
- `bytes.starts_with(&[...])`:判断这段字节是否以给定的字节序列开头。返回 `true` / `false`。
- `if / else if / else`:在 Rust 里,`if` 是**表达式**——每个分支"求值"出一个 `Format`,整个 `if` 的值就是函数返回值(所以末尾不需要写 `return`)。
- `b'E'`:字节字面量,等于字符 `E` 的 ASCII 码 `0x45`。所以 `[0x7f, b'E', b'L', b'F']` 就是 ELF 的魔数 `7f 45 4c 46`。

运行 `cargo test` → **✅ 绿**:

```
test tests::detects_macho64 ... ok
test tests::detects_elf ... ok
test tests::unknown_for_random_bytes ... ok
test result: ok. 3 passed; 0 failed
```

再用程序解剖它自己:

```
$ cargo run -- target/debug/reverse
文件: target/debug/reverse (501152 字节)
前16字节: cf fa ed fe 07 00 00 01 03 00 00 00 02 00 00 00
格式: MachO64
```

## 4. 改了哪些文件 / 加了什么
- 新增 `src/lib.rs`:定义 `Format` 枚举、`detect()` 函数,以及 3 个单元测试。
- 改写 `src/main.rs`:改为**调用库**(`use reverse::detect;`),读文件 → 打印字节 → 打印识别出的格式。
- 这就是"库 crate + 程序 crate"的拆分:`lib.rs` 放可复用、可测试的逻辑;`main.rs` 只做命令行壳子。

## 5. 学到的语法 / 技巧
- **`enum`(枚举)**:定义"一个值只能是有限几种之一"的类型。这里 `Format` 只能是 `Elf`/`MachO64`/`MachO32`/`Pe`/`Unknown` 之一。
- **`#[derive(Debug, PartialEq, Eq)]`**:属性宏,让编译器自动生成能力——`Debug` 让 `{:?}` 能打印它;`PartialEq`/`Eq` 让 `assert_eq!` 能比较两个 `Format` 是否相等。
- **`&[u8]`(切片)**:对一段连续字节的"借用",不复制数据、不拥有它。函数只读不改,用切片最合适。
- **`pub`**:可见性修饰符,把类型 / 函数公开给库外使用。
- **`#[cfg(test)] mod tests`**:只有在 `cargo test` 时才编译的测试模块;`use super::*;` 把外层(库)的东西引进来用。

## 6. 语言设计巧思
**用类型让"非法状态不可表达"**。如果用字符串表示格式(`"elf"` / `"macho"`),就可能写错成 `"ELf"` 而编译器毫不知情;用 `enum Format`,拼错一个变体名直接编译报错,且 `match` 时编译器会强制你处理每一种情况(后面会见到)。这是 Rust 的核心哲学之一:**把约束写进类型,让错误在编译期暴露,而不是运行期才崩**。

`&[u8]` 体现了**所有权 / 借用**:`detect` 只需要"看一眼"字节,不需要拥有它们,于是用借用(`&`)。数据的所有者还在调用方,函数用完借用自动归还——既零拷贝又安全。这是 Rust 内存安全模型的日常体现,后面每一步都会用到。

## 7. 领域知识
**魔数(magic number)**:大多数文件格式在开头放一段固定字节当"指纹",操作系统和工具靠它快速识别类型,而不是靠扩展名。逆向第一步永远是"这是什么"。常见值:

| 开头字节 | 格式 | 平台 |
|---|---|---|
| `7f 45 4c 46`(`.ELF`) | ELF | Linux |
| `cf fa ed fe` | Mach-O 64 位 | macOS |
| `ce fa ed fe` | Mach-O 32 位 | macOS(旧) |
| `4d 5a`(`MZ`) | PE | Windows |

注意 macOS 的魔数在文件里是**倒着存**的(小端序):真实数值 `0xFEEDFACF`,存成 `cf fa ed fe`。这个"字节序"坑下一个模块会专门讲。

## 8. 软件设计理念
**单一职责 + 依赖方向**。我们把"识别格式"这一件事独立成 `detect`,放进库里;`main` 只负责"读文件、调库、打印"。逻辑(库)不依赖界面(main),界面依赖逻辑——依赖方向健康,逻辑可被单元测试覆盖,也方便以后被 CLI、GUI、其它程序复用。这正是"库 + 程序"拆分的价值,也是工业级项目的标准骨架。

## 9. 小测验(自测)
1. 为什么把核心逻辑放进 `src/lib.rs`,而不全写在 `main.rs` 里?
2. `detect` 的参数为什么用 `&[u8]`(借用切片),而不是 `Vec<u8>`(拥有的字节数组)?
3. `#[derive(PartialEq)]` 在本步起了什么作用?去掉它,哪行代码会编译不过?
4. 文件开头是 `4d 5a 90 00`,`detect` 返回什么?为什么?
5. 为什么 macOS 的魔数在文件里是 `cf fa ed fe` 而不是 `fe ed fa cf`?

## 10. 参考答案
1. 放进库才能被**单元测试**(测试 crate 能 `use reverse::detect`),也能被其它程序复用;`main.rs` 里的逻辑只能手动运行验证。这是"逻辑与界面分离"。
2. `detect` 只需读取、不需拥有这些字节;用借用 `&[u8]` 零拷贝且不夺走调用方的所有权。而且 `&[u8]` 更通用——`Vec<u8>`、数组 `[u8; N]`、字符串字节都能传进来。
3. `PartialEq` 让 `Format` 能用 `==` 比较,`assert_eq!` 内部正是用 `==`;去掉它,三个测试里的 `assert_eq!(detect(...), Format::X)` 都会编译失败。
4. 返回 `Format::Pe`。因为它以 `4d 5a`(`MZ`)开头,`starts_with(&[0x4d, 0x5a])` 为真,后面的字节不影响判断。
5. 因为 x86/ARM 是**小端序**:存储一个多字节数值时,低位字节排在前面。`0xFEEDFACF` 的最低字节 `CF` 先写,于是文件里看到 `cf fa ed fe`。

## 11. 下一步预告
Step 02:实现一个 `ByteReader` 游标——能从字节流里"按位置、带边界检查"地读出一个字节。这会引出 Rust 处理"可能失败"的方式(`Option`),为后面读 u16/u32(多字节、要管字节序)打基础。
