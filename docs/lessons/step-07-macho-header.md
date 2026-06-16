# Step 07：解析 Mach-O 文件头(magic / CPU 类型 / 文件类型)

> 模块：B 解析文件头 ｜ 对应提交：`<待回填>` ｜ 测试：✅ 通过 ｜ 上一步：[Step 06](step-06-ci-fmt-clippy.md)

## 0. 一句话目标
把模块 A 的底层零件(游标 + 字节序 + Result)组装起来,**真正读懂一个 Mach-O 64 位文件头**:解析出 magic、CPU 类型、文件类型、加载命令数量等字段,并在真实可执行文件上跑通。

## 1. 前置回顾
模块 A 我们造好了:`detect`(认格式)、`ByteReader`(安全游标)、`read_u32`(带字节序)、`ParseError`/`Result`(带原因的错误)。它们一直是"零件"。本步第一次把它们**拼成一台能用的机器**——读懂真实文件结构,这正是逆向的核心动作。

Mach-O 64 位文件头是一段**固定布局**的数据:开头 32 字节,正好 8 个连续的 u32 字段。我们要按顺序把它们读出来,装进一个结构体。

## 2. 先写测试(TDD·红)
用一段手工构造的 32 字节头(字段值对应一个真实 x86_64 可执行文件)钉住行为:
```rust
const MACHO64_HEADER: [u8; 32] = [
    0xcf,0xfa,0xed,0xfe, // magic = 0xFEEDFACF
    0x07,0x00,0x00,0x01, // cputype = 0x01000007 (x86_64)
    0x03,0x00,0x00,0x00, // cpusubtype = 3
    0x02,0x00,0x00,0x00, // filetype = 2 (可执行)
    0x10,0x00,0x00,0x00, // ncmds = 16
    0x00,0x01,0x00,0x00, // sizeofcmds = 256
    0x85,0x00,0x20,0x00, // flags
    0x00,0x00,0x00,0x00, // reserved
];

#[test]
fn parses_macho64_header_fields() {
    let h = parse_macho_header(&MACHO64_HEADER).unwrap();
    assert_eq!(h.magic, 0xFEED_FACF);
    assert_eq!(h.cputype, 0x0100_0007);
    assert_eq!(h.filetype, 2);
    assert_eq!(h.ncmds, 16);
    assert_eq!(h.sizeofcmds, 256);
}
```
外加两个边界:截断的头 → `UnexpectedEof`;ELF 开头 → `UnknownFormat`。`parse_macho_header` 还不存在,`cargo test` → **红**:
```
error[E0425]: cannot find function `parse_macho_header` in this scope
```

## 3. 实现到通过(TDD·绿)
### 3.1 用结构体描述"头长什么样"
```rust
#[derive(Debug, PartialEq, Eq)]
pub struct MachHeader {
    pub magic: u32,
    pub cputype: u32,
    pub cpusubtype: u32,
    pub filetype: u32,
    pub ncmds: u32,
    pub sizeofcmds: u32,
    pub flags: u32,
    pub reserved: u32,
}
```
这就是 C 里 `mach_header_64` 的 Rust 版——8 个 u32,字段名一一对应。结构体让"一堆零散字段"变成一个有名字、可整体传递的值。

### 3.2 一个把 `Option` 变 `Result` 的小助手
```rust
fn read_u32_or_eof(r: &mut ByteReader, endian: Endian) -> Result<u32, ParseError> {
    let offset = r.position();
    r.read_u32(endian).ok_or(ParseError::UnexpectedEof { offset })
}
```
- 游标的 `read_u32` 返回 `Option`(读到/没读到)。但本步要的是**带原因的** `Result`。
- **`.ok_or(err)`** 正是桥梁:`Some(x)` → `Ok(x)`;`None` → `Err(err)`。我们顺手用 `r.position()` 记下"在哪个偏移读不动了",失败时就能精确报告位置。

### 3.3 顺序读 8 个字段,组装结构体
```rust
pub fn parse_macho_header(bytes: &[u8]) -> Result<MachHeader, ParseError> {
    if detect(bytes) != Format::MachO64 {   // 复用 Step 01:先确认是 Mach-O 64
        return Err(ParseError::UnknownFormat);
    }
    let mut r = ByteReader::new(bytes);
    Ok(MachHeader {
        magic:      read_u32_or_eof(&mut r, Endian::Little)?,
        cputype:    read_u32_or_eof(&mut r, Endian::Little)?,
        cpusubtype: read_u32_or_eof(&mut r, Endian::Little)?,
        filetype:   read_u32_or_eof(&mut r, Endian::Little)?,
        ncmds:      read_u32_or_eof(&mut r, Endian::Little)?,
        sizeofcmds: read_u32_or_eof(&mut r, Endian::Little)?,
        flags:      read_u32_or_eof(&mut r, Endian::Little)?,
        reserved:   read_u32_or_eof(&mut r, Endian::Little)?,
    })
}
```
- **`if detect(bytes) != Format::MachO64`**:复用模块 A 的 `detect`(还记得吗?这就是"奖励先前所学"——旧零件直接拿来用)。不是 Mach-O 64 就直接 `UnknownFormat`,不浪费力气。
- **游标按顺序读**:`ByteReader` 记着位置,8 次 `read_u32` 自动一个接一个往后挪。`?` 让任何一次读不动都立刻返回带偏移的 EOF 错误。
- **直接在 `MachHeader { ... }` 里填字段**:Rust 会按书写顺序求值,正好对应文件里的字段顺序。

`cargo test` → **✅ 绿(16 passed)**。

### 3.4 在真实文件上跑
让 `main` 在识别到 Mach-O 64 时打印头:
```
$ cargo run -- target/debug/reverse
格式: MachO64
  magic     : 0xfeedfacf
  cputype   : 0x01000007
  filetype  : 2
  加载命令数: 15
  命令总大小: 2000 字节
```
我们的工具**真的读懂了一个真实可执行文件的头**!

## 4. 改了哪些文件 / 加了什么
- `src/lib.rs`:① 新增 `struct MachHeader`(8 字段);② 新增私有助手 `read_u32_or_eof`;③ 新增 `parse_macho_header`;④ 新增 3 个测试 + 1 个测试用常量头。
- `src/main.rs`:识别到 `MachO64` 时调用 `parse_macho_header` 并打印字段。

## 5. 学到的语法 / 技巧
- **多字段 `struct` 建模固定布局**:用结构体一一对应二进制里的字段。
- **`.ok_or(err)`**:把 `Option` 转成 `Result`(`None` → 指定的 `Err`)。
- **`r.position()`**:读取游标当前偏移,用于精确报告出错位置。
- **结构体字面量里调用带 `?` 的表达式**:`field: expr?` 会先求值、失败则整函数返回。
- **`{:#010x}` 格式**:`#` 加 `0x` 前缀,`010` 补齐到 10 位(含前缀),`x` 十六进制 → `0xfeedfacf`。
- **`matches!(err, ParseError::UnexpectedEof { .. })`**:测试里判断"是不是某个变体"而不关心内部字段。

## 6. 语言设计巧思
**"按字段顺序求值"让解析代码读起来像数据布局本身**。`MachHeader { magic: read(...)?, cputype: read(...)?, ... }` 的书写顺序,正好就是字节在文件里的物理顺序——代码即文档。配合游标内部维护位置,我们**不需要手动算偏移**(不像 C 里常见的 `*(u32*)(p+8)` 那样靠手数字节,极易错位)。

更深一层:整条解析链由 `?` 串起来,**任何一步 EOF 都安全短路**,绝不会读到越界内存。对比 C 解析器要么手动检查每次读取(啰嗦易漏)、要么干脆不检查(留下漏洞),Rust 用 `Option`/`Result` + `?` 把"安全"做成了顺手的默认。这就是为什么我们说"用 Rust 写解析器,天然抵抗畸形输入"。

## 7. 领域知识:Mach-O 文件头
Mach-O 是 macOS/iOS 的可执行文件格式。64 位文件头 `mach_header_64` 固定 32 字节,关键字段:
- **magic**:`0xFEEDFACF`(64 位)/ `0xFEEDFACE`(32 位)。文件里小端存成 `cf fa ed fe`。
- **cputype**:CPU 架构。`0x01000007` = x86_64,`0x0100000C` = arm64。高位的 `0x01000000` 是"64 位"标志位。
- **filetype**:文件用途。`2`(MH_EXECUTE)= 可执行程序,`6`(MH_DYLIB)= 动态库,`1`(MH_OBJECT)= 目标文件。
- **ncmds / sizeofcmds**:文件头后面紧跟着 `ncmds` 条"加载命令(load command)",总共 `sizeofcmds` 字节。加载命令描述了段、节区、依赖库、入口点等——**这是模块 C 要啃的下一块硬骨头**。

**真实发现(诚实记录)**:我们拿系统的 `/bin/ls` 试,开头却是 `ca fe ba be`(`0xCAFEBABE`)——这不是普通 Mach-O,而是 **通用 / 胖二进制(fat/universal binary)**:一个文件里打包了多种架构(如 x86_64 + arm64)的 Mach-O,开头用 fat header 索引。我们的 `detect` 目前不认它,于是报"未知格式"。这不是 bug,是**尚未覆盖的真实情况**,已排进后续计划(模块 B 拓展步:识别并拆开 fat 二进制,取出其中某个架构再按本步解析)。逆向真实世界文件,就是不断遇到这种"还有这种情况"并逐个攻克的过程。

## 8. 软件设计理念
**用类型给无结构的字节赋予结构(parse, don't validate)**。输入是 32 个无意义的字节;输出是一个**字段有名字、有类型**的 `MachHeader`。一旦解析成功,后续代码面对的就是 `h.filetype`、`h.ncmds` 这种清晰的东西,而不是"第 12~15 字节"。把"从字节到结构"的脏活集中在一个解析函数里、一次做对,上层就能在干净的数据上工作——这是健壮系统的通用骨架:**边界处解析成强类型,内部只与强类型打交道**。

## 9. 小测验(自测)
1. `parse_macho_header` 为什么一开始先调用 `detect`?去掉这层检查会有什么风险?
2. `.ok_or(ParseError::UnexpectedEof { offset })` 做了什么?为什么需要它?
3. Mach-O 头里 `cputype` = `0x01000007` 代表什么架构?那个 `0x01000000` 高位是什么含义?
4. 我们的工具解析 `/bin/ls` 失败,原因是什么?这是不是一个需要"修复"的 bug?
5. 为什么把 8 个字段读取写在 `MachHeader { ... }` 字面量里就能保证按正确顺序读?

## 10. 参考答案
1. 先 `detect` 确认确实是 Mach-O 64,避免对一个根本不是 Mach-O 的文件瞎解析(那样即使读出 8 个 u32 也是垃圾值)。去掉它,函数会把任意文件的前 32 字节当 Mach-O 头解读,产出有误导性的"看似成功"的结果——明确报 `UnknownFormat` 更诚实、更安全。
2. 它把游标的 `Option<u32>` 转成 `Result<u32, ParseError>`:读到就 `Ok`,读不到(`None`)就变成带偏移的 `Err(UnexpectedEof)`。需要它是因为本函数对外用 `Result` 报告"为什么失败",而底层游标用的是 `Option`,两者之间要有座桥。
3. `0x01000007` = x86_64。低位 `0x07` 是 CPU_TYPE_X86 的基础类型,高位 `0x01000000`(CPU_ARCH_ABI64)是"64 位"标志位,二者相或表示"64 位的 x86",即 x86_64。
4. 因为 `/bin/ls` 是 **fat/通用二进制**,开头是 `0xCAFEBABE` 而非 Mach-O 的 `0xFEEDFACF`,`detect` 不认识于是报 `UnknownFormat`。这不是 bug,而是"尚未支持的真实格式";正确做法是后续新增一步去识别并拆解 fat 二进制,而不是把它当错误。
5. 因为 Rust 按**书写顺序**对结构体字面量的各字段表达式求值,而每个表达式都是"从同一个游标读下一个 u32"。游标内部维护位置、每读一次自动前进,于是书写顺序 = 读取顺序 = 文件里的字段顺序,三者自然对齐。

## 11. 下一步预告
Step 08:`cputype`/`filetype` 现在还是裸数字(`0x01000007`、`2`),对人不友好。我们将把它们映射成**可读枚举**(`Arch::X86_64`、`FileType::Executable`),让输出从 `cputype: 0x01000007` 变成 `架构: x86_64`。这会复习 `match` 与 `enum`,并体会"把魔法数字翻译成有意义的类型"——逆向工具可读性的关键一招。
