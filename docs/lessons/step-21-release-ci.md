# Step 21:release 构建与 CI 产物(课程收官)

> 模块：F CLI 产品化(收官) ｜ 对应提交：`<待回填>` ｜ 测试：✅ 通过(CI 含发布作业) ｜ 上一步：[Step 20](step-20-error-handling-cli.md)

## 0. 一句话目标
给 CI 增加一个**发布作业**:在测试全绿后,用 `cargo build --release` 产出优化过的可执行文件,并作为可下载的**构建产物(artifact)**上传。

## 1. 前置回顾
模块 A–F 我们把 `revx` 从零做成了一个有库、有 CLI、有测试、有 CI 的多功能逆向工具。最后一块拼图:让它**可分发**。CI 之前只做"检查"(fmt/build/test/clippy);本步让它也"产出"——自动构建发布版二进制供下载。这是整个课程的收官。

## 2. 先写测试(验证)——CI 配置,用真实运行验证
和前面的 CI 步骤一样,本步是工作流配置,没有 Rust 单元测试。验证靠:① 本地 `cargo build --release` 成功;② 推上去后 CI 的 `release` 作业变绿、产物可下载。
```
$ cargo build --release
    Finished `release` profile [optimized]
```

## 3. 实现到通过(绿)——给 workflow 加发布作业
```yaml
jobs:
  test:
    # ... 原有的 fmt/build/test/clippy 四道门 ...

  release:
    needs: test                  # 关键:test 全绿才跑,绝不发布没过测试的版本
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release --verbose
      - uses: actions/upload-artifact@v4
        with:
          name: revx-linux-x86_64
          path: target/release/reverse
          retention-days: 7
```
逐点讲:
- **`release:` 是第二个作业**。一个 workflow 可以有多个作业(`jobs:` 下并列),默认并行。
- **`needs: test`**:声明 `release` **依赖** `test`——只有 `test` 作业成功,`release` 才会启动。这保证"发布的二进制一定通过了全部检查",绝不发出一个测试挂掉的版本。
- **`cargo build --release`**:release 构建,开优化(见 §7)。产物在 `target/release/`。
- **`actions/upload-artifact@v4`**:把指定路径的文件上传为 GitHub 的**构建产物**,可在 Actions 运行页面下载。`retention-days: 7` 保留 7 天。

`cargo build --release` 本地成功。推上去后,Actions 里会看到 `test` 和 `release` 两个作业,后者在前者绿后运行并产出 `revx-linux-x86_64`。

## 4. 改了哪些文件 / 加了什么
- `.github/workflows/ci.yml`:新增 `release` 作业(`needs: test` + release 构建 + 上传产物)。

## 5. 学到的语法 / 技巧(CI/工程)
- **多作业 workflow**:`jobs:` 下可并列多个作业。
- **`needs:`**:声明作业间依赖,串起"先测试、后发布"的顺序与门禁。
- **`cargo build --release`**:优化构建,产物在 `target/release/`。
- **`actions/upload-artifact`**:把构建产物上传供下载。
- **`retention-days`**:产物保留时长。

## 6. 语言设计巧思:debug vs release 构建
Rust(cargo)内置两套构建profile,体现"开发期"与"发布期"的不同诉求:

| | debug(默认 `cargo build`) | release(`--release`) |
|---|---|---|
| 优化 | 几乎不优化,编译快 | 大量优化(`-O`),运行快 |
| 体积 | 大(本项目 **4.5M**) | 小(本项目 **1.4M**) |
| debug 断言 / 溢出检查 | 开启(`debug_assert!`、整数溢出 panic) | 关闭(更快) |
| 编译速度 | 快(改完立刻测) | 慢(值得为发布等) |
| 用途 | 开发、跑测试 | 分发给用户、性能基准 |

我们实测同一份代码,release 比 debug **小了 3 倍多**(去掉了调试信息、内联和优化压缩了代码)。这套"开发用 debug 求快速反馈、发布用 release 求性能体积"的双 profile,是 cargo 替你内置好的工程实践——你只需在对的场合加一个 `--release`。

注意一个**安全相关的细节**:debug 构建里整数溢出会 panic(帮你抓 bug),release 里默认**回绕**(wrapping,不 panic)。所以涉及不可信输入的算术,不能依赖 release 的溢出 panic 来兜底——这也是我们 Step 10 用 `checked_add` 显式处理溢出的原因(无论哪种 profile 都安全)。

## 7. 领域知识:发布逆向工具的考量
- **strip 与符号**:发布版常 strip 掉符号表减小体积(还记得 Step 13 说被 strip 的程序符号很少吗?发布版自己就是个例子)。我们这里没 strip,留作可选增强。
- **可复现构建**:`Cargo.lock` 锁定依赖版本(Step 17),保证 CI 上构建出的二进制与本地一致——逆向工具尤其要可复现,否则"你的工具"和"我的工具"分析同一样本结果可能不同。
- **跨平台分发**:真实工具会用 CI 的**矩阵(matrix)**同时构建 Linux/macOS/Windows 多个产物。我们只产 Linux x86_64,扩展成矩阵是自然的下一步。
- **release 性能对逆向的意义**:逆向工具常要扫大文件、跑反汇编 / 熵分析,release 的优化让这些重活快得多——分析一个几百 MB 的样本时,debug 与 release 可能差几倍时间。

## 8. 软件设计理念
**门禁式流水线:质量门通过才放行产物**。`needs: test` 把 CI 串成一条**有方向的流水线**:检查(test)→ 发布(release)。发布严格依赖检查通过——这是"**质量门(quality gate)**"思想:每个阶段是一道闸,前一道不过,后一道不开。它保证了一个强不变式:**任何被发布的产物,都一定通过了全部测试与 lint**。把"正确性"作为"可分发"的前提用机器强制下来,而非靠人自觉,正是成熟工程的标志。这也为整个项目画上句点:从 Step 01 的一个 `detect` 函数,到此刻一条"改动 → 自动检查 → 自动产出可分发二进制"的完整流水线。

## 9. 小测验(自测)
1. `needs: test` 起什么作用?去掉它,`release` 作业的行为会怎样变?
2. release 构建相比 debug,在优化、体积、debug 断言上各有什么不同?
3. 我们实测 release 比 debug 小了约 3 倍,主要是因为什么?
4. debug 下整数溢出会 panic、release 下默认回绕。这对"处理不可信输入的算术"有什么提醒?(联系 Step 10)
5. `actions/upload-artifact` 让产物去了哪里?有什么用?

## 10. 参考答案
1. `needs: test` 声明 `release` 作业**依赖** `test` 作业——只有 test 全绿,release 才会运行。它构成一道"质量门",保证发布的二进制一定通过了全部检查。去掉它,`release` 会与 `test` **并行**运行、不再等待,可能产出一个测试其实挂掉的版本。
2. release **开启大量优化**(运行更快、编译更慢),**体积更小**(去调试信息、内联压缩),**关闭 debug 断言与整数溢出 panic**(更快但少了开发期检查);debug 则相反——编译快、体积大、带各种开发期检查,适合开发和测试。
3. 主要因为 release 去掉了**调试信息(debug symbols)**,并通过**优化(内联、死代码消除、压缩等)**减小了代码体积。debug 为了可调试保留大量符号和未优化代码,所以更大。
4. 提醒:**不能依赖 release 的溢出行为来保证安全**——release 默认回绕(不 panic),不会帮你抓溢出。处理不可信输入(如文件里的长度 / 偏移)的算术,必须**显式**用 `checked_add` / `checked_mul` 等检查(像 Step 10 那样),这样无论 debug 还是 release 都安全。
5. 它把构建产物上传到 **GitHub Actions 的运行记录页**,可在那里下载(本步设置保留 7 天)。用途:让使用者 / 团队无需自己编译就能拿到可执行文件,是"可分发"的基础;真实项目还会进一步发到 Releases 页或包管理器。

## 11. 全课程收官 🎉
**恭喜——`revx` 完成了!** 从 Step 01 一个识别魔数的函数,到现在:一个有库、有子命令 CLI、能解析 Mach-O 全结构(头/段/节区/符号/入口)、能 hex dump、提字符串、算熵判加壳、反汇编 x86-64、有友好错误处理、有完整 CI(fmt/build/test/clippy)与自动发布的**工业级多功能逆向工具**。

你一路学到的:**Rust** 的所有权 / 借用 / 生命周期、`enum`/`struct`/`trait`、`Option`/`Result`/`?`、迭代器、宏与派生、移动 vs 复制、浮点、依赖管理;**逆向** 的文件格式、字节序、加载命令 / 段 / 节区 / 符号 / 入口、字符串 / 熵 / 反汇编;**工程** 的 TDD、分层架构、关注点分离、契约式设计、CI/质量门。

可选的继续方向(都已在课程里埋了引子):支持 **fat/通用二进制**(Step 07 的发现)、**ELF/PE** 多格式、符号 **demangle**(Step 13)、**UTF-16 字符串**(Step 15)、反汇编**多架构**(arm64)、CI **多平台矩阵**(Step 21)。每一个都是在现有干净架构上加一块,正是好设计的回报。
