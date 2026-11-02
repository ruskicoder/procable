//! `procable` binary: verb router.
//!
//! Every skill script and CLI subcommand is a verb here. Verb crates expose
//! `run(args: &[String], io: &mut Io) -> i32` (exit code) and never call
//! `std::process::exit` themselves, so this file is the single place exit codes
//! and stream flushing are decided (contract: docs/CLI-CONTRACT.md in the
//! public repo). Verb names are the JS script basenames; a few carry aliases
//! (`signals` for context-signals, `hooks` for hook-admin).

use std::io::Write;

use procable_common::Io;

mod font_render;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut io = Io::stdio();
    let code = run(&args, &mut io);
    let _ = io.stdout.flush();
    let _ = io.stderr.flush();
    std::process::exit(code);
}

fn run(args: &[String], io: &mut Io) -> i32 {
    // cli/bin/cli.js dispatch: help / version / detect / ignores / skills verbs
    let Some(verb) = args.first().map(String::as_str) else {
        io.out(procable_detect::ROOT_USAGE);
        return 0;
    };
    let rest = &args[1..];
    match verb {
        "--help" | "-h" => {
            io.out(procable_detect::ROOT_USAGE);
            0
        }
        "--version" | "-v" => {
            // The installed skill's version, the number `npx procable update` moves;
            // the npm shim answers --version itself, so this reaches direct runs only.
            let cwd = io.cwd.to_string_lossy().into_owned();
            let version = procable_context::context_cli::installed_skill_version(&io.env, &cwd)
                .unwrap_or_else(|| format!("{VERSION}+engine"));
            io.out(&format!("{version}\n"));
            0
        }
        // Launcher handshake: a cheap discriminator so the launchers can tell
        // this engine apart from the retired 3.x npm CLI (which answers any
        // unknown verb with `Unknown command`, exit 1) before exec'ing a
        // candidate found on PATH or in the unversioned user cache. Kept out
        // of --help on purpose; not part of the user-facing contract.
        "engine-probe" => {
            io.out(&format!("procable-engine {VERSION}\n"));
            0
        }
        "detect" => procable_detect::run_detect(rest, io, &engines()),
        "ignores" | "ignore" => procable_detect::run_ignores(rest, io),
        "skills" => procable_skills::run(rest, io),
        "help" | "install" | "link" | "update" | "check" => procable_skills::run(args, io),
        // skill scripts
        "context" => procable_context::run_context(rest, io),
        "pin" => procable_context::run_pin(rest, io),
        "detect-csp" => procable_context::run_detect_csp(rest, io),
        "palette" => procable_context::run_palette(rest, io),
        "surface-brief" => procable_context::run_surface_brief(rest, io),
        "critique-storage" => procable_context::run_critique_storage(rest, io),
        "embed-prompt" => procable_context::run_embed_prompt(rest, io),
        "signals" | "context-signals" => procable_context::run_signals(rest, io),
        "doctor" => procable_context::run_doctor(rest, io),
        "concept-seed" => procable_context::run_concept_seed(rest, io),
        "telemetry" => procable_context::run_telemetry(rest, io),
        "generate-image" => procable_context::run_generate_image(rest, io),
        "serve-question" => procable_context::run_serve_question(rest, io),
        "component-review" => procable_context::component_review::run_with_capturer(rest, io, Some(&mut procable::component_capture::NativeComponentCapturer)),
        // comp-fidelity verbs (crates/comp-verbs over crates/comp)
        "comp-spec" => procable_comp_verbs::run_comp_spec(rest, io),
        "comp-diff" => procable_comp_verbs::run_comp_diff(rest, io),
        "font-match" => {
            let mut renderer = font_render::CdpFontRenderer::from_process_env();
            procable_comp_verbs::run_font_match(rest, io, &mut renderer)
        }
        "capture-server" => {
            match procable::capture_service::serve(rest) { Ok(()) => 0, Err(e) => { io.err(&format!("Native capture service: {e}\n")); 1 } }
        }
        "build-phase" => {
            // Inject the organic-clip-path CSS scanner (a rule that lives in the
            // closed `core` crate) so comp-verbs stays core-free.
            let organic = |html: &str| -> Vec<(Option<String>, String)> {
                procable_core::checks::css_scan::scan_css_text_for_organic_clip_path(html)
                    .into_iter()
                    .map(|f| (f.selector, f.snippet))
                    .collect()
            };
            match procable::capture_service::RemoteEntryRenderer::from_env(&io.env) {
                Ok(Some(renderer)) => procable_comp_verbs::build_phase::run_with_renderer(rest, io, &organic, Some(&renderer)),
                Ok(None) => { let renderer=procable::reviewed_entry::ReviewedEntryRenderer::local(&io.cwd,io.home().as_deref()); procable_comp_verbs::build_phase::run_with_renderer(rest, io, &organic, Some(&renderer)) },
                Err(e) => { io.err(&format!("Native capture service: {e}\n")); 1 }
            }
        }
        "hook" => procable_hook::run_hook(rest, io, engines().html),
        "hook-before-edit" => procable_hook::run_hook_before_edit(rest, io, engines().html),
        "hooks" | "hook-admin" => procable_hook::run_hook_admin(rest, io),
        v if v.starts_with("live") => procable_live::run(v, rest, io),
        // `npx procable src/` shorthand: a path-shaped, flag, URL, or existing
        // first arg is a detect target (cli.js looksLikeDetectTarget).
        v if procable_detect::looks_like_detect_target(v, &io.cwd.to_string_lossy()) => {
            procable_detect::run_detect(args, io, &engines())
        }
        "init" => {
            io.err(procable_detect::INIT_MESSAGE);
            1
        }
        other => {
            io.err(&format!(
                "Unknown command: \"{other}\"\n\nTo see a list of supported commands, run:\n  procable --help\n"
            ));
            1
        }
    }
}

/// The engines wired into `procable detect`: the static HTML engine
/// (crates/html). The browser engine (crates/browser) plugs in here once it
/// lands; until then URL scans report the puppeteer message.
fn engines() -> procable_detect::Engines<'static> {
    static HTML: procable_html::StaticHtmlEngine = procable_html::StaticHtmlEngine {
        // The shipped binary carries the built-in rules only.
        static_rule_pack: None,
    };
    procable_detect::Engines {
        html: &HTML,
        url: Some(url_engine()),
    }
}

// --- browser engine (crates/browser) -------------------------------------
/// The URL engine, built once from the process environment (browser
/// discovery reads `PROCABLE_BROWSER` / `PUPPETEER_EXECUTABLE_PATH` /
/// `CHROME_PATH`, sandbox flags read `CI`).
fn url_engine() -> &'static procable_browser::BrowserEngine {
    static ENGINE: std::sync::OnceLock<procable_browser::BrowserEngine> =
        std::sync::OnceLock::new();
    ENGINE.get_or_init(procable_browser::BrowserEngine::from_process_env)
}
// -------------------------------------------------------------------------
