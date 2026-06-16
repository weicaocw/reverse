# Step 17：接入反汇编库,把机器码翻译成汇编

> 模块：E 反汇编 ｜ 对应提交：`8c74122` ｜ 测试：✅ 通过 ｜ 上一步：[Step 16](step-16-entropy.md)

## 0. 一句话目标
第一次引入**第三方 crate**(`iced-x86`),写一个 `disassemble` 函数,把一段 x86-64 机器码翻译成 `(地址, 汇编指令)` 列表,并在真实 `__text` 节区上跑通。

## 1. 前置回顾
模块 A–D 我们把文件结构、字符串、熵都摸清了,还在 Step 11 定位到了 `__text` 节区(真正的机器码)。现在是逆向的**圣杯**:把那些字节翻译成人能读的汇编指令。反汇编器本身极其复杂(x86 指令变长、前缀繁多、几千条指令),**绝不该自己手写**——我们站在巨人肩膀上,用成熟的 `iced-x86` 库。这一步也教我们如何用 cargo 管理依赖、调用第三方 API。

## 2. 先写测试(TDD·红)
用几条"手算得出"的指令钉住行为:
```rust
#[test]
fn disassembles_nop_and_ret() {
    let out = disassemble(&[0x90, 0xc3], 0x1000);
    assert_eq!(out[0], (0x1000, "nop".to_string()));  // 0x90 = nop
    assert_eq!(out[1], (0x1001, "ret".to_string()));  // 0xc3 = ret
}
#[test]
fn disassembles_mov_eax_imm() {
    let out = disassemble(&[0xb8, 0x01, 0x00, 0x00, 0x00], 0x2000);  // b8 imm32 = mov eax, imm
    assert!(out[0].1.starts_with("mov eax"));
}
```
注意第二条指令地址是 `0x1001`——因为 `nop` 占 1 字节,反汇编器知道每条指令多长,自动推进地址。`disassemble` 不存在 → **红**。

## 3. 实现到通过(TDD·绿)
### 3.1 加依赖
```
$ cargo add iced-x86
```
这会在 `Cargo.toml` 的 `[dependencies]` 写入 `iced-x86 = "1.21"`。`cargo build` 时自动从 crates.io 下载编译。

### 3.2 调用库
```rust
use iced_x86::{Decoder, DecoderOptions, Formatter, Instruction, NasmFormatter};

pub fn disassemble(code: &[u8], rip: u64) -> Vec<(u64, String)> {
    let mut decoder = Decoder::with_ip(64, code, rip, DecoderOptions::NONE); // 64 位模式
    let mut formatter = NasmFormatter::new();
    let mut instr = Instruction::default();
    let mut text = String::new();
    let mut out = Vec::new();
    while decoder.can_decode() {
        decoder.decode_out(&mut instr);     // 解码一条到 instr(复用,免重复分配)
        text.clear();
        formatter.format(&instr, &mut text); // 格式化成 NASM 风格文本
        out.push((instr.ip(), text.clone()));
    }
    out
}
```
- **`Decoder::with_ip(64, code, rip, ...)`**:建一个 64 位解码器。`rip`(指令指针 / 起始地址)让解码器能算出相对跳转的绝对目标、并给每条指令正确的地址。
- **`while decoder.can_decode()`**:还有字节就继续解码。
- **`decode_out(&mut instr)`**:解码一条指令、写进我们提供的 `instr`(**复用同一个对象**而非每次新建,是库给的高效用法)。
- **`NasmFormatter`**:把指令格式化成 NASM 汇编语法的文本(还有 Intel、AT&T、GAS 等风格可选)。
- **`instr.ip()`**:这条指令的地址(解码器随每条指令的长度自动推进)。

`cargo test` → **✅ 绿(40 passed)**。在真实 `__text` 上跑:
```
反汇编 __text(前 48 字节,起始地址 0x100000820):
  0x0100000820  push rbp
  0x0100000821  mov rbp,rsp
  0x0100000824  sub rsp,160h
  0x010000082b  mov [rbp-140h],rsi
  ...
```
这是一段教科书般的**函数序言(prologue)**——保存旧栈帧、建立新栈帧、为局部变量开辟空间。**我们的工具会"读代码"了。**

## 4. 改了哪些文件 / 加了什么
- `Cargo.toml`:`[dependencies]` 增加 `iced-x86`。
- `src/lib.rs`:`use iced_x86::...`;新增 `disassemble` 函数;新增 2 个测试。
- `src/main.rs`:定位 `__text` 节区,反汇编其开头 48 字节并打印。

## 5. 学到的语法 / 技巧
- **`cargo add` / `[dependencies]`**:引入并管理第三方库。
- **`use 外部crate::{...}`**:把第三方类型 / trait 引入作用域(`Formatter` 是 trait,得 `use` 进来才能调它的 `.format`)。
- **复用对象模式**:`decode_out(&mut instr)` + `text.clear()`,在循环里复用缓冲,避免反复分配。
- **库的 builder/选项**:`DecoderOptions::NONE`、`Decoder::with_ip(...)` 这类"带配置的构造"。
- **`while 条件 { }`**:基于"还能不能解码"的循环。

## 6. 语言设计巧思
**cargo + crates.io:依赖管理是语言体验的一等公民**。一句 `cargo add iced-x86`,版本解析、下载、编译、链接全自动——`Cargo.toml` 声明"要什么",`Cargo.lock` 锁定"具体哪个版本",保证你和 CI、和别人拿到的依赖完全一致(可复现构建)。对比 C/C++ 里手动找库、配 include 路径、链接 `.a`/`.so`、对付版本冲突的痛苦,Rust 的依赖体验是它生产力的重要来源。我们的 CI(Step 05)也因此零额外配置就能拉依赖、编译——`cargo build` 在干净机器上自动搞定一切。

**"不重造轮子"的工程判断**。反汇编 x86 是出了名的难(变长编码、海量指令、前缀 / REX / VEX / EVEX…)。自己写一个不仅工作量巨大,还极易出错。`iced-x86` 是经过严格测试、覆盖完整指令集的成熟库。**识别"什么该自己写、什么该用库"是工程成熟度的标志**:底层原语(字节游标、解析骨架)我们亲手写以学习和掌控;而反汇编这种"复杂、有权威实现、自己写无收益"的部分,果断用库。这不是偷懒,是把精力放在刀刃上。

## 7. 领域知识:反汇编与函数序言
- **反汇编(disassembly)**:把机器码(CPU 执行的二进制)翻译回汇编助记符(人可读的指令表示)。它是静态逆向的核心——没有源码时,汇编就是你能看到的"最接近代码"的东西。
- **变长指令**:x86 指令长度从 1 字节(`nop`)到 15 字节不等,所以必须**顺序解码**——不知道上一条多长,就不知道下一条从哪开始。这也是为什么 `disassemble` 要用解码器逐条推进,而不能随机跳。
- **函数序言(prologue)**:我们看到的 `push rbp; mov rbp,rsp; sub rsp,N` 是绝大多数函数开头的固定套路:保存调用者的栈帧基址、把当前栈顶设为新基址、再减栈指针为局部变量腾空间。识别序言能帮你**找到函数边界**——逆向时定位"一个函数从哪开始"的重要线索。
- **`rip`/IP 的意义**:指令里的跳转 / 调用常是"相对当前位置的偏移";给对起始地址,反汇编器才能算出绝对目标地址(如 `call 0x100001234`),否则地址全错。这就是 `with_ip` 要传 `rip` 的原因。
- 这正是 `objdump -d`、IDA、Ghidra、Hopper 做的核心事;`iced-x86` 也是不少工具的底层。

## 8. 软件设计理念
**在自研与复用之间画对边界**。整个项目我们坚持"底层亲手写"(游标、Mach-O 解析)以学习原理、保持掌控;但到反汇编这一层,自研既无学习增量(规则是 CPU 厂商定的、死记硬背)又风险极高,于是引入库。好的架构会清晰地划出这条线:**核心领域逻辑自研以保持竞争力与可控,通用且复杂的基础设施复用成熟方案**。同时,我们把库**包裹在自己的 `disassemble` 函数后面**(返回我们自己的 `(u64, String)` 类型,而非直接暴露 `iced-x86` 的类型)——这层"防腐层(anti-corruption layer)"让将来万一要换库(比如换 `capstone`),只需改这一个函数,上层代码不受影响。

## 9. 小测验(自测)
1. 为什么反汇编必须"顺序"进行,不能直接跳到任意字节开始解码?(提示:x86 指令长度)
2. `Decoder::with_ip` 里的 `rip`(起始地址)有什么用?传错了会导致什么?
3. 为什么我们选择用 `iced-x86` 而不自己写反汇编器?这体现了什么工程判断?
4. `disassemble` 返回我们自己的 `Vec<(u64, String)>`,而不直接返回 `iced-x86` 的指令类型,这样做有什么好处?
5. 看到 `push rbp; mov rbp,rsp; sub rsp,N` 这个模式,通常意味着什么?它在逆向里有什么用?

## 10. 参考答案
1. 因为 x86 指令是**变长的**(1~15 字节)。要知道下一条指令从哪开始,必须先解码完当前这条、得到它的长度。从任意中间字节开始解码,极可能落在某条指令的中段,解出完全错误(乱码)的指令。所以必须从已知的指令边界顺序解码。
2. `rip` 是这段代码的**起始虚拟地址**。解码器用它给每条指令标注正确地址,并把指令里的相对偏移(相对跳转 / call / RIP 相对取址)换算成**绝对目标地址**。传错会导致所有地址、跳转目标都错位,反汇编结果具误导性。
3. 因为 x86 反汇编极其复杂(变长编码、海量指令、各种前缀),自己写工作量巨大且极易出错,而 `iced-x86` 是成熟、经过严格测试的权威实现。这体现"识别什么该自研、什么该复用"的工程判断——把精力放在有学习/掌控价值的核心,复杂通用的基础设施用成熟库。
4. 这是一层**防腐层 / 隔离层**:上层代码只依赖我们自己的简单类型 `(u64, String)`,不直接耦合 `iced-x86`。将来若要更换反汇编库(如改用 capstone),只需改 `disassemble` 一个函数,所有调用方不受影响。降低了对外部依赖的耦合。
5. 它是典型的**函数序言(prologue)**:保存调用者栈帧基址(`push rbp`)、建立新栈帧(`mov rbp,rsp`)、为局部变量分配栈空间(`sub rsp,N`)。逆向时识别这个模式可用来**定位函数的起点 / 边界**,是划分和分析函数的重要线索。

## 11. 下一步预告
Step 18:把反汇编**产品化**成 objdump 风格的视图——每行同时显示`地址`、`机器码字节`、`汇编指令`三栏,并支持从入口点或指定地址反汇编、限制条数。我们会把 Step 14 的字节格式化与本步的反汇编结合,产出专业逆向工具那样的输出。这是模块 E 的收官,也让 `revx` 的"看懂代码"能力真正可用。
