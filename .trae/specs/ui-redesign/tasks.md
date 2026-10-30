# Tasks

## Phase 1: Analysis & Planning
- [ ] Task 1: Analyze current UI state
  - [ ] SubTask 1.1: List all UI-related files and their issues
  - [ ] SubTask 1.2: Document current data flow between Rust and Svelte
  - [ ] SubTask 1.3: Identify all broken Svelte syntax

## Phase 2: Fix Svelte Components
- [ ] Task 2: Fix Svelte 5 syntax in all components
  - [ ] SubTask 2.1: Fix Header.svelte - use `$state()` runes
  - [ ] SubTask 2.2: Fix Sidebar.svelte - use `$props()` for events
  - [ ] SubTask 2.3: Fix TrafficTable.svelte - use proper event handlers
  - [ ] SubTask 2.4: Fix RightPanel.svelte - use `$derived` for tab content
  - [ ] SubTask 2.5: Fix App.svelte - wire up component communication

## Phase 3: Data Binding
- [ ] Task 3: Connect Rust backend to Svelte frontend
  - [ ] SubTask 3.1: Add Tauri invoke commands for capture control
  - [ ] SubTask 3.2: Add Tauri event listeners for traffic updates
  - [ ] SubTask 3.3: Implement traffic filtering in frontend
  - [ ] SubTask 3.4: Add request detail view

## Phase 4: Theme & Styling
- [ ] Task 4: Fix CSS theming
  - [ ] SubTask 4.1: Move hardcoded colors to CSS variables
  - [ ] SubTask 4.2: Add dark/light theme toggle
  - [ ] SubTask 4.3: Fix component styling consistency

## Phase 5: Cleanup
- [ ] Task 5: Remove deprecated GUI code
  - [ ] SubTask 5.1: Remove `crates/flowlens-ui/src/gui.rs`
  - [ ] SubTask 5.2: Update Cargo.toml to remove egui deps
  - [ ] SubTask 5.3: Update README to reflect new UI

## Task Dependencies
- Task 3 depends on Task 2
- Task 4 depends on Task 2
- Task 5 depends on Task 3, Task 4
