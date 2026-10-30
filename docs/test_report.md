# Classin 视频下载工具自动化测试报告

## 目录

1. [项目概览](#项目概览)
2. [测试计划](#测试计划)
3. [测试用例列表](#测试用例列表)
4. [测试策略](#测试策略)
5. [工具选择理由](#tool-selection)
6. [预期结果](#expected-results)
7. [注意事项](#notes)

---

## 项目概览 <a name="项目概览"></a>

### 原始项目信息

| 项目属性 | 值 |
|---------|---|
| **项目名称** | Classin 视频下载工具 |
| **版本** | v1.0.0 |
| **技术栈** | WPF + C# (.NET 8) |
| **UI框架** | Avalonia 11 |
| **界面风格** | 现代化灰色主题 |

### 功能概述

1. **视频解析功能**
   - 从剪贴板读取 Classin 视频请求体（JSON 格式）
   - 自动提取视频 URL 和关键参数
   - 显示解析结果列表

2. **视频下载功能**
   - 多线程下载支持
   - 实时显示下载进度、速度、剩余时间
   - 可配置的最大并发下载数
   - 自定义下载保存路径

3. **配置管理**
   - 最大同时下载视频数量（1-16）
   - 最大下载可用线程数量（1-256）
   - 初始化解析工具

4. **文件管理**
   - 打开下载目录
   - 更改下载保存路径

---

## 测试计划 <a name="测试计划"></a>

### 测试环境

| 组件 | 版本/说明 |
|-----|----------|
| **操作系统** | Windows 10/11 |
| **测试框架** | MSTest / NUnit |
| **UI自动化工具** | WinAppDriver + Appium |
| **构建工具** | .NET 8 SDK |
| **测试数据** | 本地 Classin 视频请求 JSON |

### 测试范围

✅ **功能测试**
- 视频解析器 URL 提取
- 视频下载完整流程
- 并发下载管理
- 下载速度计算

✅ **UI测试**
- 按钮点击响应
- 状态栏更新
- 进度条动画
- 列表项渲染

✅ **边界测试**
- 空剪贴板处理
- 无效 JSON 输入
- 网络异常处理
- 下载中止

❌ **性能测试** - 除非有明确要求，此处不做

❌ **兼容性测试** - Windows 10/11 通用

### 测试优先级

| 优先级 | 功能模块 | 说明 |
|-------|---------|------|
| P0 | 视频下载 | 核心功能，必须通过 |
| P0 | 视频解析 | 依赖功能，必须通过 |
| P1 | UI交互 | 用户体验关键 |
| P1 | 并发下载 | 性能相关 |
| P2 | 边界情况 | 稳定性保障 |

---

## 测试用例列表 <a name="测试用例列表"></a>

### 视频解析功能测试 <a name="video-parser-tests"></a>

| 用例ID | 用例标题 | 前置条件 | 步骤 | 预期结果 | 优先级 |
|-------|---------|---------|------|---------|-------|
| TC-PARSER-001 | 剪贴板为空时解析行为 | 剪贴板清空 | 点击"解析剪贴板中的请求头"按钮 | 显示错误提示："剪贴板为空" | P0 |
| TC-PARSER-002 | 无效 JSON 处理 | 剪贴板存在非 JSON 内容 | 点击"解析剪贴板中的请求头"按钮 | 显示错误提示：无效的 JSON 格式 | P0 |
| TC-PARSER-003 | 有效视频 URL 解析 | 剪贴板粘贴正确的 Classin 视频请求 JSON | 点击"解析剪贴板中的请求头"按钮 | 成功解析出视频列表，显示在结果区域 | P0 |
| TC-PARSER-004 | 解析结果显示字段 | 完成 TC-PARSER-003 | 查看解析结果列表 | 显示视频标题、文件大小、时长信息 | P1 |
| TC-PARSER-005 | 剪贴板内容复制 | 准备好的 Classin 视频请求 JSON | 点击"复制筛选关键词"按钮 | 成功复制关键词到剪贴板 | P2 |

### 视频下载功能测试 <a name="video-download-tests"></a>

| 用例ID | 用例标题 | 前置条件 | 步骤 | 预期结果 | 优先级 |
|-------|---------|---------|------|---------|-------|
| TC-DL-001 | 下载单个视频 | 已解析出视频列表 | 点击"下载全部视频"按钮 | 开始下载，显示进度条 | P0 |
| TC-DL-002 | 下载取消 | 下载进行中 | 点击"停止下载"按钮 | 取消当前下载，显示失败状态 | P0 |
| TC-DL-003 | 下载速度显示 | 下载进行中 | 观察状态栏 | 显示下载速度（KB/s 或 MB/s） | P1 |
| TC-DL-004 | 剩余时间计算 | 下载进行中 | 观察状态栏 | 显示剩余下载时间 | P1 |
| TC-DL-005 | 下载完成验证 | 下载完成 | 查看结果列表 | 显示"下载完成"，文件大小正确 | P0 |
| TC-DL-006 | 下载失败处理 | 网络中断或文件不可访问 | 模拟网络错误 | 显示"下载失败"，重试选项 | P0 |
| TC-DL-007 | 文件完整性校验 | 下载完成 | 检查磁盘文件 | 文件大小与解析时一致 | P1 |
| TC-DL-008 | 自定义下载路径 | 设置新的下载目录 | 点击"更改"按钮选择目录 | 后续下载文件保存至新目录 | P1 |
| TC-DL-009 | 打开下载目录 | 下载完成 | 点击"打开下载目录"按钮 | 系统文件资源管理器打开下载文件夹 | P1 |

### UI交互测试 <a name="ui-interaction-tests"></a>

| 用例ID | 用例标题 | 前置条件 | 步骤 | 预期结果 | 优先级 |
|-------|---------|---------|------|---------|-------|
| TC-UI-001 | 按钮点击响应 | 应用程序运行 | 依次点击所有功能按钮 | 按钮背景色变化（按下效果） | P0 |
| TC-UI-002 | 窗口尺寸适应 | 无 | 拖动窗口边缘改变尺寸 | 所有控件按比例缩放 | P1 |
| TC-UI-003 | 状态栏更新 | 下载进行中 | 实时观察状态栏 | 状态信息动态更新 | P0 |
| TC-UI-004 | 进度条动画 | 下载进行中 | 观察进度条 | 进度值从0%逐步.increment 到100% | P0 |
| TC-UI-005 | 列表项点击 | 有下载项 | 点击列表中的任意视频项 | 项选中，背景色变化 | P2 |
| TC-UI-006 | 滑动条调节 | 无 | 拖动并发下载滑块 | 滑动条值变化，显示文本更新 | P1 |
| TC-UI-007 | 工具提示显示 | 鼠标悬停 | 悬停在 UI 元素上 | 显示相应的工具提示 | P2 |
| TC-UI-008 | 错误提示弹窗 | 触发错误条件 | 发生错误情况 | 显示友好的错误消息框 | P0 |

### 并发下载测试 <a name="concurrent-download-tests"></a>

| 用例ID | 用例标题 | 前置条件 | 步骤 | 预期结果 | 优先级 |
|-------|---------|---------|------|---------|-------|
| TC-CONC-001 | 多视频并发下载 | 解析多个视频 URL | 点击"下载全部视频" | 按配置数启动并发下载 | P0 |
| TC-CONC-002 | 并发数限制 | 设置滑块值为 3 | 下载 5 个视频 | 同时仅运行 3 个下载任务 | P1 |
| TC-CONC-003 | 队列管理 | 最大并发数为 2，准备 5 个视频 | 开始下载 | 任务顺序排队，2 个活跃，3 个等待 | P1 |
| TC-CONC-004 | 资源约束 | 限制线程数 | 下载大文件 | 线程数符合配置，不超限 | P1 |
| TC-CONC-005 | 下载完成统计 | 所有下载完成 | 查看状态栏 | 显示总完成数量 | P0 |

### 边界情况测试 <a name="edge-case-tests"></a>

| 用例ID | 用例标题 | 前置条件 | 步骤 | 预期结果 | 优先级 |
|-------|---------|---------|------|---------|-------|
| TC-EDGE-001 | 空剪贴板 | 剪贴板清空 | 点击"解析"按钮 | 显示友好提示，不崩溃 | P0 |
| TC-EDGE-002 | 无效 JSON | 剪贴板粘贴非 JSON 文本 | 点击"解析"按钮 | 显示 JSON 解析错误 | P0 |
| TC-EDGE-003 | 网络超时 | 网络慢或不通 | 下载视频 | 显示超时错误，提供重试 | P0 |
| TC-EDGE-004 | 重复视频 | 同一视频 URL 多次 | 解析并下载 | 显示为同一视频，避免重复 | P1 |
| TC-EDGE-005 | 大文件下载 | 大文件 URL | 开始下载 | 显示正确文件大小，下载完成 | P1 |
| TC-EDGE-006 | 磁盘空间不足 | 磁盘空间 < 下载文件大小 | 开始下载 | 显示磁盘空间不足错误 | P0 |
| TC-EDGE-007 | 非法字符路径 | 输入非法字符的下载路径 | 设置下载目录 | 显示路径无效错误 | P1 |
| TC-EDGE-008 | 低于最小并发数 | 将滑块拖到最小值 | 观察滑块 | 最小值保持为 1 | P2 |
| TC-EDGE-009 | 超过最大线程数 | 将线程数调到 256 | 下载任务 | 线程数不超过 256 | P2 |

---

## 测试策略 <a name="test-strategy"></a>

### 测试模型

视频解析 → 视频列表 → 下载队列 → 下载执行 → 结果反馈

### 测试执行顺序

环境准备 → 登录/初始化 → 剪贴板测试 → 解析功能测试 → 下载功能测试 → 并发测试 → 边界测试 → 报告生成

### 测试数据准备

1. **有效测试数据**
   - 准备几条正确格式的 Classin 视频请求 JSON
   - 包含标题、文件大小、视频 URL

2. **错误测试数据**
   - 无效的 JSON 字符串
   - 空内容
   - 格式正确但字段缺失的 JSON

3. **网络测试数据**
   - 可访问的视频链接
   - 404/403 错误链接
   - 需要登录的视频链接

### 测试执行方法

1. **单独功能测试**：每个功能独立测试，确保功能点准确性
2. **集成测试**：连贯的工作流程测试（解析 → 下载）
3. **回归测试**：每日构建后自动运行核心用例

---

## 工具选择 <a name="tool-selection"></a>

### 推荐工具：WinAppDriver

#### 优势

| 优势 | 说明 |
|------|------|
| **官方支持** | Microsoft 官方维护，支持 WPF 应用 |
| **功能完整** | 支持 UI 元素定位、属性读取、事件触发 |
| **集成便捷** | 可集成到 CI/CD 流水线 |
| **跨平台** | 支持 Windows 桌面应用 |

#### 工具链

```
测试脚本 (C#) → MSTest / NUnit → WinAppDriver (WebDriver API) → Windows Application Driver → 目标 WPF 应用程序
```

#### 安装依赖

```powershell
winget install Microsoft.WindowsApplicationDriver
dotnet new mstest -n ClassinVideoDownloader.Tests
dotnet add package Appium.WebDriver
dotnet add package Selenium.Support
```

---

## 预期结果 <a name="expected-results"></a>

### 功能完整性

| 功能模块 | 期望通过率 | 说明 |
|---------|-----------|------|
| 视频解析 | 100% | 所有有效输入都能正确解析 |
| 视频下载 | 95%+ | 大多数视频能成功下载 |
| UI交互 | 100% | 所有按钮、控件响应正常 |
| 错误处理 | 100% | 无异常崩溃，友好提示 |

### 性能指标

| 指标 | 目标值 | 测试方式 |
|-----|--------|---------|
| 启动时间 | < 3 秒 | 计时器测量 |
| 解析响应 | < 1 秒 | 点击到结果显示 |
| 界面流畅度 | 60 FPS | 性能分析器 |
| 内存占用 | < 200 MB | 资源监控 |

### 稳定性指标

| 指标 | 目标值 |
|-----|--------|
| 连续运行 100 次不崩溃 | ✓ |
| 长时间下载不泄漏 | ✓ |
| 异常输入不崩溃 | ✓ |

---

## 注意事项 <a name="notes"></a>

### ⚠️ 重要声明：FlowLens 与 Classin 工具完全不同

**FlowLens 是 Rust + TUI (ratatui) 构建的终端网络包调试工具，与 Classin 视频下载工具并非同一类应用。**

| 项目 | Classin 视频下载工具 | FlowLens |
|-----|---------------------|----------|
| UI 类型 | WPF 桌面图形界面 | 终端 TUI 界面 |
| 功能领域 | Classin 视频解析下载 | HTTP/HTTPS 网络包捕获 |
| 技术栈 | C#/.NET 8/Avalonia | Rust/ratatui |

### 审查结论

**这两个软件并非同一功能的换语言重写版本，而是完全不同的应用。**

- **Classin 视频下载工具**：面向 Classin 在线教育平台，用户复制视频播放页的请求体进行视频下载
- **FlowLens**：面向网络调试开发者，捕获和分析 HTTP/HTTPS 请求响应包

### 测试范围说明

本报告**仅针对 Classin 视频下载工具**进行自动化测试方案设计，**不需要对比 FlowLens**。

---

## 附录 A：自动化测试代码框架示例

### 测试项目结构

```
ClassinVideoDownloader.Tests/
├── TestSettings/
│   └── app.config
├── TestData/
│   ├── valid_requests.json
│   └── invalid_requests.json
├── Tests/
│   ├── ParserTests.cs
│   ├── DownloadTests.cs
│   ├── UiInteractionTests.cs
│   └── ConcurrencyTests.cs
├── PageObjects/
│   └── MainWindowPage.cs
└── Utils/
    ├── ScreenshotHelper.cs
    └── TestDataHelper.cs
```

### 代码示例：MainWindowPage.cs

```csharp
using OpenQA.Selenium.Appium;
using OpenQA.Selenium.Appium.Windows;

namespace ClassinVideoDownloader.Tests.PageObjects
{
    public class MainWindowPage
    {
        private readonly WindowsDriver<WindowsElement> _driver;

        public MainWindowPage(WindowsDriver<WindowsElement> driver)
        {
            _driver = driver;
        }

        private By ParseButton => By.Name("2. 解析剪贴板中的请求头");
        private By DownloadButton => By.Name("3. 下载全部视频");
        private By ResultsList => By.Name("lstResults");
        private By StatusBar => By.Name("progressBar");

        public void ClickParseButton()
        {
            _driver.FindElement(ParseButton).Click();
        }

        public void ClickDownloadButton()
        {
            _driver.FindElement(DownloadButton).Click();
        }

        public bool IsDownloadComplete()
        {
            var progressBar = _driver.FindElement(StatusBar);
            return progressBar.GetAttribute("Value").Equals("100");
        }

        public int GetListItemCount()
        {
            return _driver.FindElement(ResultsList).FindElements(By.XPath(".//*")).Count;
        }
    }
}
```

### 代码示例：解析测试

```csharp
[TestFixture]
public class ParserTests
{
    private WindowsDriver<WindowsElement> _driver;
    private MainWindowPage _mainPage;

    [SetUp]
    public void Setup()
    {
        var options = new AppiumOptions
        {
            App = @"C:\Users\21306\Desktop\Classin视频解析下载工具-图形化v1.0.0\Classin视频解析下载工具.exe"
        };
        _driver = new WindowsDriver<WindowsElement>(new Uri("http://127.0.0.1:4723"), options);
        _mainPage = new MainWindowPage(_driver);
    }

    [Test]
    public void Parse_ValidJson_ReturnsResults()
    {
        // Arrange - 复制有效 JSON 到剪贴板
        Clipboard.SetText(TestDataHelper.GetValidVideoRequestJson());
        
        // Act
        _mainPage.ClickParseButton();
        Thread.Sleep(1000); // 等待解析
        
        // Assert
        Assert.That(_mainPage.GetListItemCount(), Is.GreaterThan(0));
    }

    [Test]
    public void Parse_EmptyClipboard_ShowsError()
    {
        // Arrange - 清空剪贴板
        Clipboard.Clear();
        
        // Act
        _mainPage.ClickParseButton();
        Thread.Sleep(1000);
        
        // Assert
        Assert.That(_mainPage.GetListItemCount(), Is.EqualTo(0));
    }

    [TearDown]
    public void TearDown()
    {
        _driver?.Quit();
    }
}
```

---

## 附录 B：参考资源

- [WinAppDriver 官方文档](https://github.com/microsoft/WinAppDriver)
- [Avalonia 官方文档](https://docs.avaloniaui.net/)
- [.NET 8 文档](https://learn.microsoft.com/zh-cn/dotnet/core/whats-new/dotnet-8)
- [MSTest 文档](https://learn.microsoft.com/zh-cn/dotnet/core/testing/unit-testing-with-mstest)

---

## 附录 C：测试执行步骤

### 1. 启动 WinAppDriver

```powershell
WinAppDriver.exe
```

### 2. 运行测试集

```powershell
dotnet test --filter "TestCategory=Regression"
```

### 3. 生成测试报告

```powershell
dotnet test /logger:trx
ReportGenerator -reports:**\*.trx -targetdir:TestResults
```

### 4. 截图辅助

```csharp
// 在测试方法中添加截图功能
public void TakeScreenshot(string testName)
{
    var screenshot = ((ITakesScreenshot)_driver).GetScreenshot();
    screenshot.SaveAsFile($"TestResults\{testName}.png");
}
```

---

## 附录 B：变更记录

| 日期 | 版本 | 变更内容 | 作者 |
|-----|------|---------|------|
| 2026-10-30 | v1.0 | 初始版本：自动化测试方案设计 | 自动化测试工程师 |
