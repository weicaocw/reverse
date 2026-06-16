# Step 06：CI 再加两层质量门 —— 格式检查 + clippy

> 模块：CI/工程化 ｜ 对应提交：`<待回填>` ｜ 测试：✅ 通过(本地 + CI) ｜ 上一步：[Step 05](step-05-ci-build-test.md)

## 0. 一句话目标
给 CI 增加两道自动质量门:① `cargo fmt --all -- --check`(强制统一代码格式),② `cargo clippy --all-targets -- -D warnings`(把 Rust 官方 lint 的每条建议当成错误,一条都不放过)。

## 1. 前置回顾
Step 05 我们让 CI 会自动 `build + test`。但"能编译、测试过"只是底线。工业级项目还要求:**代码风格统一**(避免无谓的格式争论和 diff 噪音)、**没有可疑写法**(很多 bug 在编译能过、但写法不地道时就埋下了)。本步把这两道门也固化进 CI。

## 2. 先写测试(验证)——本步仍是配置
和 Step 05 一样,没有新函数,用"本地原样跑一遍 CI 命令"来验证:
```
$ cargo fmt --all -- --check        # 退出 0 = 格式已规范
$ cargo clippy --all-targets -- -D warnings
    Finished `dev` profile ... (无警告)
$ cargo test
test result: ok. 13 passed; 0 failed
```
三道门本地全绿,推上去 CI 也会绿。

## 3. 实现到通过(绿)——往 workflow 追加两步
```yaml
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy          # 额外装上这两个组件
      - run: cargo fmt --all -- --check         # 门一:格式
      - run: cargo build --verbose
      - run: cargo test --verbose
      - run: cargo clippy --all-targets -- -D warnings   # 门二:lint
```
逐点讲:
- **`with: components: rustfmt, clippy`**:`rustfmt`(格式化器)和 `clippy`(lint 工具)是可选组件,这里让工具链顺带装上,后面两条命令才有得用。
- **`cargo fmt --all -- --check`**:`cargo fmt` 会按官方风格**重排**代码;加 `--check` 则**只检查不改**——若有任何不符合规范处,就打印 diff 并以非 0 退出(CI 变红)。`--all` 覆盖工作区所有包,`--` 后面的参数是传给底层 `rustfmt` 的。
- **`cargo clippy --all-targets -- -D warnings`**:`clippy` 是比编译器更"啰嗦"的智能体检,能发现"虽然能编译但不地道 / 可能有坑"的写法(比如多余的克隆、可化简的表达式、易错的比较)。`--all-targets` 连测试代码也查;`-D warnings` 把**所有警告升级为错误**——这是"零警告"纪律的关键:平时容易忽略的 warning,在 CI 里直接拦住。

## 4. 改了哪些文件 / 加了什么
- `.github/workflows/ci.yml`:工具链增加 `rustfmt, clippy` 组件;新增 `cargo fmt --check` 与 `cargo clippy -D warnings` 两个步骤。

## 5. 学到的语法 / 技巧
- **`cargo fmt`**:Rust 官方代码格式化器,统一缩进 / 换行 / 空格等风格。`--check` 只检查。
- **`cargo clippy`**:Rust 官方 lint 工具,给出超出编译器范围的改进建议。
- **`-D warnings`**:`-D` = deny(拒绝),把某类诊断当错误处理;`-D warnings` 即"任何警告都视为错误"。
- **GitHub Actions 的 `with:`**:给某个 `uses` 动作传参数(这里指定要装的组件)。

## 6. 语言设计巧思
**官方统一的格式与 lint,是 Rust 生态的一大优势**。很多语言的格式风格五花八门、团队各执一词;Rust 直接给了官方 `rustfmt`,**没有"风格之争"**——格式由工具裁定,省下大量口水和 review 噪音。`clippy` 则把社区多年总结的"地道写法 / 常见坑"沉淀成数百条自动检查,相当于一位资深 Rust 工程师在旁边帮你 review。

`-D warnings` 背后是一种哲学:**警告不是"可以无视的提示",而是"还没处理的问题"**。一旦允许警告堆积,真正重要的警告就会淹没在噪音里。把警告当错误,逼你要么修掉、要么显式标注"我知道这里这样写"(`#[allow(...)]`),始终保持"零警告"的干净状态。这与 Rust"让问题在编译期暴露"的整体风格一脉相承。

## 7. 领域知识
本步不涉及逆向领域知识(属工程基础设施)。但 clippy 对逆向 / 解析代码尤其有用:解析二进制充满位运算、类型转换、边界处理,clippy 能揪出"可能截断的 `as` 转换""手写却有现成方法的循环""可疑的比较"等,正是这类代码最容易出隐蔽 bug 的地方。

## 8. 软件设计理念
**质量门分层 + 尽早失败**。我们把检查按"由快到慢、由基础到进阶"排成流水线:格式 → 构建 → 测试 → lint。每一道都是一扇必须通过的门,任何一扇红了就拦住合并。这体现"自动化纪律"——不依赖人的自觉,而是用机器保证每次改动都达到统一标准。代价几乎为零(十几行配置),收益是长期的可维护性。

## 9. 小测验(自测)
1. `cargo fmt` 和 `cargo fmt -- --check` 有什么区别?CI 里为什么用后者?
2. clippy 和编译器(rustc)的职责有何不同?
3. `-D warnings` 是什么意思?为什么要"把警告当错误"?
4. 为什么把格式检查放在构建 / 测试**之前**?(提示:从"快慢"和"反馈"角度想)
5. 如果你确实需要保留一处 clippy 看不顺眼但合理的写法,有什么办法不让 CI 因它报红?

## 10. 参考答案
1. `cargo fmt` 会**直接重排**你的代码文件;`cargo fmt -- --check` **只检查不修改**,发现不规范就报错退出。CI 不应改你的代码,只应"判定合格与否",所以用 `--check`。
2. 编译器(rustc)负责"能不能编译、类型对不对、内存安不安全";clippy 负责"虽然能编译,但写法是否地道、有没有可疑模式 / 可化简之处"。clippy 是编译之上的"经验型体检"。
3. `-D warnings` 把所有警告升级为错误,导致只要有警告 CI 就失败。这样能防止警告堆积、淹没重要提示,逼团队始终保持"零警告"的干净代码。
4. 格式检查最快,把它放最前面能**最快给出反馈**:如果只是格式问题,几秒就报红,不必白等几分钟的编译和测试。这是"尽早失败、节省反馈时间"。
5. 在那处代码上加 `#[allow(clippy::某条 lint 名)]` 显式豁免——明确表达"我知道 clippy 的意见,但此处有意为之"。这比全局关掉检查好,因为豁免是局部、有记录的。

## 11. 下一步预告
工程基础设施(CI 三道门:fmt / build+test / clippy)就位。我们会把这个 CI 模块开成一条 PR 合入主干,让之后**每个 PR 都自动受这三道门保护**。然后正式进入**模块 B**:用模块 A 的底层工具(游标 + 字节序 + Result)**真正解析 Mach-O 文件头**,读出 magic、CPU 类型、文件类型——让 `revx` 第一次"读懂"你机器上真实可执行文件的结构。
