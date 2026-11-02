/**
 * Anti-Pattern Browser Detector for Procable
 * Copyright (c) 2026 Paul Bakaus
 *
 * GENERATED -- do not edit. Source: crates/core/src/browser (rules, WASM) +
 * browser-bundle/*.js (DOM probe, overlay UI).
 * Rebuild: cargo xtask bundle
 *
 * Usage: <script src="detect-antipatterns-browser.js"></script>
 * Re-scan: window.procableScan()
 */
(function () {
if (typeof window === 'undefined') return;
