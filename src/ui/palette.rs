use ratatui::style::Color;

// ---------------------------------------------------------------------------
// Cyberpunk palette: cold neutrals + neon severity ramps
// ---------------------------------------------------------------------------
//
// This palette is designed for segmented TUI bars and dialogs that need a
// clear cold-neon mood:
//   - Error      = red drifting toward magenta
//   - Warning    = dry electric orange
//   - Success    = teal/cyan-green instead of leaf green
//   - Processing = purple into hot pink
//
// Each severity family uses the same naming ladder:
//   BASE -> GLOW -> HEAT -> PEAK
//
// The intent is that brightness/intensity rises left-to-right in status bars,
// e.g. `arch > os > host > status`.

// ---- Neutral foundation ----

/// Absolute black background.
pub const BG_0: Color = Color::Indexed(232);

/// Near-black raised background.
pub const BG_1: Color = Color::Indexed(233);

/// Dark surface for popups and panes.
pub const BG_2: Color = Color::Indexed(235);

/// Lighter surface for active containers.
pub const BG_3: Color = Color::Indexed(237);

/// Faint border / tertiary tone.
pub const GRAY_0: Color = Color::Indexed(239);

/// Muted text / secondary annotation.
pub const GRAY_1: Color = Color::Indexed(244);

/// Main foreground text.
pub const GRAY_2: Color = Color::Indexed(249);

// ---- Error: cold neon red ----

pub const ERROR_BASE: Color = Color::Indexed(53);
pub const ERROR_GLOW: Color = Color::Indexed(89);
pub const ERROR_HEAT: Color = Color::Indexed(125);
pub const ERROR_PEAK: Color = Color::Indexed(161);

pub const ERROR_RAMP: [Color; 4] = [ERROR_BASE, ERROR_GLOW, ERROR_HEAT, ERROR_PEAK];

// ---- Warning: dry electric orange ----

pub const WARNING_BASE: Color = Color::Indexed(94);
pub const WARNING_GLOW: Color = Color::Indexed(130);
pub const WARNING_HEAT: Color = Color::Indexed(166);
pub const WARNING_PEAK: Color = Color::Indexed(172);

pub const WARNING_RAMP: [Color; 4] = [WARNING_BASE, WARNING_GLOW, WARNING_HEAT, WARNING_PEAK];

// ---- Success: greener neon, still kept cold ----

pub const SUCCESS_BASE: Color = Color::Indexed(23);
pub const SUCCESS_GLOW: Color = Color::Indexed(29);
pub const SUCCESS_HEAT: Color = Color::Indexed(36);
pub const SUCCESS_PEAK: Color = Color::Indexed(44);

pub const SUCCESS_RAMP: [Color; 4] = [SUCCESS_BASE, SUCCESS_GLOW, SUCCESS_HEAT, SUCCESS_PEAK];

// ---- Processing: neon purple/pink ----

pub const PROCESSING_BASE: Color = Color::Indexed(55);
pub const PROCESSING_GLOW: Color = Color::Indexed(92);
pub const PROCESSING_HEAT: Color = Color::Indexed(129);
pub const PROCESSING_PEAK: Color = Color::Indexed(165);

pub const PROCESSING_RAMP: [Color; 4] = [PROCESSING_BASE, PROCESSING_GLOW, PROCESSING_HEAT, PROCESSING_PEAK];

pub const PROCESSING_BASE_DIMMED: Color = Color::Indexed(53);
pub const PROCESSING_GLOW_DIMMED: Color = Color::Indexed(54);
pub const PROCESSING_HEAT_DIMMED: Color = Color::Indexed(55);
pub const PROCESSING_PEAK_DIMMED: Color = Color::Indexed(56);
pub const PROCESSING_DIMMED: Color = Color::Indexed(57);

// ---- Semantic aliases for current UI usage ----

/// Primary background (raised surfaces, list panes, popups).
pub const SURFACE: Color = Color::Indexed(236);

/// Primary foreground text.
pub const FG: Color = Color::Indexed(253);

/// Muted / secondary text (dimmed, descriptions, diagnostics).
pub const MUTED: Color = Color::Indexed(243);

/// Faint / tertiary text (disabled elements, faint borders).
pub const FAINT: Color = Color::Indexed(238);

/// Cold cyan accent for focus and active success-ish elements.
pub const ACCENT: Color = Color::Indexed(36);

/// Bold background highlight (selected rows, active states).
pub const HIGHLIGHT: Color = Color::Indexed(133);

/// Text colour on top of highlight background.
pub const ON_HIGHLIGHT: Color = Color::Indexed(235);

/// Borders and dividers.
pub const BORDER: Color = Color::Indexed(240);

/// Error / failure / destructive.
pub const ERROR: Color = Color::Indexed(198);

/// Warning / caution.
pub const WARNING: Color = Color::Indexed(178);

/// Success / confirmation / positive.
pub const SUCCESS: Color = Color::Indexed(49);

/// Form labels / structured input captions.
pub const FORM_LABEL: Color = SUCCESS_PEAK;

/// Work in progress / active processing.
pub const PROCESSING: Color = Color::Indexed(171);

/// Primary accent (hot processing pink).
pub const PRIMARY: Color = Color::Indexed(200);

/// Secondary accent (processing purple).
pub const SECONDARY: Color = Color::Indexed(97);

/// Popup/dialog base background.
pub const POPUP_BG_BASE: Color = SURFACE;

/// Popup/dialog alternate (lighter) background.
pub const POPUP_BG_1: Color = BG_1;

// ---- MS‑DOS shadow colours (shared across all dialogs) ----

pub const SHADOW_BG: Color = Color::Black;
pub const SHADOW_FG: Color = SURFACE;

// ---- Absolute contrast ----

pub const WHITE: Color = Color::White;
pub const BLACK: Color = Color::Black;
