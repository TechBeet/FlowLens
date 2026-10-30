# UI Redesign Spec

## Why
The current FlowLens UI is severely broken and unusable:
- Two conflicting UI implementations (egru GUI vs Tauri/Svelte shell)
- Svelte components use incorrect syntax (`$state()` instead of Svelte 5 runes)
- No data binding - traffic table shows empty hardcoded data
- Missing Tauri event listeners for real-time packet capture
- Dark theme colors hardcoded in components instead of using CSS variables
- Header/Sidebar/RightPanel components not properly connected

## What Changes
- **Consolidate to single Tauri/Svelte UI** - Remove egui GUI code, keep Tauri as primary interface
- **Fix Svelte 5 syntax** - Convert all `$state()` to proper Svelte 5 runes (`let x = $state(...)`)
- **Implement real data binding** - Connect traffic table to Tauri events from Rust backend
- **Add CSS theming** - Move hardcoded colors to CSS variables in `app.css`
- **Add filtering/search** - Implement traffic filtering by domain/method/status
- **Add request detail panel** - Show full request/response headers and body
- **Add export functionality** - Export captured traffic to HAR/PCAP format

### Breaking Changes
- **REMOVED**: `crates/flowlens-ui/src/gui.rs` - egui GUI code will be deleted
- **MODIFIED**: `crates/flowlens-ui/Cargo.toml` - Remove egui dependencies
- **MODIFIED**: Tauri commands - Add new commands for traffic control

## Impact
- Affected specs: UI/frontend layer
- Affected code: 
  - `crates/flowlens-ui/src/` - Remove GUI module
  - `ui/src/` - Fix all Svelte components
  - `ui/src/lib.ts` - Add Tauri event types
  - `crates/flowlens-core/src/event_bus.rs` - Add UI event handlers

## ADDED Requirements
### Requirement: Real-time Traffic Display
The system SHALL display captured HTTP/HTTPS traffic in real-time.

#### Scenario: Success case
- **WHEN** packet capture starts
- **THEN** new traffic rows appear automatically in the table
- **AND** clicking a row shows full request/response details

### Requirement: Traffic Filtering
The system SHALL allow filtering traffic by domain, method, and status code.

#### Scenario: Filter traffic
- **WHEN** user enters filter criteria
- **THEN** table displays only matching traffic
- **AND** filter can be cleared to show all traffic

### Requirement: Request Details View
The system SHALL display complete request/response information.

#### Scenario: View details
- **WHEN** user clicks a traffic row
- **THEN** right panel shows method, URL, headers, and body
- **AND** tabs switch between Request/Response views

## MODIFIED Requirements
### Requirement: Theme System
**OLD**: Components use hardcoded dark colors (#1a1a1a, #2d2d2d, etc.)
**NEW**: System SHALL use CSS variables for theming in `app.css`

### Requirement: Svelte Syntax
**OLD**: Components use `$state()` macro syntax
**NEW**: All components SHALL use Svelte 5 runes syntax (`let x = $state(value)`)

## REMOVED Requirements
### Requirement: egui GUI
**Reason**: Conflicts with Tauri web UI, causes confusion
**Migration**: All GUI functionality moved to Svelte/Tauri
