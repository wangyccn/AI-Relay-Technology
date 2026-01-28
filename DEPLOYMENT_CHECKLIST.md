# 🚀 部署检查清单

## 📋 部署前必查项目

### 1. 代码编译 ✅
```bash
cd src-tauri
cargo build --release
```
- [ ] 编译成功，无错误
- [ ] 编译成功，无警告
- [ ] 生成可执行文件

---

### 2. 功能测试 ✅

#### 2.1 应用启动
```bash
cargo run --release
```
- [ ] 应用正常启动
- [ ] 端口 8787 正常监听
- [ ] 无启动错误

#### 2.2 API 连接性
```bash
curl http://127.0.0.1:8787/api/stats
```
- [ ] API 正常响应
- [ ] 返回正确的 JSON 格式

#### 2.3 日志系统
```bash
curl "http://127.0.0.1:8787/api/logs?limit=10"
```
- [ ] 日志 API 正常工作
- [ ] 返回日志数据
- [ ] 日志格式正确

---

### 3. 错误处理测试 ✅

#### 3.1 无效模型测试
```bash
curl -X POST http://127.0.0.1:8787/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer test-token" \
  -d '{"model":"non-existent-model","messages":[{"role":"user","content":"test"}]}'
```
- [ ] 返回正确的错误响应
- [ ] 错误被记录到日志
- [ ] 日志包含详细信息

#### 3.2 日志记录验证
```bash
python view_logs.py errors
```
- [ ] 可以查看错误日志
- [ ] 日志包含上下文信息
- [ ] 日志级别正确

---

### 4. 工具测试 ✅

#### 4.1 日志查看工具
```bash
python view_logs.py errors
python view_logs.py follow
python view_logs.py source openai
```
- [ ] 工具正常运行
- [ ] 输出格式正确
- [ ] 过滤功能正常

#### 4.2 诊断工具
```bash
python diagnostic.py
```
- [ ] 工具正常运行
- [ ] 生成诊断报告
- [ ] 检查项目完整

---

### 5. 性能测试 ✅

#### 5.1 日志性能
```bash
# 发送多个请求，观察日志写入性能
for i in {1..10}; do
  curl -X POST http://127.0.0.1:8787/v1/chat/completions \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer test-token" \
    -d '{"model":"test","messages":[{"role":"user","content":"test"}]}' &
done
wait
```
- [ ] 日志写入不阻塞请求
- [ ] 应用响应正常
- [ ] 无性能问题

#### 5.2 内存使用
```bash
# 观察应用内存使用
# Windows: 任务管理器
# Linux: top 或 htop
```
- [ ] 内存使用正常
- [ ] 无内存泄漏
- [ ] 长时间运行稳定

---

### 6. 文档完整性 ✅

- [ ] `COMPLETION_SUMMARY.md` - 修复完成总结
- [ ] `BUGFIX_SUMMARY.md` - 详细修复说明
- [ ] `TESTING_GUIDE.md` - 测试指南
- [ ] `VERIFICATION_REPORT.md` - 验证报告
- [ ] `README_USAGE.md` - 使用指南
- [ ] `QUICK_REFERENCE.md` - 快速参考
- [ ] `DEPLOYMENT_CHECKLIST.md` - 本检查清单

---

### 7. 工具脚本完整性 ✅

- [ ] `view_logs.py` - 日志查看工具
- [ ] `diagnostic.py` - 系统诊断工具
- [ ] `test_glm_stream.py` - GLM 测试脚本

---

## 🔍 详细验证步骤

### 步骤 1: 清理环境
```bash
# 停止现有应用
# Windows: 任务管理器结束进程
# Linux: killall tauri-app

# 清理旧的构建
cd src-tauri
cargo clean
```

### 步骤 2: 重新构建
```bash
# Release 模式构建
cargo build --release

# 检查构建结果
ls -lh target/release/tauri-app*
```

### 步骤 3: 启动应用
```bash
# 启动应用
cargo run --release

# 或直接运行可执行文件
./target/release/tauri-app
```

### 步骤 4: 基础功能测试
```bash
# 1. 检查端口
netstat -ano | findstr "8787"

# 2. 测试 API
curl http://127.0.0.1:8787/api/stats

# 3. 测试日志 API
curl "http://127.0.0.1:8787/api/logs?limit=10"
```

### 步骤 5: 错误处理测试
```bash
# 1. 测试无效模型
curl -X POST http://127.0.0.1:8787/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer test-token" \
  -d '{"model":"invalid","messages":[{"role":"user","content":"test"}]}'

# 2. 查看错误日志
python view_logs.py errors

# 3. 验证日志内容
curl -s "http://127.0.0.1:8787/api/logs?level=error&limit=5" | python -m json.tool
```

### 步骤 6: 工具测试
```bash
# 1. 测试日志查看工具
python view_logs.py errors
python view_logs.py all 10

# 2. 测试诊断工具
python diagnostic.py

# 3. 测试 GLM 脚本（如果有 GLM 配置）
python test_glm_stream.py
```

### 步骤 7: 压力测试
```bash
# 发送多个并发请求
for i in {1..20}; do
  curl -X POST http://127.0.0.1:8787/v1/chat/completions \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer test-token" \
    -d '{"model":"test","messages":[{"role":"user","content":"test '$i'"}]}' &
done
wait

# 检查日志
python view_logs.py errors
```

### 步骤 8: 长时间运行测试
```bash
# 让应用运行至少 1 小时
# 期间定期检查：
# - 内存使用
# - CPU 使用
# - 日志数量
# - 错误率

# 每 10 分钟检查一次
while true; do
  echo "=== $(date) ==="
  python diagnostic.py
  sleep 600
done
```

---

## 📊 验证标准

### 必须通过的标准 ✅

1. **编译成功**
   - 无编译错误
   - 无编译警告

2. **应用启动**
   - 正常启动
   - 端口正常监听
   - 无启动错误

3. **API 功能**
   - 所有 API 端点正常
   - 返回正确的响应格式

4. **错误处理**
   - 错误被正确捕获
   - 错误被记录到日志
   - 错误响应格式正确

5. **日志系统**
   - 日志正常写入
   - 日志查询正常
   - 日志格式正确

6. **工具脚本**
   - 所有工具正常运行
   - 输出格式正确

### 建议通过的标准 ⭐

1. **性能**
   - 响应时间 < 100ms（非流式）
   - 日志写入不阻塞请求
   - 内存使用稳定

2. **稳定性**
   - 长时间运行无崩溃
   - 无内存泄漏
   - 错误率 < 1%

3. **可维护性**
   - 日志信息完整
   - 错误易于诊断
   - 文档齐全

---

## 🚨 常见问题及解决方案

### 问题 1: 编译失败
```
error: could not compile `tauri-app`
```
**解决**:
```bash
# 清理并重新构建
cargo clean
cargo build --release
```

### 问题 2: 端口被占用
```
Error: Address already in use (os error 48)
```
**解决**:
```bash
# 查找占用端口的进程
netstat -ano | findstr "8787"

# 结束进程
taskkill /F /PID <PID>
```

### 问题 3: 日志 API 无响应
```
curl: (7) Failed to connect to 127.0.0.1 port 8787
```
**解决**:
```bash
# 检查应用是否运行
netstat -ano | findstr "8787"

# 重启应用
cargo run --release
```

### 问题 4: 工具脚本报错
```
ModuleNotFoundError: No module named 'requests'
```
**解决**:
```bash
# 安装依赖
pip install requests
```

---

## 📝 部署记录模板

### 部署信息
- **部署日期**: _______________
- **部署人员**: _______________
- **版本号**: v1.0.0
- **环境**: [ ] 测试 [ ] 生产

### 检查结果
- [ ] 代码编译成功
- [ ] 应用启动正常
- [ ] API 功能正常
- [ ] 错误处理正常
- [ ] 日志系统正常
- [ ] 工具脚本正常
- [ ] 性能测试通过
- [ ] 文档完整

### 测试结果
- **基础功能测试**: [ ] 通过 [ ] 失败
- **错误处理测试**: [ ] 通过 [ ] 失败
- **性能测试**: [ ] 通过 [ ] 失败
- **压力测试**: [ ] 通过 [ ] 失败

### 问题记录
1. _______________________________________________
2. _______________________________________________
3. _______________________________________________

### 部署决策
- [ ] 批准部署
- [ ] 需要修复后重新测试
- [ ] 不批准部署

### 签名
- **测试人员**: _______________ 日期: _______________
- **审核人员**: _______________ 日期: _______________
- **批准人员**: _______________ 日期: _______________

---

## 🎯 部署后监控

### 第一天
```bash
# 每小时检查一次
python diagnostic.py
python view_logs.py errors
```

### 第一周
```bash
# 每天检查一次
python diagnostic.py

# 查看错误趋势
curl -s "http://127.0.0.1:8787/api/logs?level=error&limit=100" | jq '.logs | length'
```

### 长期监控
```bash
# 每周检查
python diagnostic.py

# 查看日志量
curl -s "http://127.0.0.1:8787/api/logs?limit=1" | jq '.total'

# 清理旧日志（如果需要）
# 参考 README_USAGE.md 中的日志清理部分
```

---

## ✅ 最终确认

在部署到生产环境前，请确认：

- [ ] 所有测试都通过
- [ ] 文档已更新
- [ ] 团队成员已培训
- [ ] 回滚计划已准备
- [ ] 监控已设置
- [ ] 备份已完成

**签名**: _______________ 日期: _______________

---

**检查清单版本**: 1.0.0
**最后更新**: 2026-01-18
