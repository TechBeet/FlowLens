# Tasks

- [x] Task 1: Add timeout to test_tls_termination
  - [x] SubTask 1.1: Wrap tokio::join! in tokio::time::timeout with 10s deadline
  - [x] SubTask 1.2: Add panic! on timeout with clear message
- [x] Task 2: Add timeout to test_plaintext_extraction
  - [x] SubTask 2.1: Wrap tokio::join! in tokio::time::timeout with 10s deadline
  - [x] SubTask 2.2: Add panic! on timeout with clear message
- [x] Task 3: Verify all mitm_integration tests pass within timeout
  - [x] SubTask 3.1: Run cargo test -p flowlens-tls --test mitm_integration
  - [x] SubTask 3.2: Confirm no test hangs beyond 10 seconds

# Task Dependencies
- Task 2 depends on Task 1
- Task 3 depends on Task 1 and Task 2