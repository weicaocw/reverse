# Step 05：加 CI —— 每次 push/PR 自动构建 + 测试

> 模块：CI/工程化 ｜ 对应提交：`<待回填>` ｜ 测试：✅ 通过(本地 + CI) ｜ 上一步：[Step 04](step-04-error-handling.md)

## 0. 一句话目标
给仓库加上**持续集成(CI)**:每次 `git push` 或开 Pull Request,GitHub 自动在一台干净机器上跑一遍 `cargo build` + `cargo test`,确保主干永远是绿的。

## 1. 前置回顾
模块 A 完成后,我们已经有了**一套真正能跑的测试**(13 个)。现在正是引入 CI 的最佳时机——CI 一上来就有东西可跑、立刻有价值(过早加 CI 只是空转)。从这步起,任何人(包括未来的你)往仓库推坏代码,CI 都会立刻报红、挡住回归。

## 2. 先写测试(TDD)——本步是"配置",不是写函数
CI 这一步没有新的 Rust 函数要测,所以不写单元测试,而是用一个**可运行的验证**代替:本地先把 CI 将要跑的命令**原样跑一遍**,确认全绿,再让 CI 在云端复现。

```
$ cargo fmt --all -- --check   # 格式是否规范(本步先用它整理了一处)
$ cargo build --verbose        # 能否编译
$ cargo test --verbose         # 测试是否全过
test result: ok. 13 passed; 0 failed
```
本地这三条都绿,我们才有信心 CI 也会绿。(本步还顺手用 `cargo fmt` 规整了 `src/lib.rs` 里一处过长的行——这正是下一步要让 CI 强制的格式纪律的预演。)

## 3. 实现到通过(绿)——写 workflow 文件
新建 `.github/workflows/ci.yml`:
```yaml
name: CI
on:
  push:
  pull_request:
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --verbose
      - run: cargo test --verbose
```
逐行讲(这是一种叫 YAML 的配置格式,靠**缩进**表示层级):
- **`name: CI`**:这条工作流在 GitHub 页面上显示的名字。
- **`on:`**:**什么时候触发**。`push:` = 每次推代码;`pull_request:` = 每次开 / 更新 PR。两者都触发,意味着改动在合并前后都被把关。
- **`jobs:`**:工作流由一个或多个"作业(job)"组成。我们只有一个,叫 `test`。
- **`runs-on: ubuntu-latest`**:这个作业在 GitHub 提供的一台**全新的 Linux 虚拟机**上跑——干净环境,排除"在我机器上能跑"的假象。
- **`steps:`**:这个作业**按顺序**执行的步骤。
  - **`uses: actions/checkout@v4`**:`uses` 表示"用别人写好的现成动作"。`checkout` 把你仓库的代码克隆到这台机器上。
  - **`uses: dtolnay/rust-toolchain@stable`**:安装稳定版 Rust 工具链(`rustc` + `cargo`)。
  - **`run: cargo build --verbose`**:`run` 表示"执行一条 shell 命令"。这条编译项目。
  - **`run: cargo test --verbose`**:跑全部测试。**任何一个测试失败,这一步非 0 退出,整个 CI 变红。**

把它推上去后,GitHub 会自动执行,并在 commit / PR 旁边显示 ✅ 或 ❌。

## 4. 改了哪些文件 / 加了什么
- 新增 `.github/workflows/ci.yml`:定义 CI 工作流(build + test)。
- `src/lib.rs`:`cargo fmt` 规整了一处格式(无逻辑改动)。

## 5. 学到的语法 / 技巧
- **YAML**:一种靠缩进表达层级的配置文件格式,GitHub Actions 用它描述工作流。
- **GitHub Actions 概念**:`on`(触发条件)、`jobs`(作业)、`runs-on`(运行环境)、`steps`(步骤)、`uses`(复用现成动作)、`run`(跑命令)。
- **`cargo build` / `cargo test` 的 `--verbose`**:输出更详细,CI 里出错时方便定位。

## 6. 语言设计巧思
本步没有新的 Rust 语言点,但有一个工程层面的"Rust 生态巧思":**`cargo` 把"构建 / 测试 / 格式 / lint"统一成一套标准命令**(`cargo build`/`test`/`fmt`/`clippy`)。这意味着 CI 配置极其简短——不像有些语言要拼凑一堆工具。标准化的工具链让"在 CI 上复现本地检查"几乎零成本,这也是 Rust 项目 CI 通常只有十几行的原因。

## 7. 领域知识
本步不涉及逆向领域知识(属于工程基础设施)。但它对逆向项目同样重要:逆向工具要解析各种刁钻样本,**回归风险高**——今天修好的解析器,明天一次改动可能又崩在某个边界样本上。CI + 测试套件就是防止"修好的东西又坏掉"的安全网,让我们后续放心大胆地重构和加功能。

## 8. 软件设计理念
**自动化纪律 / 快速反馈**。人会忘记跑测试、会"我觉得这点小改动不用测吧",CI 把"每次都检查"变成不可绕过的机器纪律。它体现两个原则:① **主干始终可用(keep main green)**——任何时刻 checkout 出来都能编译、测试通过;② **尽早失败(fail fast)**——问题在 push / PR 阶段就暴露,而不是拖到很久以后才发现、难以定位。

## 9. 小测验(自测)
1. CI 是什么?它在每次 push / PR 时替你做了什么?
2. `on: push` 和 `on: pull_request` 同时配置,有什么意义?
3. 为什么要在 `ubuntu-latest` 这种"干净机器"上跑,而不是信任"我本地能跑"?
4. 如果某次改动让一个测试挂了,CI 会发生什么?这如何保护主干?
5. `uses` 和 `run` 两种步骤有什么区别?

## 10. 参考答案
1. CI(持续集成)= 每次推代码 / 开 PR 时,自动在云端干净环境跑一遍构建和测试等检查。它替你保证"改动没有破坏现有功能",无需手动记得跑。
2. `push` 在你直接推分支时触发,`pull_request` 在合并请求上触发。两者都配,能在"改动进入主干前(PR)"和"任何分支推送时"都进行把关,覆盖更全。
3. 本地环境可能装了特殊依赖、残留旧产物、或恰好掩盖了某个问题("在我机器上能跑")。干净虚拟机从零开始,能复现真实的"别人拿到代码能不能跑",排除环境偏差。
4. 跑测试的那一步会非 0 退出,整个 CI 标红,PR 上显示 ❌。这让"带着失败测试的代码"非常显眼、难以被误合并,从而保护主干始终是绿的。
5. `uses` 复用社区写好的现成动作(如 checkout、安装工具链),`run` 直接执行一条 shell 命令(如 `cargo test`)。前者是"拿来用的积木",后者是"自己敲的命令"。

## 11. 下一步预告
Step 06:给 CI **再加两层质量门**——`cargo fmt --all -- --check`(强制统一代码格式)和 `cargo clippy -D warnings`(静态检查,把 Rust 官方 lint 的建议当成错误)。这会让我们顺带认识 Rust 的"代码风格自动化"与"编译器之外的智能体检",并把"warning 也不放过"的工业级标准固化进 CI。
