# Step 18：反汇编产品化 —— objdump 风格三栏视图

> 模块：E 反汇编(收官) ｜ 对应提交：`<待回填>` ｜ 测试：✅ 通过 ｜ 上一步：[Step 17](step-17-disassemble.md)

## 0. 一句话目标
把反汇编输出成专业工具那样的三栏:**地址 ｜ 机器码字节 ｜ 汇编指令**,并支持"最多 N 条"的限制。

## 1. 前置回顾
Step 17 的 `disassemble` 给出 `(地址, 汇编)`,但缺了逆向工程师爱看的**机器码字节列**(用来核对指令编码、找 patch 点、对照 hex)。本步加上字节列、做成 `objdump -d` 那样的视图,并限制条数避免刷屏。这是模块 E 收官。

## 2. 先写测试(TDD·红)
```rust
#[test]
fn disassemble_view_shows_bytes_and_asm() {
    let v = disassemble_view(&[0x90, 0xc3], 0x1000, 10);
    let lines: Vec<&str> = v.lines().collect();
    assert!(lines[0].contains("90") && lines[0].contains("nop"));   // 字节 + 助记符同行
    assert!(lines[1].contains("c3") && lines[1].contains("ret"));
}
#[test]
fn disassemble_view_respects_max() {
    let v = disassemble_view(&[0x90, 0x90, 0x90, 0x90], 0, 2);
    assert_eq!(v.lines().count(), 2);   // 4 个 nop 但只要 2 条
}
```
`disassemble_view` 不存在 → **红**。

## 3. 实现到通过(TDD·绿)
```rust
pub fn disassemble_view(code: &[u8], rip: u64, max: usize) -> String {
    let mut decoder = Decoder::with_ip(64, code, rip, DecoderOptions::NONE);
    let mut formatter = NasmFormatter::new();
    let mut instr = Instruction::default();
    let mut text = String::new();
    let mut out = String::new();
    let mut count = 0;
    while decoder.can_decode() && count < max {
        decoder.decode_out(&mut instr);
        text.clear();
        formatter.format(&instr, &mut text);
        // 关键:取出本条指令的原始字节。
        let start = (instr.ip() - rip) as usize;        // 它在 code 里的偏移
        let bytes = &code[start..start + instr.len()];  // 长度由 iced 给出
        let mut hex = String::new();
        for b in bytes { let _ = write!(hex, "{b:02x} "); }
        let _ = writeln!(out, "{:#012x}  {hex:<22}{text}", instr.ip());
        count += 1;
    }
    out
}
```
- **`instr.len()`**:iced 解码后告诉我们这条指令占几个字节。`(instr.ip() - rip)` 是它在 `code` 切片里的起始下标——两者相减把"虚拟地址"换算回"切片偏移"(因为 `ip = rip + 偏移`)。
- **`&code[start..start + instr.len()]`**:切出这条指令的原始字节,逐个格式化成 hex(复用 Step 14 的字节格式化思路)。
- **`{hex:<22}`**:字节列左对齐到宽度 22(最长 15 字节 × 3 字符 = 45,但常见指令短,22 够大多数对齐;过长就自然顶开)。
- **`count < max`**:限制输出条数,避免把整个 `.text`(几万条)刷满屏。

`cargo test` → **✅ 绿(42 passed)**。真实 `__text`:
```
0x0100000820  55                    push rbp
0x0100000821  48 89 e5              mov rbp,rsp
0x0100000824  48 81 ec 60 01 00 00  sub rsp,160h
0x0100000840  48 c7 45 98 01 00 00 00 mov qword [rbp-68h],1
...
```
**这就是专业逆向工具的核心视图**:左看地址(定位/下断点)、中看字节(核对编码/找 patch 点)、右看汇编(读逻辑)。我们的 `revx` 现在具备真正可用的反汇编能力。

## 4. 改了哪些文件 / 加了什么
- `src/lib.rs`:新增 `disassemble_view`;新增 2 个测试。
- `src/main.rs`:用 `disassemble_view` 输出 `__text` 前 12 条三栏反汇编。

## 5. 学到的语法 / 技巧
- **`instr.len()`**:指令字节长度,用于切出原始字节、推进。
- **地址 ↔ 偏移换算**:`offset = ip - rip`。
- **`writeln!` 多列格式化**:`{:#012x}` 地址、`{:<22}` 定宽字节列、`{}` 汇编。
- **带上限的循环**:`while can_decode() && count < max`,两个条件同时控制。

## 6. 语言设计巧思
**借用切片做"零拷贝取子串"**。`&code[start..start + len]` 没有复制任何字节,只是**借用** `code` 的一段——我们对它格式化成 hex 后即弃,原始数据始终归 `code`(进而归 main 里的 `bytes`)所有。整个反汇编流程里,机器码字节从未被拷贝过一份,只是不断被"借看"。这种"数据只有一份、到处借用"的风格,是 Rust 高效处理大缓冲(如整个可执行文件)的常态——没有 GC 扫描、没有隐式深拷贝,性能可预测。

**`String` 累积 vs 直接打印,再次选择前者**:`disassemble_view` 返回 `String` 而非直接 `println!`,延续了我们一贯的"纯函数、可测试、可复用"原则——测试能直接断言返回的多行文本(`v.lines()`),main 才决定怎么输出。

## 7. 领域知识:为什么三栏缺一不可
- **地址列**:每条指令的虚拟地址。逆向时用来定位、下断点、对照交叉引用("跳到 0x100000824 看看")。
- **机器码字节列**:指令的原始编码。用途:① 核对反汇编是否正确;② **打补丁(patch)**——比如把某条 `jz`(74)改成 `jnz`(75)绕过校验,必须知道原始字节和位置;③ 识别花指令 / 反调试。
- **汇编列**:人可读的逻辑。读程序到底在干什么。
- 三者并置正是 `objdump -d`、IDA、x64dbg 的反汇编窗口标配。少了字节列就没法做 patch 分析;少了地址列就没法定位跳转。
- **限制条数**的现实意义:真实 `.text` 动辄几万条指令,工具默认只反汇编一个函数或一屏,按需展开——这也是为什么我们加 `max`。

## 8. 软件设计理念
**在已有原语上做"表现层"增强,核心逻辑不动**。我们没有改动 Step 17 的 `disassemble`(它仍是简洁的 `(addr, asm)` 版本),而是另起 `disassemble_view` 专门负责"漂亮的三栏字符串"。**解析/解码(拿数据)与呈现(怎么显示)分离**:`disassemble` 是数据层,`disassemble_view` 是呈现层。这样想要别的输出(JSON、HTML、彩色)都可以再加一个呈现函数,而不必碰解码逻辑。关注点分离让每一层都简单、可测、可独立演化——这是贯穿全项目的主线。

## 9. 小测验(自测)
1. `disassemble_view` 怎么拿到每条指令的原始字节?`instr.len()` 和 `instr.ip() - rip` 各起什么作用?
2. 机器码字节列在逆向里有什么独特用途,是地址列和汇编列替代不了的?
3. `&code[start..start + instr.len()]` 这步有没有复制字节?这对处理大文件有什么意义?
4. 为什么要给 `disassemble_view` 加一个 `max` 参数?不加会怎样?
5. 我们保留了 Step 17 的 `disassemble`、另写 `disassemble_view`,而不是把字节列直接塞进 `disassemble`。这样分开有什么好处?

## 10. 参考答案
1. `instr.ip() - rip` 把指令的虚拟地址换算成它在 `code` 切片里的**起始偏移**(因为 ip = rip + 偏移);`instr.len()` 给出这条指令占**几个字节**。两者结合 `&code[start..start+len]` 就切出了这条指令的原始字节。
2. 机器码字节列用于:**核对编码正确性**、**打补丁**(知道原始字节和确切位置才能改,如把条件跳转的操作码 74↔75 反转)、识别异常编码 / 花指令。这些都需要看到"实际是哪些字节",地址和汇编列无法提供。
3. **没有复制**。它只是借用 `code` 的一段子切片(`&[u8]`),零拷贝。对处理大文件(整个可执行文件可能几十 MB)意义重大:全程不产生数据副本,内存占用低、速度快、行为可预测,没有隐式的昂贵拷贝。
4. 因为真实 `.text` 可能有几万条指令,不限制会瞬间刷屏、淹没有用信息。`max` 让我们只看开头一段 / 一个函数,按需查看。不加就会把整段代码全部输出,实际不可用。
5. **关注点分离**:`disassemble` 是简洁的数据层(给出结构化的 `(addr, asm)`),`disassemble_view` 是呈现层(给出格式化字符串)。分开后,想要别的输出形式(JSON、彩色、HTML)只需再加呈现函数,核心解码逻辑不动;两层各自简单、可独立测试与演化。

## 11. 下一步预告
**模块 E 到此完成**(反汇编原语 + objdump 视图),开模块 E 的 PR 复盘。接着进入**模块 F — CLI 产品化**:目前 `main` 把所有信息一股脑全打出来。我们将用 `clap`(Rust 主流命令行库)做成**子命令**——`revx info`、`revx sections`、`revx strings`、`revx disasm` 等,让工具像真正的产品一样按需调用。这会教我们用派生宏定义命令行接口、处理参数与帮助信息。
