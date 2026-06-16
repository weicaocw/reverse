# Step 10：解析段(LC_SEGMENT_64)—— 读出内存布局蓝图

> 模块：C 加载命令/段/节区/符号 ｜ 对应提交：`6fd208f` ｜ 测试：✅ 通过 ｜ 上一步：[Step 09](step-09-load-commands.md)

## 0. 一句话目标
深入 `LC_SEGMENT_64` 加载命令,解析出每个**段(segment)**的名字(`__TEXT`/`__DATA`)、虚拟地址、大小、节区数量,看到程序在内存里的布局蓝图。

## 1. 前置回顾
Step 09 我们列出了所有加载命令,知道哪些是 `LC_SEGMENT_64`(0x19)。但当时只读了每条命令的"通用头"(类型 + 长度),没看里面。本步把 `LC_SEGMENT_64` 的"血肉"读出来——段是 Mach-O 最核心的结构,它回答"代码放哪、数据放哪、加载到内存的什么地址"。

## 2. 先写测试(TDD·红)
构造一个含单个 `__TEXT` 段(vmaddr=0x100000000、vmsize=0x1000、0 节区)的最小 Mach-O,断言解析结果:
```rust
#[test]
fn parses_one_segment() {
    let segs = parse_segments(&macho_with_one_segment()).unwrap();
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].name, "__TEXT");
    assert_eq!(segs[0].vmaddr, 0x1_0000_0000);
    assert_eq!(segs[0].vmsize, 0x1000);
}
```
另加一个"段 + 符号表混在一起时,只数段"的测试。`parse_segments` 不存在 → **红**。

## 3. 实现到通过(TDD·绿)
### 3.1 游标新增"读 N 个字节"`read_bytes`
段名是定长 16 字节,需要一次读一段:
```rust
pub fn read_bytes(&mut self, n: usize) -> Option<&'a [u8]> {
    let end = self.pos.checked_add(n)?;      // checked_add 防整数溢出
    let slice = self.data.get(self.pos..end)?; // 越界返回 None
    self.pos = end;
    Some(slice)
}
```
- **`checked_add`**:`pos + n` 若溢出(超过 usize 上限)返回 `None`,而不是回绕成一个小数字——又一道防畸形输入的闸。
- **`self.data.get(a..b)`**:取子切片,越界返回 `None`(对比 `data[a..b]` 越界 panic)。

### 3.2 把定长名字字段转成字符串
```rust
fn cstr16_to_string(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}
```
段名存在 16 字节里、不足部分用 `0` 补齐(如 `__TEXT\0\0...`)。我们找到第一个 `0` 截断,再把前面的字节当文本。
- **`.position(|&b| b == 0)`**:找第一个等于 0 的字节的下标;找不到则用 `unwrap_or(raw.len())`(整段都没 0 就全要)。
- **`String::from_utf8_lossy`**:把字节按 UTF-8 解释成字符串,遇到非法字节用 `�` 替代而**不报错**——解析不可信数据时,"宽容"比"崩溃"好。

### 3.3 解析段命令的字段
```rust
pub const LC_SEGMENT_64: u32 = 0x19;

pub fn parse_segments(bytes: &[u8]) -> Result<Vec<Segment>, ParseError> {
    let header = parse_macho_header(bytes)?;
    let mut r = ByteReader::new(bytes);
    r.seek(32)...?;                          // 跳过文件头
    let mut segs = Vec::new();
    for _ in 0..header.ncmds {
        let start = r.position();
        let cmd = read_u32_or_eof(&mut r, Endian::Little)?;
        let cmdsize = read_u32_or_eof(&mut r, Endian::Little)?;
        if cmd == LC_SEGMENT_64 {            // 只对段命令深入
            let name = cstr16_to_string(r.read_bytes(16)...?);
            let vmaddr   = read_u64_or_eof(&mut r, Endian::Little)?;
            let vmsize   = read_u64_or_eof(&mut r, Endian::Little)?;
            let fileoff  = read_u64_or_eof(&mut r, Endian::Little)?;
            let filesize = read_u64_or_eof(&mut r, Endian::Little)?;
            let _maxprot  = read_u32_or_eof(&mut r, Endian::Little)?;
            let _initprot = read_u32_or_eof(&mut r, Endian::Little)?;
            let nsects   = read_u32_or_eof(&mut r, Endian::Little)?;
            let _flags    = read_u32_or_eof(&mut r, Endian::Little)?;
            segs.push(Segment { name, vmaddr, vmsize, fileoff, filesize, nsects });
        }
        r.seek(start + cmdsize as usize)...?;  // 无论是不是段,都跳到下一条
    }
    Ok(segs)
}
```
关键点:
- **`if cmd == LC_SEGMENT_64`**:沿用 Step 09 的遍历骨架,只在遇到段命令时多读字段;别的命令照常跳过。
- **`segment_command_64` 的字段布局**:segname[16] → vmaddr(u64) → vmsize(u64) → fileoff(u64) → filesize(u64) → maxprot(u32) → initprot(u32) → nsects(u32) → flags(u32)。我们按这个顺序读,用不到的字段以 `_` 前缀命名(`_maxprot`)告诉编译器"故意不用"。
- **`r.seek(start + cmdsize ...)`**:即使我们读了段的部分字段,最后仍统一按 `cmdsize` 跳到下一条——保证遍历的健壮(不依赖"我恰好读了多少")。

`cargo test` → **✅ 绿(23 passed)**。真实文件:
```
段:
  __PAGEZERO   vmaddr=0x0000000000000000 vmsize=0x100000000 节区数=0
  __TEXT       vmaddr=0x0000000100000000 vmsize=0x52000   节区数=8
  __DATA       vmaddr=0x0000000100052000 vmsize=0x3000    节区数=9
  __LINKEDIT   vmaddr=0x0000000100055000 vmsize=0x35000   节区数=0
```
这就是程序在内存里的**真实布局图**。

## 4. 改了哪些文件 / 加了什么
- `src/lib.rs`:① 游标加 `read_bytes`;② 加 `read_u64_or_eof`、`cstr16_to_string` 助手;③ 新增 `struct Segment` 与 `parse_segments`、常量 `LC_SEGMENT_64`;④ 新增 2 个测试 + 夹具。
- `src/main.rs`:打印段表(段名 / vmaddr / vmsize / 节区数)。

## 5. 学到的语法 / 技巧
- **`read_bytes` / 子切片 `data.get(a..b)`**:一次读一段定长字节。
- **`checked_add`**:溢出返回 `None` 的安全加法。
- **`.iter().position(|&b| b == 0)`**:在切片里找第一个满足条件的元素下标;`|&b| ...` 是闭包(匿名函数)。
- **`String::from_utf8_lossy(...).into_owned()`**:字节→字符串(宽容),并得到拥有所有权的 `String`。
- **`_` 前缀变量名**:`let _maxprot = ...` 表示"读出来但故意不用",避免未使用警告。
- **`pub const`**:定义公开常量(`LC_SEGMENT_64`),比到处写 `0x19` 可读。

## 6. 语言设计巧思
**`&[u8]`(借来的字节)与 `String`(自己拥有的字符串)的分界**。`read_bytes` 返回 `&'a [u8]`——只是借用文件数据的一段,零拷贝。但 `Segment.name` 要长期保存、要能脱离原始文件数据独立存在,所以必须**拥有**自己的数据,用 `String`(`from_utf8_lossy(...).into_owned()` 完成"借用→拥有"的拷贝)。Rust 把"借用"和"拥有"在类型上分得清清楚楚(`&str` vs `String`、`&[u8]` vs `Vec<u8>`),你时刻知道"这块数据是借的还是我的、谁负责释放"。这种显式,正是它无 GC 还安全的基础。

**宽容解析 vs 严格解析**:`from_utf8_lossy` 选择"遇到坏字节不崩、用 � 顶替"。逆向不可信样本时,这种宽容让工具能继续工作、给出尽可能多的信息,而不是一遇到脏数据就罢工——这是健壮工具的务实选择。

## 7. 领域知识:段(Segment)与虚拟地址
- **段**是 Mach-O 装载到内存的基本单位,每个段有统一的权限(读/写/执行)。常见段:
  - **`__PAGEZERO`**:开头一大块"什么都不映射"的区域(vmsize 常为 4GB),用来捕获"空指针解引用"——访问地址 0 附近立即崩溃,是一种安全设计。
  - **`__TEXT`**:代码 + 只读数据(可执行、不可写)。逆向最关心的机器码就在这里的 `__text` 节区。
  - **`__DATA`**:可读写的全局 / 静态数据。
  - **`__LINKEDIT`**:符号表、重定位、动态链接信息(给加载器和调试器用)。
- **vmaddr(虚拟地址)**:段被加载到进程地址空间的位置。注意 `__TEXT` 在 `0x100000000`——这是 64 位程序的典型加载基址。逆向时,文件偏移 ↔ 虚拟地址的换算(借助段的 fileoff / vmaddr)是基本功:反汇编显示的地址是虚拟地址,而字节在文件里的位置是文件偏移。
- 这正是 `otool -l` / `size` / `vmmap` 展示的信息。

## 8. 软件设计理念
**复用遍历骨架,按需深入(开放-封闭的雏形)**。`parse_segments` 和 `parse_load_commands` 共享同一套"循环 + 按 cmdsize 跳"的骨架,只在 `if cmd == LC_SEGMENT_64` 处插入段特有的解析。将来要解析 `LC_SYMTAB`、`LC_MAIN`,都是在这同一骨架上加一个 `if` 分支,不必重写遍历逻辑。稳定的骨架 + 可插拔的"每类命令处理",让系统对"新增命令类型"是开放的、对"已有遍历逻辑"是封闭的(不用改)。我们也刻意只读需要的字段、用 `_` 忽略其余,保持每步聚焦。

## 9. 小测验(自测)
1. 段名为什么要用 `cstr16_to_string` 处理,而不能直接把 16 字节当字符串?
2. `read_bytes` 里的 `checked_add` 防的是什么?普通的 `self.pos + n` 有什么隐患?
3. `Segment.name` 用 `String` 而不是 `&str`,为什么?(从"借用 vs 拥有"角度答)
4. 解析段字段时把 `maxprot` 写成 `_maxprot`,这个下划线起什么作用?
5. `__PAGEZERO` 段为什么存在?它的 vmsize 为何那么大?

## 10. 参考答案
1. 段名存在固定 16 字节里、用 `0` 补齐(如 `__TEXT\0\0...`)。直接全当字符串会带上尾部的 0 和垃圾;必须找到第一个 `0` 截断、只取有效部分,才能得到 `"__TEXT"`。
2. 防**整数溢出**:若 `pos` 已经很大,`pos + n` 可能超过 `usize` 上限而回绕成一个小数字,导致错误地"读到"本不该读的位置。`checked_add` 溢出时返回 `None`,安全失败。畸形文件可能故意触发这种溢出。
3. 因为 `Segment` 要独立保存段名、可能脱离原始文件字节而长期存在,必须**拥有**自己的数据,所以用 `String`(拥有)。`&str` 只是借用,会被生命周期绑死在原始数据上,无法自由保存。
4. `_` 前缀告诉编译器"我读了这个值但故意不用它",从而**不触发"未使用变量"警告**(我们的 CI 里 `-D warnings` 会因警告报红)。它表达了"刻意忽略"的意图。
5. `__PAGEZERO` 是地址 0 附近一大片"不映射任何内容"的区域。任何对空指针(地址 0)或其附近的访问都会落进这片无效区域、立即触发崩溃,从而把"空指针解引用"这类 bug 暴露出来。它覆盖整个低 4GB(vmsize=0x100000000)以确保 32 位范围内的空指针都被捕获。

## 11. 下一步预告
Step 11:每个段里还含若干**节区(section)**(`__TEXT` 段里的 `__text` 代码、`__cstring` 字符串等)。我们将进入段命令内部、循环读出它的 `nsects` 个节区,列出节名、地址、大小——直抵"机器码到底在文件哪个位置"这个反汇编的前置问题(为模块 E 反汇编铺路)。
