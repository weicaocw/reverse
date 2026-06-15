# Step 04：自定义错误类型 `ParseError`,从 `Option` 升级到 `Result`

> 模块：A 基础(收官) ｜ 对应提交：`<待回填>` ｜ 测试：✅ 通过 ｜ 上一步：[Step 03](step-03-endianness.md)

## 0. 一句话目标
造一个**携带失败原因**的错误类型 `ParseError`,把"识别格式"的返回从 `Option`(只会说"没有")升级到 `Result`(能说"为什么失败"),并第一次让 `main` 用 `?` 优雅地传播错误。

## 1. 前置回顾
模块 A 一路下来:Step 01 能识别格式、Step 02 做了安全游标、Step 03 能读多字节整数并处理字节序。它们表达"失败"都靠 `Option` 的 `None`。但 `None` 是个**哑巴**——它不告诉你为什么失败:是文件被截断了?还是魔数压根不认识?工业级工具必须能讲清原因。本步补上这一环,也为模块 B 真正解析文件头打好"错误处理"的地基。

## 2. 先写测试(TDD·红)
我们要一个新函数 `identify`,它返回 `Result<Format, ParseError>`:
```rust
// 已知格式 → Ok
assert_eq!(identify(&[0x7f, b'E', b'L', b'F']), Ok(Format::Elf));
// 文件太短 → Err,并指出在偏移 1 处缺字节
assert_eq!(identify(&[0x7f]), Err(ParseError::UnexpectedEof { offset: 1 }));
// 魔数不认识 → Err
assert_eq!(identify(&[0x12, 0x34, 0x56, 0x78]), Err(ParseError::UnknownFormat));
// 错误能打印成人话
let msg = format!("{}", ParseError::UnknownFormat);
assert!(msg.contains("无法识别"));
```
`ParseError` / `identify` 还不存在,`cargo test` → **红**:
```
error[E0433]: cannot find type `ParseError` in this scope
error[E0425]: cannot find function `identify` in this scope
```

## 3. 实现到通过(TDD·绿)
### 3.1 定义错误类型(让它带原因)
```rust
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    UnexpectedEof { offset: usize }, // 文件读到一半没了,记下缺在哪个偏移
    UnknownFormat,                   // 魔数不认识
}
```
注意 `UnexpectedEof { offset: usize }`:**枚举的变体可以携带数据**!这是 Rust 枚举比其它语言强大的地方——它不只是"几个名字",每个分支还能挂上自己的"案情细节"。`UnexpectedEof` 带一个 `offset`(缺在哪),`UnknownFormat` 不需要额外信息就光秃秃的。

### 3.2 让错误能打印成人话 —— 实现 `Display`
```rust
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::UnexpectedEof { offset } =>
                write!(f, "文件意外结束:在偏移 {offset} 处还需要更多字节"),
            ParseError::UnknownFormat =>
                write!(f, "无法识别的文件格式(未知魔数)"),
        }
    }
}
impl std::error::Error for ParseError {}
```
- **`impl fmt::Display for ParseError`**:给我们的类型"实现一个 trait(特质)"。`Display` 这个 trait 规定"怎么把自己显示给人看"。实现了它,`println!("{}", err)` 才能用。
- **`match self { ... }`**:按是哪个变体分支;`UnexpectedEof { offset }` 这种写法叫**模式匹配**,顺手把里面的 `offset` 解出来用。
- **`impl std::error::Error for ParseError {}`**:声明"我是一个标准错误类型"。花括号是空的——因为该有的(`Debug` + `Display`)都齐了,这行只是"挂上身份",好让它能被 `Box<dyn Error>`、`?` 等通用错误机制接纳。

### 3.3 用 `Result` 的识别函数
```rust
pub fn identify(bytes: &[u8]) -> Result<Format, ParseError> {
    if bytes.len() < 2 {
        return Err(ParseError::UnexpectedEof { offset: bytes.len() });
    }
    match detect(bytes) {
        Format::Unknown => Err(ParseError::UnknownFormat),
        known => Ok(known),
    }
}
```
- **`Result<Format, ParseError>`**:要么 `Ok(Format)`(成功,带结果),要么 `Err(ParseError)`(失败,带原因)。这是 `Option` 的"升级版":`Option` 的失败是空的 `None`,`Result` 的失败 `Err(e)` 里**装着原因**。
- **`return Err(...)`**:提前返回一个错误。
- 末尾的 `match`:把 Step 01 的 `Format::Unknown` 翻译成 `Err(UnknownFormat)`,其余已知格式包成 `Ok`。

### 3.4 让 `main` 用 `?` 传播错误
```rust
fn main() -> Result<(), Box<dyn Error>> {
    ...
    let bytes = fs::read(path)?;   // 读文件失败,? 直接把 io 错误抛给 main 的返回
    ...
    match identify(&bytes) {
        Ok(fmt) => println!("格式: {fmt:?}"),
        Err(e)  => println!("格式: 无法解析 —— {e}"),
    }
    Ok(())
}
```
`cargo test` → **✅ 绿(13 passed)**。真实运行三种情况:
```
$ cargo run -- target/debug/reverse        # 正常
格式: MachO64
$ cargo run -- /no/such/file               # 文件不存在,? 抛出 io 错误
Error: Os { code: 2, kind: NotFound, ... }   (退出码 1)
$ cargo run -- src/main.rs                  # 拿源码当输入,未知魔数
格式: 无法解析 —— 无法识别的文件格式(未知魔数)
```

## 4. 改了哪些文件 / 加了什么
- `src/lib.rs`:① 新增 `enum ParseError`;② 为它实现 `Display` 和 `Error`;③ 新增 `identify` 函数;④ 新增 4 个测试。
- `src/main.rs`:改为 `fn main() -> Result<(), Box<dyn Error>>`,用 `?` 传播 `fs::read` 的错误,用 `identify` 给出友好提示。

## 5. 学到的语法 / 技巧
- **`Result<T, E>`**:表达"成功带结果 `Ok(T)` / 失败带原因 `Err(E)`"。
- **携带数据的枚举变体**:`UnexpectedEof { offset: usize }`——每个分支可挂自己的数据。
- **`trait`(特质)与 `impl Trait for Type`**:给类型实现某种"能力 / 接口"。这里实现了 `Display`(可打印)和 `Error`(是错误)。
- **模式匹配解构**:`ParseError::UnexpectedEof { offset } => ...` 在分支里直接取出字段。
- **`write!(f, "...")`**:往格式化器里写字符串,是实现 `Display` 的标准写法。
- **`?` 用在 `Result` 上**:`Ok(v)` 取出 `v`,`Err(e)` 立刻把 `e` 返回给上层。和 Step 02 在 `Option` 上的 `?` 是同一个运算符、同一种思路。
- **`Box<dyn Error>`**:一个"能装下任意错误类型"的盒子,让 `main` 可以用 `?` 接住不同来源的错误(io 错误、解析错误……)。

## 6. 语言设计巧思:错误是"值",不是"异常"
很多语言用**异常**处理错误:函数随时可能"抛出"一个异常,沿调用栈往上飞,你不看文档根本不知道某个调用会不会炸。Rust 反其道而行:**错误是普通的返回值**(`Result`),写在函数签名里,明明白白。

这带来三个好处:
1. **看签名就知道会不会失败**。`fn identify(...) -> Result<Format, ParseError>` 一眼看出"这函数可能失败,失败类型是 ParseError"。`fn detect(...) -> Format` 则保证不失败。契约写在类型里。
2. **编译器逼你处理**。一个 `Result` 你不处理(不 `match`、不 `?`、不 `unwrap`),会收到警告甚至用不了里面的值。错误无法被"默默忽略"——这正是 `Option`/`Result` 一脉相承的哲学:**把"可能出问题"编码进类型,逼调用者面对**。
3. **`?` 让"传播错误"不啰嗦**。没有 `?` 的话,每次调用都要写一坨 `match` 把错误往上递;`?` 把这件事压成一个字符,既简洁又不失显式(你仍能从 `?` 看出"这里可能提前返回")。

还有一处巧思:**枚举变体携带数据**,让我们能把"错误的种类"和"错误的细节"统一进一个类型。`UnexpectedEof { offset }` 既说了"是什么错"(EOF),又说了"具体在哪"(offset)。配合 `match`,处理错误时可以按种类分别应对,还能拿到细节——这比"错误就是一个字符串"健壮得多(字符串没法被可靠地分类处理)。

## 7. 领域知识
**健壮的解析器为何必须区分错误种类**。逆向工具面对的二进制五花八门:正常文件、被截断的样本、加了壳的、故意构造来攻击解析器的畸形文件。把"文件太短"和"魔数不认识"区分开,意义在于:
- "文件太短(`UnexpectedEof`)" → 可能是下载不完整 / 被恶意截断,提示用户检查来源;
- "魔数不认识(`UnknownFormat`)" → 可能是不支持的格式 / 数据文件,而非可执行文件。

带 `offset` 的 EOF 错误尤其实用:逆向分析时,"在偏移 0x1F0 处缺字节"能直接定位文件哪里断了。专业逆向工具(readelf、LIEF 等)的报错都精确到偏移,正是这个道理。

## 8. 软件设计理念
**契约式设计 + 让错误可恢复**。`identify` 的函数签名 `-> Result<Format, ParseError>` 就是一份**契约**:调用者被明确告知"我可能失败,失败时给你一个 ParseError"。对比 Step 01 的 `detect -> Format`(用 `Unknown` 表示"不知道"):`detect` 是"绝不失败、但可能返回 Unknown"的底层原语,`identify` 是"会失败、但解释原因"的上层封装。两者各司其职——底层提供不会 panic 的纯逻辑,上层把"语义上的失败"翻译成带原因的 `Result`。这种"底层原语 + 上层语义封装"的分层,是健壮库的常见骨架。

## 9. 小测验(自测)
1. `Option` 和 `Result` 都能表达"失败",它们最大的区别是什么?
2. `UnexpectedEof { offset: usize }` 里的 `offset` 有什么用?为什么不把错误简单写成一个字符串?
3. 我们为 `ParseError` 实现了 `Display`,它让什么成为可能?
4. `main` 里 `fs::read(path)?` 的 `?` 在文件不存在时做了什么?程序最终退出码是多少?
5. 为什么保留底层的 `detect`(返回 `Format`,含 `Unknown`),而不直接把它改成返回 `Result`?

## 10. 参考答案
1. `Option<T>` 的失败是空的 `None`,**不带任何原因**;`Result<T, E>` 的失败是 `Err(e)`,**`e` 里装着失败的原因**。需要解释"为什么失败"时用 `Result`,只关心"有没有"时用 `Option`。
2. `offset` 记录"在文件的哪个位置发现字节不够",便于精确定位损坏 / 截断点(如"偏移 0x1F0 处缺字节")。用结构化的枚举而非字符串,好处是错误可以**被可靠地分类处理**(`match` 不同变体走不同逻辑),还能携带类型化的细节;字符串只能打印、无法可靠地程序化判断。
3. 实现 `Display` 让 `ParseError` 能用 `println!("{}", err)` / `format!("{}", err)` 打印成**给人看的中文信息**,而不是 `Debug` 那种偏开发者的形式。我们的 `main` 正是靠它输出"无法识别的文件格式"这句话。
4. `fs::read` 失败时返回 `Err(io 错误)`,`?` 立刻让 `main` 返回这个 `Err`;Rust 运行时会打印 `Error: ...` 并以**非 0 退出码(1)**结束。我们实测看到了 `kind: NotFound` 和退出码 1。
5. `detect` 是不会失败的**纯底层原语**(任何输入都返回某个 `Format`,认不出就是 `Unknown`),适合被复用和单元测试;`identify` 在它之上做"语义判断"(太短 / 未知都算失败)并给出带原因的 `Result`。分层让底层稳定、上层灵活,各自职责清晰。

## 11. 下一步预告
**模块 A 到此完成**(4 步:格式识别 → 字节游标 → 多字节/字节序 → 错误处理),我们会开一条中英双语 PR 把它作为一个整体复盘。接着进入**模块 B**:用这套底层工具(游标 + 字节序 + Result 错误)**真正解析 Mach-O 文件头**——读出 magic、CPU 类型、文件类型,第一次让工具"读懂"你机器上真实可执行文件的结构。
