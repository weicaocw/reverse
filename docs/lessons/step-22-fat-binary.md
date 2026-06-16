# Step 22:支持 fat/通用二进制(终于打开 `/bin/ls`)

> 模块：G 扩展 ｜ 对应提交：`<待回填>` ｜ 测试：✅ 通过 ｜ 上一步：[Step 21](step-21-release-ci.md)

## 0. 一句话目标
解析 Mach-O **胖/通用二进制(fat/universal binary,magic 0xCAFEBABE)**:列出它打包的各个架构,取出某个架构的 Mach-O 切片,让既有的全部分析命令对 `/bin/ls` 这类胖二进制也能工作。

## 1. 前置回顾
还记得 Step 07 的"真实发现"吗?我们拿 `/bin/ls` 试,开头是 `ca fe ba be` 而非 Mach-O 的 `cf fa ed fe`,工具报"未知格式"。当时把它记为"尚未覆盖的真实情况"并排进计划。现在来兑现:胖二进制是把**多个架构的 Mach-O 打包进一个文件**(如 x86_64 + arm64),开头一个 fat 头索引各切片的位置。本步解析它,并复用我们早早造好却一直闲置的**大端读取**。

## 2. 先写测试(TDD·红)
手工构造一个含 1 个架构、切片指向一个 Mach-O 头的胖二进制:
```rust
fn fat_with_one_arch() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&FAT_MAGIC.to_be_bytes());        // 大端 ca fe ba be
    v.extend_from_slice(&1u32.to_be_bytes());             // nfat_arch = 1
    v.extend_from_slice(&0x0100_0007u32.to_be_bytes());   // cputype x86_64(大端!)
    v.extend_from_slice(&3u32.to_be_bytes());             // cpusubtype
    v.extend_from_slice(&28u32.to_be_bytes());            // offset = 28
    v.extend_from_slice(&32u32.to_be_bytes());            // size = 32
    v.extend_from_slice(&0u32.to_be_bytes());             // align
    v.extend_from_slice(&MACHO64_HEADER);                 // 偏移 28 放 Mach-O 头
    v
}
#[test]
fn parses_fat_and_extracts_macho_slice() {
    let data = fat_with_one_arch();
    let arches = parse_fat_arches(&data).unwrap();
    assert_eq!(arches[0].arch(), Arch::X86_64);
    let slice = fat_slice(&data, &arches[0]).unwrap();
    assert_eq!(detect(slice), Format::MachO64);                 // 切片是正常 Mach-O
    assert_eq!(parse_macho_header(slice).unwrap().magic, 0xFEED_FACF);
}
```
`parse_fat_arches`/`fat_slice` 不存在 → **红**。

## 3. 实现到通过(TDD·绿)
### 3.1 识别胖二进制
给 `Format` 加 `FatBinary` 变体,`detect` 加一条魔数判断:
```rust
} else if bytes.starts_with(&[0xca, 0xfe, 0xba, 0xbe]) {
    Format::FatBinary
}
```

### 3.2 解析 fat 头(全大端!)
```rust
pub const FAT_MAGIC: u32 = 0xCAFE_BABE;
pub struct FatArch { pub cputype: u32, pub offset: u32, pub size: u32 }

pub fn parse_fat_arches(bytes: &[u8]) -> Result<Vec<FatArch>, ParseError> {
    let mut r = ByteReader::new(bytes);
    let magic = read_u32_or_eof(&mut r, Endian::Big)?;   // ← 大端!
    if magic != FAT_MAGIC { return Err(ParseError::UnknownFormat); }
    let nfat = read_u32_or_eof(&mut r, Endian::Big)?;
    let mut arches = Vec::new();
    for _ in 0..nfat {
        let cputype = read_u32_or_eof(&mut r, Endian::Big)?;
        let _cpusubtype = read_u32_or_eof(&mut r, Endian::Big)?;
        let offset = read_u32_or_eof(&mut r, Endian::Big)?;
        let size = read_u32_or_eof(&mut r, Endian::Big)?;
        let _align = read_u32_or_eof(&mut r, Endian::Big)?;
        arches.push(FatArch { cputype, offset, size });
    }
    Ok(arches)
}

pub fn fat_slice<'a>(bytes: &'a [u8], arch: &FatArch) -> Option<&'a [u8]> {
    let start = arch.offset as usize;
    let end = start.checked_add(arch.size as usize)?;
    bytes.get(start..end)   // 越界安全
}
```
**最关键的一点:fat 头是大端的!** 这是 Mach-O 格式的历史怪癖——内部各架构切片是各自的字节序(x86 小端),但**外层 fat 头固定大端**。我们 Step 03 把字节序做成了参数 `Endian`,现在只需传 `Endian::Big`——当时埋的伏笔在这里兑现,一行不用改读取逻辑。

### 3.3 CLI 透明支持
加 `revx fat` 子命令列架构;并让 info/sections/symbols/disasm 在遇到胖二进制时**自动取第一个架构切片**再分析:
```rust
fn first_macho(bytes: &[u8]) -> &[u8] {
    if detect_is_fat(bytes) {
        if let Ok(arches) = parse_fat_arches(bytes) {
            if let Some(s) = arches.first().and_then(|a| fat_slice(bytes, a)) { return s; }
        }
    }
    bytes   // 不是胖二进制就原样返回
}
```

`cargo test` → **✅ 绿(46 passed)**。**终于能分析 `/bin/ls` 了**:
```
$ revx fat /bin/ls
胖二进制: 2 个架构
  X86_64  offset=0x4000 size=0xbbf0
  Arm64   offset=0x10000 size=0x15c00
$ revx disasm /bin/ls --count 4
0x0100000718  55              push rbp
0x0100000719  48 89 e5        mov rbp,rsp
...
```
Step 07 的遗留限制,圆满解决。

## 4. 改了哪些文件 / 加了什么
- `src/lib.rs`:`Format::FatBinary`;`detect` 加判断;`FAT_MAGIC`、`FatArch`、`parse_fat_arches`、`fat_slice`;2 个测试。
- `src/main.rs`:`first_macho`/`detect_is_fat` 辅助;`fat` 子命令;info/sections/symbols/disasm 透明取切片。

## 5. 学到的语法 / 技巧
- **复用 `Endian::Big`**:同一套读取逻辑,传不同字节序参数即可处理大端格式。
- **`fat_slice` 返回 `Option<&'a [u8]>`**:借用原始数据的一段(零拷贝),越界返回 `None`。
- **`first_macho` 的优雅降级**:不是胖二进制就原样返回,既有命令无需区分。
- **`.first().and_then(|a| fat_slice(...))`**:`Option` 链式处理(取第一个、再尝试切片)。

## 6. 语言设计巧思
**当初参数化的设计,现在零成本兑现**。Step 03 我们本可以把 `read_u32` 写死成小端(那时所有 Mach-O 数据都是小端,够用)。但我们选择把字节序抽成参数 `Endian`。当时看像"过度设计",此刻却**一行读取逻辑都不用改**就支持了大端的 fat 头——只是在调用处传 `Endian::Big`。这就是"识别变化点、抽成参数"的复利:好的抽象在未来需求到来时**无痛扩展**。反之若写死,现在就得到处改 `from_le_bytes`→分情况。**为合理的可变性预留接口,但不为臆想的需求过度设计**——`Endian` 恰好踩在这个平衡点上(字节序是二进制格式里真实且常见的变化维度)。

## 7. 领域知识:胖/通用二进制
- **为什么存在**:Apple 多次做架构迁移(PowerPC→Intel→Apple Silicon)。胖二进制让**一个文件同时含多架构**,系统按当前 CPU 自动挑合适的切片运行——用户无感。`/bin/ls` 含 x86_64 + arm64,在 Intel Mac 和 M 系列 Mac 上都能跑。
- **结构**:`fat_header`(magic + nfat_arch)+ nfat_arch 个 `fat_arch`(cputype/cpusubtype/offset/size/align)+ 各架构的完整 Mach-O 切片。逆向时**先看 fat 头挑架构,再对那个切片按普通 Mach-O 分析**——正是我们 `first_macho` 做的。
- **大端怪癖**:fat 头用大端是历史原因(早期 Mac 是大端的 PowerPC)。这是逆向里"格式内不同部分字节序不同"的经典例子,务必当心。
- **`FAT_MAGIC_64`(0xCAFEBABF)**:还有 64 位的 fat 头变体(offset/size 用 u64),用于超大切片。我们先支持 32 位 fat,扩展它是自然的下一步。
- 工具 `lipo -info /bin/ls`、`file /bin/ls` 看的就是这个。我们的 `revx fat` ≈ `lipo -info`。

## 8. 软件设计理念
**优雅降级 + 适配层:新格式不打扰旧代码**。`first_macho` 是一个**适配层**:它把"可能是胖二进制"这件麻烦事消化在一处——是胖的就取切片,不是就原样返回。于是 info/sections/disasm 等命令**几乎不用改**(只在开头加一句 `first_macho(&raw)`),就都获得了胖二进制支持。把"格式差异"收敛到一个转换函数后面、让上层逻辑面对统一的"一个 Mach-O 切片",正是 Step 07 "parse 成强类型后内部只跟干净数据打交道"的延续。新增能力却几乎不动既有代码,再次印证了分层架构的价值。

## 9. 小测验(自测)
1. 胖二进制和普通 Mach-O 在文件开头怎么区分?胖二进制里装的是什么?
2. fat 头有什么"字节序怪癖"?我们靠什么早有准备、几乎没改代码就支持了它?
3. `fat_slice` 为什么返回 `Option<&[u8]>` 而不是 `Vec<u8>`?这对性能有什么意义?
4. `first_macho` 对"不是胖二进制"的文件返回什么?这种设计让既有命令受到多大改动?
5. 在 Intel Mac 和 Apple Silicon Mac 上运行同一个 `/bin/ls`,系统是怎么选对代码的?

## 10. 参考答案
1. 看开头魔数:普通 Mach-O 64 位是 `cf fa ed fe`(0xFEEDFACF),胖二进制是 `ca fe ba be`(0xCAFEBABE)。胖二进制里装的是**多个不同架构的完整 Mach-O**(如 x86_64 一份、arm64 一份),外加一个索引它们位置的 fat 头。
2. fat 头是**大端**的(尽管内部各 Mach-O 切片可能是小端)。我们靠 Step 03 把字节序抽成了参数 `Endian`——支持大端只需在读取时传 `Endian::Big`,读取逻辑一行不改。这是"参数化变化点"的复利。
3. 因为切片只是**借用** `bytes` 里已有的那一段(`&[u8]`),零拷贝。返回 `Vec<u8>` 则要复制出一份。对可能几十 MB 的二进制,零拷贝意味着更省内存、更快。`Option` 则因为 offset/size 可能越界(畸形文件),用 `None` 安全表示"切不出来"。
4. 返回**原始 bytes 本身**(优雅降级)。这让既有命令几乎不用改——只需在读文件后加一句 `let bytes = first_macho(&raw)`,无论是不是胖二进制都拿到"一个可分析的 Mach-O 切片",后续逻辑统一。
5. 系统加载器读 fat 头,根据当前 CPU 架构(Intel→x86_64,M 系列→arm64)挑选对应的那个切片来加载执行。一个文件、多份代码,按需取用——对用户完全透明。

## 11. 下一步预告
Step 23:**符号反修饰(demangle)**。还记得 Step 13 看到的 `__ZN7reverse4main17h...E` 吗?那是编译器修饰过的名字。我们将引入一个 demangle 库,把它还原成人类可读的 `reverse::main`,让 `symbols` 命令的输出真正好用——这是逆向时快速看懂"有哪些函数"的关键一步。
