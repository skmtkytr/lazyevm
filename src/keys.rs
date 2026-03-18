// Keybinding Reference:
//
// Global:    Ctrl+C force quit
// Sidebar:   j/k switch panels, l/Enter focus content, q quit, 1-5 panels
// Content:   h focus sidebar, j/k navigate, l/Enter select, Esc back
// Sub-tabs:  [/] switch sub-tabs
// Panels:    1-5 switch panels, q quit
//
// Panel-specific keys are handled by each panel's handle_key_events().
// When a panel is in an input mode (e.g. Cast editing), it consumes all
// character keys so they don't trigger navigation or quit.
