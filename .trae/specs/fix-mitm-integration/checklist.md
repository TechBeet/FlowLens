# 检查清单

- [ ] test_tls_termination 使用 tokio::time::timeout 包装 tokio::join!
- [ ] test_plaintext_extraction 使用 tokio::time::timeout 包装 tokio::join!
- [ ] cargo test -p flowlens-tls --test mitm_integration 在 15 秒内完成
- [ ] 所有 6 个测试通过
- [ ] 无测试输出 "has been running for over 60 seconds"