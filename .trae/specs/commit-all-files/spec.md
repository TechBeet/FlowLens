# Commit All Files to GitHub Spec

## Why
用户要求将工作区所有代码提交到 GitHub，每个文件单独提交一次，禁止合并提交。

## What Changes
- 将所有修改的文件（M）逐个提交
- 将所有新增的文件（??）逐个提交
- 将所有删除的文件（D）逐个提交
- 每个文件使用独立的 git commit
- 推送到远程 GitHub 仓库

## Impact
- Affected code: 整个工作区（63个文件变更）
- 包括：Cargo.lock, Cargo.toml, crates/*, ui/*, docs/*, .trae/*, LICENSE, README.md

## ADDED Requirements
### Requirement: 逐文件提交
系统 SHALL 将每个变更的文件作为独立的 git commit 提交。

#### Scenario: 提交单个文件
- **WHEN** 用户执行提交操作
- **THEN** 每个文件生成一个独立的 commit，commit message 包含文件路径

## MODIFIED Requirements
### Requirement: 无合并提交
系统 SHALL NOT 使用 merge commit 或 squash commit。

## REMOVED Requirements
无