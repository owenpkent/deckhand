// Icon-button glyphs for the header and the settings panel
// (docs/DECISIONS.md#adr-034). Drawn in the same minimal stroke style as
// the session-state GLYPHS in format.ts: viewBox 24x24, currentColor
// stroke, no fill. Every one of these is hand-drawn for this project,
// not copied from Lucide or any other icon set, so none of them carries
// a Lucide (ISC) attribution; if a future change does copy a Lucide
// path, attribute it here when it lands.
//
// Pure string constants, DOM-free like format.ts, so they can sit in
// innerHTML the same way GLYPHS already does.

/// The header's gear, closed state: opens the settings panel.
export const GEAR_ICON = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="6.5"/><circle cx="12" cy="12" r="1.75" fill="currentColor" stroke="none"/><path d="M12 2.5v3M12 18.5v3M21.5 12h-3M5.5 12h-3M18.6 5.4l-2.1 2.1M7.5 16.5l-2.1 2.1M18.6 18.6l-2.1-2.1M7.5 7.5 5.4 5.4"/></svg>`;

/// The header's gear, open state: an arrow back to the session list,
/// so the pressed state reads as a different shape, not only a
/// different background (docs/ACCESSIBILITY.md).
export const BACK_ICON = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M19 12H5M11 6l-6 6 6 6"/></svg>`;

/// Quit's icon-only cross, replacing the literal &#10005; entity.
export const QUIT_ICON = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.25" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg>`;

/// The Reset position row's decorative right-side affordance. The row
/// itself is the click target (docs/ACCESSIBILITY.md forbids a control
/// that clicking does nothing to explain, so this is not a second,
/// nested button); the icon only signals what kind of action the row
/// performs.
export const RESET_ICON = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 12a8.5 8.5 0 1 0 2.9-6.4"/><path d="M3.2 4.5v4.3h4.3"/></svg>`;
