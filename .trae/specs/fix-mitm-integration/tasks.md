# Tasks

- [ ] Task 1: Add timeout to test_tls_termination
  - [ ] SubTask 1.1: Wrap tokio::join! in tokio::time::timeout with 10s deadline
  - [ ] SubTask 1.2: Add panic! on timeout with clear message
- [ ] Task 2: Add timeout to test_plaintext_extraction
  - [ ] SubTask 2.1: Wrap tokio::join! in tokio::time::timeout with 10s deadline
  - [ ] SubTask 2.2: Add panic! on timeout with clear message
- [ ] Task 3: Verify all mitm_integration tests pass within timeout
  - [ ] SubTask 3.1: Run cargo test -p flowlens-tls --test mitm_integration
  - [ ] SubTask 3.2: Confirm no test hangs beyond 10 seconds

# Task Dependencies
- Task 2 depends on Task 1
- Task 3 depends on Task 1 and Task 2