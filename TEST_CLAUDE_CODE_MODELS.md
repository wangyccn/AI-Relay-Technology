# Claude Code 特殊模型功能测试说明

## 功能概述
当将模型接入 Claude Code 时，系统会自动生成两个系统保留的特殊模型：
- `claude-sonnet-4-5-20250929` - 指向用户选择的主模型
- `claude-haiku-4-5-20251001` - 指向用户选择的快速模型

## 测试步骤

### 1. 启动应用
```bash
npm run tauri dev
```

### 2. 配置模型路由
1. 进入"模型路由"页面
2. 确保已配置至少一个上游（在"设置"页面）
3. 添加至少一个模型，例如：
   - 模型ID: `claude-3-5-sonnet-20241022`
   - 上游: 选择已配置的上游
   - 优先级: 50
4. 点击"保存"

### 3. 配置 Claude Code
1. 在"自动配置 AI CLI"部分
2. "选择主模型"下拉框中选择刚添加的模型
3. （可选）"选择快速模型"选择另一个模型
4. 点击 Claude Code 卡片上的"配置"按钮

### 4. 验证特殊模型创建
配置成功后，在模型路由表格中应该能看到：

```
模型 ID                    显示名                                  提供方
claude-sonnet-4-5-20250929 Claude Code Main Model (Reserved)     anthropic [CLAUDE CODE]
claude-haiku-4-5-20251001  Claude Code Fast Model (Reserved)     anthropic [CLAUDE CODE]
```

**特殊标识：**
- ✅ 橙色渐变背景
- ✅ "CLAUDE CODE" 徽章（橙色）
- ✅ "仅限Claude Code" 标签
- ✅ 优先级显示为 `100*`（系统保留）
- ✅ 编辑和删除按钮被禁用

### 5. 验证 Auto 路由
- Auto 路由应该正常工作
- Auto 不会调用这两个特殊模型
- Auto 仍然使用优先级最高的非临时模型

### 6. 验证模型选择列表
- 在"自动配置 AI CLI"的模型选择下拉框中
- **不应该**看到 `auto` 模型
- **不应该**看到临时模型
- **不应该**看到 `claude-sonnet-4-5-20250929` 或 `claude-haiku-4-5-20251001`

## 预期行为

### ✅ 应该看到
1. 配置成功后，表格中出现两个新的特殊模型
2. 这两个模型有橙色背景和 "CLAUDE CODE" 徽章
3. 这两个模型的编辑/删除按钮是禁用的
4. 配置后需要刷新页面才能看到新模型（已自动刷新）
5. 表格支持横向滚动，不会出现内容换行

### ❌ 不应该看到
1. Auto 模型出现在 AI CLI 配置的模型选择列表中
2. 表格内容换行显示
3. 特殊模型可以被手动编辑或删除

## 故障排查

### 如果特殊模型没有显示
1. 检查浏览器控制台是否有错误
2. 检查后端日志：`src-tauri/target/debug/ccr.log`
3. 确认配置确实成功了（检查 `~/.claude/settings.json`）
4. 手动刷新页面

### 如果 Auto 模型仍然在选择列表中
- 清除浏览器缓存
- 刷新页面
- 检查 `availableModels` 过滤逻辑

## 代码变更位置

### 前端
- `src/pages/Models.tsx:170-174` - 过滤 Auto 和临时模型
- `src/pages/Models.tsx:300-338` - 配置后重新加载设置
- `src/pages/Models.tsx:662-755` - 特殊模型的显示和禁用编辑/删除
- `src/App.css:887-909` - 表格滚动条和防止换行

### 后端
- `src-tauri/src/autoconfig.rs:408-527` - 创建特殊模型的逻辑
- `src-tauri/src/autoconfig.rs:433-440` - 使用固定模型名称配置 Claude Code

## 技术细节

### 特殊模型的工作原理
1. 用户选择真实模型（如 `claude-3-5-sonnet-20241022`）
2. 系统创建临时模型 `claude-sonnet-4-5-20250929`，指向真实模型
3. Claude Code 配置使用 `claude-sonnet-4-5-20250929` 作为模型名
4. 当 Claude Code 请求时，CCR 解析这个名称并路由到真实模型
5. Auto 路由不受影响，继续使用正常模型优先级

### 为什么需要这个功能
- Claude Code 需要固定的模型名称
- 用户希望能够更换底层模型而不需要重新配置
- 保留真实模型的灵活性
