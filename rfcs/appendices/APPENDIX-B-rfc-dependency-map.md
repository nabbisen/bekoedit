# Appendix B: RFC Dependency Map

```text
M0 Foundation
  RFC-000 Project Charter
  RFC-001 Repository / CI
  RFC-002 Runtime Boundary

M1 Workspace
  RFC-003 Workspace Model
    -> RFC-004 File Tree Index
    -> RFC-005 File Operations / Watching

M2 Document Core
  RFC-006 Document Session
    -> RFC-007 Save / Recovery
    -> RFC-008 Conflicts
    -> RFC-009 State Store

M3 Text / Preview UX
  RFC-010 Main Shell
    -> RFC-011 CodeMirror Text Mode
    -> RFC-012 Preview Mode

M4 Source Preservation Core
  RFC-013 Markdown Index
    -> RFC-014 Block Identity
    -> RFC-015 SourcePatch Engine

M5 Form Mode MVP
  RFC-016 Form Surface
    -> RFC-017 Raw Islands
    -> RFC-018 JS Form Adapter
    -> RFC-019 Mode Synchronization

M6 UX Completion
  RFC-020 Command Palette
  RFC-021 Accessibility
  RFC-022 Settings
  RFC-023 Error Surfaces

M7 Release
  RFC-024 Packaging
  RFC-025 Release CI
  RFC-026 MVP Gates

Post-MVP / Future
  RFC-027..RFC-040
```

Critical path for first implementation:

```text
RFC-000 -> RFC-001 -> RFC-002 -> RFC-003 -> RFC-004 -> RFC-006 -> RFC-007 -> RFC-010 -> RFC-011 -> RFC-013 -> RFC-015 -> RFC-016 -> RFC-017 -> RFC-018 -> RFC-019 -> RFC-026
```
