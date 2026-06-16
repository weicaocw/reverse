# Step 13：解析符号表(LC_SYMTAB)—— 列出函数名

> 模块：C 加载命令/段/节区/符号(收官) ｜ 对应提交：`4b02240` ｜ 测试：✅ 通过 ｜ 上一步：[Step 12](step-12-entry-point.md)

## 0. 一句话目标
解析 `LC_SYMTAB`,读出程序的**符号表**——函数 / 全局变量的名字和地址。核心难点:符号项里存的是"名字在字符串表中的偏移",要做一次**间接查表**。

## 1. 前置回顾
Step 09–12 我们读了命令、段、节区、入口点。`LC_SYMTAB` 指向两张表:**符号表**(定长 16 字节的 `nlist_64` 数组,每项含名字偏移、类型、地址)和**字符串表**(所有名字拼在一起、用 `\0` 分隔的一大块)。符号项不直接存名字,而存"名字在字符串表里的偏移"——这是二进制格式里极常见的"用偏移做间接引用"。读出符号后,工具就能列出函数清单,逼近真正的逆向能力。这是模块 C 收官步。

## 2. 先写测试(TDD·红)
构造一个含 1 个符号 `_main`(value=0x100001fe0)的 Mach-O:`LC_SYMTAB` 给出 symoff/nsyms/stroff/strsize,后面跟一个 `nlist_64` 和字符串表 `"\0_main\0"`:
```rust
#[test]
fn parses_symbol_table() {
    // ... 头 + LC_SYMTAB(symoff=56,nsyms=1,stroff=72,strsize=7)
    //     + nlist_64(n_strx=1, n_value=0x100001fe0) + "\0_main\0" ...
    let syms = parse_symbols(&v).unwrap();
    assert_eq!(syms[0].name, "_main");
    assert_eq!(syms[0].value, 0x1_0000_1fe0);
}
```
注意字符串表第 0 字节是 `\0`(表示"无名"),`_main` 从偏移 1 开始,所以 `n_strx=1`。`parse_symbols` 不存在 → **红**。

## 3. 实现到通过(TDD·绿)
分三步:找命令 → 切字符串表 → 逐个读符号并查名字。
```rust
pub const LC_SYMTAB: u32 = 0x02;
pub struct Symbol { pub name: String, pub value: u64 }

pub fn parse_symbols(bytes: &[u8]) -> Result<Vec<Symbol>, ParseError> {
    // 1. 遍历命令,找到 LC_SYMTAB,读出 4 个 u32 字段
    //    symoff(符号表偏移) nsyms(符号数) stroff(字符串表偏移) strsize(字符串表大小)
    let Some((symoff, nsyms, stroff, strsize)) = symtab else {
        return Ok(Vec::new());   // 没有符号表 → 空
    };

    // 2. 切出字符串表(一整块字节)
    let strtab = bytes.get(stroff as usize .. (stroff+strsize) as usize)
        .ok_or(ParseError::UnexpectedEof { offset: stroff as usize })?;

    // 3. 跳到符号表,逐个读 nlist_64(16 字节),用 n_strx 去字符串表查名字
    let mut sr = ByteReader::new(bytes);
    sr.seek(symoff as usize)...?;
    let mut syms = Vec::new();
    for _ in 0..nsyms {
        let n_strx = read_u32_or_eof(&mut sr, Endian::Little)?;
        sr.read_bytes(4)...?;          // 跳过 n_type+n_sect+n_desc 共 4 字节
        let n_value = read_u64_or_eof(&mut sr, Endian::Little)?;
        let name = cstr_from_strtab(strtab, n_strx as usize);   // 间接查名字
        syms.push(Symbol { name, value: n_value });
    }
    Ok(syms)
}
```
查名字的辅助函数——从字符串表 `off` 处读到下一个 `\0`:
```rust
fn cstr_from_strtab(strtab: &[u8], off: usize) -> String {
    let Some(rest) = strtab.get(off..) else { return String::new(); };
    let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
    String::from_utf8_lossy(&rest[..end]).into_owned()
}
```
- **`nlist_64` 布局**:n_strx(u32 名字偏移)→ n_type(u8)→ n_sect(u8)→ n_desc(u16)→ n_value(u64 地址)。我们要 n_strx 和 n_value,中间 4 字节用 `read_bytes(4)` 跳过。
- **两个游标**:`r` 用来遍历命令找 SYMTAB,`sr` 用来读符号项——它们是各自独立的游标,互不干扰(都只读同一份 `bytes`)。
- **`let Some(..) = .. else { return .. };`**:`let-else` 语法,匹配不上就走 `else` 分支(这里提前返回空表)。

`cargo test` → **✅ 绿(28 passed)**。真实文件:
```
符号: 共 4693 个(有名字 2237 个),前 10 个:
  0x0000000100000b40  __ZN7reverse4main17h18c33da12f55fe7eE
  0x0000000100000af0  __ZN4core4iter6traits8iterator8Iterator7collect...
  ...
```
看到 `__ZN7reverse4main...E` 了吗?**那就是我们自己程序的 `main` 函数**!

## 4. 改了哪些文件 / 加了什么
- `src/lib.rs`:① 常量 `LC_SYMTAB`;② `struct Symbol`;③ 助手 `cstr_from_strtab`;④ `parse_symbols`;⑤ 2 个测试。
- `src/main.rs`:打印符号总数、有名字的数量、前 10 个符号(地址 + 名字)。

## 5. 学到的语法 / 技巧
- **间接引用 / 查表**:符号项存"名字在字符串表的偏移",再去表里取——`cstr_from_strtab(strtab, n_strx)`。
- **`let-else`**:`let Some(x) = opt else { return ... };` 解构失败就提前退出,主流程保持平铺。
- **切片 `bytes.get(a..b)`** 得到字符串表这一整块。
- **两个独立 `ByteReader`** 读同一份数据的不同区域。
- **`.filter(|s| !s.name.is_empty())`**(main 里):过滤掉无名符号。

## 6. 语言设计巧思
**借用让"多个游标看同一份数据"既安全又零拷贝**。`r`、`sr` 和 `strtab` 同时**只读地借用**同一个 `bytes`。Rust 允许任意多个**不可变借用**并存(只读不冲突),所以三者并用毫无问题,且都不复制数据。如果其中有谁想改 `bytes`,编译器会立刻拒绝(可变借用必须独占)。这套"共享只读 / 独占可写"的借用规则,在编译期就保证了不会有"一边读一边被改"的数据竞争——我们享受了 C 指针的零拷贝效率,却没有它的悬空 / 别名风险。

**`from_utf8_lossy` 的宽容再次救场**:符号名理论上是 ASCII,但畸形文件可能塞非法字节;宽容解析保证我们不会因为一个坏符号名就崩掉整个分析。

## 7. 领域知识:符号表与名字修饰(name mangling)
- **符号表(symbol table)**:记录程序里有名字的实体(函数、全局变量)及其地址。它是连接器、调试器、逆向工具的核心信息源——`nm`、`objdump -t`、`otool` 都在读它。
- **字符串表分离**:名字不内嵌在符号项里,而是集中放进字符串表、符号项只存偏移。好处:等长的符号项(16 字节)便于数组式随机访问,变长的名字另置一处、可去重共享。这种"定长记录 + 偏移指向变长数据"的设计在文件格式里无处不在。
- **名字修饰(mangling)**:我们看到的 `__ZN7reverse4main17h...E` 不是原始的 `reverse::main`,而是编译器**修饰**后的名字——把模块路径、泛型参数、哈希等编码进一个唯一字符串(C++ 和 Rust 都这么干,以支持重载 / 泛型 / 避免重名)。逆向时常需**反修饰(demangle)**还原成可读名(`reverse::main`)。`__ZN...E` 是 Itanium C++ ABI 的修饰格式,Rust 复用了它。**这是一个自然的下一步增强点。**
- **符号被剥离(stripped)**:release 版常用 `strip` 去掉符号表以减小体积 / 增加逆向难度——那时 `parse_symbols` 会返回很少甚至 0 个符号,逆向就得靠反汇编硬啃了。

## 8. 软件设计理念
**分阶段解析:定位 → 切块 → 逐项**。`parse_symbols` 清晰地分三步:先在命令流里**定位**符号表元数据(symoff/stroff…),再**切出**字符串表这块原料,最后**逐项**解析并查名字。每步职责单一、依赖前一步的产物,读起来像一条流水线。这种"先拿到坐标、再切原料、再精加工"的结构,比把三件事搅在一个大循环里清晰得多,也更好测试和排错。它再次体现"边界处一次性解析成强类型(`Vec<Symbol>`),内部就只跟干净数据打交道"。

## 9. 小测验(自测)
1. 符号项为什么不直接存名字字符串,而是存"名字在字符串表里的偏移"?这样设计有什么好处?
2. 代码里同时存在 `r`、`sr`、`strtab` 三个对 `bytes` 的借用,为什么编译器允许?如果其中一个要修改 `bytes` 会怎样?
3. 我们看到的符号名 `__ZN7reverse4main...E` 为什么不是 `reverse::main`?要还原成可读名需要做什么?
4. 一个被 `strip` 过的 release 程序,`parse_symbols` 大概会返回什么?这对逆向意味着什么?
5. `let Some((..)) = symtab else { return Ok(Vec::new()); };` 这行在文件没有符号表时做了什么?

## 10. 参考答案
1. 因为符号项是**定长**的(每个 16 字节),便于像数组一样随机访问、计算第 i 个的位置;而名字是**变长**的,塞进定长项里会很别扭。把名字集中到字符串表、符号项只存偏移,既保持符号项定长,又能让变长名字自由存放(还能多个符号共享同一段名字)。这是"定长记录 + 偏移指向变长数据"的经典设计。
2. 因为它们都是**不可变(只读)借用**。Rust 允许同一数据同时存在任意多个不可变借用(只读不会互相干扰)。但若其中一个想**可变借用**去修改 `bytes`,编译器会拒绝——可变借用必须独占,不能与任何其它借用并存。这条规则在编译期消除了"边读边改"的数据竞争。
3. 因为编译器对符号名做了**名字修饰(mangling)**,把模块路径、类型、哈希等编码进去以保证唯一性。还原(demangle)需要按对应的修饰规则(这里是 Itanium/Rust 格式)解码 `__ZN7reverse4main...E` → `reverse::main`,通常用专门的 demangle 库 / 工具完成。
4. 大概返回**很少甚至 0 个**符号(`strip` 移除了符号表 / 字符串表)。这意味着逆向时拿不到函数名等线索,只能靠反汇编机器码、结合字符串、交叉引用等手段硬啃——难度显著增加,这正是 strip 的目的之一。
5. 它在 `symtab` 为 `None`(遍历完所有命令都没找到 LC_SYMTAB)时,**提前返回一个空的符号表** `Ok(Vec::new())`,表示"这个文件没有符号表",而不是报错——没有符号是合法情况(如被 strip 的文件)。

## 11. 下一步预告
**模块 C 到此完成**(命令 → 段 → 节区 → 入口 → 符号),我们会开模块 C 的中英双语 PR 复盘。接着进入**模块 D — 分析工具**:先做 **hex dump**(把任意字节按"地址 + 十六进制 + ASCII"三栏漂亮打印),这是每个逆向工具的标配视图,也会教我们格式化输出与分块迭代的技巧。
