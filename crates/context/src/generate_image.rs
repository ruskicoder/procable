//! `procable generate-image`: local placeholder images only.
//!
//! Upstream Impeccable sent prompts and reference images to the OpenAI
//! images API. procable runs offline, so that path is removed. The verb keeps
//! its offline modes: `IMPECCABLE_IMAGE_GEN_FAKE` writes deterministic
//! placeholder comps (SVG or PNG) with no request, which the comp round, the
//! tests and code-led builds use. Anything else fails with a message that
//! points the agent at the harness-native image tool or a code-led comp.

use crate::jsp;
use crate::util::{node_read_error, utf16_len, Env};
use impeccable_common::Io;
use std::io::Write;

/// What the verb prints when a real image is requested.
pub const OFFLINE_MESSAGE: &str = "generate-image: procable runs offline and does not call an image API. Use the harness-native image tool if this session has one, else build the comp code-led.\n";


fn arg(args: &[String], name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == &format!("--{}", name))?;
    let v = args.get(i + 1)?;
    if !v.is_empty() && !v.starts_with("--") {
        Some(v.clone())
    } else {
        None
    }
}

fn hash32(s: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for cu in s.encode_utf16() {
        h ^= cu as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}

fn hsl_to_rgb(h_deg: f64, s: f64, l: f64) -> [u8; 3] {
    let h = ((h_deg % 360.0) + 360.0) % 360.0 / 360.0;
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue = |t: f64| -> f64 {
        let mut tt = t;
        if tt < 0.0 {
            tt += 1.0;
        }
        if tt > 1.0 {
            tt -= 1.0;
        }
        if tt < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * tt;
        }
        if tt < 1.0 / 2.0 {
            return q;
        }
        if tt < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - tt) * 6.0;
        }
        p
    };
    let round = |c: f64| -> u8 { js_round(c * 255.0) as u8 };
    [round(hue(h + 1.0 / 3.0)), round(hue(h)), round(hue(h - 1.0 / 3.0))]
}

/// Math.round: half up toward +inf
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn to_hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn palette(prompt: &str) -> Vec<[u8; 3]> {
    let h = hash32(prompt);
    let base = (h % 360) as f64;
    let bands = 2 + ((h >> 9) % 2) as usize;
    let spread = (40 + (h >> 3) % 120) as f64;
    let mut out = Vec::new();
    for i in 0..bands {
        let hue = base + i as f64 * spread;
        let light = 0.32 + ((h >> (i * 5)) % 40) as f64 / 100.0;
        out.push(hsl_to_rgb(hue, 0.55, light));
    }
    out
}

fn collapse_ws(s: &str) -> String {
    // .replace(/\s+/g, ' ').trim()
    let mut out = String::new();
    let mut in_ws = false;
    for c in s.chars() {
        if c.is_whitespace() || c == '\u{FEFF}' {
            if !in_ws {
                out.push(' ');
                in_ws = true;
            }
        } else {
            in_ws = false;
            out.push(c);
        }
    }
    crate::util::js_trim(&out).to_string()
}

fn num(v: f64) -> String {
    crate::util::js_number_to_string(v)
}

fn svg_fake(prompt: &str, w: f64, h: f64) -> String {
    let colors: Vec<String> = palette(prompt).into_iter().map(to_hex).collect();
    let n = colors.len();
    let stops: String = colors
        .iter()
        .enumerate()
        .map(|(i, c)| format!("<stop offset=\"{}%\" stop-color=\"{}\"/>", num(js_round(i as f64 / (n as f64 - 1.0) * 100.0)), c))
        .collect();
    let per_line = (12.0f64).max((w / 26.0).floor()) as usize;
    let words: Vec<&str> = {
        let collapsed = collapse_ws(prompt);
        // split(' ') on the collapsed string; leak for lifetime simplicity
        Box::leak(collapsed.into_boxed_str()).split(' ').collect()
    };
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in words {
        let candidate = crate::util::js_trim(&format!("{} {}", cur, word)).to_string();
        if utf16_len(&candidate) > per_line {
            if !cur.is_empty() {
                lines.push(cur.clone());
            }
            cur = word.to_string();
        } else {
            cur = candidate;
        }
        if lines.len() >= 10 {
            break;
        }
    }
    if !cur.is_empty() && lines.len() < 11 {
        lines.push(cur);
    }
    let escape = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let font_size = js_round(w / 24.0);
    let start_y = h / 2.0 - ((lines.len() as f64 - 1.0) * font_size * 1.3) / 2.0;
    let text: String = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            format!(
                "<text x=\"{}\" y=\"{}\" font-family=\"Helvetica, Arial, sans-serif\" font-size=\"{}\" fill=\"#ffffff\" text-anchor=\"middle\" dominant-baseline=\"middle\">{}</text>",
                num(w / 2.0),
                num(js_round(start_y + i as f64 * font_size * 1.3)),
                num(font_size),
                escape(line)
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n  <defs><linearGradient id=\"g\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\">{stops}</linearGradient></defs>\n  <rect width=\"{w}\" height=\"{h}\" fill=\"url(#g)\"/>\n  <rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"#000000\" fill-opacity=\"0.22\"/>\n  {text}\n  <rect x=\"{x1}\" y=\"{y1}\" width=\"{bw}\" height=\"{bh}\" fill=\"#000000\" fill-opacity=\"0.55\"/>\n  <text x=\"{tx}\" y=\"{ty}\" font-family=\"Helvetica, Arial, sans-serif\" font-size=\"{fs}\" letter-spacing=\"2\" fill=\"#ffffff\" text-anchor=\"middle\" dominant-baseline=\"middle\">SYNTHETIC COMP</text>\n</svg>\n",
        w = num(w),
        h = num(h),
        stops = stops,
        text = text,
        x1 = num(w - js_round(w / 4.2)),
        y1 = num(h - js_round(h / 16.0)),
        bw = num(js_round(w / 4.2)),
        bh = num(js_round(h / 16.0)),
        tx = num(w - js_round(w / 8.4)),
        ty = num(h - js_round(h / 32.0)),
        fs = num(js_round(w / 60.0)),
    )
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffffffffu32;
    for b in data {
        c ^= *b as u32;
        for _ in 0..8 {
            c = if c & 1 == 1 { 0xedb88320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    c ^ 0xffffffff
}

fn png_chunk(ty: &[u8], data: &[u8]) -> Vec<u8> {
    let mut body = ty.to_vec();
    body.extend_from_slice(data);
    let mut out = Vec::new();
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
    out
}

fn png_fake(prompt: &str, w: usize, h: usize) -> Vec<u8> {
    png_fake_background(prompt, w, h, false)
}

fn png_fake_background(prompt: &str, w: usize, h: usize, transparent: bool) -> Vec<u8> {
    let colors = palette(prompt);
    let band_h = (h as f64 / colors.len() as f64).ceil() as usize;
    let channels = if transparent { 4 } else { 3 };
    let stride = w * channels;
    let mut raw = vec![0u8; h * (stride + 1)];
    for y in 0..h {
        let row = y * (stride + 1);
        raw[row] = 0;
        let idx = (colors.len() - 1).min(if band_h == 0 { 0 } else { y / band_h });
        let [r, g, b] = colors[idx];
        for x in 0..w {
            let p = row + 1 + x * channels;
            raw[p] = r;
            raw[p + 1] = g;
            raw[p + 2] = b;
            if transparent {
                raw[p + 3] = if x < w / 8 || x >= w - w / 8 || y < h / 8 || y >= h - h / 8 {
                    0
                } else {
                    255
                };
            }
        }
    }
    let mut ihdr = vec![0u8; 13];
    ihdr[..4].copy_from_slice(&(w as u32).to_be_bytes());
    ihdr[4..8].copy_from_slice(&(h as u32).to_be_bytes());
    ihdr[8] = 8;
    ihdr[9] = if transparent { 6 } else { 2 };
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(9));
    let _ = enc.write_all(&raw);
    let idat = enc.finish().unwrap_or_default();
    let mut text = b"Comment".to_vec();
    text.push(0);
    // latin1: chars > 0xff become their low byte in Node's 'latin1' encoding
    for c in format!("SYNTHETIC COMP: {}", collapse_ws(prompt)).encode_utf16() {
        text.push((c & 0xff) as u8);
    }
    let mut out = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    out.extend(png_chunk(b"IHDR", &ihdr));
    out.extend(png_chunk(b"tEXt", &text));
    out.extend(png_chunk(b"IDAT", &idat));
    out.extend(png_chunk(b"IEND", &[]));
    out
}

fn parse_size(s: &str) -> (usize, usize) {
    if let Some((a, b)) = s.split_once('x') {
        if !a.is_empty() && !b.is_empty() && a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit()) {
            return (a.parse().unwrap_or(1536), b.parse().unwrap_or(1024));
        }
    }
    (1536, 1024)
}

/// How long one `--ref <url>` download may take, connect to last byte.

/// The largest reference image a URL may hand back.
const MAX_IMAGE_BYTES: u64 = 30 * 1024 * 1024;

/// Whether a `--ref` value (or a card image) is a URL rather than a path.
pub(crate) fn is_url(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

/// The URL without its query and fragment.
fn url_path(url: &str) -> &str {
    let end = url.find(['?', '#']).unwrap_or(url.len());
    &url[..end]
}

fn sniffed_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else {
        None
    }
}

/// Downloads one png, webp, or jpeg image. The bytes' signature decides the
/// type and is required: a `200` HTML page behind a `.webp` URL or an image
/// Content-Type (a soft 404, an auth wall, a proxy interstitial) is refused
/// rather than uploaded or kept as an image. `redirects` is how many the
/// request may follow. The error is a short reason with no OS-specific text;
/// the caller names the URL.
pub(crate) fn fetch_image(url: &str, timeout: std::time::Duration, redirects: u32) -> Result<(Vec<u8>, &'static str), String> {
    use std::io::Read;
    let agent = crate::http::agent_builder().timeout_connect(timeout).timeout(timeout).redirects(redirects).build();
    let response = match agent.get(url).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(code, _)) => return Err(format!("HTTP {}", code)),
        Err(_) => return Err("the request failed".into()),
    };
    if !(200..300).contains(&response.status()) {
        return Err(format!("HTTP {}", response.status()));
    }
    let mut bytes = Vec::new();
    if response.into_reader().take(MAX_IMAGE_BYTES + 1).read_to_end(&mut bytes).is_err() {
        return Err("the response was cut short".into());
    }
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err("the image is larger than 30 MB".into());
    }
    if bytes.is_empty() {
        return Err("the response was empty".into());
    }
    sniffed_type(&bytes).map(|t| (bytes, t)).ok_or_else(|| "the response is not a png, webp, or jpeg image".into())
}

pub fn run(args: &[String], io: &mut Io) -> i32 {
    run_offline(args, io)
}

fn run_offline(args: &[String], io: &mut Io) -> i32 {
    let background = arg(args, "background");
    if args.iter().any(|a| a == "--background") && !matches!(background.as_deref(), Some("transparent" | "opaque" | "auto")) {
        io.err("generate-image: --background must be transparent, opaque, or auto.\n");
        return 1;
    }
    let transparent = background.as_deref() == Some("transparent");
    if background.is_some() && arg(args, "out").is_some_and(|out| !out.to_ascii_lowercase().ends_with(".png")) {
        io.err("generate-image: --background requires a .png --out path.\n");
        return 1;
    }
    let cwd = io.cwd.to_string_lossy().into_owned();
    let env: Env = io.env.clone();
    let abs = |p: &str| jsp::resolve(&cwd, &[p]);
    let read_prompt_file = |io: &mut Io, pf: &str| -> Result<String, i32> {
        match std::fs::read(abs(pf)) {
            Ok(b) => Ok(String::from_utf8_lossy(&b).into_owned()),
            Err(e) => {
                io.err(&format!("Error: {}\n", node_read_error(pf, &e)));
                Err(1)
            }
        }
    };
    if env.get("IMPECCABLE_IMAGE_GEN_FAKE").map(|v| !v.is_empty()).unwrap_or(false) {
        let prompt = match arg(args, "prompt-file") {
            Some(pf) => match read_prompt_file(io, &pf) {
                Ok(p) => Some(p),
                Err(c) => return c,
            },
            None => arg(args, "prompt"),
        };
        let out = arg(args, "out");
        let (Some(prompt), Some(out)) = (prompt.filter(|p| !p.is_empty()), out) else {
            io.err("generate-image: --prompt (or --prompt-file) and --out are required.\n");
            return 1;
        };
        let (w, h) = parse_size(&arg(args, "size").unwrap_or_else(|| "1536x1024".into()));
        let bytes = if out.ends_with(".svg") {
            svg_fake(&prompt, w as f64, h as f64).into_bytes()
        } else if transparent {
            png_fake_background(&prompt, w, h, true)
        } else {
            png_fake(&prompt, w, h)
        };
        if let Err(e) = std::fs::write(abs(&out), bytes) {
            io.err(&format!("Error: {}\n", node_read_error(&out, &e)));
            return 1;
        }
        io.out(&format!("IMAGE: {} ({}x{}, fake synthetic comp, $0.00, no API call)\n", out, w, h));
        note_generated_slot(io, &out);
        return 0;
    }
    io.err(OFFLINE_MESSAGE);
    1
}

/// A written image that fills a decision page's declared slot counts as that
/// hand's comp even when its bytes match what sat there before (the same
/// prompt regenerates the same bytes). A failed record is reported, not fatal:
/// the image itself was written.
fn note_generated_slot(io: &mut Io, out: &str) {
    let cwd = io.cwd.to_string_lossy().into_owned();
    if let Err(e) = crate::serve_question::record_generated(&cwd, out) {
        io.err(&format!("generate-image: could not mark {} as generated for its decision page ({}); a --wait may report it stale\n", out, e));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("procable-image-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn fake_mode_never_requests_a_url_ref() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/cards/world.webp", server.server_addr());
        let dir = temp("url-ref-fake");
        let env = Env::from([("IMPECCABLE_IMAGE_GEN_FAKE".into(), "1".into()), ("OPENAI_API_KEY".into(), "test-key".into())]);
        let (mut io, captured) = Io::captured("", dir.clone(), env);
        let args: Vec<String> = ["--prompt", "p", "--out", "comp.png", "--ref", &url].iter().map(|s| s.to_string()).collect();
        assert_eq!(run(&args, &mut io), 0);
        assert!(String::from_utf8(captured.stdout.borrow().clone()).unwrap().contains("fake synthetic comp"));
        assert!(server.recv_timeout(Duration::from_millis(300)).unwrap().is_none());
        assert!(dir.join("comp.png").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn url_helpers_and_types() {
        assert_eq!(sniffed_type(b"<html>"), None);
        assert_eq!(sniffed_type(b"RIFF\x10\0\0\0WEBPVP8 "), Some("image/webp"));
        assert!(is_url("http://a") && is_url("https://a") && !is_url("refs/a.png") && !is_url("ftp://a"));
    }

    #[test]
    fn a_real_image_request_fails_offline_and_writes_nothing() {
        let dir = temp("offline");
        let env = Env::from([("OPENAI_API_KEY".into(), "test-key".into())]);
        let (mut io, captured) = Io::captured("", dir.clone(), env);
        let args: Vec<String> = ["--prompt", "p", "--out", "comp.png"].iter().map(|s| s.to_string()).collect();
        assert_eq!(run(&args, &mut io), 1);
        assert_eq!(String::from_utf8(captured.stderr.borrow().clone()).unwrap(), OFFLINE_MESSAGE);
        assert!(!dir.join("comp.png").exists() && !dir.join("comp.png.json").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn fake_cutout_has_real_alpha_and_default_fake_stays_rgb() {
        use std::io::Read;
        let png = png_fake_background("cutout", 16, 16, true);
        assert_eq!(png[25], 6); // RGBA
        assert_eq!(png_fake("comp", 16, 16)[25], 2); // RGB, legacy fake output
        let mut offset = 8;
        let mut raw = Vec::new();
        while offset + 12 <= png.len() {
            let size = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
            if &png[offset + 4..offset + 8] == b"IDAT" {
                flate2::read::ZlibDecoder::new(&png[offset + 8..offset + 8 + size])
                    .read_to_end(&mut raw)
                    .unwrap();
            }
            offset += size + 12;
        }
        assert_eq!(raw[4], 0); // transparent corner
        assert_eq!(raw[8 * (16 * 4 + 1) + 1 + 8 * 4 + 3], 255); // opaque subject
    }

    #[test]
    fn invalid_background_requests_fail_before_network_or_output() {
        for flags in [
            vec!["--background"],
            vec!["--background", "white"],
            vec!["--background", "transparent", "--out", "cutout.jpg"],
            vec!["--background", "opaque", "--out", "hero.webp"],
            vec!["--background", "auto", "--out", "hero.svg"],
        ] {
            let (mut io, captured) = Io::captured("", std::env::temp_dir(), Env::new());
            let args = flags.iter().map(|s| s.to_string()).collect::<Vec<_>>();
            assert_eq!(run(&args, &mut io), 1);
            let stderr = String::from_utf8(captured.stderr.borrow().clone()).unwrap();
            assert!(stderr.contains("--background"), "{stderr}");
            assert!(!stderr.contains("OPENAI_API_KEY"));
        }
    }
}
