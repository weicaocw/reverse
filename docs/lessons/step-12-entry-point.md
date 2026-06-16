# Step 12：找到程序入口点(LC_MAIN)

> 模块：C 加载命令/段/节区/符号 ｜ 对应提交：`<待回填>` ｜ 测试：✅ 通过 ｜ 上一步：[Step 11](step-11-sections.md)

## 0. 一句话目标
解析 `LC_MAIN` 加载命令,读出程序的**入口偏移(entryoff)**——`main` 距文件起点多少字节,即"程序从哪开始执行"。文件没有入口(如动态库)时,优雅地返回"无"。

## 1. 前置回顾
Step 09 列命令、Step 10/11 解析段与节区。我们注意到真实文件里有一条 `LC_MAIN`(0x80000028)。它专门记录程序入口——逆向头号问题"代码从哪跑起来"的答案就在这。本步把它读出来。

## 2. 先写测试(TDD·红)
构造一条 `LC_MAIN`(entryoff=0x1234):
```rust
#[test]
fn finds_entry_point_from_lc_main() {
    // ... 头 + 一条 LC_MAIN(entryoff=0x1234) ...
    assert_eq!(parse_entry_point(&v).unwrap(), Some(0x1234));
}
#[test]
fn no_entry_point_when_no_lc_main() {
    assert_eq!(parse_entry_point(&macho_with_one_segment()).unwrap(), None);
}
```
`parse_entry_point` 不存在 → **红**。

## 3. 实现到通过(TDD·绿)
```rust
pub const LC_MAIN: u32 = 0x8000_0028;

pub fn parse_entry_point(bytes: &[u8]) -> Result<Option<u64>, ParseError> {
    let header = parse_macho_header(bytes)?;
    let mut r = ByteReader::new(bytes);
    r.seek(32)...?;
    for _ in 0..header.ncmds {
        let start = r.position();
        let cmd = read_u32_or_eof(&mut r, Endian::Little)?;
        let cmdsize = read_u32_or_eof(&mut r, Endian::Little)?;
        if cmd == LC_MAIN {
            let entryoff = read_u64_or_eof(&mut r, Endian::Little)?; // LC_MAIN 体首字段
            return Ok(Some(entryoff));   // 找到就提前返回
        }
        r.seek(start + cmdsize as usize)...?;
    }
    Ok(None)   // 走完都没找到 → 没有入口
}
```
关键:返回类型是 **`Result<Option<u64>, ParseError>`**——这里有**两层"可能"**:
- 外层 `Result`:解析过程可能**出错**(文件畸形)。
- 内层 `Option`:即便解析成功,也可能**没有**入口(动态库没有 `LC_MAIN`)——`Some(off)` 找到、`None` 没有。

"找到就 `return Ok(Some(...))`,走完循环 `Ok(None)`"是"查找型"函数的经典写法。

`cargo test` → **✅ 绿(26 passed)**。真实文件:
```
$ cargo run -- target/debug/reverse
入口点: entryoff=0x1fe0
```
`0x1fe0` 就是这个程序 `main` 在文件里的偏移——执行从这里起步。

## 4. 改了哪些文件 / 加了什么
- `src/lib.rs`:① 常量 `LC_MAIN`;② `parse_entry_point`;③ 2 个测试。
- `src/main.rs`:打印入口点(或"无")。

## 5. 学到的语法 / 技巧
- **`Result<Option<T>, E>`**:同时表达"可能出错"和"可能没有"两种语义,互不混淆。
- **查找型循环 + 提前 `return`**:命中即返回,未命中走到末尾返回"无"。
- **`pub const LC_MAIN`**:具名常量替代魔法数字。
- **`match Ok(Some(..)) / Ok(None) / Err(..)`**(main 里):对嵌套的 `Result<Option<..>>` 分三种情况处理。

## 6. 语言设计巧思
**用类型精确区分"出错"与"没有"**。很多语言会把这两者混为一谈(比如返回 -1 或 null 既可能表示"出错"也可能表示"没有"),调用者难以分辨。Rust 用 `Result<Option<u64>>` 把它们**正交地**分开:`Err(_)` = 解析失败,`Ok(None)` = 解析成功但确实没有入口,`Ok(Some(x))` = 有入口 x。三种情况在类型层面互斥且穷尽,`main` 里的 `match` 必须把它们都处理掉——不可能"忘了某种情况"。这是 Rust"让非法/遗漏状态无法表达"哲学的又一体现:类型越精确,bug 越无处藏身。

## 7. 领域知识:入口点(entry point)
- **entryoff** 是 `main`(更准确说是运行时启动后调用用户 `main` 的那个起点)相对**文件起点**的偏移。要得到它在内存里的**虚拟地址**,用 `__TEXT` 段的 vmaddr 加上去:`vmaddr(__TEXT) + entryoff`(本例 `0x100000000 + 0x1fe0 = 0x100001fe0`)。
- **历史**:老式 Mach-O 用 `LC_UNIXTHREAD` 直接给出初始寄存器(含起始 PC);现代用 `LC_MAIN` 给一个偏移,更简洁、更适合 ASLR(地址随机化)。逆向时两者都可能遇到。
- **为什么重要**:反汇编 / 调试一个陌生程序,第一步往往就是"跳到入口点开始读"。入口点是静态分析的起跑线,也是动态调试下第一个断点的常选位置。
- 动态库(`MH_DYLIB`)没有 `LC_MAIN`——它不是被"执行"的,而是被"加载"的,入口概念不适用,所以我们返回 `None` 而非报错。

## 8. 软件设计理念
**契约的精确性:返回类型即文档**。`parse_entry_point(...) -> Result<Option<u64>, ParseError>` 这一行签名,把"可能失败、也可能没有、有的话是个 u64"三件事说得明明白白,调用者无需读实现就知道要处理哪些情况。相比"返回 0 表示没有"(那 0 到底是合法 entryoff 还是"没有"?)这种含糊约定,精确类型杜绝了二义性。设计 API 时,**让类型承载完整契约**,比写一堆"注意:返回 0 表示…"的注释可靠得多——注释会过时,类型不会骗人。

## 9. 小测验(自测)
1. `parse_entry_point` 返回 `Result<Option<u64>, ParseError>`,这里的 `Result` 和 `Option` 各自表达什么?为什么要套两层?
2. 动态库没有入口点,函数对它返回什么?为什么这不算"错误"?
3. 已知 entryoff=0x1fe0、`__TEXT` 段 vmaddr=0x100000000,入口的虚拟地址是多少?
4. 函数里"命中 `LC_MAIN` 就 `return`、走到末尾才 `Ok(None)`"这种结构,适合哪类任务?
5. 为什么用 `pub const LC_MAIN: u32 = 0x8000_0028;` 而不是在代码里直接写 `0x80000028`?

## 10. 参考答案
1. `Result` 表达"解析**可能出错**"(文件畸形 → `Err`);`Option` 表达"即使解析成功,入口也**可能不存在**"(`Some` 有 / `None` 无)。套两层是因为这是两件正交的事:出错 ≠ 没有。合在一起会丢失区分。
2. 返回 `Ok(None)`。因为"动态库没有入口"是一种**正常、合法**的情况,不是解析失败;用 `Ok(None)` 表示"成功地确认了它没有入口",而 `Err` 应留给真正的异常(如文件损坏)。
3. `0x100000000 + 0x1fe0 = 0x100001fe0`。entryoff 是相对文件起点的偏移,加上 `__TEXT` 的加载基址得到虚拟地址。
4. 适合"在一堆元素里查找满足条件的第一个"的任务(线性查找)。命中即返回结果、走完仍未命中返回"无",简洁且高效(不必遍历完)。
5. 具名常量更可读(`LC_MAIN` 一眼知道含义,`0x80000028` 不知道)、可复用、且集中定义便于维护;直接写魔法数字会散落各处、含义不明、改起来易漏。

## 11. 下一步预告
Step 13:解析 **LC_SYMTAB** 命令,定位符号表,读出程序里的**符号(函数 / 全局变量的名字)**。符号表涉及"字符串表(string table)+ 符号项数组"的配合——符号项里存的是"名字在字符串表中的偏移",又一次经典的"用偏移做间接引用"。读出符号名后,我们的工具就能列出函数清单,逼近真正的逆向分析能力。这也将是模块 C 的收官步,之后开 PR 复盘。
