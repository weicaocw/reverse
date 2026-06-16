# Step 09：遍历加载命令(load commands)—— 变长记录的循环解析

> 模块：C 加载命令/段/节区/符号 ｜ 对应提交：`3c29bd3` ｜ 测试：✅ 通过 ｜ 上一步：[Step 08](step-08-arch-filetype-enums.md)

## 0. 一句话目标
遍历 Mach-O 文件头之后的所有**加载命令(load commands)**——一串变长记录,逐条读出它的类型(`cmd`)和长度(`cmdsize`),装进一个 `Vec`,并把类型翻译成 `LC_SEGMENT_64` 这样的名字。

## 1. 前置回顾
Step 07/08 我们解析了固定的 32 字节文件头,其中 `ncmds` 告诉我们"后面跟着多少条加载命令"。加载命令才是 Mach-O 的"目录":它们描述了段、节区、依赖的动态库、程序入口等一切。本步迈出关键一步——**学会遍历变长记录**,这是从"读固定头"进阶到"读真实结构"的分水岭。

## 2. 先写测试(TDD·红)
真实命令数据太复杂,我们在测试里**手工构造**一个最小 Mach-O:32 字节头(`ncmds=2`)+ 两条各 16 字节的命令(`LC_SEGMENT_64`、`LC_SYMTAB`):
```rust
fn macho_with_two_load_commands() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&[0xcf,0xfa,0xed,0xfe]);          // magic
    v.extend_from_slice(&0x0100_0007u32.to_le_bytes());   // cputype
    // ... cpusubtype/filetype ...
    v.extend_from_slice(&2u32.to_le_bytes());             // ncmds = 2
    v.extend_from_slice(&32u32.to_le_bytes());            // sizeofcmds = 32
    // ... flags/reserved ...
    v.extend_from_slice(&0x19u32.to_le_bytes());          // LC_SEGMENT_64
    v.extend_from_slice(&16u32.to_le_bytes());            // cmdsize = 16
    v.extend_from_slice(&[0u8; 8]);                       // 填充
    v.extend_from_slice(&0x02u32.to_le_bytes());          // LC_SYMTAB
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&[0u8; 8]);
    v
}

#[test]
fn parses_two_load_commands() {
    let cmds = parse_load_commands(&macho_with_two_load_commands()).unwrap();
    assert_eq!(cmds.len(), 2);
    assert_eq!(cmds[0].name(), "LC_SEGMENT_64");
    assert_eq!(cmds[1].name(), "LC_SYMTAB");
}
```
`parse_load_commands` 还不存在,`cargo test` → **红**:`cannot find function 'parse_load_commands'`。

## 3. 实现到通过(TDD·绿)
### 3.1 给游标加"跳转"能力 `seek`
变长记录意味着"读完这条的头,要按它的长度跳到下一条"。游标需要能跳:
```rust
pub fn seek(&mut self, pos: usize) -> Option<()> {
    if pos > self.data.len() {
        return None;   // 跳出末尾 → 失败
    }
    self.pos = pos;
    Some(())
}
```
注意允许 `pos == len`(正好停在末尾,代表读完了)。越过末尾才算越界。

### 3.2 一条命令的通用头 + 类型名映射
```rust
#[derive(Debug, PartialEq, Eq)]
pub struct LoadCommand {
    pub cmd: u32,
    pub cmdsize: u32,
}
impl LoadCommand {
    pub fn name(&self) -> &'static str {
        match self.cmd {
            0x19 => "LC_SEGMENT_64",
            0x02 => "LC_SYMTAB",
            0x0C => "LC_LOAD_DYLIB",
            0x80000028 => "LC_MAIN",
            // ... 其余若干 ...
            _ => "LC_UNKNOWN",
        }
    }
}
```
所有加载命令**开头都是相同的 8 字节**:`cmd`(类型)+ `cmdsize`(本条总长度)。我们先只解析这通用头(各类命令的私有内容留给后续步骤)。`name()` 复用 Step 08 学过的 `match` 映射(奖励先前所学)。

### 3.3 循环解析变长记录
```rust
pub fn parse_load_commands(bytes: &[u8]) -> Result<Vec<LoadCommand>, ParseError> {
    let header = parse_macho_header(bytes)?;        // 复用 Step 07
    let mut r = ByteReader::new(bytes);
    r.seek(32).ok_or(ParseError::UnexpectedEof { offset: 32 })?;  // 跳过 32 字节头

    let mut cmds = Vec::new();
    for _ in 0..header.ncmds {                        // 正好循环 ncmds 次
        let start = r.position();
        let cmd = read_u32_or_eof(&mut r, Endian::Little)?;
        let cmdsize = read_u32_or_eof(&mut r, Endian::Little)?;
        cmds.push(LoadCommand { cmd, cmdsize });
        let next = start + cmdsize as usize;          // 关键:按本条长度跳到下一条
        r.seek(next).ok_or(ParseError::UnexpectedEof { offset: next })?;
    }
    Ok(cmds)
}
```
逐点讲:
- **`let mut cmds = Vec::new();`**:`Vec` 是**可增长的数组**(类似 Python 的 list)。先建空的,循环里往里 `push`。
- **`for _ in 0..header.ncmds`**:循环 `ncmds` 次。`0..n` 是"从 0 到 n-1"的范围;`_` 表示"我不关心第几次"。
- **`cmds.push(...)`**:把一条命令追加进 `Vec`。
- **`let next = start + cmdsize as usize;`**:**变长记录的精髓**——下一条的起点 = 本条起点 + 本条长度。每条命令长度不同,靠 `cmdsize` 才能跳对。`as usize` 把 `u32` 转成下标类型。
- 每个 `?` / `ok_or` 都防住畸形文件(命令声称的长度超出文件、命令数对不上等)。

`cargo test` → **✅ 绿(21 passed)**。

### 3.4 在真实文件上跑
```
$ cargo run -- target/debug/reverse
加载命令:
  [ 0] LC_SEGMENT_64        (72 字节)
  [ 4] LC_DYLD_INFO_ONLY    (48 字节)
  [ 5] LC_SYMTAB            (24 字节)
  [11] LC_MAIN              (24 字节)
  [12] LC_LOAD_DYLIB        (56 字节)
  ...
```
我们的工具**列出了真实可执行文件的完整命令目录**!`LC_MAIN` 就藏着程序入口地址(Step 12 解开它),`LC_SEGMENT_64` 描述了各个段(Step 10)。

## 4. 改了哪些文件 / 加了什么
- `src/lib.rs`:① 给 `ByteReader` 加 `seek`;② 新增 `struct LoadCommand` + `name()`;③ 新增 `parse_load_commands`;④ 新增测试夹具与 2 个测试。
- `src/main.rs`:解析并列出全部加载命令(序号 + 名字 + 长度)。

## 5. 学到的语法 / 技巧
- **`Vec<T>`**:可增长的动态数组。`Vec::new()` 建空、`.push(x)` 追加、`.len()` 取长度、`cmds[i]` 取第 i 个、`.iter().enumerate()` 带序号遍历。
- **`for _ in 0..n`**:范围循环;`_` 忽略循环变量。
- **`as usize`**:基本数值类型间的显式转换(`u32` → `usize`)。
- **`&'static str`**:指向程序里固定字符串字面量的引用,生命周期是整个程序(`'static`),用作返回固定名字最省事。
- **`.extend_from_slice(&...)` / `u32::to_le_bytes()`**(测试里):把一个数按小端拆成字节、追加到 `Vec`——正是 Step 03 `from_le_bytes` 的逆操作,用来"造"测试数据。

## 6. 语言设计巧思
**`Vec` 与所有权:动态内存却无需手动 free**。`Vec` 在堆上分配、可自动扩容,这在 C 里要手动 `malloc`/`realloc`/`free`,稍不留神就内存泄漏或重复释放。Rust 里 `Vec` **拥有**它的数据,当 `cmds` 离开作用域,其堆内存被自动、确定地释放(这套机制叫 RAII / Drop)——没有垃圾回收器、也没有手动释放,却同样安全。我们 `return Ok(cmds)` 时,所有权干净地**移动**给调用方,数据不复制、也不会被提前释放。这就是 Rust"没有 GC 也内存安全"的日常体现。

**用 `Result` + `?` 守住"不可信的长度字段"**:`cmdsize` 来自文件,恶意样本可能填一个超大值想让你跳到文件外、甚至读越界内存。我们的 `r.seek(next).ok_or(...)?` 把每次跳转都做了边界检查,跳出文件就安全报错。变长解析最危险的地方(信任文件里的长度)被类型系统逼着处理了。

## 7. 领域知识:加载命令(load commands)
加载命令是 Mach-O 的"指令清单",紧跟在文件头后,共 `ncmds` 条、合计 `sizeofcmds` 字节。每条以统一的 8 字节头开始(`cmd` + `cmdsize`),其后是该类型特有的数据。常见类型:
- **LC_SEGMENT_64(0x19)**:定义一个"段"(segment),如 `__TEXT`(代码)、`__DATA`(数据)。段里再含节区(section)。**Step 10/11 要解开它。**
- **LC_MAIN(0x80000028)**:程序入口——记录 `main` 距文件起点的偏移。**Step 12 解开它。**
- **LC_SYMTAB(0x02)**:符号表位置,里面有函数 / 全局变量的名字。**Step 13 解开它。**
- **LC_LOAD_DYLIB(0x0C)**:依赖的动态库(如 `/usr/lib/libSystem.dylib`)。
- 高位带 `0x80000000` 的(如 LC_MAIN)表示"加载器必须理解这条命令,否则拒绝加载"。

"先遍历命令目录、再按需深入每条"正是所有专业工具(`otool -l`、LIEF)的工作方式。我们刚复刻了 `otool -l` 的骨架。

## 8. 软件设计理念
**先建骨架,再填血肉(垂直切片的反面:这里是"统一头先行")**。各类加载命令内容千差万别,但它们**共享同一个 8 字节头**。我们没有一上来就解析每种命令的全部细节,而是先只解析这个**公共头**,拿到一张"命令清单"。这让我们用最小代价获得了完整的全局视图,后续再逐类深入(段、入口、符号),每类是独立的一小步。识别"共性先抽出来"是控制复杂度的关键——否则一开始就陷进十几种命令的细节里,寸步难行。

## 9. 小测验(自测)
1. 为什么遍历加载命令必须依赖每条的 `cmdsize`,而不能像读文件头那样按固定宽度一条条读?
2. `Vec::new()` 创建的 `Vec` 在堆上,程序结束时谁负责释放它的内存?Rust 怎么做到不用手动 free 也不泄漏?
3. `r.seek(next).ok_or(...)?` 这一句防住了什么攻击 / 错误?
4. 循环写成 `for _ in 0..header.ncmds`,这里的 `_` 是什么意思?换成 `for i in 0..header.ncmds` 有区别吗?
5. `LoadCommand::name()` 返回 `&'static str`,这个 `'static` 表示什么?为什么这里能用它?

## 10. 参考答案
1. 因为加载命令是**变长记录**——每条长度不同,由它自己的 `cmdsize` 决定。只有读出 `cmdsize` 才知道下一条从哪开始(`本条起点 + cmdsize`)。文件头是固定 32 字节,所以能按固定宽度读;命令不行。
2. `cmds` 这个 `Vec` 的所有者负责。当它离开作用域(或所有权移动给调用方后,调用方再离开作用域),Rust 自动调用其 `Drop` 释放堆内存——这套 RAII 机制在编译期确定,既不用手动 free、也不会泄漏或重复释放,还没有 GC。
3. 防住"`cmdsize` 是个超大或畸形值,导致跳到文件外 / 读越界"的情况(损坏文件或恶意构造的样本)。`seek` 越过末尾返回 `None`,`ok_or(...)?` 把它变成安全的 `UnexpectedEof` 错误,而不是崩溃或读非法内存。
4. `_` 表示"循环变量我不需要用",只是想循环固定次数。换成 `i` 也能跑,但因为我们没用到序号(下一条位置靠 `cmdsize` 算),编译器会对未使用的 `i` 发警告——用 `_` 更干净。
5. `'static` 表示这个字符串引用在**整个程序运行期间都有效**。因为 `name()` 返回的是写死在程序里的字符串字面量("LC_SEGMENT_64" 等),它们本就常驻于程序的只读数据段,生命周期天然是 `'static`,直接返回即可,无需分配新字符串。

## 11. 下一步预告
Step 10:深入 **LC_SEGMENT_64** 命令——解析出段的名字(`__TEXT`/`__DATA`)、虚拟地址、大小。这会引入"读固定长度的名字字段(16 字节、可能带尾随 0)"和"读 u64 地址",让我们看到程序在内存里的布局蓝图,逼近"代码 / 数据到底放在哪"这个逆向核心问题。
