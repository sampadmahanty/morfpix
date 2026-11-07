use morfpix_engine::ConversionResult;
use std::io::{stdout, IsTerminal};
use std::path::Path;

#[derive(Clone, Copy)]
pub struct Styler {
    pub enabled: bool,
}

impl Styler {
    pub fn new() -> Self {
        let is_terminal = stdout().is_terminal();
        let no_color = std::env::var_os("NO_COLOR").is_some();
        Self {
            enabled: is_terminal && !no_color,
        }
    }

    pub fn green(&self, text: &str) -> String {
        self.paint("32", text)
    }

    pub fn cyan(&self, text: &str) -> String {
        self.paint("36", text)
    }

    pub fn yellow(&self, text: &str) -> String {
        self.paint("33", text)
    }

    pub fn red(&self, text: &str) -> String {
        self.paint("31", text)
    }

    pub fn bold(&self, text: &str) -> String {
        self.paint("1", text)
    }

    pub fn dim(&self, text: &str) -> String {
        self.paint("2", text)
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\x1b[{}m{}\x1b[0m", code, text)
        } else {
            text.to_string()
        }
    }
}

pub fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.2} KB", bytes as f32 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f32 / (1024.0 * 1024.0))
    }
}

pub fn format_change(result: &ConversionResult, styler: &Styler) -> String {
    if result.saved_bytes > 0 {
        styler.green(&format!("-{:.1}%", result.saved_percent))
    } else if result.saved_bytes < 0 {
        styler.yellow(&format!("+{:.1}%", -result.saved_percent))
    } else {
        "0.0%".to_string()
    }
}

pub fn print_help(styler: &Styler, cmd_name: &str) {
    println!(
        "{} {}",
        styler.bold("morfpix"),
        styler.dim(&format!(
            "{} - Image compression and format conversion",
            env!("CARGO_PKG_VERSION")
        ))
    );
    println!();
    println!("{}", styler.bold("USAGE:"));
    println!("    morfpix [OPTIONS] <FILE...>");
    println!("    morfpix [OPTIONS] <DIRECTORY>");
    println!("    (alias: mpix)");
    println!();
    println!("{}", styler.bold("OPTIONS:"));
    println!("    -o, --output <PATH>      Output file (single) or directory (batch)");
    println!("    -f, --format <FORMAT>    Target format: jpeg, png, webp, gif, bmp, avif, jxl");
    println!("    -q, --quality <1-100>    Quality for JPEG, WebP, and AVIF (default: 80)");
    println!(
        "        --baseline           Encode baseline (sequential) JPEG instead of progressive"
    );
    println!("        --subsample <MODE>   Chroma subsampling for JPEG: 444, 422, 420, 440, auto");
    println!("        --smoothing <0-100>  Smoothing factor for JPEG (default: 0)");
    println!("    -s, --speed <1-10>       AVIF encoding speed (default: 4)");
    println!("        --alpha-quality <1-100>  AVIF alpha quality (default: quality)");
    println!("        --bit-depth <8|10|auto>  AVIF bit depth (default: auto)");
    println!("        --color <ycbcr|rgb>  AVIF internal color model (default: ycbcr)");
    println!("    -t, --threads <N>        Encoding threads for AVIF/JXL, 0 = auto (default: 0)");
    println!("        --alpha-mode <MODE>  AVIF alpha mode: clean, dirty, premultiplied");
    println!("        --optimize           Optimize PNG output with oxipng (preset 2)");
    println!("        --png-level <0-6>    PNG oxipng optimization level (0 = none, default: 2)");
    println!("        --interlace          Enable PNG Adam7 interlacing");
    println!("        --lossless           WebP lossless encoding mode");
    println!("    -m, --method <0-6>       WebP compression method/effort (default: 4)");
    println!("        --exact              Preserve RGB values under transparent area in WebP");
    println!("    -e, --effort <1-9>       JXL encoding effort (default: 4)");
    println!("        --gif-repeat <MODE>  GIF repeat: infinite, none, or loop count 1-65535 (default: infinite)");
    println!("        --gif-speed <1-30>   GIF color quantization speed/quality (default: 10)");
    println!(
        "        --bmp-bpp <8|24|32|auto> BMP bits per pixel: 8, 24, 32, or auto (default: auto)"
    );
    println!("    -j, --jobs <N>           Concurrent jobs for batch mode, 0 = all cores (default: auto up to 4)");
    println!("    -h, --help               Print this help manual");
    println!("    -V, --version            Print version information");
    println!();
    println!("{}", styler.bold("EXAMPLES:"));
    println!("    {} photo.png -f webp -q 80", cmd_name);
    println!("    {} photo.png -f webp --lossless", cmd_name);
    println!(
        "    {} photo.png -f avif -q 60 -s 4 --bit-depth 10",
        cmd_name
    );
    println!("    {} photo.png -f jxl -e 7", cmd_name);
    println!(
        "    {} photo.png -f jpeg -q 85 --baseline --subsample 444",
        cmd_name
    );
    println!(
        "    {} photo.jpg -f png --png-level 4 --interlace -o photo.png",
        cmd_name
    );
    println!("    {} photo.png -f gif --repeat infinite", cmd_name);
    println!("    {} photo.png -f bmp --bmp-bpp 24", cmd_name);
    println!("    {} ./images -f webp -o ./compressed", cmd_name);
}

pub fn print_result(input: &Path, output: &Path, result: &ConversionResult, styler: &Styler) {
    println!(
        "{}: {} -> {}",
        styler.bold("Converted"),
        input.display(),
        styler.cyan(&output.display().to_string())
    );
    println!(
        "  {}: {} -> {}",
        styler.dim("Format"),
        result.input_format.extension().to_uppercase(),
        result.output_format.extension().to_uppercase()
    );
    println!(
        "  {}: {} x {} px",
        styler.dim("Dimensions"),
        result.width,
        result.height
    );
    println!(
        "  {}: {} -> {} ({})",
        styler.dim("Size"),
        format_bytes(result.input_size_bytes),
        format_bytes(result.output_size_bytes),
        format_change(result, styler)
    );
    println!("  {}: {} ms", styler.dim("Duration"), result.duration_ms);
}

pub struct BatchItemResult {
    pub input_size: usize,
    pub output_size: usize,
}

pub fn print_batch_summary(
    items: &[BatchItemResult],
    failed: usize,
    total_duration_ms: u64,
    styler: &Styler,
) {
    let mut total_input: usize = 0;
    let mut total_output: usize = 0;

    for item in items {
        total_input += item.input_size;
        total_output += item.output_size;
    }

    let total_change = total_input as i64 - total_output as i64;
    let total_percent = if total_input > 0 {
        (total_change as f32 / total_input as f32) * 100.0
    } else {
        0.0
    };
    let change = if total_change > 0 {
        format!(
            "{} saved, -{:.1}%",
            format_bytes(total_change as usize),
            total_percent
        )
    } else if total_change < 0 {
        format!(
            "{} larger, +{:.1}%",
            format_bytes((-total_change) as usize),
            -total_percent
        )
    } else {
        "no change".to_string()
    };

    println!();
    println!(
        "{} {} succeeded, {} failed in {} ms",
        styler.bold("Done:"),
        items.len(),
        failed,
        total_duration_ms
    );
    if !items.is_empty() {
        println!(
            "  {}: {} -> {} ({})",
            styler.dim("Total Size"),
            format_bytes(total_input),
            format_bytes(total_output),
            change
        );
    }
}
