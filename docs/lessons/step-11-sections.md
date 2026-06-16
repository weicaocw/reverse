# Step 11：列出段内节区(section)

> 模块：C 加载命令/段/节区/符号 ｜ 对应提交：`<待回填>` ｜ 测试：✅ 通过 ｜ 上一步：[Step 10](step-10-segments.md)

## 0. 一句话目标
进入每个段的内部,循环读出它的 `nsects` 个**节区(section)**,列出节名、虚拟地址、大小——精确定位到 `__text`(代码)、`__cstring`(字符串)等具体数据块。

## 1. 前置回顾
Step 10 我们解析出了段(`__TEXT`/`__DATA`…),并读到每个段的 `nsects`(含几个节区)。段是粗粒度的内存区域,**节区**才是细粒度的内容块:`__TEXT` 段里有 `__text`(机器码)、`__cstring`(只读字符串)、`__const`(常量)等。本步把这层读出来——这直接回答了反汇编的前置问题:"机器码到底在文件 / 内存的哪个位置?"

## 2. 先写测试(TDD·红)
构造一个含 `__TEXT` 段、段内一个 `__text` 节区(addr=0x100000f00、size=0x100、offset=0xf00)的 Mach-O:
```rust
#[test]
fn parses_section_inside_segment() {
    let segs = parse_segments(&macho_with_one_section()).unwrap();
    assert_eq!(segs[0].sections.len(), 1);
    let sec = &segs[0].sections[0];
    assert_eq!(sec.sectname, "__text");
    assert_eq!(sec.addr, 0x1_0000_0f00);
    assert_eq!(sec.offset, 0xf00);
}
```
`Segment` 还没有 `sections` 字段 → **红**(编译错误:no field `sections`)。

## 3. 实现到通过(TDD·绿)
### 3.1 新增 `Section` 结构 + 给 `Segment` 加 `sections` 字段
```rust
pub struct Section {
    pub sectname: String,  // 如 "__text"
    pub segname: String,   // 所属段,如 "__TEXT"
    pub addr: u64,
    pub size: u64,
    pub offset: u32,       // 在文件里的偏移
}
// Segment 新增:
pub sections: Vec<Section>,
```

### 3.2 在段解析里嵌一个"读节区"的内层循环
节区紧跟在段头之后,每个 `section_64` 固定 80 字节。我们在读完段的 9 个字段(已拿到 `nsects`)后,循环 `nsects` 次:
```rust
let mut sections = Vec::new();
for _ in 0..nsects {
    let sectname = cstr16_to_string(r.read_bytes(16)...?);  // 节名
    let segname  = cstr16_to_string(r.read_bytes(16)...?);  // 所属段名
    let addr   = read_u64_or_eof(&mut r, Endian::Little)?;
    let size   = read_u64_or_eof(&mut r, Endian::Little)?;
    let offset = read_u32_or_eof(&mut r, Endian::Little)?;
    for _ in 0..7 {                  // 跳过 align/reloff/nreloc/flags/reserved1..3
        read_u32_or_eof(&mut r, Endian::Little)?;
    }
    sections.push(Section { sectname, segname, addr, size, offset });
}
segs.push(Segment { name, vmaddr, vmsize, fileoff, filesize, nsects, sections });
```
- **内层循环**复用了我们已有的所有原语(`read_bytes`、`cstr16_to_string`、`read_u64_or_eof`),没有新增解析机制——只是"在段循环里再套一层节区循环"。
- `section_64` 布局:sectname[16] → segname[16] → addr(u64) → size(u64) → offset(u32) → align/reloff/nreloc/flags/reserved1/2/3(7 个 u32)。我们只留 addr/size/offset,其余用一个 `for _ in 0..7` 一次跳过。

`cargo test` → **✅ 绿(24 passed)**。真实文件(节选):
```
  __TEXT       vmaddr=0x100000000 vmsize=0x53000 节区数=8
      __text           addr=0x0000000100000820 size=0x48360
      __cstring        addr=0x000000010004a6c0 size=0x1cf1
      __const          addr=0x000000010004c3c0 size=0x36a0
  __DATA       vmaddr=0x100053000 vmsize=0x3000 节区数=9
      __data           addr=0x00000001000553c0 size=0x9d0
      __bss            addr=0x0000000100055ec0 size=0x90
```
**`__text` 就是真正的机器码**(在 0x100000820,长 0x48360)——模块 E 反汇编就拿它开刀。

## 4. 改了哪些文件 / 加了什么
- `src/lib.rs`:① 新增 `struct Section`;② `Segment` 加 `sections: Vec<Section>`;③ `parse_segments` 内嵌节区循环;④ 新增 1 个测试 + 夹具。
- `src/main.rs`:每个段下缩进列出其节区(节名 / addr / size)。

## 5. 学到的语法 / 技巧
- **嵌套 `Vec`**:`Segment` 持有 `Vec<Section>`,形成"段含多个节区"的树状数据。
- **嵌套循环解析**:外层遍历加载命令、内层遍历节区,复用同一批读取原语。
- **`for _ in 0..7 { read...?; }`**:用循环统一跳过一批不关心的定长字段,比写 7 行更紧凑。
- **结构体引用 `&segs[0].sections[0]`**:链式索引取到深层元素的借用。

## 6. 语言设计巧思
**用拥有式嵌套结构(`Vec<Section>` 嵌在 `Segment`)表达层级,所有权树自动管理内存**。`Segment` **拥有**它的 `Vec<Section>`,每个 `Section` 又拥有自己的 `String` 名字。这形成一棵"所有权树":当一个 `Segment` 被释放,它的 `sections`、每个 section 的字符串,会沿着树**自动、递归地**释放——你不写一行清理代码,也不可能漏掉或重复释放。C 里手动管理这种嵌套结构(段数组、每段的节区数组、每个名字的字符串)是内存 bug 的重灾区;Rust 的所有权 + `Drop` 把它变成零负担。这就是"结构即所有权"的威力。

## 7. 领域知识:节区(Section)
段是"粗块"(按内存权限划分),节区是段内"细块"(按内容用途划分)。`__TEXT` 段里常见节区:
- **`__text`**:真正的机器指令——**反汇编的主战场**。
- **`__stubs` / `__stub_helper`**:调用动态库函数的跳板(桩代码),与"延迟绑定(lazy binding)"有关,逆向分析调用外部函数时要懂。
- **`__cstring`**:程序里的 C 字符串字面量(你 `printf("hello")` 里的 "hello" 就在这)——逆向找线索常从这里翻字符串。
- **`__const`**:只读常量。
- **`__unwind_info` / `__eh_frame`**:异常处理 / 栈展开信息。

`__DATA` 段里:`__data`(已初始化全局变量)、`__bss`(未初始化全局,文件里不占空间)、`__got`(全局偏移表,动态链接用)等。

**关键三元组**:每个节区有 `addr`(虚拟地址)、`size`、`offset`(文件偏移)。逆向时,**反汇编器要用 `offset` 从文件里取出字节、用 `addr` 标注这些指令在内存里的地址**。这正是模块 E 的输入。专业工具 `otool -s __TEXT __text`、`objdump -d` 就是据此工作。

## 8. 软件设计理念
**层次化数据建模:让数据结构镜像问题域**。Mach-O 的现实结构就是"文件 → 段 → 节区"的三层树,我们的类型 `Vec<Segment>` / `Segment.sections: Vec<Section>` **一比一地镜像**了它。好的数据建模让代码读起来就是在描述问题本身:`segs[0].sections[2].addr` 直白地表达"第 1 个段的第 3 个节区的地址"。当类型结构贴合领域结构,后续每个操作(打印、查找、反汇编某节区)都顺理成章。这是"让数据结构承载领域知识"的体现——也呼应 Step 07 "parse, don't validate":一次解析成强类型的树,之后都在干净的树上工作。

## 9. 小测验(自测)
1. 段(segment)和节区(section)是什么关系?各自按什么维度划分?
2. `Segment` 拥有 `Vec<Section>`,当一个 `Segment` 被释放时,它的节区和节区里的字符串会怎样?需要你手动清理吗?
3. 反汇编 `__text` 节区时,为什么既需要它的 `offset` 又需要它的 `addr`?
4. 解析节区的内层循环里 `for _ in 0..7 { read...?; }` 在做什么?为什么是 7 次?
5. 逆向时想快速找程序里出现的可见字符串,应该重点看哪个节区?

## 10. 参考答案
1. 段是粗粒度的内存区域,按**访问权限**划分(如 `__TEXT` 可执行只读、`__DATA` 可读写);节区是段**内部**更细的划分,按**内容用途**划分(代码 `__text`、字符串 `__cstring`、数据 `__data`…)。一个段含 0 到多个节区。
2. 它们会沿所有权树**自动、递归释放**:`Segment` 释放时,它拥有的 `Vec<Section>` 释放,每个 `Section` 拥有的 `String` 也释放。**完全不用手动清理**,也不会泄漏或重复释放——这是 Rust 所有权 + `Drop` 的保证。
3. `offset` 是节区数据在**文件**里的位置,反汇编器要用它从文件字节中取出机器码;`addr` 是这些指令装载到**内存**后的虚拟地址,用于给反汇编输出标注正确的地址、解析跳转 / 调用目标。两者一个管"从哪取字节"、一个管"标成什么地址"。
4. 它一次性跳过节区结构里我们不关心的 7 个 `u32` 字段(align、reloff、nreloc、flags、reserved1/2/3)。用循环读 7 次(每次推进 4 字节)把游标推到本节区末尾,以便读下一个节区。
5. 重点看 **`__cstring`** 节区(以及 `__TEXT` 段里其它字符串相关节区),程序里的 C 字符串字面量大多在那里——这也是模块 D "提取字符串(strings)" 会用到的。

## 11. 下一步预告
Step 12:解析 **LC_MAIN** 命令,读出程序**入口点(entry point)**——`main` 函数距文件起点的偏移(`entryoff`)。结合段信息,我们还能把它换算成虚拟地址。至此"程序从哪开始执行"这个逆向头号问题就有了答案,模块 C 的结构解析也接近完整。
