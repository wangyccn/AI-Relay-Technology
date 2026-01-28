# 🎯 立即开始使用 - 3 分钟快速指南

## 🚀 第一步：验证修复（30 秒）

```bash
# 1. 应用已经在运行，检查状态
python diagnostic.py
```

**预期结果**：
```
✅ API connectivity: OK
✅ Recent errors: None found (或少量错误)
✅ Panic logs: None found
✅ Log volume is healthy
```

---

## 📊 第二步：查看日志（1 分钟）

### 查看错误摘要
```bash
python view_logs.py errors
```

### 实时监控日志
```bash
python view_logs.py follow
```

**提示**：按 `Ctrl+C` 停止实时监控

---

## 🧪 第三步：测试修复（1 分钟）

### 测试错误日志记录
```bash
# 发送一个会失败的请求
curl -X POST http://127.0.0.1:8787/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer test-token" \
  -d '{"model":"invalid-model","messages":[{"role":"user","content":"test"}]}'
```

### 查看刚才的错误日志
```bash
python view_logs.py errors
```

**预期看到**：
```
[ERROR] [middleware] Model lookup failed: model_id='invalid-model', error=...
[ERROR] [forward_error] Returning error response: status=404, type=model_not_found, message=...
```

---

## ✅ 完成！修复已生效

### 现在您可以：

1. **查看日志**
   ```bash
   python view_logs.py errors      # 查看错误
   python view_logs.py follow      # 实时监控
   python view_logs.py source openai  # 查看特定来源
   ```

2. **运行诊断**
   ```bash
   python diagnostic.py
   ```

3. **测试 GLM**（如果您有 GLM 配置）
   ```bash
   python test_glm_stream.py
   ```

---

## 📚 需要更多帮助？

### 快速参考
- **常用命令**: 查看 `QUICK_REFERENCE.md`
- **详细使用**: 查看 `README_USAGE.md`
- **问题排查**: 查看 `TESTING_GUIDE.md`

### 一键查看文档
```bash
# Windows
notepad QUICK_REFERENCE.md

# Linux/Mac
cat QUICK_REFERENCE.md | less
```

---

## 🎉 修复内容回顾

### ✅ 已修复的问题
1. ✅ GLM `reasoning_content` 字段现在可以正确处理
2. ✅ 流式响应不再返回空内容
3. ✅ Rust panic 会被捕获并记录到日志
4. ✅ 所有错误都有详细的日志记录

### ✅ 新增功能
1. ✅ 19 个新的日志记录点
2. ✅ 3 个实用工具脚本
3. ✅ 7 份详细文档

---

## 💡 日常使用建议

### 每天检查（推荐）
```bash
python view_logs.py errors
```

### 遇到问题时
```bash
# 1. 运行诊断
python diagnostic.py

# 2. 查看错误
python view_logs.py errors

# 3. 实时监控
python view_logs.py follow
```

### 部署前检查
```bash
# 运行完整诊断
python diagnostic.py

# 查看最近的错误
python view_logs.py errors

# 检查 Panic 日志
python view_logs.py panic
```

---

## 🔥 最常用的 3 个命令

记住这 3 个命令就够了：

```bash
# 1. 诊断系统健康状况
python diagnostic.py

# 2. 查看错误日志
python view_logs.py errors

# 3. 实时监控日志
python view_logs.py follow
```

---

## 📞 获取帮助

### 查看文档
```bash
# 快速参考（最常用）
cat QUICK_REFERENCE.md

# 完整使用指南
cat README_USAGE.md

# 测试指南
cat TESTING_GUIDE.md

# 修复详情
cat BUGFIX_SUMMARY.md
```

### 文档索引
| 文档 | 用途 | 推荐度 |
|------|------|--------|
| `QUICK_REFERENCE.md` | 快速参考卡片 | ⭐⭐⭐⭐⭐ |
| `README_USAGE.md` | 完整使用指南 | ⭐⭐⭐⭐ |
| `TESTING_GUIDE.md` | 测试和排查 | ⭐⭐⭐ |
| `BUGFIX_SUMMARY.md` | 修复详情 | ⭐⭐ |
| `FINAL_REPORT.md` | 工作报告 | ⭐ |

---

## ✨ 提示

### Windows 用户
如果看到乱码，使用：
```bash
python view_logs.py errors | more
```

### 想要彩色输出
工具已经支持彩色输出，直接使用即可！

### 想要导出日志
```bash
curl -s "http://127.0.0.1:8787/api/logs?level=error&limit=100" > errors.json
```

---

## 🎊 恭喜！

您已经完成了快速入门。现在您可以：

- ✅ 查看和监控日志
- ✅ 诊断系统问题
- ✅ 测试 GLM 功能
- ✅ 排查错误

**开始使用吧！** 🚀

---

**版本**: 1.0.0
**更新**: 2026-01-18

---

## 🔖 收藏这个页面

把这个文件加入书签，随时查看常用命令！

```bash
# 最常用的命令
python diagnostic.py              # 诊断
python view_logs.py errors        # 查看错误
python view_logs.py follow        # 实时监控
```

**就这么简单！** ✨
