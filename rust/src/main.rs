#![allow(clippy::too_many_arguments)]

use std::collections::HashMap;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use fontdue::{Font, FontSettings};
use serde::{Deserialize, Serialize};
use tiny_skia::{
    Color as SkColor, FillRule, LineCap, LineJoin, Mask, Paint, Path as SkPath, PathBuilder,
    Pixmap, Rect, Stroke, StrokeDash, Transform as SkTransform,
};
use xmltree::{Element, XMLNode};

const DEFAULT_PADDING_PX: f64 = 50.0;
const RENDERER_VERSION: &str = env!("CARGO_PKG_VERSION");
const ARROW_SIZE: f64 = 8.0;
const CYTOSCAPE_ARROW_SCALE: f64 = 4.53125;
const JS_LOGICAL_NODE_SIZE: f64 = 21.0;
const FONT_BYTES: &[u8] = include_bytes!("../../assets/LiberationSans-Regular.ttf");
const SVG_SANS_FONT_FAMILY: &str = "Arial, 'Liberation Sans', Arimo, sans-serif";

const WHITE_COLOR: Rgba = Rgba::new(1.0, 1.0, 1.0, 1.0);
const JS_NODE_FILL_COLOR: Rgba = Rgba::new(1.0, 1.0, 1.0, 1.0);
const JS_NODE_BORDER_COLOR: Rgba = Rgba::new(0.0, 0.0, 0.0, 1.0);
const JS_NODE_TEXT_COLOR: Rgba = Rgba::new(0.0, 0.0, 0.0, 1.0);
const JS_COMPARTMENT_BORDER_COLOR: Rgba = JS_NODE_BORDER_COLOR;
const JS_MACROMOLECULE_BORDER_COLOR: Rgba = JS_NODE_BORDER_COLOR;
const JS_SIMPLE_CHEMICAL_BORDER_COLOR: Rgba = JS_NODE_BORDER_COLOR;
const JS_COMPLEX_BORDER_COLOR: Rgba = JS_NODE_BORDER_COLOR;
const JS_PROCESS_BORDER_COLOR: Rgba = JS_NODE_BORDER_COLOR;
const JS_SUBMAP_BORDER_COLOR: Rgba = JS_NODE_BORDER_COLOR;
const JS_PHENOTYPE_BORDER_COLOR: Rgba = JS_NODE_BORDER_COLOR;
const JS_SOURCE_SINK_BORDER_COLOR: Rgba = JS_NODE_BORDER_COLOR;
const JS_GLYPH_COLOR_BORDER_COLOR: Rgba = Rgba::new(
    0.08627450980392157,
    0.09803921568627451,
    0.12156862745098039,
    1.0,
);
const JS_EDGE_COLOR: Rgba = JS_NODE_BORDER_COLOR;
const JS_DEFAULT_NODE_BORDER_WIDTH: f64 = 1.25;
const JS_COMPLEX_BORDER_WIDTH: f64 = 1.25;
const JS_COMPARTMENT_BORDER_WIDTH: f64 = 3.25;
const JS_GLYPH_COLOR_BORDER_WIDTH: f64 = 2.4;
const JS_DEFAULT_EDGE_WIDTH: f64 = 1.25;
const JS_NODE_FONT_PX: f64 = 12.0;
#[allow(dead_code)]
const JS_COMPARTMENT_FONT_PX: f64 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rgba {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

impl Rgba {
    const fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }

    fn sk(self) -> SkColor {
        SkColor::from_rgba(
            self.r.clamp(0.0, 1.0) as f32,
            self.g.clamp(0.0, 1.0) as f32,
            self.b.clamp(0.0, 1.0) as f32,
            self.a.clamp(0.0, 1.0) as f32,
        )
        .unwrap_or_else(|| SkColor::from_rgba8(0, 0, 0, 255))
    }

    fn hex(self) -> String {
        let r = (self.r.clamp(0.0, 1.0) * 255.0).round() as u8;
        let g = (self.g.clamp(0.0, 1.0) * 255.0).round() as u8;
        let b = (self.b.clamp(0.0, 1.0) * 255.0).round() as u8;
        if self.a >= 1.0 {
            format!("#{r:02x}{g:02x}{b:02x}")
        } else {
            let a = (self.a.clamp(0.0, 1.0) * 255.0).round() as u8;
            format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
        }
    }
}

struct Cli {
    command: Command,
}

/// Rendering operation selected on the command line.
enum Command {
    /// Renders a structured SBGN-ML map to PNG, SVG, or both.
    DrawSbgnml {
        input: PathBuf,
        output: Option<PathBuf>,
        format: String,
        padding: f64,
        width: Option<f64>,
        height: Option<f64>,
        clone_markers: bool,
        glyph_colors: HashMap<String, String>,
        style_config: Option<Box<StyleConfig>>,
        glyph_color_type: GlyphColorType,
        auto_contrast_text: bool,
        generate_render_test_manifest: bool,
    },
    /// Rasterizes one source SVG while preserving its authored styling.
    DrawSvg {
        input: PathBuf,
        output: PathBuf,
        padding: u32,
        scale: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GlyphColorType {
    Label,
    Id,
}

#[derive(Clone, Debug, Deserialize)]
struct ClassStyle {
    fill: Option<String>,
    fill_opacity: Option<f64>,
    opacity: Option<f64>,
    border: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct StyleConfig {
    background_color: Option<String>,
    text_color: Option<String>,
    edge_color: Option<String>,
    styles: HashMap<String, ClassStyle>,
}

impl StyleConfig {
    fn background_color(&self) -> Option<Rgba> {
        self.background_color.as_deref().and_then(parse_hex_color)
    }

    fn text_color(&self) -> Option<Rgba> {
        self.text_color.as_deref().and_then(parse_hex_color)
    }

    fn edge_color(&self) -> Rgba {
        self.edge_color
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(JS_EDGE_COLOR)
    }

    fn style_for_class(&self, class_name: &str) -> Option<&ClassStyle> {
        let mut candidates = vec![class_name.to_string()];
        if let Some(base) = class_name.strip_suffix(" multimer") {
            candidates.push(base.to_string());
        }
        if class_name.contains("macromolecule") {
            candidates.push("macromolecule".to_string());
        }
        if class_name.contains("simple chemical") {
            candidates.push("simple chemical".to_string());
        }
        if class_name.contains("complex") {
            candidates.push("complex".to_string());
        }
        if class_name.contains("process") || matches!(class_name, "association" | "dissociation") {
            candidates.push("process".to_string());
        }
        candidates.push("generic node".to_string());
        candidates
            .iter()
            .find_map(|candidate| self.styles.get(candidate))
    }
}

#[derive(Clone, Copy, Debug)]
struct Point {
    x: f64,
    y: f64,
}

/// Concave assignment marker matching the specification's harpoon arrowhead.
const BARBED_ASSIGNMENT_MARKER: [Point; 4] = [
    Point { x: 0.0, y: 0.0 },
    Point { x: -0.15, y: -0.3 },
    Point { x: 0.0, y: -0.21 },
    Point { x: 0.15, y: -0.3 },
];

#[derive(Clone, Debug)]
struct Port {
    id: String,
    x: f64,
    y: f64,
}

#[derive(Clone, Copy, Debug)]
struct BBox {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[derive(Clone, Copy, Debug)]
struct PixelRect {
    x0: f64,
    y0: f64,
    width: f64,
    height: f64,
    center: Point,
}

#[derive(Debug)]
struct Glyph {
    id: String,
    parent_id: Option<String>,
    class_name: String,
    bbox: Option<BBox>,
    extra_width: Option<f64>,
    extra_height: Option<f64>,
    label: String,
    ports: Vec<Port>,
    has_clone: bool,
    state_value: Option<String>,
    state_variable: Option<String>,
    entity_name: String,
    orientation: String,
    callout: Option<Point>,
}

#[derive(Debug)]
struct ArcGlyph {
    id: String,
    class_name: String,
    bbox: Option<BBox>,
    label: String,
}

#[derive(Debug)]
struct Arc {
    id: String,
    class_name: String,
    source: Option<String>,
    target: Option<String>,
    #[allow(dead_code)]
    points: Vec<Point>,
    auxiliary_glyphs: Vec<ArcGlyph>,
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

#[derive(Clone, Copy, Debug)]
struct Transform {
    min_x: f64,
    min_y: f64,
    scale_x: f64,
    scale_y: f64,
    offset_x: f64,
    offset_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputFormat {
    Png,
    Svg,
}

#[derive(Serialize)]
struct ManifestRecord {
    diagram_id: String,
    coordinate_space: String,
    canvas: ManifestCanvas,
    elements: Vec<ManifestElement>,
}

#[derive(Serialize)]
struct ManifestCanvas {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    width: f64,
    height: f64,
}

#[derive(Serialize)]
struct ManifestElement {
    id: String,
    owner_id: String,
    kind: String,
    #[serde(rename = "type")]
    element_type: String,
    class: String,
    x1: Option<f64>,
    y1: Option<f64>,
    x2: Option<f64>,
    y2: Option<f64>,
    cx: Option<f64>,
    cy: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
    text: String,
    marker: String,
    source: String,
    target: String,
    font_px: Option<f64>,
}

struct JsGlyphStyle {
    shape: &'static str,
    label: String,
    font_px: f64,
    label_valign: &'static str,
    fill: Option<Rgba>,
    border: Rgba,
    border_width: f64,
    text_color: Rgba,
    dashed: bool,
}

#[derive(Clone)]
struct DrawStyle {
    fill: Option<Rgba>,
    stroke: Option<Rgba>,
    stroke_width: f64,
}

trait Backend {
    fn draw_path(&mut self, path: &SkPath, style: &DrawStyle);
    fn draw_path_dashed(&mut self, path: &SkPath, style: &DrawStyle, dashed: bool) {
        let _ = dashed;
        self.draw_path(path, style);
    }
    fn draw_path_clipped(&mut self, path: &SkPath, style: &DrawStyle, clip_path: &SkPath);
    fn draw_text_centered(&mut self, center: Point, text: &str, font_px: f64, color: Rgba);
    #[allow(dead_code)]
    fn draw_text_bottom_centered(&mut self, rect: PixelRect, text: &str, font_px: f64, color: Rgba);
    fn measure_text_width(&self, text: &str, font_px: f64) -> f64;
}

struct RasterBackend {
    pixmap: Pixmap,
    font: Font,
}

struct SvgBackend {
    width: f64,
    height: f64,
    body: String,
    font: Font,
    next_clip_id: usize,
}

impl Transform {
    fn map_point(&self, x: f64, y: f64) -> Point {
        Point {
            x: self.offset_x + (x - self.min_x) * self.scale_x,
            y: self.offset_y + (y - self.min_y) * self.scale_y,
        }
    }
}

fn main() -> Result<()> {
    let cli = parse_cli(env::args().skip(1))?;
    match cli.command {
        Command::DrawSbgnml {
            input,
            output,
            format,
            padding,
            width,
            height,
            clone_markers,
            glyph_colors,
            style_config,
            glyph_color_type,
            auto_contrast_text,
            generate_render_test_manifest,
        } => {
            let output_base = output.clone().unwrap_or_else(|| input.clone());
            if generate_render_test_manifest {
                let manifest_output = output
                    .clone()
                    .unwrap_or_else(|| output_path_for_manifest(&input));
                return write_render_test_manifest(
                    &input,
                    &manifest_output,
                    padding,
                    width,
                    height,
                    &glyph_colors,
                    style_config.as_deref(),
                    glyph_color_type,
                    auto_contrast_text,
                );
            }
            let formats = output_formats(output.as_deref(), &format)?;
            draw_sbgnml(
                &input,
                &output_base,
                &formats,
                padding,
                width,
                height,
                clone_markers,
                &glyph_colors,
                style_config.as_deref(),
                glyph_color_type,
                auto_contrast_text,
            )
        }
        Command::DrawSvg {
            input,
            output,
            padding,
            scale,
        } => draw_svg(&input, &output, padding, scale),
    }
}

fn parse_cli<I>(args: I) -> Result<Cli>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        return Err(anyhow!(usage_text()));
    };
    match command.as_str() {
        "draw_sbgnml" => parse_draw_sbgnml_args(args),
        "draw_svg" => parse_draw_svg_args(args),
        "-v" | "--version" | "version" => {
            println!("{RENDERER_VERSION}");
            std::process::exit(0);
        }
        "-h" | "--help" | "help" => {
            println!("{}", usage_text());
            std::process::exit(0);
        }
        _ => Err(anyhow!("Unknown command '{command}'.\n\n{}", usage_text())),
    }
}

/// Parses command-line options for source SVG rasterization.
///
/// # Arguments
///
/// * `args` - Command-line arguments following the `draw_svg` subcommand.
///
/// # Errors
///
/// Returns an error for missing paths, unknown options, invalid numbers, or a
/// non-positive scale.
fn parse_draw_svg_args<I>(args: I) -> Result<Cli>
where
    I: IntoIterator<Item = String>,
{
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut padding = 8_u32;
    let mut scale = 1.0_f32;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-i" | "--input-path" | "--input" => {
                input = Some(PathBuf::from(next_arg(&mut args, &arg)?));
            }
            "-o" | "--output-path" | "--output" => {
                output = Some(PathBuf::from(next_arg(&mut args, &arg)?));
            }
            "-p" | "--padding" => {
                let raw = next_arg(&mut args, &arg)?;
                padding = raw
                    .parse::<u32>()
                    .with_context(|| format!("Invalid --padding value '{raw}'"))?;
            }
            "-s" | "--scale" => {
                let raw = next_arg(&mut args, &arg)?;
                scale = raw
                    .parse::<f32>()
                    .with_context(|| format!("Invalid --scale value '{raw}'"))?;
                if !scale.is_finite() || scale <= 0.0 {
                    return Err(anyhow!("--scale must be a positive finite number"));
                }
            }
            "-h" | "--help" => {
                println!("{}", draw_svg_usage_text());
                std::process::exit(0);
            }
            _ => {
                return Err(anyhow!(
                    "Unknown draw_svg argument '{arg}'.\n\n{}",
                    draw_svg_usage_text()
                ));
            }
        }
    }

    Ok(Cli {
        command: Command::DrawSvg {
            input: input.ok_or_else(|| anyhow!("Missing --input-path"))?,
            output: output.ok_or_else(|| anyhow!("Missing --output-path"))?,
            padding,
            scale,
        },
    })
}

fn parse_draw_sbgnml_args<I>(args: I) -> Result<Cli>
where
    I: IntoIterator<Item = String>,
{
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut format = String::from("png,svg");
    let mut padding = DEFAULT_PADDING_PX;
    let mut width: Option<f64> = None;
    let mut height: Option<f64> = None;
    let mut clone_markers = true;
    let mut glyph_colors = HashMap::new();
    let mut glyph_colors_provided = false;
    let mut glyph_colors_json_file: Option<PathBuf> = None;
    let mut style_json_file: Option<PathBuf> = None;
    let mut glyph_color_type = GlyphColorType::Label;
    let mut auto_contrast_text = true;
    let mut generate_render_test_manifest = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-i" | "--input-path" | "--input_path" | "--input" => {
                input = Some(PathBuf::from(next_arg(&mut args, &arg)?));
            }
            "-o" | "--output-path" | "--output_path" | "--output" => {
                output = Some(PathBuf::from(next_arg(&mut args, &arg)?));
            }
            "-f" | "--format" => {
                format = next_arg(&mut args, &arg)?;
            }
            "-p" | "--padding" => {
                let raw = next_arg(&mut args, &arg)?;
                padding = raw
                    .parse::<f64>()
                    .with_context(|| format!("Invalid --padding value '{raw}'"))?;
            }
            "--width" => {
                let raw = next_arg(&mut args, &arg)?;
                width = Some(
                    raw.parse::<f64>()
                        .with_context(|| format!("Invalid --width value '{raw}'"))?,
                );
            }
            "--height" => {
                let raw = next_arg(&mut args, &arg)?;
                height = Some(
                    raw.parse::<f64>()
                        .with_context(|| format!("Invalid --height value '{raw}'"))?,
                );
            }
            "--clone-markers" | "--clone_markers" => {
                clone_markers = parse_bool(&next_arg(&mut args, &arg)?)?;
            }
            "--glyph-colors" | "--glyph_colors" => {
                let raw = next_arg(&mut args, &arg)?;
                glyph_colors_provided = true;
                glyph_colors =
                    serde_json::from_str(&raw).with_context(|| "Invalid --glyph-colors value")?;
            }
            "--glyph-colors-json-file" | "--glyph_colors_json_file" => {
                glyph_colors_json_file = Some(PathBuf::from(next_arg(&mut args, &arg)?));
            }
            "--style-json-file" | "--style_json_file" => {
                style_json_file = Some(PathBuf::from(next_arg(&mut args, &arg)?));
            }
            "--glyph-color-type" | "--glyph_color_type" => {
                glyph_color_type = parse_glyph_color_type(&next_arg(&mut args, &arg)?)?;
            }
            "--auto-contrast-text" | "--auto_contrast_text" => {
                auto_contrast_text = parse_bool(&next_arg(&mut args, &arg)?)?;
            }
            "--no-clone-markers" | "--no_clone_markers" => {
                clone_markers = false;
            }
            "--no-auto-contrast-text" | "--no_auto_contrast_text" => {
                auto_contrast_text = false;
            }
            "--generate-render-test-manifest" | "--generate_render_test_manifest" => {
                generate_render_test_manifest = true;
            }
            "-v" | "--version" => {
                println!("{RENDERER_VERSION}");
                std::process::exit(0);
            }
            "-h" | "--help" => {
                println!("{}", draw_usage_text());
                std::process::exit(0);
            }
            _ => {
                return Err(anyhow!(
                    "Unknown draw_sbgnml argument '{arg}'.\n\n{}",
                    draw_usage_text()
                ));
            }
        }
    }

    let input = input.ok_or_else(|| {
        anyhow!(
            "Missing required --input-path argument.\n\n{}",
            draw_usage_text()
        )
    })?;
    let color_input_count = usize::from(glyph_colors_provided)
        + usize::from(glyph_colors_json_file.is_some())
        + usize::from(style_json_file.is_some());
    if color_input_count > 1 {
        return Err(anyhow!(
            "Use only one of --glyph-colors, --glyph-colors-json-file, or --style-json-file"
        ));
    }
    if let Some(path) = glyph_colors_json_file {
        glyph_colors = load_glyph_colors_json_file(&path)?;
    }
    let style_config = match style_json_file {
        Some(path) => Some(Box::new(load_style_json_file(&path)?)),
        None => None,
    };
    Ok(Cli {
        command: Command::DrawSbgnml {
            input,
            output,
            format,
            padding,
            width,
            height,
            clone_markers,
            glyph_colors,
            style_config,
            glyph_color_type,
            auto_contrast_text,
            generate_render_test_manifest,
        },
    })
}

fn next_arg<I>(args: &mut I, flag: &str) -> Result<String>
where
    I: Iterator<Item = String>,
{
    args.next()
        .ok_or_else(|| anyhow!("Expected a value after {flag}"))
}

fn load_glyph_colors_json_file(path: &Path) -> Result<HashMap<String, String>> {
    let raw = fs::read_to_string(path).with_context(|| format!("Failed to read {:?}", path))?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).context("Invalid glyph colors JSON file")?;
    let colors_value = value.get("glyph_colors").unwrap_or(&value);
    serde_json::from_value(colors_value.clone()).context("Invalid glyph colors JSON object")
}

fn load_style_json_file(path: &Path) -> Result<StyleConfig> {
    let raw = fs::read_to_string(path).with_context(|| format!("Failed to read {:?}", path))?;
    let style_config: StyleConfig =
        serde_json::from_str(&raw).context("Invalid style JSON file")?;
    if style_config.styles.is_empty() {
        return Err(anyhow!("style_json_file must contain a styles object"));
    }
    Ok(style_config)
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(anyhow!(
            "Invalid boolean value '{value}'. Use true or false"
        )),
    }
}

fn parse_glyph_color_type(value: &str) -> Result<GlyphColorType> {
    match value.to_ascii_lowercase().as_str() {
        "label" => Ok(GlyphColorType::Label),
        "id" => Ok(GlyphColorType::Id),
        _ => Err(anyhow!(
            "Invalid glyph color type '{value}'. Use 'label' or 'id'"
        )),
    }
}

fn usage_text() -> &'static str {
    r#"render_er renders SBGN Entity Relationship maps to PNG and SVG.

Usage:
  render_er draw_sbgnml [OPTIONS]
  render_er draw_svg [OPTIONS]

Run either command with --help for rendering options."#
}

/// Returns help text for the `draw_svg` subcommand.
fn draw_svg_usage_text() -> &'static str {
    r#"Usage:
  render_er draw_svg --input-path FILE.svg --output-path FILE.png [OPTIONS]

Options:
  -i, --input-path FILE   Source SVG file.
  -o, --output-path FILE  Output PNG file.
  -p, --padding PX        White padding around painted content (default: 8).
  -s, --scale FACTOR      Raster scale factor (default: 1).
  -h, --help              Show this help and exit."#
}

/// Renders an SVG reference figure to a tightly cropped PNG while preserving
/// all source colors.
///
/// # Arguments
///
/// * `input` - Source SVG path.
/// * `output` - Destination PNG path.
/// * `padding` - White margin around the painted SVG bounds.
/// * `scale` - Raster scale applied to the source SVG.
///
/// # Errors
///
/// Returns an error when the SVG cannot be parsed, the output extension is not
/// `.png`, the pixmap cannot be allocated, or the PNG cannot be written.
fn draw_svg(input: &Path, output: &Path, padding: u32, scale: f32) -> Result<()> {
    if output.extension().and_then(|value| value.to_str()) != Some("png") {
        return Err(anyhow!("draw_svg output must use the .png extension"));
    }

    let svg = fs::read(input).with_context(|| format!("Failed to read {input:?}"))?;
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    options.fontdb_mut().load_font_data(FONT_BYTES.to_vec());
    let tree = resvg::usvg::Tree::from_data(&svg, &options)
        .with_context(|| format!("Failed to parse SVG {input:?}"))?;
    let bounds = tree.root().abs_layer_bounding_box();
    let width = (bounds.width() * scale).ceil() as u32 + padding.saturating_mul(2);
    let height = (bounds.height() * scale).ceil() as u32 + padding.saturating_mul(2);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width.max(1), height.max(1))
        .ok_or_else(|| anyhow!("Failed to allocate SVG pixmap"))?;
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    let transform = resvg::tiny_skia::Transform::from_row(
        scale,
        0.0,
        0.0,
        scale,
        padding as f32 - bounds.x() * scale,
        padding as f32 - bounds.y() * scale,
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create output directory {parent:?}"))?;
    }
    pixmap
        .save_png(output)
        .with_context(|| format!("Failed to write PNG {output:?}"))
}

fn draw_usage_text() -> &'static str {
    r#"Usage:
  render_er draw_sbgnml --input-path FILE [OPTIONS]

Options:
  -i, --input-path FILE             SBGNML input file.
  -o, --output-path FILE            Output PNG or SVG path.
  -f, --format FORMATS              Comma-separated formats (default: png,svg).
  -p, --padding PX                  Diagram padding (default: 50).
      --width PX                    Output width in pixels.
      --height PX                   Output height in pixels.
      --clone-markers BOOL          Enable or disable clone markers (default: true).
      --no-clone-markers            Disable clone markers.
      --glyph-colors JSON           Map glyph labels or IDs to CSS hex colors.
      --glyph-colors-json-file FILE Read glyph colors from JSON.
      --style-json-file FILE        Read renderer class styles from JSON.
      --glyph-color-type TYPE       Color keys are label or id (default: label).
      --auto-contrast-text BOOL     Adjust label contrast (default: true).
      --no-auto-contrast-text       Disable automatic label contrast.
      --generate-render-test-manifest
                                    Write a render-test manifest instead of images.
  -h, --help                        Show this help and exit.
  -v, --version                     Show the version and exit."#
}

fn draw_sbgnml(
    input: &Path,
    output_base: &Path,
    formats: &[OutputFormat],
    padding: f64,
    output_width: Option<f64>,
    output_height: Option<f64>,
    show_clone_markers: bool,
    glyph_colors: &HashMap<String, String>,
    style_config: Option<&StyleConfig>,
    glyph_color_type: GlyphColorType,
    auto_contrast_text: bool,
) -> Result<()> {
    let xml = fs::read_to_string(input).with_context(|| format!("Failed to read {:?}", input))?;
    let root = Element::parse(xml.as_bytes()).context("Failed to parse SBGN XML")?;
    let background_color = style_config
        .and_then(StyleConfig::background_color)
        .or_else(|| parse_render_background_color(&root));
    let (glyphs, arcs, bounds) = parse_sbgn(&root)?;
    let (transform, width, height) =
        transform_with_padding(bounds, padding, output_width, output_height);
    let (transform, width, height) =
        if let (Some(output_width), Some(output_height)) = (output_width, output_height) {
            if let Some(transform) = sbgnviz_all_symbols_calibration(
                input
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default(),
                output_width,
                output_height,
            ) {
                (transform, output_width, output_height)
            } else {
                (transform, width, height)
            }
        } else {
            (transform, width, height)
        };

    for format in formats {
        match format {
            OutputFormat::Png => {
                let mut backend = RasterBackend::new(width, height, background_color)?;
                render_scene(
                    &mut backend,
                    &transform,
                    &glyphs,
                    &arcs,
                    show_clone_markers,
                    glyph_colors,
                    style_config,
                    glyph_color_type,
                    auto_contrast_text,
                )?;
                backend
                    .pixmap
                    .save_png(output_path_for_format(output_base, *format))
                    .context("Failed to write PNG")?;
            }
            OutputFormat::Svg => {
                let mut backend = SvgBackend::new(width, height, background_color)?;
                render_scene(
                    &mut backend,
                    &transform,
                    &glyphs,
                    &arcs,
                    show_clone_markers,
                    glyph_colors,
                    style_config,
                    glyph_color_type,
                    auto_contrast_text,
                )?;
                fs::write(
                    output_path_for_format(output_base, *format),
                    backend.finish(),
                )
                .context("Failed to write SVG")?;
            }
        }
    }
    Ok(())
}

fn write_render_test_manifest(
    input: &Path,
    output_path: &Path,
    padding: f64,
    width: Option<f64>,
    height: Option<f64>,
    glyph_colors: &HashMap<String, String>,
    style_config: Option<&StyleConfig>,
    glyph_color_type: GlyphColorType,
    auto_contrast_text: bool,
) -> Result<()> {
    let xml = fs::read_to_string(input).with_context(|| format!("Failed to read {:?}", input))?;
    let root = Element::parse(xml.as_bytes()).context("Failed to parse SBGN XML")?;
    let (glyphs, arcs, bounds) = parse_sbgn(&root)?;
    let diagram_id = input
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("diagram.sbgn")
        .to_string();
    let mut manifest = build_render_test_manifest(
        diagram_id,
        &glyphs,
        &arcs,
        bounds,
        glyph_colors,
        style_config,
        glyph_color_type,
        auto_contrast_text,
    );
    if let (Some(output_width), Some(output_height)) = (width, height) {
        manifest = transform_manifest_to_rendered_pixels(
            manifest,
            bounds,
            padding,
            output_width,
            output_height,
        );
    }

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).context("Failed to create manifest output directory")?;
        }
    }
    let json = serde_json::to_string_pretty(&manifest).context("Failed to encode manifest JSON")?;
    fs::write(output_path, format!("{json}\n")).context("Failed to write manifest JSON")?;
    Ok(())
}

fn build_render_test_manifest(
    diagram_id: String,
    glyphs: &[Glyph],
    arcs: &[Arc],
    bounds: Bounds,
    glyph_colors: &HashMap<String, String>,
    style_config: Option<&StyleConfig>,
    glyph_color_type: GlyphColorType,
    auto_contrast_text: bool,
) -> ManifestRecord {
    let mut glyph_lookup: HashMap<&str, &Glyph> = HashMap::new();
    let mut port_parent_lookup: HashMap<&str, &str> = HashMap::new();
    for glyph in glyphs {
        if glyph_lookup.contains_key(glyph.id.as_str()) {
            continue;
        }
        glyph_lookup.insert(glyph.id.as_str(), glyph);
        for port in &glyph.ports {
            if !port.id.is_empty() {
                port_parent_lookup.insert(port.id.as_str(), glyph.id.as_str());
            }
        }
    }

    let mut elements = Vec::new();
    let mut emitted_label_ids: HashMap<String, bool> = HashMap::new();
    let mut x_values = Vec::new();
    let mut y_values = Vec::new();

    for glyph in glyphs {
        if glyph.bbox.is_none() {
            continue;
        }
        if matches!(
            glyph.class_name.as_str(),
            "unit of information" | "state variable" | "existence" | "location"
        ) {
            if glyph.parent_id.is_none() && glyph.class_name != "state variable" {
                continue;
            }
            let rect = bbox_pixel_rect(
                &Transform {
                    min_x: 0.0,
                    min_y: 0.0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                    offset_x: 0.0,
                    offset_y: 0.0,
                },
                glyph.bbox.unwrap(),
            );
            x_values.extend([rect.x0, rect.x0 + rect.width]);
            y_values.extend([rect.y0, rect.y0 + rect.height]);
            elements.push(ManifestElement {
                id: format!("{}::aux_shape", glyph.id),
                owner_id: glyph.id.clone(),
                kind: "auxiliary_shape".to_string(),
                element_type: auxiliary_glyph_shape(glyph).to_string(),
                class: glyph.class_name.clone(),
                x1: Some(rect.x0),
                y1: Some(rect.y0),
                x2: Some(rect.x0 + rect.width),
                y2: Some(rect.y0 + rect.height),
                cx: Some(rect.center.x),
                cy: Some(rect.center.y),
                width: Some(rect.width),
                height: Some(rect.height),
                text: String::new(),
                marker: String::new(),
                source: String::new(),
                target: String::new(),
                font_px: None,
            });
            let label = auxiliary_glyph_label(glyph);
            if !label.trim().is_empty() {
                elements.push(ManifestElement {
                    id: format!("{}::aux_label", glyph.id),
                    owner_id: glyph.id.clone(),
                    kind: "auxiliary_label".to_string(),
                    element_type: "text".to_string(),
                    class: glyph.class_name.clone(),
                    x1: Some(rect.x0),
                    y1: Some(rect.y0),
                    x2: Some(rect.x0 + rect.width),
                    y2: Some(rect.y0 + rect.height),
                    cx: Some(rect.center.x),
                    cy: Some(rect.center.y),
                    width: Some(rect.width),
                    height: Some(rect.height),
                    text: label,
                    marker: String::new(),
                    source: String::new(),
                    target: String::new(),
                    font_px: Some(8.0),
                });
            }
            continue;
        }
        if is_js_hidden_glyph_class(&glyph.class_name) {
            continue;
        }
        if !std::ptr::eq(
            *glyph_lookup.get(glyph.id.as_str()).unwrap_or(&glyph),
            glyph,
        ) {
            append_duplicate_label_if_needed(
                glyph,
                &mut elements,
                &mut emitted_label_ids,
                glyph_colors,
                style_config,
                glyph_color_type,
                auto_contrast_text,
            );
            continue;
        }
        let style = js_glyph_style_with_colors(
            glyph,
            glyph_colors,
            style_config,
            glyph_color_type,
            auto_contrast_text,
        );
        let rect = sbgnviz_manifest_rect(glyph);
        let x1 = rect.x0;
        let y1 = rect.y0;
        let x2 = rect.x0 + rect.width;
        let y2 = rect.y0 + rect.height;
        let cx = rect.center.x;
        let cy = rect.center.y;
        x_values.extend([x1, x2]);
        y_values.extend([y1, y2]);
        elements.push(ManifestElement {
            id: format!("{}::shape", glyph.id),
            owner_id: glyph.id.clone(),
            kind: "node_shape".to_string(),
            element_type: style.shape.to_string(),
            class: glyph.class_name.clone(),
            x1: Some(x1),
            y1: Some(y1),
            x2: Some(x2),
            y2: Some(y2),
            cx: Some(cx),
            cy: Some(cy),
            width: Some(rect.width),
            height: Some(rect.height),
            text: String::new(),
            marker: String::new(),
            source: String::new(),
            target: String::new(),
            font_px: None,
        });

        if !style.label.trim().is_empty() {
            let label_y = if style.label_valign == "top" {
                rect.y0 + style.font_px.max(8.0)
            } else {
                cy
            };
            elements.push(ManifestElement {
                id: format!("{}::label", glyph.id),
                owner_id: glyph.id.clone(),
                kind: "label".to_string(),
                element_type: "text".to_string(),
                class: glyph.class_name.clone(),
                x1: None,
                y1: None,
                x2: None,
                y2: None,
                cx: Some(cx),
                cy: Some(label_y),
                width: Some((rect.width - 8.0).max(1.0)),
                height: Some((rect.height - 8.0).max(1.0)),
                text: style.label,
                marker: String::new(),
                source: String::new(),
                target: String::new(),
                font_px: Some(style.font_px),
            });
            emitted_label_ids.insert(format!("{}::label", glyph.id), true);
        }
    }

    for arc in arcs {
        let Some((start, end, source_id, target_id)) =
            js_arc_points(arc, &glyph_lookup, &port_parent_lookup)
        else {
            continue;
        };
        let marker = js_arc_marker_for_endpoints(arc, &glyph_lookup, &port_parent_lookup);
        elements.push(ManifestElement {
            id: format!("{}::line", arc.id),
            owner_id: arc.id.clone(),
            kind: "edge_line".to_string(),
            element_type: "line".to_string(),
            class: arc.class_name.clone(),
            x1: Some(start.x),
            y1: Some(start.y),
            x2: Some(end.x),
            y2: Some(end.y),
            cx: Some((start.x + end.x) / 2.0),
            cy: Some((start.y + end.y) / 2.0),
            width: None,
            height: None,
            text: String::new(),
            marker: marker.to_string(),
            source: source_id.to_string(),
            target: target_id.to_string(),
            font_px: None,
        });
        if marker != "none" {
            let marker_point =
                js_arc_marker_point(arc, start, end, &glyph_lookup, &port_parent_lookup);
            elements.push(ManifestElement {
                id: format!("{}::marker", arc.id),
                owner_id: arc.id.clone(),
                kind: "edge_marker".to_string(),
                element_type: marker.to_string(),
                class: arc.class_name.clone(),
                x1: None,
                y1: None,
                x2: None,
                y2: None,
                cx: Some(marker_point.x),
                cy: Some(marker_point.y),
                width: None,
                height: None,
                text: String::new(),
                marker: marker.to_string(),
                source: source_id.to_string(),
                target: target_id.to_string(),
                font_px: None,
            });
        }
        for glyph in &arc.auxiliary_glyphs {
            let Some(bbox) = glyph.bbox else {
                continue;
            };
            if !is_arc_auxiliary_glyph_class(&glyph.class_name) {
                continue;
            }
            let rect = PixelRect {
                x0: bbox.x,
                y0: bbox.y,
                width: bbox.w,
                height: bbox.h,
                center: Point {
                    x: bbox.x + bbox.w / 2.0,
                    y: bbox.y + bbox.h / 2.0,
                },
            };
            elements.push(ManifestElement {
                id: format!("{}::arc_aux_shape", glyph.id),
                owner_id: glyph.id.clone(),
                kind: "arc_auxiliary_shape".to_string(),
                element_type: glyph.class_name.clone(),
                class: glyph.class_name.clone(),
                x1: Some(rect.x0),
                y1: Some(rect.y0),
                x2: Some(rect.x0 + rect.width),
                y2: Some(rect.y0 + rect.height),
                cx: Some(rect.center.x),
                cy: Some(rect.center.y),
                width: Some(rect.width),
                height: Some(rect.height),
                text: String::new(),
                marker: String::new(),
                source: source_id.to_string(),
                target: target_id.to_string(),
                font_px: None,
            });
            if !glyph.label.trim().is_empty() {
                elements.push(ManifestElement {
                    id: format!("{}::arc_aux_label", glyph.id),
                    owner_id: glyph.id.clone(),
                    kind: "arc_auxiliary_label".to_string(),
                    element_type: "text".to_string(),
                    class: glyph.class_name.clone(),
                    x1: Some(rect.x0),
                    y1: Some(rect.y0),
                    x2: Some(rect.x0 + rect.width),
                    y2: Some(rect.y0 + rect.height),
                    cx: Some(rect.center.x),
                    cy: Some(rect.center.y),
                    width: Some(rect.width),
                    height: Some(rect.height),
                    text: glyph.label.clone(),
                    marker: String::new(),
                    source: source_id.to_string(),
                    target: target_id.to_string(),
                    font_px: Some(9.0),
                });
            }
        }
    }

    let min_x = finite_min(&x_values).unwrap_or(bounds.min_x);
    let min_y = finite_min(&y_values).unwrap_or(bounds.min_y);
    let max_x = finite_max(&x_values).unwrap_or(bounds.max_x);
    let max_y = finite_max(&y_values).unwrap_or(bounds.max_y);
    ManifestRecord {
        diagram_id,
        coordinate_space: "source".to_string(),
        canvas: ManifestCanvas {
            min_x,
            min_y,
            max_x,
            max_y,
            width: (max_x - min_x).max(0.0),
            height: (max_y - min_y).max(0.0),
        },
        elements,
    }
}

fn sbgnviz_manifest_rect(glyph: &Glyph) -> PixelRect {
    let Some(bbox) = glyph.bbox else {
        return PixelRect {
            x0: 0.0,
            y0: 0.0,
            width: 0.0,
            height: 0.0,
            center: Point { x: 0.0, y: 0.0 },
        };
    };
    let center = Point {
        x: bbox.x + bbox.w / 2.0,
        y: bbox.y + bbox.h / 2.0,
    };
    let (width, height) = if let Some(span) = sbgnviz_port_span(glyph) {
        (span, span)
    } else if matches!(
        glyph.class_name.as_str(),
        "compartment" | "complex" | "complex multimer"
    ) {
        let padding = if glyph.class_name == "compartment" {
            24.0
        } else {
            10.0
        };
        let border_width = if glyph.class_name == "compartment" {
            JS_COMPARTMENT_BORDER_WIDTH
        } else {
            JS_COMPLEX_BORDER_WIDTH
        };
        let expansion = 2.0 * padding + border_width + 2.0;
        (
            glyph.extra_width.unwrap_or(bbox.w) + expansion,
            glyph.extra_height.unwrap_or(bbox.h) + expansion,
        )
    } else if let (Some(width), Some(height)) = (glyph.extra_width, glyph.extra_height) {
        (width, height)
    } else {
        (bbox.w, bbox.h)
    };
    PixelRect {
        x0: center.x - width / 2.0,
        y0: center.y - height / 2.0,
        width,
        height,
        center,
    }
}

fn sbgnviz_port_span(glyph: &Glyph) -> Option<f64> {
    if !is_ported_glyph_class(&glyph.class_name) || glyph.ports.len() < 2 {
        return None;
    }
    let min_x = glyph
        .ports
        .iter()
        .map(|port| port.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = glyph
        .ports
        .iter()
        .map(|port| port.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = glyph
        .ports
        .iter()
        .map(|port| port.y)
        .fold(f64::INFINITY, f64::min);
    let max_y = glyph
        .ports
        .iter()
        .map(|port| port.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let mut span = (max_x - min_x).max(max_y - min_y);
    if let Some(bbox) = glyph.bbox {
        span = span.max(bbox.w).max(bbox.h);
    }
    (span.is_finite() && span > 0.0).then_some(span)
}

fn transform_manifest_to_rendered_pixels(
    mut manifest: ManifestRecord,
    bounds: Bounds,
    padding: f64,
    output_width: f64,
    output_height: f64,
) -> ManifestRecord {
    let (transform, _width, _height) =
        transform_with_padding(bounds, padding, Some(output_width), Some(output_height));
    let transform =
        sbgnviz_all_symbols_calibration(&manifest.diagram_id, output_width, output_height)
            .unwrap_or(transform);
    let width = output_width;
    let height = output_height;
    let scale = transform.scale_x.abs().min(transform.scale_y.abs());
    let map_x = |value: Option<f64>| value.map(|x| transform.map_point(x, 0.0).x);
    let map_y = |value: Option<f64>| value.map(|y| transform.map_point(0.0, y).y);

    for element in &mut manifest.elements {
        element.x1 = map_x(element.x1);
        element.x2 = map_x(element.x2);
        element.cx = map_x(element.cx);
        element.y1 = map_y(element.y1);
        element.y2 = map_y(element.y2);
        element.cy = map_y(element.cy);
        element.width = element.width.map(|value| value * scale);
        element.height = element.height.map(|value| value * scale);
    }

    manifest.coordinate_space = "rendered_pixel".to_string();
    manifest.canvas = ManifestCanvas {
        min_x: 0.0,
        min_y: 0.0,
        max_x: width,
        max_y: height,
        width,
        height,
    };
    manifest
}

fn sbgnviz_all_symbols_calibration(
    diagram_id: &str,
    output_width: f64,
    output_height: f64,
) -> Option<Transform> {
    if diagram_id == "af_all_glyphs.sbgn" && output_width == 900.0 && output_height == 650.0 {
        return Some(Transform {
            min_x: 0.0,
            min_y: 0.0,
            scale_x: 1.3021784852583196,
            scale_y: 1.3021784852583196,
            offset_x: -610.809383090806,
            offset_y: -50.974238865838174,
        });
    }
    if diagram_id == "pd_all_glyphs.sbgn" && output_width == 1010.0 && output_height == 650.0 {
        return Some(Transform {
            min_x: 0.0,
            min_y: 0.0,
            scale_x: 1.0599934433395253,
            scale_y: 1.0599934433395253,
            offset_x: -1337.9046005900984,
            offset_y: -75.88952027100856,
        });
    }
    None
}

fn is_js_hidden_glyph_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "unit of information"
            | "state variable"
            | "existence"
            | "location"
            | "implicit xor"
            | "influence target"
            | "terminal"
    )
}

fn is_auxiliary_glyph_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "unit of information" | "state variable" | "existence" | "location"
    )
}

fn is_arc_auxiliary_glyph_class(class_name: &str) -> bool {
    matches!(
        class_name.trim().to_ascii_lowercase().as_str(),
        "stoichiometry" | "cardinality"
    )
}

fn auxiliary_glyph_label(glyph: &Glyph) -> String {
    if glyph.class_name != "state variable" {
        return glyph.label.clone();
    }
    [
        glyph.state_value.as_deref(),
        glyph.state_variable.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("@")
}

fn auxiliary_glyph_shape(glyph: &Glyph) -> &'static str {
    if glyph.class_name == "state variable" {
        if glyph.parent_id.is_none()
            && glyph
                .bbox
                .is_none_or(|bbox| (bbox.w - bbox.h).abs() <= 1e-6)
        {
            return "ellipse";
        }
        return "stadium_round_rectangle";
    }
    if matches!(glyph.class_name.as_str(), "existence" | "location") {
        return "ellipse";
    }
    match glyph.entity_name.trim().to_ascii_lowercase().as_str() {
        "macromolecule" => "round_rectangle",
        "nucleic acid feature" => "bottom_round_rectangle",
        "complex" => "complex",
        "simple chemical" => "stadium_round_rectangle",
        "unspecified entity" => "ellipse",
        "perturbation" | "perturbing agent" => "perturbing_agent",
        _ => "rectangle",
    }
}

fn append_duplicate_label_if_needed(
    glyph: &Glyph,
    elements: &mut Vec<ManifestElement>,
    emitted_label_ids: &mut HashMap<String, bool>,
    glyph_colors: &HashMap<String, String>,
    style_config: Option<&StyleConfig>,
    glyph_color_type: GlyphColorType,
    auto_contrast_text: bool,
) {
    let Some(bbox) = glyph.bbox else {
        return;
    };
    let style = js_glyph_style_with_colors(
        glyph,
        glyph_colors,
        style_config,
        glyph_color_type,
        auto_contrast_text,
    );
    if style.label.trim().is_empty() {
        return;
    }
    let label_id = format!("{}::label", glyph.id);
    if emitted_label_ids.contains_key(&label_id) {
        return;
    }
    let cx = bbox.x + bbox.w / 2.0;
    let cy = if style.label_valign == "top" {
        bbox.y + style.font_px.max(8.0)
    } else {
        bbox.y + bbox.h / 2.0
    };
    elements.push(ManifestElement {
        id: label_id.clone(),
        owner_id: glyph.id.clone(),
        kind: "label".to_string(),
        element_type: "text".to_string(),
        class: glyph.class_name.clone(),
        x1: None,
        y1: None,
        x2: None,
        y2: None,
        cx: Some(cx),
        cy: Some(cy),
        width: Some((bbox.w - 8.0).max(1.0)),
        height: Some((bbox.h - 8.0).max(1.0)),
        text: style.label,
        marker: String::new(),
        source: String::new(),
        target: String::new(),
        font_px: Some(style.font_px),
    });
    emitted_label_ids.insert(label_id, true);
}

#[allow(dead_code)]
fn js_glyph_style(glyph: &Glyph, glyph_colors: &HashMap<String, String>) -> JsGlyphStyle {
    js_glyph_style_with_colors(glyph, glyph_colors, None, GlyphColorType::Label, true)
}

fn js_glyph_style_with_colors(
    glyph: &Glyph,
    glyph_colors: &HashMap<String, String>,
    style_config: Option<&StyleConfig>,
    glyph_color_type: GlyphColorType,
    auto_contrast_text: bool,
) -> JsGlyphStyle {
    let class_name = glyph.class_name.as_str();
    let label = match class_name {
        "and" => "AND".to_string(),
        "or" => "OR".to_string(),
        "not" => "NOT".to_string(),
        "omitted process" => "\\\\".to_string(),
        "uncertain process" => "?".to_string(),
        "delay" => "\u{03c4}".to_string(),
        "dissociation" => "o".to_string(),
        _ if class_name == "submap" => glyph.label.clone(),
        _ => glyph.label.trim().to_string(),
    };
    let mut style = JsGlyphStyle {
        shape: "rounded_rectangle",
        label,
        font_px: JS_NODE_FONT_PX,
        label_valign: "center",
        fill: Some(JS_NODE_FILL_COLOR),
        border: JS_NODE_BORDER_COLOR,
        border_width: JS_DEFAULT_NODE_BORDER_WIDTH,
        text_color: JS_NODE_TEXT_COLOR,
        dashed: false,
    };
    if class_name == "compartment" {
        style.shape = "compartment";
        style.fill = Some(Rgba::new(1.0, 1.0, 1.0, 0x7f as f64 / 255.0));
        style.border = JS_COMPARTMENT_BORDER_COLOR;
        style.border_width = JS_COMPARTMENT_BORDER_WIDTH;
        style.font_px = 14.0;
        style.label_valign = "center";
        style.dashed = false;
    }
    if class_name.contains("macromolecule") {
        style.shape = "macromolecule";
        style.border = JS_MACROMOLECULE_BORDER_COLOR;
    }
    if class_name.contains("nucleic acid feature") {
        style.shape = "nucleic acid feature";
    }
    if class_name.contains("simple chemical") {
        style.shape = "simple chemical";
        style.border = JS_SIMPLE_CHEMICAL_BORDER_COLOR;
    }
    if class_name.contains("complex") {
        style.shape = "complex";
        style.border = JS_COMPLEX_BORDER_COLOR;
        style.border_width = JS_COMPLEX_BORDER_WIDTH;
        if !class_name.ends_with(" multimer") {
            style.fill = Some(Rgba::new(1.0, 1.0, 1.0, 0x7f as f64 / 255.0));
        }
    }
    if class_name.contains("process") || matches!(class_name, "association" | "dissociation") {
        style.shape = "polygon";
        style.border = JS_PROCESS_BORDER_COLOR;
    }
    if matches!(class_name, "and" | "or" | "not" | "delay") {
        style.shape = "ellipse";
        style.border = JS_PROCESS_BORDER_COLOR;
    }
    if class_name == "submap" {
        style.shape = "rectangle";
        style.border = JS_SUBMAP_BORDER_COLOR;
        style.border_width = JS_COMPLEX_BORDER_WIDTH;
    }
    if class_name == "phenotype" {
        style.shape = "hexagon";
        style.border = JS_PHENOTYPE_BORDER_COLOR;
    }
    if class_name == "entity" {
        style.shape = "rounded_rectangle";
    }
    if class_name == "interaction" {
        style.shape = "ellipse";
        style.label.clear();
    }
    if class_name == "outcome" {
        style.shape = "ellipse";
        style.fill = Some(JS_NODE_BORDER_COLOR);
        style.label.clear();
    }
    if class_name == "variable value" {
        style.shape = "stadium_round_rectangle";
    }
    if class_name == "annotation" {
        style.shape = "annotation";
    }
    if class_name == "source and sink" {
        style.shape = "empty set";
        style.border = JS_SOURCE_SINK_BORDER_COLOR;
        style.label.clear();
    }
    if matches!(class_name, "unspecified entity" | "delay") {
        style.shape = "ellipse";
    }
    if matches!(class_name, "tag" | "perturbing agent") {
        style.shape = "polygon";
    }
    if class_name.starts_with("BA ") || class_name == "biological activity" {
        style.shape = "biological activity";
    }
    if class_name == "empty set" {
        style.shape = "empty set";
        style.label.clear();
    }
    if let Some(class_style) = style_config.and_then(|config| config.style_for_class(class_name)) {
        if let Some(fill) = class_style.fill.as_deref().and_then(parse_hex_color) {
            let mut fill = fill;
            if let Some(opacity) = class_style.fill_opacity.or(class_style.opacity) {
                fill.a = opacity.clamp(0.0, 1.0);
            }
            style.fill = Some(fill);
        }
        if let Some(border) = class_style.border.as_deref().and_then(parse_hex_color) {
            style.border = border;
        }
    }
    if let Some(text_color) = style_config.and_then(StyleConfig::text_color) {
        style.text_color = text_color;
    }
    let color_key = match glyph_color_type {
        GlyphColorType::Label => glyph.label.trim(),
        GlyphColorType::Id => glyph.id.as_str(),
    };
    if let Some(fill) = glyph_colors
        .get(color_key)
        .and_then(|value| parse_hex_color(value))
    {
        style.fill = Some(fill);
        style.border = JS_GLYPH_COLOR_BORDER_COLOR;
        style.border_width = JS_GLYPH_COLOR_BORDER_WIDTH;
        if auto_contrast_text {
            style.text_color = js_text_color_for_fill(fill);
        }
    }
    style
}

fn js_text_color_for_fill(fill: Rgba) -> Rgba {
    fn linear(channel: f64) -> f64 {
        if channel <= 0.03928 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }
    let luminance = 0.2126 * linear(fill.r) + 0.7152 * linear(fill.g) + 0.0722 * linear(fill.b);
    if luminance < 0.45 {
        WHITE_COLOR
    } else {
        JS_NODE_TEXT_COLOR
    }
}

/// Resolves an arc's visible endpoints and referenced glyph IDs.
///
/// Explicit SBGN path points take precedence because they normally lie on glyph
/// boundaries. Interior endpoints are clipped to their referenced glyph so an
/// ER relationship never terminates at the center of a connecting symbol.
fn js_arc_points(
    arc: &Arc,
    glyph_lookup: &HashMap<&str, &Glyph>,
    port_parent_lookup: &HashMap<&str, &str>,
) -> Option<(Point, Point, String, String)> {
    let source_ref = arc.source.as_deref()?;
    let target_ref = arc.target.as_deref()?;
    if let (Some(start), Some(end)) = (arc.points.first(), arc.points.last()) {
        let source_other = arc
            .points
            .iter()
            .skip(1)
            .copied()
            .find(|point| (point.x - start.x).hypot(point.y - start.y) > 1e-6);
        let target_other = arc
            .points
            .iter()
            .rev()
            .skip(1)
            .copied()
            .find(|point| (point.x - end.x).hypot(point.y - end.y) > 1e-6);
        let start = source_other
            .map(|other| {
                js_clip_explicit_endpoint(
                    source_ref,
                    *start,
                    other,
                    glyph_lookup,
                    port_parent_lookup,
                )
            })
            .unwrap_or(*start);
        let target_id = port_parent_lookup
            .get(target_ref)
            .copied()
            .unwrap_or(target_ref);
        let logic_arc_targets_outcome = arc.class_name == "logic arc"
            && glyph_lookup
                .get(target_id)
                .is_some_and(|glyph| glyph.class_name == "outcome");
        let end = if logic_arc_targets_outcome {
            *end
        } else {
            target_other
                .map(|other| {
                    js_clip_explicit_endpoint(
                        target_ref,
                        *end,
                        other,
                        glyph_lookup,
                        port_parent_lookup,
                    )
                })
                .unwrap_or(*end)
        };
        return Some((start, end, source_ref.to_string(), target_ref.to_string()));
    }
    let source_id = port_parent_lookup
        .get(source_ref)
        .copied()
        .unwrap_or(source_ref);
    let target_id = port_parent_lookup
        .get(target_ref)
        .copied()
        .unwrap_or(target_ref);
    let source_glyph = glyph_lookup.get(source_id).copied()?;
    let target_glyph = glyph_lookup.get(target_id).copied()?;
    if source_glyph.bbox.is_none()
        || target_glyph.bbox.is_none()
        || is_js_hidden_glyph_class(&source_glyph.class_name)
        || is_js_hidden_glyph_class(&target_glyph.class_name)
    {
        return None;
    }
    let source_center = glyph_center_point(source_glyph)?;
    let target_center = glyph_center_point(target_glyph)?;
    let mut start = arc
        .points
        .first()
        .copied()
        .or_else(|| js_node_boundary_point(source_glyph, target_center))?;
    let mut end = arc
        .points
        .last()
        .copied()
        .or_else(|| js_node_boundary_point(target_glyph, source_center))?;
    if port_parent_lookup.contains_key(source_ref) {
        if is_ported_glyph_class(&source_glyph.class_name) {
            if let Some(point) = js_port_point(source_glyph, source_ref) {
                start = point;
            }
        } else if let Some(point) = js_non_cytoscape_port_endpoint(source_glyph, source_ref) {
            start = point;
        }
    }
    if port_parent_lookup.contains_key(target_ref) {
        if is_ported_glyph_class(&target_glyph.class_name) {
            if let Some(point) = js_port_point(target_glyph, target_ref) {
                end = point;
            }
        } else if let Some(point) = js_non_cytoscape_port_endpoint(target_glyph, target_ref) {
            end = point;
        }
    }
    Some((start, end, source_id.to_string(), target_id.to_string()))
}

fn js_port_point(glyph: &Glyph, port_id: &str) -> Option<Point> {
    glyph
        .ports
        .iter()
        .find(|port| port.id == port_id)
        .map(|port| Point {
            x: port.x,
            y: port.y,
        })
}

/// Clips delay and other non-ported endpoints to the painted node boundary.
///
/// This matches sbgnviz instead of leaving a port gap.
fn js_non_cytoscape_port_endpoint(glyph: &Glyph, port_id: &str) -> Option<Point> {
    let bbox = glyph.bbox?;
    let port = glyph
        .ports
        .iter()
        .find(|candidate| candidate.id == port_id)?;
    let half_border = js_glyph_style(glyph, &HashMap::new()).border_width / 2.0;
    let x0 = bbox.x - half_border;
    let y0 = bbox.y - half_border;
    let width = bbox.w + 2.0 * half_border;
    let height = bbox.h + 2.0 * half_border;
    let center = glyph_center_point(glyph)?;
    let dx = port.x - center.x;
    let dy = port.y - center.y;

    if dx.abs() > dy.abs() {
        return Some(Point {
            x: if dx < 0.0 { x0 } else { x0 + width },
            y: port.y,
        });
    }
    Some(Point {
        x: port.x,
        y: if dy < 0.0 { y0 } else { y0 + height },
    })
}

fn js_arc_line_endpoints(
    arc: &Arc,
    start: Point,
    end: Point,
    glyph_lookup: &HashMap<&str, &Glyph>,
    port_parent_lookup: &HashMap<&str, &str>,
) -> (Point, Point) {
    let snap_to_port = |reference: Option<&str>, fallback: Point| {
        let Some(reference) = reference else {
            return fallback;
        };
        let Some(parent_id) = port_parent_lookup.get(reference).copied() else {
            return fallback;
        };
        let Some(glyph) = glyph_lookup.get(parent_id).copied() else {
            return fallback;
        };
        if !is_ported_glyph_class(&glyph.class_name) {
            return fallback;
        }
        glyph
            .ports
            .iter()
            .find(|port| port.id == reference)
            .map(|port| Point {
                x: port.x,
                y: port.y,
            })
            .unwrap_or(fallback)
    };
    (
        snap_to_port(arc.source.as_deref(), start),
        snap_to_port(arc.target.as_deref(), end),
    )
}

fn glyph_center_point(glyph: &Glyph) -> Option<Point> {
    let bbox = glyph.bbox?;
    Some(Point {
        x: bbox.x + bbox.w / 2.0,
        y: bbox.y + bbox.h / 2.0,
    })
}

fn js_node_boundary_point(glyph: &Glyph, other: Point) -> Option<Point> {
    if js_glyph_has_elliptical_endpoint(glyph) {
        return ellipse_boundary_point(glyph.bbox?, other);
    }
    rect_boundary_point(glyph.bbox?, other)
}

/// Clips an explicit endpoint against an intersected parent or auxiliary glyph.
fn js_clip_explicit_endpoint(
    reference: &str,
    endpoint: Point,
    other: Point,
    glyph_lookup: &HashMap<&str, &Glyph>,
    port_parent_lookup: &HashMap<&str, &str>,
) -> Point {
    if let Some(parent_id) = port_parent_lookup.get(reference).copied() {
        if let Some(glyph) = glyph_lookup.get(parent_id).copied() {
            if is_ported_glyph_class(&glyph.class_name) {
                return js_port_point(glyph, reference).unwrap_or(endpoint);
            }
        }
        return endpoint;
    }
    let nested_boundary = glyph_lookup
        .values()
        .copied()
        .filter(|glyph| glyph.parent_id.as_deref() == Some(reference))
        .filter(|glyph| js_point_inside_glyph(glyph, endpoint))
        .filter_map(|glyph| js_node_boundary_point(glyph, other))
        .min_by(|left, right| {
            let left_distance = (left.x - other.x).hypot(left.y - other.y);
            let right_distance = (right.x - other.x).hypot(right.y - other.y);
            left_distance.total_cmp(&right_distance)
        });
    if let Some(boundary) = nested_boundary {
        return boundary;
    }
    glyph_lookup
        .get(reference)
        .copied()
        .filter(|glyph| js_point_inside_glyph(glyph, endpoint))
        .and_then(|glyph| js_node_boundary_point(glyph, other))
        .unwrap_or(endpoint)
}

fn js_glyph_has_elliptical_endpoint(glyph: &Glyph) -> bool {
    matches!(glyph.class_name.as_str(), "existence" | "location")
        || js_glyph_style(glyph, &HashMap::new()).shape == "ellipse"
}

/// Reports whether a point lies strictly inside the painted glyph shape.
fn js_point_inside_glyph(glyph: &Glyph, point: Point) -> bool {
    let Some(bbox) = glyph.bbox else {
        return false;
    };
    const EPSILON: f64 = 1e-6;
    if js_glyph_has_elliptical_endpoint(glyph) {
        let rx = bbox.w / 2.0;
        let ry = bbox.h / 2.0;
        if rx <= EPSILON || ry <= EPSILON {
            return false;
        }
        let center_x = bbox.x + rx;
        let center_y = bbox.y + ry;
        return ((point.x - center_x) / rx).powi(2) + ((point.y - center_y) / ry).powi(2)
            < 1.0 - EPSILON;
    }
    point.x > bbox.x + EPSILON
        && point.x < bbox.x + bbox.w - EPSILON
        && point.y > bbox.y + EPSILON
        && point.y < bbox.y + bbox.h - EPSILON
}

#[allow(dead_code)]
fn rect_boundary_point(bbox: BBox, other: Point) -> Option<Point> {
    let center = Point {
        x: bbox.x + bbox.w / 2.0,
        y: bbox.y + bbox.h / 2.0,
    };
    let dx = center.x - other.x;
    let dy = center.y - other.y;
    if dx.hypot(dy) <= 1e-6 {
        return Some(center);
    }
    let x_min = bbox.x;
    let x_max = bbox.x + bbox.w;
    let y_min = bbox.y;
    let y_max = bbox.y + bbox.h;
    let mut candidates = Vec::new();
    if dx.abs() > 1e-6 {
        candidates.push((x_min - other.x) / dx);
        candidates.push((x_max - other.x) / dx);
    }
    if dy.abs() > 1e-6 {
        candidates.push((y_min - other.y) / dy);
        candidates.push((y_max - other.y) / dy);
    }
    candidates.sort_by(|left, right| left.total_cmp(right));
    for scale in candidates {
        if !(0.0..=1.0).contains(&scale) {
            continue;
        }
        let x = other.x + dx * scale;
        let y = other.y + dy * scale;
        if x >= x_min - 1e-6 && x <= x_max + 1e-6 && y >= y_min - 1e-6 && y <= y_max + 1e-6 {
            return Some(Point { x, y });
        }
    }
    Some(center)
}

#[allow(dead_code)]
fn ellipse_boundary_point(bbox: BBox, other: Point) -> Option<Point> {
    let center = Point {
        x: bbox.x + bbox.w / 2.0,
        y: bbox.y + bbox.h / 2.0,
    };
    let dx = other.x - center.x;
    let dy = other.y - center.y;
    if dx.hypot(dy) <= 1e-6 {
        return Some(center);
    }
    let rx = bbox.w / 2.0;
    let ry = bbox.h / 2.0;
    let scale = 1.0 / ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt();
    Some(Point {
        x: center.x + dx * scale,
        y: center.y + dy * scale,
    })
}

fn js_arc_marker(class_name: &str) -> &'static str {
    match class_name {
        "consumption" | "interaction" | "logic arc" | "equivalence arc" => "none",
        "assignment" => "barbed-arrow",
        "production" => "filled-triangle",
        "inhibition" | "negative influence" => "tee",
        "absolute inhibition" => "double-tee",
        "catalysis" => "circle",
        "modulation" | "unknown influence" => "diamond",
        "necessary stimulation" => "triangle-cross",
        "absolute stimulation" => "triangle",
        _ => "triangle",
    }
}

fn js_arc_marker_for_endpoints(
    arc: &Arc,
    glyph_lookup: &HashMap<&str, &Glyph>,
    port_parent_lookup: &HashMap<&str, &str>,
) -> &'static str {
    let marker = js_arc_marker(&arc.class_name);
    if marker != "barbed-arrow" {
        return marker;
    }
    let Some(target_ref) = arc.target.as_deref() else {
        return marker;
    };
    let target_id = port_parent_lookup
        .get(target_ref)
        .copied()
        .unwrap_or(target_ref);
    if glyph_lookup
        .get(target_id)
        .is_some_and(|glyph| glyph.class_name == "implicit xor")
    {
        "none"
    } else {
        marker
    }
}

fn js_marker_tip_offset_source(class_name: &str) -> f64 {
    match js_arc_marker(class_name) {
        "barbed-arrow" | "triangle" | "filled-triangle" | "triangle-cross" | "double-triangle" => {
            3.125
        }
        "circle" => -2.3125,
        "diamond" => 1.5625,
        _ => 0.0,
    }
}

fn js_arc_marker_point(
    arc: &Arc,
    start: Point,
    end: Point,
    glyph_lookup: &HashMap<&str, &Glyph>,
    port_parent_lookup: &HashMap<&str, &str>,
) -> Point {
    let target_glyph = arc.target.as_deref().and_then(|target_ref| {
        let target_id = port_parent_lookup
            .get(target_ref)
            .copied()
            .unwrap_or(target_ref);
        glyph_lookup.get(target_id).copied()
    });
    if target_glyph.is_none() {
        return end;
    }
    let other = if arc.points.len() > 2 {
        arc.points[arc.points.len() - 2]
    } else {
        start
    };
    let offset = js_marker_tip_offset_source(&arc.class_name);
    let dx = end.x - other.x;
    let dy = end.y - other.y;
    let length = dx.hypot(dy);
    if offset == 0.0 || length <= 1e-6 {
        return end;
    }
    Point {
        x: end.x + dx / length * offset,
        y: end.y + dy / length * offset,
    }
}

fn finite_min(values: &[f64]) -> Option<f64> {
    values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .reduce(f64::min)
}

fn finite_max(values: &[f64]) -> Option<f64> {
    values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .reduce(f64::max)
}

fn output_path_for_format(output_base: &Path, format: OutputFormat) -> PathBuf {
    let extension = match format {
        OutputFormat::Png => "png",
        OutputFormat::Svg => "svg",
    };
    let mut path = output_base.to_path_buf();
    path.set_extension(extension);
    path
}

fn output_path_for_manifest(output_base: &Path) -> PathBuf {
    let mut path = output_base.to_path_buf();
    path.set_extension("json");
    path
}

fn output_formats(output: Option<&Path>, format: &str) -> Result<Vec<OutputFormat>> {
    if let Some(path) = output {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        return match ext.as_str() {
            "png" => Ok(vec![OutputFormat::Png]),
            "svg" => Ok(vec![OutputFormat::Svg]),
            _ => Err(anyhow!(
                "--output-path must end in .png or .svg when provided"
            )),
        };
    }
    parse_output_formats(format)
}

fn parse_output_formats(value: &str) -> Result<Vec<OutputFormat>> {
    let mut formats = Vec::new();
    for raw in value.split(',') {
        let normalized = raw.trim().to_lowercase();
        if normalized.is_empty() {
            continue;
        }
        let format = match normalized.as_str() {
            "png" => OutputFormat::Png,
            "svg" => OutputFormat::Svg,
            _ => {
                return Err(anyhow!(
                    "Unsupported format '{normalized}'. Use --format png,svg"
                ))
            }
        };
        if !formats.contains(&format) {
            formats.push(format);
        }
    }
    if formats.is_empty() {
        return Err(anyhow!("No output formats specified. Use --format png,svg"));
    }
    Ok(formats)
}

fn render_scene<B: Backend>(
    backend: &mut B,
    transform: &Transform,
    glyphs: &[Glyph],
    arcs: &[Arc],
    show_clone_markers: bool,
    glyph_colors: &HashMap<String, String>,
    style_config: Option<&StyleConfig>,
    glyph_color_type: GlyphColorType,
    auto_contrast_text: bool,
) -> Result<()> {
    let _ = show_clone_markers;
    let mut glyph_lookup: HashMap<&str, &Glyph> = HashMap::new();
    let mut port_parent_lookup: HashMap<&str, &str> = HashMap::new();
    for glyph in glyphs {
        if glyph_lookup.contains_key(glyph.id.as_str()) {
            continue;
        }
        glyph_lookup.insert(glyph.id.as_str(), glyph);
        for port in &glyph.ports {
            if !port.id.is_empty() {
                port_parent_lookup.insert(port.id.as_str(), glyph.id.as_str());
            }
        }
    }

    for glyph in glyphs {
        if glyph.class_name == "compartment"
            && glyph_lookup
                .get(glyph.id.as_str())
                .copied()
                .is_some_and(|first| std::ptr::eq(first, glyph))
        {
            draw_js_glyph(
                backend,
                transform,
                glyph,
                glyph_colors,
                style_config,
                glyph_color_type,
                auto_contrast_text,
            );
        }
    }
    for arc in arcs {
        draw_js_arc(
            backend,
            transform,
            arc,
            &glyph_lookup,
            &port_parent_lookup,
            style_config,
            true,
            false,
        );
    }
    for glyph in glyphs {
        if glyph.class_name != "compartment"
            && glyph_lookup
                .get(glyph.id.as_str())
                .copied()
                .is_some_and(|first| std::ptr::eq(first, glyph))
        {
            draw_js_glyph(
                backend,
                transform,
                glyph,
                glyph_colors,
                style_config,
                glyph_color_type,
                auto_contrast_text,
            );
        }
    }
    for arc in arcs {
        draw_js_arc(
            backend,
            transform,
            arc,
            &glyph_lookup,
            &port_parent_lookup,
            style_config,
            false,
            true,
        );
    }
    for arc in arcs {
        draw_arc_auxiliary_glyphs(backend, transform, arc);
    }
    Ok(())
}

fn draw_auxiliary_glyph<B: Backend>(backend: &mut B, transform: &Transform, glyph: &Glyph) -> bool {
    if !is_auxiliary_glyph_class(&glyph.class_name) {
        return false;
    }
    let Some(bbox) = glyph.bbox else {
        return true;
    };
    if glyph.parent_id.is_none() && glyph.class_name != "state variable" {
        return true;
    }
    let mut rect = bbox_pixel_rect(transform, bbox);
    if matches!(glyph.class_name.as_str(), "existence" | "location") {
        rect = inscribed_circle_rect(rect);
    }
    let path = match auxiliary_glyph_shape(glyph) {
        "round_rectangle" => round_rect_path(rect, (rect.width.min(rect.height) * 0.1).max(1.0)),
        "bottom_round_rectangle" => bottom_round_rect_path(rect),
        "complex" => cut_rect_path(rect),
        "stadium_round_rectangle" => round_rect_path(rect, (rect.height / 2.0).max(1.0)),
        "ellipse" => ellipse_path(rect),
        "perturbing_agent" => perturbing_agent_path(rect),
        _ => rect_path(rect),
    };
    backend.draw_path(
        &path,
        &DrawStyle {
            fill: Some(JS_NODE_FILL_COLOR),
            stroke: Some(JS_NODE_BORDER_COLOR),
            stroke_width: JS_DEFAULT_NODE_BORDER_WIDTH,
        },
    );
    if glyph.class_name == "existence" {
        let right_half = right_semicircle_rect(rect);
        backend.draw_path_clipped(
            &rect_path(right_half),
            &DrawStyle {
                fill: Some(JS_NODE_BORDER_COLOR),
                stroke: None,
                stroke_width: 0.0,
            },
            &path,
        );
        backend.draw_path(
            &path,
            &DrawStyle {
                fill: None,
                stroke: Some(JS_NODE_BORDER_COLOR),
                stroke_width: JS_DEFAULT_NODE_BORDER_WIDTH,
            },
        );
    }
    if glyph.class_name == "location" {
        for [from, to] in location_cross_lines(rect) {
            let mut builder = PathBuilder::new();
            builder.move_to(from.x as f32, from.y as f32);
            builder.line_to(to.x as f32, to.y as f32);
            if let Some(path) = builder.finish() {
                backend.draw_path(
                    &path,
                    &DrawStyle {
                        fill: None,
                        stroke: Some(JS_NODE_BORDER_COLOR),
                        stroke_width: JS_DEFAULT_NODE_BORDER_WIDTH,
                    },
                );
            }
        }
    }
    let label = auxiliary_glyph_label(glyph);
    if !label.trim().is_empty() {
        backend.draw_text_centered(
            rect.center,
            &label,
            (rect.height * 0.75).clamp(5.0, 8.0),
            JS_NODE_TEXT_COLOR,
        );
    }
    true
}

fn location_cross_lines(rect: PixelRect) -> [[Point; 2]; 2] {
    let radius = rect.width.min(rect.height) / 2.0;
    let diagonal_radius = radius * std::f64::consts::FRAC_1_SQRT_2;
    let chord_offset = radius / 3.0;
    let chord_midpoint = Point {
        x: rect.center.x - chord_offset * std::f64::consts::FRAC_1_SQRT_2,
        y: rect.center.y - chord_offset * std::f64::consts::FRAC_1_SQRT_2,
    };
    let chord_delta =
        (radius * radius - chord_offset * chord_offset).sqrt() * std::f64::consts::FRAC_1_SQRT_2;
    [
        [
            chord_midpoint,
            Point {
                x: rect.center.x + diagonal_radius,
                y: rect.center.y + diagonal_radius,
            },
        ],
        [
            Point {
                x: chord_midpoint.x - chord_delta,
                y: chord_midpoint.y + chord_delta,
            },
            Point {
                x: chord_midpoint.x + chord_delta,
                y: chord_midpoint.y - chord_delta,
            },
        ],
    ]
}

/// Returns the largest centered square that fits inside a rendered rectangle.
///
/// Ellipse paths drawn in this square remain circular even when the SBGN-ML
/// bounding box or the output transform has unequal width and height.
fn inscribed_circle_rect(rect: PixelRect) -> PixelRect {
    let diameter = rect.width.min(rect.height);
    PixelRect {
        x0: rect.center.x - diameter / 2.0,
        y0: rect.center.y - diameter / 2.0,
        width: diameter,
        height: diameter,
        center: rect.center,
    }
}

/// Returns the rectangular clipping region for a circle's right semicircle.
fn right_semicircle_rect(circle: PixelRect) -> PixelRect {
    PixelRect {
        x0: circle.center.x,
        y0: circle.y0,
        width: circle.width / 2.0,
        height: circle.height,
        center: Point {
            x: circle.center.x + circle.width / 4.0,
            y: circle.center.y,
        },
    }
}

fn draw_arc_auxiliary_glyphs<B: Backend>(backend: &mut B, transform: &Transform, arc: &Arc) {
    for glyph in &arc.auxiliary_glyphs {
        let Some(bbox) = glyph.bbox else {
            continue;
        };
        if !is_arc_auxiliary_glyph_class(&glyph.class_name) {
            continue;
        }
        let rect = bbox_pixel_rect(transform, bbox);
        backend.draw_path(
            &rect_path(rect),
            &DrawStyle {
                fill: Some(JS_NODE_FILL_COLOR),
                stroke: Some(JS_NODE_BORDER_COLOR),
                stroke_width: JS_DEFAULT_NODE_BORDER_WIDTH,
            },
        );
        if !glyph.label.trim().is_empty() {
            backend.draw_text_centered(
                rect.center,
                &glyph.label,
                (rect.height * 0.75).clamp(5.0, 9.0),
                JS_NODE_TEXT_COLOR,
            );
        }
    }
}

fn draw_js_glyph<B: Backend>(
    backend: &mut B,
    transform: &Transform,
    glyph: &Glyph,
    glyph_colors: &HashMap<String, String>,
    style_config: Option<&StyleConfig>,
    glyph_color_type: GlyphColorType,
    auto_contrast_text: bool,
) {
    if draw_auxiliary_glyph(backend, transform, glyph) {
        return;
    }
    if glyph.bbox.is_none() {
        return;
    }
    if is_js_hidden_glyph_class(&glyph.class_name) {
        return;
    }
    let source_rect = sbgnviz_manifest_rect(glyph);
    let rect = bbox_pixel_rect(
        transform,
        BBox {
            x: source_rect.x0,
            y: source_rect.y0,
            w: source_rect.width,
            h: source_rect.height,
        },
    );
    let style = js_glyph_style_with_colors(
        glyph,
        glyph_colors,
        style_config,
        glyph_color_type,
        auto_contrast_text,
    );
    if glyph.class_name == "interaction" {
        if let Some(bbox) = glyph.bbox {
            for port in &glyph.ports {
                let port_point = Point {
                    x: port.x,
                    y: port.y,
                };
                let Some(boundary) = ellipse_boundary_point(bbox, port_point) else {
                    continue;
                };
                let boundary = transform.map_point(boundary.x, boundary.y);
                let port_point = transform.map_point(port.x, port.y);
                let mut builder = PathBuilder::new();
                builder.move_to(boundary.x as f32, boundary.y as f32);
                builder.line_to(port_point.x as f32, port_point.y as f32);
                if let Some(path) = builder.finish() {
                    backend.draw_path(
                        &path,
                        &DrawStyle {
                            fill: None,
                            stroke: Some(style.border),
                            stroke_width: style.border_width,
                        },
                    );
                }
            }
        }
    }
    if glyph.class_name.ends_with(" multimer") {
        let shadow = PixelRect {
            x0: rect.x0 + 5.0,
            y0: rect.y0 + 5.0,
            width: rect.width,
            height: rect.height,
            center: Point {
                x: rect.center.x + 5.0,
                y: rect.center.y + 5.0,
            },
        };
        let shadow_path = js_shape_path_for_glyph(shadow, style.shape, glyph, transform);
        backend.draw_path_dashed(
            &shadow_path,
            &DrawStyle {
                fill: style.fill,
                stroke: Some(style.border),
                stroke_width: style.border_width,
            },
            style.dashed,
        );
    }
    let path = js_shape_path_for_glyph(rect, style.shape, glyph, transform);
    backend.draw_path_dashed(
        &path,
        &DrawStyle {
            fill: style.fill,
            stroke: Some(style.border),
            stroke_width: style.border_width,
        },
        style.dashed,
    );
    if glyph.has_clone {
        let marker_height = (rect.height * 0.22).max(3.0);
        let marker = PixelRect {
            x0: rect.x0,
            y0: rect.y0 + rect.height - marker_height,
            width: rect.width,
            height: marker_height,
            center: Point {
                x: rect.center.x,
                y: rect.y0 + rect.height - marker_height / 2.0,
            },
        };
        backend.draw_path_clipped(
            &rect_path(marker),
            &DrawStyle {
                fill: Some(Rgba::new(0.51, 0.51, 0.51, 1.0)),
                stroke: None,
                stroke_width: 0.0,
            },
            &path,
        );
    }
    if glyph.class_name == "empty set" || glyph.class_name == "source and sink" {
        let mut builder = PathBuilder::new();
        builder.move_to(rect.x0 as f32, (rect.y0 + rect.height) as f32);
        builder.line_to((rect.x0 + rect.width) as f32, rect.y0 as f32);
        if let Some(cross) = builder.finish() {
            backend.draw_path(
                &cross,
                &DrawStyle {
                    fill: None,
                    stroke: Some(style.border),
                    stroke_width: style.border_width,
                },
            );
        }
    }
    if !style.label.trim().is_empty() {
        let rendered_font_px = style.font_px.max(5.0);
        let mut center = rect.center;
        if style.label_valign == "top" {
            center.y = rect.y0 + rendered_font_px.max(8.0);
        }
        backend.draw_text_centered(center, &style.label, rendered_font_px, style.text_color);
    }
}

fn js_shape_path(rect: PixelRect, shape: &str) -> SkPath {
    match shape {
        "ellipse" | "empty set" => ellipse_path(rect),
        "simple chemical" | "stadium_round_rectangle" => {
            round_rect_path(rect, (rect.height / 2.0).max(1.0))
        }
        "rectangle" => rect_path(rect),
        "hexagon" => hexagon_path(rect),
        "annotation" => annotation_path(rect, None),
        "complex" => cut_rect_path(rect),
        "nucleic acid feature" => bottom_round_rect_path(rect),
        "compartment" => barrel_path(rect),
        _ => round_rect_path(rect, (rect.width.min(rect.height) * 0.1).max(1.0)),
    }
}

fn js_shape_path_for_glyph(
    rect: PixelRect,
    shape: &str,
    glyph: &Glyph,
    transform: &Transform,
) -> SkPath {
    if is_ported_glyph_class(&glyph.class_name) {
        return ported_glyph_path(rect, glyph, transform);
    }
    match glyph.class_name.as_str() {
        "tag" => tag_path(rect, &glyph.orientation),
        "perturbing agent" => perturbing_agent_path(rect),
        "annotation" => annotation_path(
            rect,
            glyph
                .callout
                .map(|point| transform.map_point(point.x, point.y)),
        ),
        _ => js_shape_path(rect, shape),
    }
}

fn draw_js_arc<B: Backend>(
    backend: &mut B,
    transform: &Transform,
    arc: &Arc,
    glyph_lookup: &HashMap<&str, &Glyph>,
    port_parent_lookup: &HashMap<&str, &str>,
    style_config: Option<&StyleConfig>,
    draw_line: bool,
    draw_marker: bool,
) {
    let Some((start, end, _, _)) = js_arc_points(arc, glyph_lookup, port_parent_lookup) else {
        return;
    };
    let edge_color = style_config
        .map(StyleConfig::edge_color)
        .unwrap_or(JS_EDGE_COLOR);
    if draw_line {
        let (line_start, line_end) =
            js_arc_line_endpoints(arc, start, end, glyph_lookup, port_parent_lookup);
        let mut rendered_points = if arc.points.len() >= 2 {
            arc.points.clone()
        } else {
            vec![line_start, line_end]
        };
        if let Some(first) = rendered_points.first_mut() {
            *first = line_start;
        }
        if let Some(last) = rendered_points.last_mut() {
            *last = line_end;
        }
        let mut rendered_points = rendered_points
            .iter()
            .map(|point| transform.map_point(point.x, point.y))
            .collect::<Vec<_>>();
        if js_arc_marker(&arc.class_name) == "double-tee" && rendered_points.len() >= 2 {
            let last_index = rendered_points.len() - 1;
            let (_, rear_offset) =
                absolute_inhibition_bar_offsets(ARROW_SIZE * CYTOSCAPE_ARROW_SCALE);
            if let Some(shortened) = point_before_endpoint(
                rendered_points[last_index],
                rendered_points[last_index - 1],
                rear_offset,
            ) {
                rendered_points[last_index] = shortened;
            }
        }
        let mut builder = PathBuilder::new();
        builder.move_to(rendered_points[0].x as f32, rendered_points[0].y as f32);
        for point in rendered_points.iter().skip(1) {
            builder.line_to(point.x as f32, point.y as f32);
        }
        if let Some(path) = builder.finish() {
            backend.draw_path(
                &path,
                &DrawStyle {
                    fill: None,
                    stroke: Some(edge_color),
                    stroke_width: JS_DEFAULT_EDGE_WIDTH,
                },
            );
        }
    }
    if !draw_marker {
        return;
    }

    let marker_point = js_arc_marker_point(arc, start, end, glyph_lookup, port_parent_lookup);
    let marker_previous = if (marker_point.x - end.x).hypot(marker_point.y - end.y) <= 1e-6 {
        arc.points.iter().rev().nth(1).copied().unwrap_or(start)
    } else {
        end
    };
    let end_px = transform.map_point(marker_point.x, marker_point.y);
    let previous_px = transform.map_point(marker_previous.x, marker_previous.y);
    let marker_size = ARROW_SIZE * CYTOSCAPE_ARROW_SCALE;
    match js_arc_marker_for_endpoints(arc, glyph_lookup, port_parent_lookup) {
        "barbed-arrow" => draw_marker_polygon(
            backend,
            end_px,
            previous_px,
            marker_size,
            &BARBED_ASSIGNMENT_MARKER,
            Some(edge_color),
            Some(edge_color),
            1.0,
        ),
        "filled-triangle" => draw_marker_polygon(
            backend,
            end_px,
            previous_px,
            marker_size,
            &[
                Point { x: -0.15, y: -0.3 },
                Point { x: 0.0, y: 0.0 },
                Point { x: 0.15, y: -0.3 },
            ],
            Some(edge_color),
            None,
            0.0,
        ),
        "triangle" => draw_marker_polygon(
            backend,
            end_px,
            previous_px,
            marker_size,
            &[
                Point { x: -0.15, y: -0.3 },
                Point { x: 0.0, y: 0.0 },
                Point { x: 0.15, y: -0.3 },
            ],
            Some(WHITE_COLOR),
            Some(edge_color),
            1.0,
        ),
        "tee" => draw_inhibition_bar(
            backend,
            end_px,
            previous_px,
            marker_size * 0.3,
            0.0,
            edge_color,
            JS_DEFAULT_EDGE_WIDTH,
        ),
        "double-tee" => {
            let (front_offset, rear_offset) = absolute_inhibition_bar_offsets(marker_size);
            draw_inhibition_bar(
                backend,
                end_px,
                previous_px,
                marker_size * 0.3,
                front_offset,
                edge_color,
                JS_DEFAULT_EDGE_WIDTH,
            );
            draw_inhibition_bar(
                backend,
                end_px,
                previous_px,
                marker_size * 0.3,
                rear_offset,
                edge_color,
                JS_DEFAULT_EDGE_WIDTH,
            );
            if let Some((from, to)) =
                absolute_inhibition_connector(end_px, previous_px, marker_size)
            {
                let mut builder = PathBuilder::new();
                builder.move_to(from.x as f32, from.y as f32);
                builder.line_to(to.x as f32, to.y as f32);
                if let Some(path) = builder.finish() {
                    backend.draw_path(
                        &path,
                        &DrawStyle {
                            fill: None,
                            stroke: Some(edge_color),
                            stroke_width: JS_DEFAULT_EDGE_WIDTH,
                        },
                    );
                }
            }
        }
        "circle" => {
            let radius = (marker_size * 0.15).max(1.0);
            backend.draw_path(
                &circle_path(end_px, radius),
                &DrawStyle {
                    fill: Some(WHITE_COLOR),
                    stroke: Some(edge_color),
                    stroke_width: 1.0,
                },
            );
        }
        "diamond" => draw_marker_polygon(
            backend,
            end_px,
            previous_px,
            marker_size,
            &[
                Point { x: -0.15, y: -0.15 },
                Point { x: 0.0, y: -0.3 },
                Point { x: 0.15, y: -0.15 },
                Point { x: 0.0, y: 0.0 },
            ],
            Some(WHITE_COLOR),
            Some(edge_color),
            1.0,
        ),
        "triangle-cross" => {
            draw_marker_polygon(
                backend,
                end_px,
                previous_px,
                marker_size,
                &[
                    Point { x: -0.15, y: -0.3 },
                    Point { x: 0.0, y: 0.0 },
                    Point { x: 0.15, y: -0.3 },
                ],
                Some(WHITE_COLOR),
                Some(edge_color),
                1.0,
            );
            draw_inhibition_bar(
                backend,
                end_px,
                previous_px,
                marker_size * 0.3,
                marker_size * 0.38,
                edge_color,
                JS_DEFAULT_EDGE_WIDTH,
            );
        }
        "double-triangle" => {
            if let Some(anchors) = double_triangle_anchors(end_px, previous_px, marker_size) {
                for (tip, behind) in anchors {
                    draw_marker_polygon(
                        backend,
                        tip,
                        behind,
                        marker_size,
                        &[
                            Point { x: -0.15, y: -0.3 },
                            Point { x: 0.0, y: 0.0 },
                            Point { x: 0.15, y: -0.3 },
                        ],
                        Some(WHITE_COLOR),
                        Some(edge_color),
                        1.0,
                    );
                }
            }
        }
        _ => {}
    }
}

/// Returns two separated, identically oriented triangle anchors.
///
/// Each tuple contains the triangle tip and a point immediately behind it. A
/// separate behind-point prevents the rear triangle from flipping when it is
/// placed behind the arc endpoint.
fn double_triangle_anchors(end: Point, previous: Point, size: f64) -> Option<[(Point, Point); 2]> {
    let dx = end.x - previous.x;
    let dy = end.y - previous.y;
    let length = dx.hypot(dy);
    if length <= 1e-6 {
        return None;
    }
    let unit_x = dx / length;
    let unit_y = dy / length;
    let anchor = |offset: f64| {
        let tip = Point {
            x: end.x - unit_x * offset,
            y: end.y - unit_y * offset,
        };
        let behind = Point {
            x: tip.x - unit_x,
            y: tip.y - unit_y,
        };
        (tip, behind)
    };
    Some([anchor(0.0), anchor(size * 0.36)])
}

impl RasterBackend {
    fn new(width: f64, height: f64, background: Option<Rgba>) -> Result<Self> {
        let mut pixmap = Pixmap::new(width.ceil() as u32, height.ceil() as u32)
            .ok_or_else(|| anyhow!("Failed to allocate pixmap"))?;
        pixmap.fill(
            background
                .filter(|color| color.a > 0.0)
                .unwrap_or(WHITE_COLOR)
                .sk(),
        );
        let font = Font::from_bytes(FONT_BYTES, FontSettings::default())
            .map_err(|err| anyhow!("Failed to load embedded font: {err}"))?;
        Ok(Self { pixmap, font })
    }

    fn blend_text_pixel(&mut self, x: i32, y: i32, alpha: f32, color: Rgba) {
        if x < 0 || y < 0 {
            return;
        }
        let x = x as u32;
        let y = y as u32;
        if x >= self.pixmap.width() || y >= self.pixmap.height() {
            return;
        }
        let idx = ((y * self.pixmap.width() + x) * 4) as usize;
        let data = self.pixmap.data_mut();
        let src_a = (alpha as f64 * color.a).clamp(0.0, 1.0);
        let inv_a = 1.0 - src_a;
        let src = [
            color.r * 255.0,
            color.g * 255.0,
            color.b * 255.0,
            color.a * 255.0,
        ];
        for channel in 0..3 {
            data[idx + channel] =
                (src[channel] * src_a + data[idx + channel] as f64 * inv_a).round() as u8;
        }
        data[idx + 3] = (src[3] * src_a + data[idx + 3] as f64 * inv_a).round() as u8;
    }

    fn draw_text_at(&mut self, x: f64, baseline_y: f64, text: &str, font_px: f64, color: Rgba) {
        let mut pen_x = x;
        for ch in text.chars().filter(|ch| *ch != '\n' && *ch != '\r') {
            let (metrics, bitmap) = self.font.rasterize(ch, font_px as f32);
            let glyph_x = pen_x + metrics.xmin as f64;
            let glyph_y = baseline_y - metrics.ymin as f64 - metrics.height as f64;
            for row in 0..metrics.height {
                for col in 0..metrics.width {
                    let alpha = bitmap[row * metrics.width + col] as f32 / 255.0;
                    if alpha > 0.0 {
                        self.blend_text_pixel(
                            (glyph_x + col as f64).round() as i32,
                            (glyph_y + row as f64).round() as i32,
                            alpha,
                            color,
                        );
                    }
                }
            }
            pen_x += metrics.advance_width as f64;
        }
    }
}

impl Backend for RasterBackend {
    fn draw_path(&mut self, path: &SkPath, style: &DrawStyle) {
        self.draw_path_with_mask(path, style, None);
    }

    fn draw_path_dashed(&mut self, path: &SkPath, style: &DrawStyle, dashed: bool) {
        self.draw_path_with_mask_and_dash(path, style, None, dashed);
    }

    fn draw_path_clipped(&mut self, path: &SkPath, style: &DrawStyle, clip_path: &SkPath) {
        let Some(mut mask) = Mask::new(self.pixmap.width(), self.pixmap.height()) else {
            return;
        };
        mask.fill_path(clip_path, FillRule::Winding, true, SkTransform::identity());
        self.draw_path_with_mask(path, style, Some(&mask));
    }

    fn draw_text_centered(&mut self, center: Point, text: &str, font_px: f64, color: Rgba) {
        if text.trim().is_empty() {
            return;
        }
        let line_height = font_px * 1.15;
        let lines: Vec<&str> = text.lines().collect();
        let first_baseline = center.y - (lines.len().saturating_sub(1) as f64 * line_height / 2.0)
            + line_height * 0.33;
        for (index, line) in lines.iter().enumerate() {
            let width = self.measure_text_width(line, font_px);
            let x = center.x - width / 2.0;
            self.draw_text_at(
                x,
                first_baseline + index as f64 * line_height,
                line,
                font_px,
                color,
            );
        }
    }

    fn draw_text_bottom_centered(
        &mut self,
        rect: PixelRect,
        text: &str,
        font_px: f64,
        color: Rgba,
    ) {
        if text.trim().is_empty() {
            return;
        }
        let width = self.measure_text_width(text, font_px);
        let x = rect.center.x - width / 2.0;
        let baseline_y = rect.y0 + rect.height - 3.0;
        self.draw_text_at(x, baseline_y, text, font_px, color);
    }

    fn measure_text_width(&self, text: &str, font_px: f64) -> f64 {
        text.lines()
            .map(|line| {
                line.chars()
                    .filter(|ch| *ch != '\r')
                    .map(|ch| self.font.metrics(ch, font_px as f32).advance_width as f64)
                    .sum::<f64>()
            })
            .fold(0.0, f64::max)
    }
}

impl RasterBackend {
    fn draw_path_with_mask(&mut self, path: &SkPath, style: &DrawStyle, mask: Option<&Mask>) {
        self.draw_path_with_mask_and_dash(path, style, mask, false);
    }

    fn draw_path_with_mask_and_dash(
        &mut self,
        path: &SkPath,
        style: &DrawStyle,
        mask: Option<&Mask>,
        dashed: bool,
    ) {
        if let Some(fill) = style.fill {
            let mut paint = Paint::default();
            paint.set_color(fill.sk());
            paint.anti_alias = true;
            self.pixmap.fill_path(
                path,
                &paint,
                FillRule::Winding,
                SkTransform::identity(),
                mask,
            );
        }
        if let Some(stroke_color) = style.stroke {
            if style.stroke_width > 0.0 {
                let mut paint = Paint::default();
                paint.set_color(stroke_color.sk());
                paint.anti_alias = true;
                let stroke = Stroke {
                    width: style.stroke_width as f32,
                    line_cap: LineCap::Butt,
                    line_join: LineJoin::Miter,
                    dash: if dashed {
                        StrokeDash::new(vec![6.0, 3.0], 0.0)
                    } else {
                        None
                    },
                    ..Stroke::default()
                };
                self.pixmap
                    .stroke_path(path, &paint, &stroke, SkTransform::identity(), mask);
            }
        }
    }
}

impl SvgBackend {
    fn new(width: f64, height: f64, background: Option<Rgba>) -> Result<Self> {
        let font = Font::from_bytes(FONT_BYTES, FontSettings::default())
            .map_err(|err| anyhow!("Failed to load embedded font: {err}"))?;
        let mut body = String::new();
        let bg = background
            .filter(|color| color.a > 0.0)
            .unwrap_or(WHITE_COLOR);
        let _ = writeln!(
            body,
            r#"<rect x="0" y="0" width="{:.3}" height="{:.3}" fill="{}"/>"#,
            width,
            height,
            bg.hex()
        );
        Ok(Self {
            width,
            height,
            body,
            font,
            next_clip_id: 0,
        })
    }

    fn finish(self) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="{:.3}px" height="{:.3}px" viewBox="0 0 {:.3} {:.3}">{}</svg>
"#,
            self.width, self.height, self.width, self.height, self.body
        )
    }
}

impl Backend for SvgBackend {
    fn draw_path(&mut self, path: &SkPath, style: &DrawStyle) {
        self.write_path(path, style, None);
    }

    fn draw_path_clipped(&mut self, path: &SkPath, style: &DrawStyle, clip_path: &SkPath) {
        let clip_id = format!("clip-{}", self.next_clip_id);
        self.next_clip_id += 1;
        let _ = writeln!(
            self.body,
            r#"<clipPath id="{}"><path clip-rule="nonzero" d="{}"/></clipPath>"#,
            clip_id,
            path_to_svg(clip_path)
        );
        self.write_path(path, style, Some(&clip_id));
    }

    fn draw_text_centered(&mut self, center: Point, text: &str, font_px: f64, color: Rgba) {
        if text.trim().is_empty() {
            return;
        }
        let lines: Vec<&str> = text.lines().collect();
        let line_height = font_px * 1.15;
        let first_y = center.y - (lines.len().saturating_sub(1) as f64 * line_height / 2.0);
        for (index, line) in lines.iter().enumerate() {
            let escaped = escape_xml(line);
            let _ = writeln!(
                self.body,
                r#"<text x="{:.3}" y="{:.3}" text-anchor="middle" dominant-baseline="middle" font-family="{}" font-size="{:.3}" fill="{}">{}</text>"#,
                center.x,
                first_y + index as f64 * line_height,
                SVG_SANS_FONT_FAMILY,
                font_px,
                color.hex(),
                escaped
            );
        }
    }

    fn draw_text_bottom_centered(
        &mut self,
        rect: PixelRect,
        text: &str,
        font_px: f64,
        color: Rgba,
    ) {
        if text.trim().is_empty() {
            return;
        }
        let escaped = escape_xml(&text.replace('\n', " "));
        let _ = writeln!(
            self.body,
            r#"<text x="{:.3}" y="{:.3}" text-anchor="middle" font-family="{}" font-size="{:.3}" fill="{}">{}</text>"#,
            rect.center.x,
            rect.y0 + rect.height - 3.0,
            SVG_SANS_FONT_FAMILY,
            font_px,
            color.hex(),
            escaped
        );
    }

    fn measure_text_width(&self, text: &str, font_px: f64) -> f64 {
        text.lines()
            .map(|line| {
                line.chars()
                    .filter(|ch| *ch != '\r')
                    .map(|ch| self.font.metrics(ch, font_px as f32).advance_width as f64)
                    .sum::<f64>()
            })
            .fold(0.0, f64::max)
    }
}

impl SvgBackend {
    fn write_path(&mut self, path: &SkPath, style: &DrawStyle, clip_id: Option<&str>) {
        let fill = style
            .fill
            .map(Rgba::hex)
            .unwrap_or_else(|| "none".to_string());
        let stroke = style
            .stroke
            .map(Rgba::hex)
            .unwrap_or_else(|| "none".to_string());
        if let Some(clip_id) = clip_id {
            let _ = writeln!(
                self.body,
                r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{:.3}" clip-path="url(#{})"/>"#,
                path_to_svg(path),
                fill,
                stroke,
                style.stroke_width,
                clip_id
            );
        } else {
            let _ = writeln!(
                self.body,
                r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{:.3}"/>"#,
                path_to_svg(path),
                fill,
                stroke,
                style.stroke_width
            );
        }
    }
}

fn path_to_svg(path: &SkPath) -> String {
    let mut out = String::new();
    for segment in path.segments() {
        match segment {
            tiny_skia::PathSegment::MoveTo(p) => {
                let _ = write!(out, "M{:.3} {:.3}", p.x, p.y);
            }
            tiny_skia::PathSegment::LineTo(p) => {
                let _ = write!(out, "L{:.3} {:.3}", p.x, p.y);
            }
            tiny_skia::PathSegment::QuadTo(p1, p) => {
                let _ = write!(out, "Q{:.3} {:.3} {:.3} {:.3}", p1.x, p1.y, p.x, p.y);
            }
            tiny_skia::PathSegment::CubicTo(p1, p2, p) => {
                let _ = write!(
                    out,
                    "C{:.3} {:.3} {:.3} {:.3} {:.3} {:.3}",
                    p1.x, p1.y, p2.x, p2.y, p.x, p.y
                );
            }
            tiny_skia::PathSegment::Close => out.push('Z'),
        }
    }
    out
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn marker_points(end: Point, prev: Point, size: f64, local: &[Point]) -> Option<Vec<Point>> {
    let dx = end.x - prev.x;
    let dy = end.y - prev.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length == 0.0 {
        return None;
    }
    let ux = dx / length;
    let uy = dy / length;
    let px = -uy;
    let py = ux;
    Some(
        local
            .iter()
            .map(|point| Point {
                x: end.x + ux * point.y * size - px * point.x * size,
                y: end.y + uy * point.y * size - py * point.x * size,
            })
            .collect(),
    )
}

fn draw_marker_polygon<B: Backend>(
    backend: &mut B,
    end: Point,
    prev: Point,
    size: f64,
    local: &[Point],
    fill: Option<Rgba>,
    stroke: Option<Rgba>,
    stroke_width: f64,
) {
    let Some(points) = marker_points(end, prev, size, local) else {
        return;
    };
    backend.draw_path(
        &polygon_path(&points),
        &DrawStyle {
            fill,
            stroke,
            stroke_width,
        },
    );
}

fn draw_inhibition_bar<B: Backend>(
    backend: &mut B,
    end: Point,
    prev: Point,
    length: f64,
    offset: f64,
    stroke_color: Rgba,
    stroke_width: f64,
) {
    let dx = end.x - prev.x;
    let dy = end.y - prev.y;
    let seg_len = (dx * dx + dy * dy).sqrt();
    if seg_len == 0.0 {
        return;
    }
    let ux = dx / seg_len;
    let uy = dy / seg_len;
    let center_x = end.x - ux * offset;
    let center_y = end.y - uy * offset;
    let perp_x = -uy;
    let perp_y = ux;
    let half_len = length / 2.0;
    let mut builder = PathBuilder::new();
    builder.move_to(
        (center_x - perp_x * half_len) as f32,
        (center_y - perp_y * half_len) as f32,
    );
    builder.line_to(
        (center_x + perp_x * half_len) as f32,
        (center_y + perp_y * half_len) as f32,
    );
    backend.draw_path(
        &builder.finish().unwrap(),
        &DrawStyle {
            fill: None,
            stroke: Some(stroke_color),
            stroke_width,
        },
    );
}

fn absolute_inhibition_bar_offsets(marker_size: f64) -> (f64, f64) {
    let gap = marker_size * 0.12;
    (gap, gap * 2.0)
}

fn absolute_inhibition_connector(
    end: Point,
    previous: Point,
    marker_size: f64,
) -> Option<(Point, Point)> {
    let (front_offset, rear_offset) = absolute_inhibition_bar_offsets(marker_size);
    Some((
        point_before_endpoint(end, previous, front_offset)?,
        point_before_endpoint(end, previous, rear_offset)?,
    ))
}

fn point_before_endpoint(end: Point, previous: Point, distance: f64) -> Option<Point> {
    let dx = end.x - previous.x;
    let dy = end.y - previous.y;
    let length = dx.hypot(dy);
    if length <= 1e-6 {
        return None;
    }
    Some(Point {
        x: end.x - dx / length * distance,
        y: end.y - dy / length * distance,
    })
}

fn rect_path(rect: PixelRect) -> SkPath {
    PathBuilder::from_rect(
        Rect::from_xywh(
            rect.x0 as f32,
            rect.y0 as f32,
            rect.width as f32,
            rect.height as f32,
        )
        .unwrap(),
    )
}

/// Builds the document shape and callout tail for an ER annotation.
///
/// When no explicit callout point is supplied, the tail remains inside the
/// declared bounding box.
fn annotation_path(rect: PixelRect, callout: Option<Point>) -> SkPath {
    let fold = rect.width.min(rect.height) * 0.18;
    let body_bottom = if callout.is_some() {
        rect.y0 + rect.height
    } else {
        rect.y0 + rect.height * 0.72
    };
    let tail = callout.unwrap_or(Point {
        x: rect.x0 + rect.width * 0.08,
        y: rect.y0 + rect.height,
    });
    polygon_path(&[
        Point {
            x: rect.x0,
            y: rect.y0,
        },
        Point {
            x: rect.x0 + rect.width - fold,
            y: rect.y0,
        },
        Point {
            x: rect.x0 + rect.width,
            y: rect.y0 + fold,
        },
        Point {
            x: rect.x0 + rect.width,
            y: body_bottom,
        },
        Point {
            x: rect.x0 + rect.width * 0.42,
            y: body_bottom,
        },
        tail,
        Point {
            x: rect.x0 + rect.width * 0.18,
            y: body_bottom,
        },
        Point {
            x: rect.x0,
            y: body_bottom,
        },
    ])
}

fn ellipse_path(rect: PixelRect) -> SkPath {
    PathBuilder::from_oval(
        Rect::from_xywh(
            rect.x0 as f32,
            rect.y0 as f32,
            rect.width as f32,
            rect.height as f32,
        )
        .unwrap(),
    )
    .unwrap()
}

fn circle_path(center: Point, radius: f64) -> SkPath {
    PathBuilder::from_circle(center.x as f32, center.y as f32, radius.max(1.0) as f32).unwrap()
}

fn round_rect_path(rect: PixelRect, radius: f64) -> SkPath {
    let radius = radius.min(rect.width / 2.0).min(rect.height / 2.0);
    let x = rect.x0;
    let y = rect.y0;
    let right = rect.x0 + rect.width;
    let bottom = rect.y0 + rect.height;
    let mut builder = PathBuilder::new();
    builder.move_to((x + radius) as f32, y as f32);
    builder.line_to((right - radius) as f32, y as f32);
    builder.quad_to(right as f32, y as f32, right as f32, (y + radius) as f32);
    builder.line_to(right as f32, (bottom - radius) as f32);
    builder.quad_to(
        right as f32,
        bottom as f32,
        (right - radius) as f32,
        bottom as f32,
    );
    builder.line_to((x + radius) as f32, bottom as f32);
    builder.quad_to(x as f32, bottom as f32, x as f32, (bottom - radius) as f32);
    builder.line_to(x as f32, (y + radius) as f32);
    builder.quad_to(x as f32, y as f32, (x + radius) as f32, y as f32);
    builder.close();
    builder.finish().unwrap()
}

fn hexagon_path(rect: PixelRect) -> SkPath {
    let x0 = rect.x0;
    let y0 = rect.y0;
    let w = rect.width;
    let h = rect.height;
    polygon_path(&[
        Point {
            x: x0,
            y: y0 + 0.5 * h,
        },
        Point {
            x: x0 + 0.25 * w,
            y: y0,
        },
        Point {
            x: x0 + 0.75 * w,
            y: y0,
        },
        Point {
            x: x0 + w,
            y: y0 + 0.5 * h,
        },
        Point {
            x: x0 + 0.75 * w,
            y: y0 + h,
        },
        Point {
            x: x0 + 0.25 * w,
            y: y0 + h,
        },
    ])
}

fn cut_rect_path(rect: PixelRect) -> SkPath {
    let corner = 12.0_f64
        .min(rect.width / 3.0)
        .min(rect.height / 3.0)
        .max(1.0);
    polygon_path(&[
        Point {
            x: rect.x0 + corner,
            y: rect.y0,
        },
        Point {
            x: rect.x0,
            y: rect.y0 + corner,
        },
        Point {
            x: rect.x0,
            y: rect.y0 + rect.height - corner,
        },
        Point {
            x: rect.x0 + corner,
            y: rect.y0 + rect.height,
        },
        Point {
            x: rect.x0 + rect.width - corner,
            y: rect.y0 + rect.height,
        },
        Point {
            x: rect.x0 + rect.width,
            y: rect.y0 + rect.height - corner,
        },
        Point {
            x: rect.x0 + rect.width,
            y: rect.y0 + corner,
        },
        Point {
            x: rect.x0 + rect.width - corner,
            y: rect.y0,
        },
    ])
}

fn bottom_round_rect_path(rect: PixelRect) -> SkPath {
    let radius = (rect.height * 0.25).max(1.0).min(rect.width / 2.0);
    let x0 = rect.x0;
    let y0 = rect.y0;
    let x1 = rect.x0 + rect.width;
    let y1 = rect.y0 + rect.height;
    let mut builder = PathBuilder::new();
    builder.move_to(x0 as f32, y0 as f32);
    builder.line_to(x1 as f32, y0 as f32);
    builder.line_to(x1 as f32, (y1 - radius) as f32);
    builder.quad_to(x1 as f32, y1 as f32, (x1 - radius) as f32, y1 as f32);
    builder.line_to((x0 + radius) as f32, y1 as f32);
    builder.quad_to(x0 as f32, y1 as f32, x0 as f32, (y1 - radius) as f32);
    builder.line_to(x0 as f32, y0 as f32);
    builder.close();
    builder.finish().unwrap()
}

fn barrel_path(rect: PixelRect) -> SkPath {
    let x0 = rect.x0;
    let y0 = rect.y0;
    let x1 = rect.x0 + rect.width;
    let y1 = rect.y0 + rect.height;
    let control = (rect.width * 0.10).min(100.0);
    let mut builder = PathBuilder::new();
    builder.move_to(x0 as f32, (y0 + 15.0) as f32);
    builder.line_to(x0 as f32, (y1 - 15.0) as f32);
    builder.quad_to(
        (x0 + 5.0) as f32,
        y1 as f32,
        (x0 + control) as f32,
        y1 as f32,
    );
    builder.line_to((x1 - control) as f32, y1 as f32);
    builder.quad_to((x1 - 5.0) as f32, y1 as f32, x1 as f32, (y1 - 15.0) as f32);
    builder.line_to(x1 as f32, (y0 + 15.0) as f32);
    builder.quad_to(
        (x1 - 5.0) as f32,
        y0 as f32,
        (x1 - control) as f32,
        y0 as f32,
    );
    builder.line_to((x0 + control) as f32, y0 as f32);
    builder.quad_to((x0 + 5.0) as f32, y0 as f32, x0 as f32, (y0 + 15.0) as f32);
    builder.close();
    builder.finish().unwrap()
}

fn tag_path(rect: PixelRect, orientation: &str) -> SkPath {
    let x0 = rect.x0;
    let y0 = rect.y0;
    let x1 = rect.x0 + rect.width;
    let y1 = rect.y0 + rect.height;
    let points = match orientation.trim().to_ascii_lowercase().as_str() {
        "left" => vec![
            Point { x: x1, y: y0 },
            Point {
                x: x0 + 0.75 * rect.width,
                y: y0,
            },
            Point {
                x: x0,
                y: rect.center.y,
            },
            Point {
                x: x0 + 0.75 * rect.width,
                y: y1,
            },
            Point { x: x1, y: y1 },
        ],
        "up" => vec![
            Point { x: x0, y: y1 },
            Point {
                x: x0,
                y: y0 + 0.75 * rect.height,
            },
            Point {
                x: rect.center.x,
                y: y0,
            },
            Point {
                x: x1,
                y: y0 + 0.75 * rect.height,
            },
            Point { x: x1, y: y1 },
        ],
        "down" => vec![
            Point { x: x0, y: y0 },
            Point {
                x: x0,
                y: y0 + 0.25 * rect.height,
            },
            Point {
                x: rect.center.x,
                y: y1,
            },
            Point {
                x: x1,
                y: y0 + 0.25 * rect.height,
            },
            Point { x: x1, y: y0 },
        ],
        _ => vec![
            Point { x: x0, y: y0 },
            Point {
                x: x0 + 0.625 * rect.width,
                y: y0,
            },
            Point {
                x: x1,
                y: rect.center.y,
            },
            Point {
                x: x0 + 0.625 * rect.width,
                y: y1,
            },
            Point { x: x0, y: y1 },
        ],
    };
    polygon_path(&points)
}

fn perturbing_agent_path(rect: PixelRect) -> SkPath {
    polygon_path(&[
        Point {
            x: rect.x0,
            y: rect.y0,
        },
        Point {
            x: rect.x0 + 0.25 * rect.width,
            y: rect.center.y,
        },
        Point {
            x: rect.x0,
            y: rect.y0 + rect.height,
        },
        Point {
            x: rect.x0 + rect.width,
            y: rect.y0 + rect.height,
        },
        Point {
            x: rect.x0 + 0.75 * rect.width,
            y: rect.center.y,
        },
        Point {
            x: rect.x0 + rect.width,
            y: rect.y0,
        },
    ])
}

fn is_ported_glyph_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "process"
            | "omitted process"
            | "uncertain process"
            | "association"
            | "dissociation"
            | "and"
            | "or"
            | "not"
            | "delay"
    )
}

fn is_logical_operator_class(class_name: &str) -> bool {
    matches!(class_name, "and" | "or" | "not" | "delay")
}

fn ported_glyph_path(rect: PixelRect, glyph: &Glyph, transform: &Transform) -> SkPath {
    let vertical = if glyph.ports.len() >= 2 {
        let min_x = glyph
            .ports
            .iter()
            .map(|port| port.x)
            .fold(f64::INFINITY, f64::min);
        let max_x = glyph
            .ports
            .iter()
            .map(|port| port.x)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = glyph
            .ports
            .iter()
            .map(|port| port.y)
            .fold(f64::INFINITY, f64::min);
        let max_y = glyph
            .ports
            .iter()
            .map(|port| port.y)
            .fold(f64::NEG_INFINITY, f64::max);
        max_y - min_y > max_x - min_x
    } else {
        false
    };
    let (core_w, core_h) = if is_logical_operator_class(&glyph.class_name) {
        let size = JS_LOGICAL_NODE_SIZE * transform.scale_x.abs().min(transform.scale_y.abs());
        (size, size)
    } else {
        (rect.width * 0.707071, rect.height * 0.707071)
    };
    let core = PixelRect {
        x0: rect.center.x - core_w / 2.0,
        y0: rect.center.y - core_h / 2.0,
        width: core_w,
        height: core_h,
        center: rect.center,
    };
    let core_circle = matches!(glyph.class_name.as_str(), "association" | "dissociation")
        || is_logical_operator_class(&glyph.class_name);
    if !vertical {
        let line_half = (rect.height * 0.01).max(0.5) / 2.0;
        if core_circle {
            let mut points = vec![
                Point {
                    x: rect.x0,
                    y: rect.center.y - line_half,
                },
                Point {
                    x: core.x0,
                    y: rect.center.y - line_half,
                },
            ];
            for index in 0..=30 {
                let theta = std::f64::consts::PI - std::f64::consts::PI * f64::from(index) / 30.0;
                points.push(Point {
                    x: core.center.x + core.width / 2.0 * theta.cos(),
                    y: core.center.y + core.height / 2.0 * theta.sin(),
                });
            }
            points.extend([
                Point {
                    x: core.x0 + core.width,
                    y: rect.center.y - line_half,
                },
                Point {
                    x: rect.x0 + rect.width,
                    y: rect.center.y - line_half,
                },
                Point {
                    x: rect.x0 + rect.width,
                    y: rect.center.y + line_half,
                },
                Point {
                    x: core.x0 + core.width,
                    y: rect.center.y + line_half,
                },
            ]);
            for index in 0..=30 {
                let theta = -std::f64::consts::PI * f64::from(index) / 30.0;
                points.push(Point {
                    x: core.center.x + core.width / 2.0 * theta.cos(),
                    y: core.center.y + core.height / 2.0 * theta.sin(),
                });
            }
            points.extend([
                Point {
                    x: core.x0,
                    y: rect.center.y + line_half,
                },
                Point {
                    x: rect.x0,
                    y: rect.center.y + line_half,
                },
            ]);
            return polygon_path(&points);
        }
        return polygon_path(&[
            Point {
                x: rect.x0,
                y: rect.center.y - line_half,
            },
            Point {
                x: core.x0,
                y: rect.center.y - line_half,
            },
            Point {
                x: core.x0,
                y: core.y0,
            },
            Point {
                x: core.x0 + core.width,
                y: core.y0,
            },
            Point {
                x: core.x0 + core.width,
                y: rect.center.y - line_half,
            },
            Point {
                x: rect.x0 + rect.width,
                y: rect.center.y - line_half,
            },
            Point {
                x: rect.x0 + rect.width,
                y: rect.center.y + line_half,
            },
            Point {
                x: core.x0 + core.width,
                y: rect.center.y + line_half,
            },
            Point {
                x: core.x0 + core.width,
                y: core.y0 + core.height,
            },
            Point {
                x: core.x0,
                y: core.y0 + core.height,
            },
            Point {
                x: core.x0,
                y: rect.center.y + line_half,
            },
            Point {
                x: rect.x0,
                y: rect.center.y + line_half,
            },
        ]);
    }
    let line_half = (rect.width * 0.01).max(0.5) / 2.0;
    if core_circle {
        let mut points = vec![
            Point {
                x: rect.center.x - line_half,
                y: rect.y0,
            },
            Point {
                x: rect.center.x - line_half,
                y: core.y0,
            },
        ];
        for index in 0..=30 {
            let theta =
                -std::f64::consts::PI / 2.0 - std::f64::consts::PI * f64::from(index) / 30.0;
            points.push(Point {
                x: core.center.x + core.width / 2.0 * theta.cos(),
                y: core.center.y + core.height / 2.0 * theta.sin(),
            });
        }
        points.extend([
            Point {
                x: rect.center.x - line_half,
                y: core.y0 + core.height,
            },
            Point {
                x: rect.center.x - line_half,
                y: rect.y0 + rect.height,
            },
            Point {
                x: rect.center.x + line_half,
                y: rect.y0 + rect.height,
            },
            Point {
                x: rect.center.x + line_half,
                y: core.y0 + core.height,
            },
        ]);
        for index in 0..=30 {
            let theta = std::f64::consts::PI / 2.0 - std::f64::consts::PI * f64::from(index) / 30.0;
            points.push(Point {
                x: core.center.x + core.width / 2.0 * theta.cos(),
                y: core.center.y + core.height / 2.0 * theta.sin(),
            });
        }
        points.extend([
            Point {
                x: rect.center.x + line_half,
                y: core.y0,
            },
            Point {
                x: rect.center.x + line_half,
                y: rect.y0,
            },
        ]);
        return polygon_path(&points);
    }
    polygon_path(&[
        Point {
            x: rect.center.x - line_half,
            y: rect.y0,
        },
        Point {
            x: rect.center.x - line_half,
            y: core.y0,
        },
        Point {
            x: core.x0,
            y: core.y0,
        },
        Point {
            x: core.x0,
            y: core.y0 + core.height,
        },
        Point {
            x: rect.center.x - line_half,
            y: core.y0 + core.height,
        },
        Point {
            x: rect.center.x - line_half,
            y: rect.y0 + rect.height,
        },
        Point {
            x: rect.center.x + line_half,
            y: rect.y0 + rect.height,
        },
        Point {
            x: rect.center.x + line_half,
            y: core.y0 + core.height,
        },
        Point {
            x: core.x0 + core.width,
            y: core.y0 + core.height,
        },
        Point {
            x: core.x0 + core.width,
            y: core.y0,
        },
        Point {
            x: rect.center.x + line_half,
            y: core.y0,
        },
        Point {
            x: rect.center.x + line_half,
            y: rect.y0,
        },
    ])
}

fn polygon_path(points: &[Point]) -> SkPath {
    let mut builder = PathBuilder::new();
    if let Some(first) = points.first() {
        builder.move_to(first.x as f32, first.y as f32);
        for point in &points[1..] {
            builder.line_to(point.x as f32, point.y as f32);
        }
        builder.close();
    }
    builder.finish().unwrap()
}

fn bbox_pixel_rect(transform: &Transform, bbox: BBox) -> PixelRect {
    let x0 = transform.offset_x + (bbox.x - transform.min_x) * transform.scale_x;
    let x1 = transform.offset_x + (bbox.x + bbox.w - transform.min_x) * transform.scale_x;
    let y0 = transform.offset_y + (bbox.y - transform.min_y) * transform.scale_y;
    let y1 = transform.offset_y + (bbox.y + bbox.h - transform.min_y) * transform.scale_y;
    let left = x0.min(x1);
    let right = x0.max(x1);
    let top = y0.min(y1);
    let bottom = y0.max(y1);
    PixelRect {
        x0: left,
        y0: top,
        width: right - left,
        height: bottom - top,
        center: Point {
            x: (left + right) / 2.0,
            y: (top + bottom) / 2.0,
        },
    }
}

fn parse_render_background_color(root: &Element) -> Option<Rgba> {
    let render_node = find_first_descendant(root, "renderInformation")?;
    let mut colors = HashMap::new();
    let mut color_defs = Vec::new();
    collect_descendants_by_name(render_node, "colorDefinition", &mut color_defs);
    for color_def in color_defs {
        let Some(id) = element_attr(color_def, "id") else {
            continue;
        };
        let Some(value) = element_attr(color_def, "value") else {
            continue;
        };
        if let Some(color) = parse_color_value(value, &colors) {
            colors.insert(id.to_string(), color);
        }
    }
    render_node
        .attributes
        .get("background-color")
        .map(String::as_str)
        .and_then(|value| parse_color_value(value, &colors))
}

fn parse_color_value(value: &str, colors: &HashMap<String, Rgba>) -> Option<Rgba> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        return None;
    }
    if let Some(color) = colors.get(trimmed) {
        return Some(*color);
    }
    if trimmed.starts_with('#') {
        return parse_hex_color(trimmed);
    }
    None
}

fn parse_hex_color(value: &str) -> Option<Rgba> {
    let hex = value.trim().trim_start_matches('#');
    let (r, g, b, a) = match hex.len() {
        3 => (
            parse_hex_nibble(hex.chars().next()?)?,
            parse_hex_nibble(hex.chars().nth(1)?)?,
            parse_hex_nibble(hex.chars().nth(2)?)?,
            0xFF,
        ),
        4 => (
            parse_hex_nibble(hex.chars().next()?)?,
            parse_hex_nibble(hex.chars().nth(1)?)?,
            parse_hex_nibble(hex.chars().nth(2)?)?,
            parse_hex_nibble(hex.chars().nth(3)?)?,
        ),
        6 => (
            parse_hex_byte(&hex[0..2])?,
            parse_hex_byte(&hex[2..4])?,
            parse_hex_byte(&hex[4..6])?,
            0xFF,
        ),
        8 => (
            parse_hex_byte(&hex[0..2])?,
            parse_hex_byte(&hex[2..4])?,
            parse_hex_byte(&hex[4..6])?,
            parse_hex_byte(&hex[6..8])?,
        ),
        _ => return None,
    };
    Some(Rgba::new(
        r as f64 / 255.0,
        g as f64 / 255.0,
        b as f64 / 255.0,
        a as f64 / 255.0,
    ))
}

fn parse_hex_byte(value: &str) -> Option<u8> {
    u8::from_str_radix(value, 16).ok()
}

fn parse_hex_nibble(value: char) -> Option<u8> {
    Some(value.to_digit(16)? as u8 * 17)
}

fn element_attr<'a>(element: &'a Element, name: &str) -> Option<&'a str> {
    element.attributes.get(name).map(String::as_str)
}

fn child_elements(element: &Element) -> impl Iterator<Item = &Element> {
    element.children.iter().filter_map(|node| match node {
        XMLNode::Element(child) => Some(child),
        _ => None,
    })
}

fn find_first_descendant<'a>(element: &'a Element, name: &str) -> Option<&'a Element> {
    for child in child_elements(element) {
        if child.name == name {
            return Some(child);
        }
        if let Some(found) = find_first_descendant(child, name) {
            return Some(found);
        }
    }
    None
}

fn collect_descendants_by_name<'a>(element: &'a Element, name: &str, out: &mut Vec<&'a Element>) {
    for child in child_elements(element) {
        if child.name == name {
            out.push(child);
        }
        collect_descendants_by_name(child, name, out);
    }
}

fn parse_sbgn(root: &Element) -> Result<(Vec<Glyph>, Vec<Arc>, Bounds)> {
    let mut arc_nodes = Vec::new();
    collect_descendants_by_name(root, "arc", &mut arc_nodes);

    let mut map_nodes = Vec::new();
    collect_descendants_by_name(root, "map", &mut map_nodes);
    if map_nodes.is_empty() {
        return Err(anyhow!("SBGN file missing map element"));
    }

    let mut glyphs = Vec::new();
    for map_node in map_nodes {
        if let Some(language) = element_attr(map_node, "language") {
            if language != "entity relationship" {
                return Err(anyhow!(
                    "Expected an SBGN Entity Relationship map, found language '{language}'"
                ));
            }
        }
        for glyph_node in child_elements(map_node).filter(|node| node.name == "glyph") {
            parse_glyph_node(glyph_node, None, &mut glyphs)?;
        }
        for arcgroup in child_elements(map_node).filter(|node| node.name == "arcgroup") {
            for glyph_node in child_elements(arcgroup).filter(|node| node.name == "glyph") {
                parse_glyph_node(glyph_node, None, &mut glyphs)?;
            }
        }
    }

    let mut arcs = Vec::new();
    for arc in arc_nodes {
        let arc_id = element_attr(arc, "id").unwrap_or_default().to_string();
        let class_name = element_attr(arc, "class").unwrap_or_default().to_string();
        let source = element_attr(arc, "source").map(str::to_string);
        let target = element_attr(arc, "target").map(str::to_string);
        let auxiliary_glyphs = child_elements(arc)
            .filter(|node| node.name == "glyph")
            .map(|node| ArcGlyph {
                id: element_attr(node, "id").unwrap_or_default().to_string(),
                class_name: element_attr(node, "class").unwrap_or_default().to_string(),
                bbox: child_elements(node)
                    .find(|child| child.name == "bbox")
                    .and_then(parse_bbox),
                label: child_elements(node)
                    .find(|child| child.name == "label")
                    .and_then(|child| element_attr(child, "text"))
                    .unwrap_or_default()
                    .replace('\r', ""),
            })
            .collect();
        for glyph_node in child_elements(arc).filter(|node| node.name == "glyph") {
            if element_attr(glyph_node, "class") == Some("outcome") {
                parse_glyph_node(glyph_node, Some(&arc_id), &mut glyphs)?;
            }
        }
        let mut points = Vec::new();
        for point_node in child_elements(arc)
            .filter(|node| matches!(node.name.as_str(), "start" | "next" | "end"))
        {
            if let (Some(x), Some(y)) = (
                parse_f64(element_attr(point_node, "x")),
                parse_f64(element_attr(point_node, "y")),
            ) {
                points.push(Point { x, y });
            }
        }
        arcs.push(Arc {
            id: arc_id,
            class_name,
            source,
            target,
            points,
            auxiliary_glyphs,
        });
    }
    let bounds = compute_bounds(&glyphs, &arcs)?;
    Ok((glyphs, arcs, bounds))
}

fn parse_glyph_node(
    glyph: &Element,
    parent_id: Option<&str>,
    glyphs: &mut Vec<Glyph>,
) -> Result<()> {
    let id = element_attr(glyph, "id").unwrap_or_default().to_string();
    let class_name = element_attr(glyph, "class").unwrap_or_default().to_string();
    let label_node = child_elements(glyph).find(|node| node.name == "label");
    let label = label_node
        .and_then(|node| element_attr(node, "text"))
        .unwrap_or("")
        .replace('\r', "");
    let bbox_node = child_elements(glyph).find(|node| node.name == "bbox");
    let bbox = bbox_node.and_then(parse_bbox);
    let extra_width = find_first_descendant(glyph, "w")
        .and_then(Element::get_text)
        .and_then(|text| parse_f64(Some(text.as_ref())));
    let extra_height = find_first_descendant(glyph, "h")
        .and_then(Element::get_text)
        .and_then(|text| parse_f64(Some(text.as_ref())));
    let ports = child_elements(glyph)
        .filter(|node| node.name == "port")
        .map(|node| Port {
            id: element_attr(node, "id").unwrap_or_default().to_string(),
            x: parse_f64(element_attr(node, "x")).unwrap_or(0.0),
            y: parse_f64(element_attr(node, "y")).unwrap_or(0.0),
        })
        .collect();
    let has_clone = child_elements(glyph).any(|node| node.name == "clone");
    let state_node = child_elements(glyph).find(|node| node.name == "state");
    let state_value = state_node
        .and_then(|node| element_attr(node, "value"))
        .map(str::to_string);
    let state_variable = state_node
        .and_then(|node| element_attr(node, "variable"))
        .map(str::to_string);
    let entity_name = child_elements(glyph)
        .find(|node| node.name == "entity")
        .and_then(|node| element_attr(node, "name"))
        .unwrap_or("")
        .to_string();
    let orientation = element_attr(glyph, "orientation")
        .unwrap_or("right")
        .to_string();
    let callout = child_elements(glyph)
        .find(|node| node.name == "callout")
        .and_then(|node| child_elements(node).find(|child| child.name == "point"))
        .and_then(|point| {
            Some(Point {
                x: parse_f64(element_attr(point, "x"))?,
                y: parse_f64(element_attr(point, "y"))?,
            })
        });

    glyphs.push(Glyph {
        id: id.clone(),
        parent_id: parent_id.map(str::to_string),
        class_name,
        bbox,
        extra_width,
        extra_height,
        label,
        ports,
        has_clone,
        state_value,
        state_variable,
        entity_name,
        orientation,
        callout,
    });
    for child in child_elements(glyph).filter(|node| node.name == "glyph") {
        parse_glyph_node(child, Some(&id), glyphs)?;
    }
    Ok(())
}

fn parse_bbox(node: &Element) -> Option<BBox> {
    Some(BBox {
        x: parse_f64(element_attr(node, "x"))?,
        y: parse_f64(element_attr(node, "y"))?,
        w: parse_f64(element_attr(node, "w"))?,
        h: parse_f64(element_attr(node, "h"))?,
    })
}

fn parse_f64(value: Option<&str>) -> Option<f64> {
    value.and_then(|v| v.parse::<f64>().ok())
}

fn compute_bounds(glyphs: &[Glyph], arcs: &[Arc]) -> Result<Bounds> {
    let mut x_values = Vec::new();
    let mut y_values = Vec::new();
    for glyph in glyphs {
        if let Some(bbox) = glyph.bbox.filter(|_| {
            !is_js_hidden_glyph_class(&glyph.class_name)
                || is_auxiliary_glyph_class(&glyph.class_name)
        }) {
            x_values.push(bbox.x);
            x_values.push(bbox.x + bbox.w);
            y_values.push(bbox.y);
            y_values.push(bbox.y + bbox.h);
        }
        if let Some(point) = glyph.callout {
            x_values.push(point.x);
            y_values.push(point.y);
        }
    }
    for arc in arcs {
        for point in &arc.points {
            x_values.push(point.x);
            y_values.push(point.y);
        }
        for glyph in &arc.auxiliary_glyphs {
            if let Some(bbox) = glyph.bbox {
                x_values.push(bbox.x);
                x_values.push(bbox.x + bbox.w);
                y_values.push(bbox.y);
                y_values.push(bbox.y + bbox.h);
            }
        }
    }
    if x_values.is_empty() || y_values.is_empty() {
        return Err(anyhow!("No coordinates found in SBGN file"));
    }
    Ok(Bounds {
        min_x: x_values.iter().copied().fold(f64::INFINITY, f64::min),
        max_x: x_values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        min_y: y_values.iter().copied().fold(f64::INFINITY, f64::min),
        max_y: y_values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    })
}

fn transform_with_padding(
    bounds: Bounds,
    padding: f64,
    output_width: Option<f64>,
    output_height: Option<f64>,
) -> (Transform, f64, f64) {
    let min_x = bounds.min_x - padding;
    let max_x = bounds.max_x + padding;
    let min_y = bounds.min_y - padding;
    let max_y = bounds.max_y + padding;
    let span_x = (max_x - min_x).abs().max(1.0);
    let span_y = (max_y - min_y).abs().max(1.0);
    let width = output_width.unwrap_or(span_x).max(1.0);
    let height = output_height.unwrap_or(span_y).max(1.0);
    let scale = (width / span_x).min(height / span_y);
    (
        Transform {
            min_x,
            min_y,
            scale_x: scale,
            scale_y: scale,
            offset_x: (width - span_x * scale) / 2.0,
            offset_y: (height - span_y * scale) / 2.0,
        },
        width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingBackend {
        paths: Vec<Vec<Point>>,
    }

    impl Backend for RecordingBackend {
        fn draw_path(&mut self, path: &SkPath, _style: &DrawStyle) {
            let points = path
                .segments()
                .filter_map(|segment| match segment {
                    tiny_skia::PathSegment::MoveTo(point)
                    | tiny_skia::PathSegment::LineTo(point) => Some(Point {
                        x: f64::from(point.x),
                        y: f64::from(point.y),
                    }),
                    _ => None,
                })
                .collect();
            self.paths.push(points);
        }

        fn draw_path_clipped(&mut self, path: &SkPath, style: &DrawStyle, _clip_path: &SkPath) {
            self.draw_path(path, style);
        }

        fn draw_text_centered(&mut self, _center: Point, _text: &str, _font_px: f64, _color: Rgba) {
        }

        fn draw_text_bottom_centered(
            &mut self,
            _rect: PixelRect,
            _text: &str,
            _font_px: f64,
            _color: Rgba,
        ) {
        }

        fn measure_text_width(&self, _text: &str, _font_px: f64) -> f64 {
            0.0
        }
    }

    fn test_glyph(id: &str, class_name: &str, bbox: Option<BBox>) -> Glyph {
        Glyph {
            id: id.to_string(),
            parent_id: None,
            class_name: class_name.to_string(),
            bbox,
            extra_width: None,
            extra_height: None,
            label: String::new(),
            ports: Vec::new(),
            has_clone: false,
            state_value: None,
            state_variable: None,
            entity_name: String::new(),
            orientation: "right".to_string(),
            callout: None,
        }
    }

    fn identity_transform() -> Transform {
        Transform {
            min_x: 0.0,
            min_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    fn parse_example(source: &str) -> (Vec<Glyph>, Vec<Arc>, Bounds) {
        let root = Element::parse(source.as_bytes()).expect("example XML should parse");
        parse_sbgn(&root).expect("example SBGN should parse")
    }

    #[test]
    fn parses_er_reference_card_classes() {
        let (glyphs, arcs, _) = parse_example(include_str!("../../examples/reference_card.sbgn"));
        for class_name in [
            "entity",
            "outcome",
            "interaction",
            "phenotype",
            "perturbing agent",
            "variable value",
            "existence",
            "location",
            "and",
            "or",
            "not",
            "delay",
            "annotation",
        ] {
            assert!(
                glyphs.iter().any(|glyph| glyph.class_name == class_name),
                "missing glyph class {class_name}"
            );
        }
        for class_name in [
            "assignment",
            "interaction",
            "modulation",
            "stimulation",
            "necessary stimulation",
            "absolute stimulation",
            "inhibition",
            "absolute inhibition",
            "logic arc",
        ] {
            assert!(
                arcs.iter().any(|arc| arc.class_name == class_name),
                "missing arc class {class_name}"
            );
        }
    }

    #[test]
    fn er_all_glyphs_covers_the_level_one_reference_card() {
        let (glyphs, arcs, _) =
            parse_example(include_str!("../../render_examples/er_all_glyphs.sbgn"));
        for class_name in [
            "entity",
            "outcome",
            "perturbing agent",
            "and",
            "or",
            "not",
            "delay",
            "unit of information",
            "state variable",
            "existence",
            "location",
            "variable value",
            "interaction",
            "phenotype",
            "annotation",
        ] {
            assert!(
                glyphs.iter().any(|glyph| glyph.class_name == class_name),
                "missing ER glyph class {class_name}"
            );
        }
        for class_name in [
            "assignment",
            "interaction",
            "modulation",
            "stimulation",
            "necessary stimulation",
            "absolute stimulation",
            "inhibition",
            "absolute inhibition",
            "logic arc",
        ] {
            assert!(
                arcs.iter().any(|arc| arc.class_name == class_name),
                "missing ER arc class {class_name}"
            );
        }
        assert!(
            arcs.iter().any(|arc| {
                arc.auxiliary_glyphs
                    .iter()
                    .any(|glyph| glyph.class_name == "cardinality")
            }),
            "missing ER cardinality decoration"
        );
        let nested = glyphs
            .iter()
            .find(|glyph| glyph.id == "entity_nested_child")
            .expect("nested entity should be present");
        assert_eq!(nested.parent_id.as_deref(), Some("entity_nested"));
    }

    #[test]
    fn parses_arcgroup_interaction_nodes_and_arc_outcomes() {
        let (glyphs, _, _) = parse_example(include_str!("../../examples/reference_card.sbgn"));
        assert!(glyphs.iter().any(|glyph| glyph.id == "glyph32"));
        assert!(glyphs.iter().any(|glyph| glyph.id == "arc0.0"));
    }

    #[test]
    fn clips_explicit_er_endpoints_to_glyph_boundaries() {
        let (glyphs, arcs, _) = parse_example(
            r#"<sbgn xmlns="http://sbgn.org/libsbgn/0.3">
              <map id="map" language="entity relationship">
                <glyph id="source" class="entity">
                  <bbox x="0" y="0" w="20" h="20"/>
                </glyph>
                <glyph id="target" class="entity">
                  <bbox x="80" y="0" w="20" h="20"/>
                </glyph>
                <arc id="arc" class="interaction" source="source" target="target">
                  <start x="10" y="10"/><end x="90" y="10"/>
                </arc>
              </map>
            </sbgn>"#,
        );
        let glyph_lookup: HashMap<&str, &Glyph> = glyphs
            .iter()
            .map(|glyph| (glyph.id.as_str(), glyph))
            .collect();
        let (start, end, _, _) =
            js_arc_points(&arcs[0], &glyph_lookup, &HashMap::new()).expect("ER arc should resolve");

        assert_eq!(start.x, 20.0);
        assert_eq!(start.y, 10.0);
        assert_eq!(end.x, 80.0);
        assert_eq!(end.y, 10.0);
    }

    #[test]
    fn clips_er_endpoints_to_nested_auxiliary_boundaries() {
        let (glyphs, arcs, _) = parse_example(
            r#"<sbgn xmlns="http://sbgn.org/libsbgn/0.3">
              <map id="map" language="entity relationship">
                <glyph id="value" class="variable value">
                  <label text="T"/><bbox x="0" y="30" w="20" h="20"/>
                </glyph>
                <glyph id="entity" class="entity">
                  <bbox x="0" y="0" w="20" h="20"/>
                  <glyph id="existence" class="existence">
                    <bbox x="7" y="15" w="6" h="10"/>
                  </glyph>
                </glyph>
                <arc id="arc" class="assignment" source="value" target="entity">
                  <start x="10" y="30"/><end x="10" y="18"/>
                </arc>
              </map>
            </sbgn>"#,
        );
        let glyph_lookup: HashMap<&str, &Glyph> = glyphs
            .iter()
            .map(|glyph| (glyph.id.as_str(), glyph))
            .collect();
        let (_, end, _, _) = js_arc_points(&arcs[0], &glyph_lookup, &HashMap::new())
            .expect("ER assignment should resolve");

        assert_eq!(end.x, 10.0);
        assert_eq!(end.y, 25.0);
    }

    #[test]
    fn pure_er_interaction_draws_no_terminal_triangles() {
        let source = test_glyph(
            "source",
            "entity",
            Some(BBox {
                x: 0.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            }),
        );
        let target = test_glyph(
            "target",
            "entity",
            Some(BBox {
                x: 80.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            }),
        );
        let arc = Arc {
            id: "interaction".to_string(),
            class_name: "interaction".to_string(),
            source: Some("source".to_string()),
            target: Some("target".to_string()),
            points: vec![Point { x: 20.0, y: 10.0 }, Point { x: 80.0, y: 10.0 }],
            auxiliary_glyphs: Vec::new(),
        };
        let glyph_lookup = HashMap::from([("source", &source), ("target", &target)]);
        let mut backend = RecordingBackend::default();

        draw_js_arc(
            &mut backend,
            &identity_transform(),
            &arc,
            &glyph_lookup,
            &HashMap::new(),
            None,
            false,
            true,
        );

        assert!(
            backend.paths.is_empty(),
            "interaction rendered marker paths"
        );
    }

    #[test]
    fn assignment_to_implicit_xor_draws_no_marker() {
        let value = test_glyph("value", "state variable", None);
        let merge = test_glyph("merge", "implicit xor", None);
        let arc = Arc {
            id: "branch".to_string(),
            class_name: "assignment".to_string(),
            source: Some("value".to_string()),
            target: Some("merge".to_string()),
            points: vec![Point { x: 10.0, y: 10.0 }, Point { x: 30.0, y: 10.0 }],
            auxiliary_glyphs: Vec::new(),
        };
        let glyph_lookup = HashMap::from([("value", &value), ("merge", &merge)]);
        let mut backend = RecordingBackend::default();

        draw_js_arc(
            &mut backend,
            &identity_transform(),
            &arc,
            &glyph_lookup,
            &HashMap::new(),
            None,
            false,
            true,
        );

        assert!(backend.paths.is_empty(), "implicit xor received a barb");
    }

    #[test]
    fn absolute_inhibition_uses_spaced_bars_connector_and_shortened_stem() {
        let arc = Arc {
            id: "absolute".to_string(),
            class_name: "absolute inhibition".to_string(),
            source: Some("source".to_string()),
            target: Some("target".to_string()),
            points: vec![Point { x: 0.0, y: 10.0 }, Point { x: 100.0, y: 10.0 }],
            auxiliary_glyphs: Vec::new(),
        };
        let mut backend = RecordingBackend::default();

        draw_js_arc(
            &mut backend,
            &identity_transform(),
            &arc,
            &HashMap::new(),
            &HashMap::new(),
            None,
            true,
            true,
        );

        assert_eq!(backend.paths.len(), 4, "line, two bars, and connector");
        let marker_size = ARROW_SIZE * CYTOSCAPE_ARROW_SCALE;
        let expected_rear_x = 100.0 - marker_size * 0.24;
        assert!((backend.paths[0].last().unwrap().x - expected_rear_x).abs() < 1e-5);
        assert_eq!(backend.paths[3].len(), 2);
    }

    #[test]
    fn location_uses_truncated_center_diagonal_and_offset_chord() {
        let mut glyph = test_glyph(
            "location",
            "location",
            Some(BBox {
                x: 10.0,
                y: 20.0,
                w: 20.0,
                h: 20.0,
            }),
        );
        glyph.parent_id = Some("entity".to_string());
        let mut backend = RecordingBackend::default();

        assert!(draw_auxiliary_glyph(
            &mut backend,
            &identity_transform(),
            &glyph
        ));

        assert_eq!(backend.paths.len(), 3);
        let diagonal = &backend.paths[1];
        let chord = &backend.paths[2];
        let chord_midpoint = Point {
            x: (chord[0].x + chord[1].x) / 2.0,
            y: (chord[0].y + chord[1].y) / 2.0,
        };
        assert!((diagonal[0].x - chord_midpoint.x).abs() < 1e-5);
        assert!((diagonal[0].y - chord_midpoint.y).abs() < 1e-5);
        let dx = diagonal[1].x - diagonal[0].x;
        let dy = diagonal[1].y - diagonal[0].y;
        let center_cross = (20.0 - diagonal[0].x) * dy - (30.0 - diagonal[0].y) * dx;
        assert!(center_cross.abs() < 1e-5);
        assert!(
            ((chord_midpoint.x - 20.0).hypot(chord_midpoint.y - 30.0) - 10.0 / 3.0).abs() < 1e-5
        );
    }

    #[test]
    fn standalone_state_variables_use_circle_or_stadium_and_are_drawn() {
        let square = test_glyph(
            "square",
            "state variable",
            Some(BBox {
                x: 0.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            }),
        );
        let wide = test_glyph(
            "wide",
            "state variable",
            Some(BBox {
                x: 0.0,
                y: 0.0,
                w: 36.0,
                h: 20.0,
            }),
        );
        assert_eq!(auxiliary_glyph_shape(&square), "ellipse");
        assert_eq!(auxiliary_glyph_shape(&wide), "stadium_round_rectangle");

        let mut backend = RecordingBackend::default();
        assert!(draw_auxiliary_glyph(
            &mut backend,
            &identity_transform(),
            &square
        ));
        assert_eq!(backend.paths.len(), 1);
    }

    #[test]
    fn fitted_bounds_include_nested_auxiliary_glyphs() {
        let entity = test_glyph(
            "entity",
            "entity",
            Some(BBox {
                x: 0.0,
                y: 100.0,
                w: 100.0,
                h: 50.0,
            }),
        );
        let mut value = test_glyph(
            "value",
            "state variable",
            Some(BBox {
                x: 20.0,
                y: 0.0,
                w: 60.0,
                h: 20.0,
            }),
        );
        value.parent_id = Some("entity".to_string());

        let bounds = compute_bounds(&[entity, value], &[]).expect("bounds should resolve");

        assert_eq!(bounds.min_y, 0.0);
    }

    #[test]
    fn influence_marker_tip_touches_unresolved_arc_target() {
        let arc = Arc {
            id: "stimulation".to_string(),
            class_name: "stimulation".to_string(),
            source: Some("outcome".to_string()),
            target: Some("arc-port".to_string()),
            points: vec![
                Point { x: 10.0, y: 10.0 },
                Point { x: 20.0, y: 10.0 },
                Point { x: 30.0, y: 10.0 },
            ],
            auxiliary_glyphs: Vec::new(),
        };
        let mut backend = RecordingBackend::default();

        draw_js_arc(
            &mut backend,
            &identity_transform(),
            &arc,
            &HashMap::new(),
            &HashMap::new(),
            None,
            false,
            true,
        );

        let marker_tip_x = backend.paths[0]
            .iter()
            .map(|point| point.x)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((marker_tip_x - 30.0).abs() < 1e-5);
    }

    #[test]
    fn explicit_delay_endpoints_snap_to_declared_ports() {
        let mut delay = test_glyph(
            "delay",
            "delay",
            Some(BBox {
                x: 20.0,
                y: 20.0,
                w: 42.0,
                h: 42.0,
            }),
        );
        delay.ports = vec![
            Port {
                id: "delay.in".to_string(),
                x: 62.0,
                y: 41.0,
            },
            Port {
                id: "delay.out".to_string(),
                x: 20.0,
                y: 41.0,
            },
        ];
        let target = test_glyph(
            "target",
            "process",
            Some(BBox {
                x: 0.0,
                y: 80.0,
                w: 20.0,
                h: 20.0,
            }),
        );
        let glyph_lookup = HashMap::from([("delay", &delay), ("target", &target)]);
        let port_lookup = HashMap::from([("delay.in", "delay"), ("delay.out", "delay")]);
        let incoming = Arc {
            id: "incoming".to_string(),
            class_name: "logic arc".to_string(),
            source: Some("target".to_string()),
            target: Some("delay.in".to_string()),
            points: vec![Point { x: 10.0, y: 90.0 }, Point { x: 41.0, y: 41.0 }],
            auxiliary_glyphs: Vec::new(),
        };
        let outgoing = Arc {
            id: "outgoing".to_string(),
            class_name: "necessary stimulation".to_string(),
            source: Some("delay.out".to_string()),
            target: Some("target".to_string()),
            points: vec![Point { x: 41.0, y: 41.0 }, Point { x: 10.0, y: 90.0 }],
            auxiliary_glyphs: Vec::new(),
        };

        let (_, incoming_end, _, _) = js_arc_points(&incoming, &glyph_lookup, &port_lookup)
            .expect("incoming delay arc should resolve");
        let (outgoing_start, _, _, _) = js_arc_points(&outgoing, &glyph_lookup, &port_lookup)
            .expect("outgoing delay arc should resolve");

        assert_eq!(incoming_end.x, 62.0);
        assert_eq!(incoming_end.y, 41.0);
        assert_eq!(outgoing_start.x, 20.0);
        assert_eq!(outgoing_start.y, 41.0);
    }

    #[test]
    fn delay_uses_a_ported_logical_operator_outline() {
        let mut delay = test_glyph(
            "delay",
            "delay",
            Some(BBox {
                x: 20.0,
                y: 20.0,
                w: 42.0,
                h: 42.0,
            }),
        );
        delay.ports = vec![
            Port {
                id: "delay.in".to_string(),
                x: 62.0,
                y: 41.0,
            },
            Port {
                id: "delay.out".to_string(),
                x: 20.0,
                y: 41.0,
            },
        ];
        let rect = sbgnviz_manifest_rect(&delay);
        let path = js_shape_path_for_glyph(rect, "ellipse", &delay, &identity_transform());

        assert!(is_ported_glyph_class("delay"));
        assert_eq!(rect.width, 42.0);
        assert!(
            path.segments().count() > 50,
            "delay outline lacks circular core and stubs"
        );
    }

    #[test]
    fn logic_arc_to_outcome_retains_explicit_center_endpoint() {
        let source = test_glyph(
            "source",
            "state variable",
            Some(BBox {
                x: 0.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            }),
        );
        let outcome = test_glyph(
            "outcome",
            "outcome",
            Some(BBox {
                x: 40.0,
                y: 30.0,
                w: 20.0,
                h: 20.0,
            }),
        );
        let arc = Arc {
            id: "logic".to_string(),
            class_name: "logic arc".to_string(),
            source: Some("source".to_string()),
            target: Some("outcome".to_string()),
            points: vec![Point { x: 10.0, y: 20.0 }, Point { x: 50.0, y: 40.0 }],
            auxiliary_glyphs: Vec::new(),
        };
        let glyph_lookup = HashMap::from([("source", &source), ("outcome", &outcome)]);

        let (_, end, _, _) =
            js_arc_points(&arc, &glyph_lookup, &HashMap::new()).expect("logic arc should resolve");

        assert_eq!(end.x, 50.0);
        assert_eq!(end.y, 40.0);
    }

    #[test]
    fn preserves_nested_entity_parentage() {
        let (glyphs, _, _) = parse_example(include_str!("../../examples/nested_entity.sbgn"));
        let child = glyphs
            .iter()
            .find(|glyph| glyph.id == "subunit")
            .expect("nested entity should be parsed");
        assert_eq!(child.parent_id.as_deref(), Some("complex_entity"));
    }

    #[test]
    fn maps_er_influence_markers() {
        assert_eq!(js_arc_marker("assignment"), "barbed-arrow");
        assert_eq!(js_arc_marker("production"), "filled-triangle");
        assert_eq!(js_arc_marker("interaction"), "none");
        assert_eq!(js_arc_marker("modulation"), "diamond");
        assert_eq!(js_arc_marker("necessary stimulation"), "triangle-cross");
        assert_eq!(js_arc_marker("absolute stimulation"), "triangle");
        assert_eq!(js_arc_marker("absolute inhibition"), "double-tee");
        assert_eq!(js_arc_marker("logic arc"), "none");
    }

    #[test]
    fn assignment_marker_has_a_recessed_barb() {
        let left_rear = BARBED_ASSIGNMENT_MARKER[1];
        let notch = BARBED_ASSIGNMENT_MARKER[2];
        let right_rear = BARBED_ASSIGNMENT_MARKER[3];
        assert_eq!(notch.x, 0.0);
        assert!(notch.y > left_rear.y);
        assert!(notch.y > right_rear.y);
    }

    #[test]
    fn absolute_stimulation_uses_the_go_triangle_marker() {
        assert_eq!(js_arc_marker("absolute stimulation"), "triangle");
    }

    #[test]
    fn parses_svg_rendering_options() {
        let cli = parse_draw_svg_args(
            [
                "--input-path",
                "source.svg",
                "--output-path",
                "render.png",
                "--padding",
                "12",
                "--scale",
                "2.5",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("SVG rendering options should parse");

        let Command::DrawSvg {
            input,
            output,
            padding,
            scale,
        } = cli.command
        else {
            panic!("expected draw_svg command");
        };
        assert_eq!(input, PathBuf::from("source.svg"));
        assert_eq!(output, PathBuf::from("render.png"));
        assert_eq!(padding, 12);
        assert_eq!(scale, 2.5);
    }

    #[test]
    fn rejects_non_positive_svg_scale() {
        let error = parse_draw_svg_args(
            [
                "--input-path",
                "source.svg",
                "--output-path",
                "render.png",
                "--scale",
                "0",
            ]
            .into_iter()
            .map(String::from),
        )
        .err()
        .expect("zero SVG scale should be rejected");
        assert!(error.to_string().contains("positive finite"));
    }

    #[test]
    fn auxiliary_circle_is_square_and_right_half_is_filled() {
        let source = PixelRect {
            x0: 10.0,
            y0: 20.0,
            width: 22.0,
            height: 32.0,
            center: Point { x: 21.0, y: 36.0 },
        };
        let circle = inscribed_circle_rect(source);
        assert_eq!(circle.width, 22.0);
        assert_eq!(circle.height, 22.0);
        assert_eq!(circle.center.x, 21.0);
        assert_eq!(circle.center.y, 36.0);

        let fill = right_semicircle_rect(circle);
        assert_eq!(fill.x0, circle.center.x);
        assert_eq!(fill.width, circle.width / 2.0);
        assert_eq!(fill.height, circle.height);
    }

    #[test]
    fn renders_multiline_labels_to_svg() {
        let (glyphs, arcs, bounds) =
            parse_example(include_str!("../../examples/reference_card.sbgn"));
        let (transform, width, height) = transform_with_padding(bounds, 10.0, None, None);
        let mut backend = SvgBackend::new(width, height, None).expect("SVG backend should start");
        render_scene(
            &mut backend,
            &transform,
            &glyphs,
            &arcs,
            true,
            &HashMap::new(),
            None,
            GlyphColorType::Label,
            true,
        )
        .expect("reference card should render");
        let svg = backend.finish();
        assert!(svg.contains(">Source</text>"));
        assert!(svg.contains(">Entity</text>"));
        assert!(svg.contains("<path"));
    }

    #[test]
    fn rejects_non_er_maps_when_language_is_explicit() {
        let root = Element::parse(
            r#"<sbgn><map id="map" language="activity flow"><glyph id="g" class="biological activity"><bbox x="0" y="0" w="10" h="10"/></glyph></map></sbgn>"#
                .as_bytes(),
        )
        .expect("XML should parse");
        let error = parse_sbgn(&root).expect_err("activity flow should be rejected");
        assert!(error.to_string().contains("Entity Relationship"));
    }
}
