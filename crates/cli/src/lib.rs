mod ui;

use morfpix_engine::{
    convert, estimate_memory_from_header, estimate_memory_usage_with_options, AvifAlphaMode,
    AvifBitDepth, AvifColorModel, BmpBitsPerPixel, ConversionOptions, GifRepeat, ImageFormat,
    JpegSubsampling, ResizeFilter, ResizeOptions, MAX_IMAGE_HEIGHT, MAX_IMAGE_PIXELS,
    MAX_IMAGE_WIDTH, MAX_INPUT_FILE_SIZE, MAX_REASONABLE_MEMORY_ESTIMATE,
};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex};
use std::time::Instant;
use ui::{format_bytes, print_batch_summary, print_help, print_result, BatchItemResult, Styler};

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jfif", "png", "apng", "webp", "bmp", "gif", "avif", "jxl",
];

struct CliArgs {
    inputs: Vec<PathBuf>,
    output: Option<PathBuf>,
    options: ConversionOptions,
    jobs: Option<usize>,
    explicit_threads: bool,
}

struct InputFile {
    path: PathBuf,
    base: Option<PathBuf>,
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| SUPPORTED_EXTENSIONS.contains(&extension.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn same_path(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    #[cfg(unix)]
    if let (Ok(meta_a), Ok(meta_b)) = (fs::metadata(a), fs::metadata(b)) {
        use std::os::unix::fs::MetadataExt;
        if meta_a.dev() == meta_b.dev() && meta_a.ino() == meta_b.ino() {
            return true;
        }
    }
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

const DEFAULT_BATCH_MEMORY_BUDGET: usize = 512 * 1024 * 1024; // 512 MiB aggregate memory budget

struct MemoryBudgetCoordinator {
    max_bytes: usize,
    state: Mutex<usize>,
    condvar: Condvar,
}

impl MemoryBudgetCoordinator {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            state: Mutex::new(0),
            condvar: Condvar::new(),
        }
    }

    pub fn acquire(&self, amount: usize) -> MemoryPermit<'_> {
        let mut allocated = self.state.lock().unwrap();
        while *allocated > 0
            && allocated
                .checked_add(amount)
                .is_none_or(|sum| sum > self.max_bytes)
        {
            allocated = self.condvar.wait(allocated).unwrap();
        }
        *allocated = allocated.saturating_add(amount);
        MemoryPermit {
            coordinator: self,
            amount,
        }
    }
}

struct MemoryPermit<'a> {
    coordinator: &'a MemoryBudgetCoordinator,
    amount: usize,
}

impl<'a> MemoryPermit<'a> {
    pub fn amount(&self) -> usize {
        self.amount
    }

    pub fn try_resize(&mut self, new_amount: usize) -> bool {
        if new_amount <= self.amount {
            let mut allocated = self.coordinator.state.lock().unwrap();
            let diff = self.amount - new_amount;
            *allocated = allocated.saturating_sub(diff);
            self.amount = new_amount;
            self.coordinator.condvar.notify_all();
            return true;
        }

        let mut allocated = self.coordinator.state.lock().unwrap();
        let other_allocated = allocated.saturating_sub(self.amount);
        if other_allocated == 0
            || other_allocated
                .checked_add(new_amount)
                .is_some_and(|sum| sum <= self.coordinator.max_bytes)
        {
            let additional = new_amount - self.amount;
            *allocated = allocated.saturating_add(additional);
            self.amount = new_amount;
            true
        } else {
            false
        }
    }
}

impl Drop for MemoryPermit<'_> {
    fn drop(&mut self) {
        let mut allocated = self.coordinator.state.lock().unwrap();
        *allocated = allocated.saturating_sub(self.amount);
        self.coordinator.condvar.notify_all();
    }
}

fn collect_from_dir(
    dir: &Path,
    exclude: Option<&Path>,
    files: &mut Vec<PathBuf>,
    visited: &mut HashSet<PathBuf>,
) {
    let canonical_dir = match fs::canonicalize(dir) {
        Ok(c) => c,
        Err(_) => return,
    };
    if !visited.insert(canonical_dir) {
        return;
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            // Skip all symlinks (directories, image files, or broken symlinks)
            // to ensure recursive scanning strictly stays within the selected directory tree.
            if file_type.is_symlink() {
                continue;
            }

            if file_type.is_dir() {
                if exclude.is_some_and(|exclude| same_path(&path, exclude)) {
                    continue;
                }
                collect_from_dir(&path, exclude, files, visited);
            } else if file_type.is_file() && is_supported_image(&path) {
                files.push(path);
            }
        }
    }
}

fn missing_value(option: &str, styler: &Styler) -> ! {
    eprintln!(
        "{} Option '{}' requires a value",
        styler.red("Error:"),
        option
    );
    std::process::exit(1);
}

fn parse_args(args: &[String], styler: &Styler, cmd_name: &str) -> Option<CliArgs> {
    let mut inputs = Vec::new();
    let mut output = None;
    let mut options = ConversionOptions::default();
    let mut jobs = None;
    let mut explicit_threads = false;
    let mut only_inputs = false;
    let mut resize_width: Option<u32> = None;
    let mut resize_height: Option<u32> = None;
    let mut resize_filter: Option<ResizeFilter> = None;
    let mut i = 1;

    while i < args.len() {
        let arg = args[i].as_str();

        if only_inputs {
            inputs.push(PathBuf::from(arg));
            i += 1;
            continue;
        }

        match arg {
            "-h" | "--help" => {
                print_help(styler, cmd_name);
                return None;
            }
            "-V" | "--version" | "-v" | "version" => {
                run_version();
                return None;
            }
            "--" => only_inputs = true,
            "-o" | "--output" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                output = Some(PathBuf::from(&args[i]));
            }
            "-f" | "--format" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                let format = ImageFormat::from_extension(&args[i]);
                if !format.is_supported() {
                    eprintln!(
                        "{} Unsupported format '{}'. Supported formats: jpeg, png, webp, gif, bmp, avif, jxl",
                        styler.red("Error:"),
                        args[i]
                    );
                    std::process::exit(1);
                }
                options.format = Some(format);
            }
            "-q" | "--quality" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                match args[i].parse::<u8>() {
                    Ok(quality) if (1..=100).contains(&quality) => options.quality = quality,
                    _ => {
                        eprintln!(
                            "{} Quality must be an integer between 1 and 100",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                }
            }
            "-s" | "--speed" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                match args[i].parse::<u8>() {
                    Ok(speed) if (1..=10).contains(&speed) => options.avif.speed = speed,
                    _ => {
                        eprintln!(
                            "{} Speed must be an integer between 1 and 10",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                }
            }
            "--alpha-quality" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                match args[i].parse::<u8>() {
                    Ok(quality) if (1..=100).contains(&quality) => {
                        options.avif.alpha_quality = Some(quality);
                    }
                    _ => {
                        eprintln!(
                            "{} Alpha quality must be an integer between 1 and 100",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                }
            }
            "--bit-depth" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                options.avif.bit_depth = match args[i].to_lowercase().as_str() {
                    "8" => AvifBitDepth::Eight,
                    "10" => AvifBitDepth::Ten,
                    "auto" => AvifBitDepth::Auto,
                    _ => {
                        eprintln!(
                            "{} Bit depth must be one of: 8, 10, auto",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                };
            }
            "--color" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                options.avif.color_model = match args[i].to_lowercase().as_str() {
                    "ycbcr" => AvifColorModel::Ycbcr,
                    "rgb" => AvifColorModel::Rgb,
                    _ => {
                        eprintln!(
                            "{} Color model must be one of: ycbcr, rgb",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                };
            }
            "-t" | "--threads" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                match args[i].parse::<usize>() {
                    Ok(threads) => {
                        options.avif.threads = threads;
                        options.jxl.threads = threads;
                        explicit_threads = threads > 0;
                    }
                    Err(_) => {
                        eprintln!(
                            "{} Threads must be a non-negative integer",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                }
            }
            "-e" | "--effort" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                match args[i].parse::<u8>() {
                    Ok(effort) if (1..=9).contains(&effort) => options.jxl.effort = effort,
                    _ => {
                        eprintln!(
                            "{} JXL encoding effort must be an integer between 1 and 9",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                }
            }
            "--alpha-mode" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                options.avif.alpha_mode = match args[i].to_lowercase().as_str() {
                    "clean" => AvifAlphaMode::Clean,
                    "dirty" => AvifAlphaMode::Dirty,
                    "premultiplied" => AvifAlphaMode::Premultiplied,
                    _ => {
                        eprintln!(
                            "{} Alpha mode must be one of: clean, dirty, premultiplied",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                };
            }
            "--optimize" | "--optimize-png" => {
                options.png.optimize = true;
            }
            "--png-level" | "--oxipng-level" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                match args[i].parse::<u8>() {
                    Ok(level) if (0..=6).contains(&level) => {
                        options.png.level = level;
                        options.png.optimize = level > 0;
                    }
                    _ => {
                        eprintln!(
                            "{} PNG optimization level must be an integer between 0 and 6",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                }
            }
            "--interlace" => {
                options.png.interlace = true;
            }
            "--lossless" => {
                options.webp.lossless = true;
            }
            "-m" | "--method" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                match args[i].parse::<u8>() {
                    Ok(method) if (0..=6).contains(&method) => options.webp.method = method,
                    _ => {
                        eprintln!(
                            "{} WebP compression method must be an integer between 0 and 6",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                }
            }
            "--exact" => {
                options.webp.exact = true;
            }
            "--baseline" => {
                options.jpeg.progressive = false;
            }
            "--progressive" => {
                options.jpeg.progressive = true;
            }
            "--subsample" | "--subsampling" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                options.jpeg.subsampling = match args[i].to_lowercase().as_str() {
                    "444" | "4:4:4" | "s444" => JpegSubsampling::S444,
                    "422" | "4:2:2" | "s422" => JpegSubsampling::S422,
                    "420" | "4:2:0" | "s420" => JpegSubsampling::S420,
                    "440" | "4:4:0" | "s440" => JpegSubsampling::S440,
                    "auto" => JpegSubsampling::Auto,
                    _ => {
                        eprintln!(
                            "{} Chroma subsampling must be one of: 444, 422, 420, 440, auto",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                };
            }
            "--smoothing" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                match args[i].parse::<u8>() {
                    Ok(factor) if (0..=100).contains(&factor) => options.jpeg.smoothing = factor,
                    _ => {
                        eprintln!(
                            "{} Smoothing factor must be an integer between 0 and 100",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                }
            }
            "--gif-repeat" | "--repeat" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                let val = args[i].to_lowercase();
                options.gif.repeat = match val.as_str() {
                    "infinite" | "inf" | "loop" => Some(GifRepeat::Infinite),
                    "none" | "0" | "once" | "never" => None,
                    s => match s.parse::<u16>() {
                        Ok(n) if n > 0 => Some(GifRepeat::Finite(n)),
                        _ => {
                            eprintln!(
                                "{} GIF repeat count must be 'infinite', 'none', or an integer between 1 and 65535",
                                styler.red("Error:")
                            );
                            std::process::exit(1);
                        }
                    },
                };
            }
            "--gif-speed" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                match args[i].parse::<u8>() {
                    Ok(speed) if (1..=30).contains(&speed) => options.gif.speed = speed,
                    _ => {
                        eprintln!(
                            "{} GIF speed must be an integer between 1 and 30",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                }
            }
            "--bmp-bpp" | "--bmp-bit-depth" | "--bpp" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                options.bmp.bpp = match args[i].to_lowercase().as_str() {
                    "8" | "bpp8" => BmpBitsPerPixel::Bpp8,
                    "24" | "bpp24" => BmpBitsPerPixel::Bpp24,
                    "32" | "bpp32" => BmpBitsPerPixel::Bpp32,
                    "auto" => BmpBitsPerPixel::Auto,
                    _ => {
                        eprintln!(
                            "{} BMP bits per pixel (bpp) must be one of: 8, 24, 32, auto",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                };
            }
            "-j" | "--jobs" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                match args[i].parse::<usize>() {
                    Ok(j) => jobs = Some(j),
                    Err(_) => {
                        eprintln!(
                            "{} Jobs must be a non-negative integer",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                }
            }
            "--width" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                match args[i].parse::<u32>() {
                    Ok(w) if w > 0 && w <= MAX_IMAGE_WIDTH => resize_width = Some(w),
                    Ok(0) => {
                        eprintln!("{} Width must be greater than 0", styler.red("Error:"));
                        std::process::exit(1);
                    }
                    Ok(_) => {
                        eprintln!(
                            "{} Width exceeds maximum allowed limit of {} pixels",
                            styler.red("Error:"),
                            MAX_IMAGE_WIDTH
                        );
                        std::process::exit(1);
                    }
                    Err(_) => {
                        eprintln!("{} Width must be a positive integer", styler.red("Error:"));
                        std::process::exit(1);
                    }
                }
            }
            "--height" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                match args[i].parse::<u32>() {
                    Ok(h) if h > 0 && h <= MAX_IMAGE_HEIGHT => resize_height = Some(h),
                    Ok(0) => {
                        eprintln!("{} Height must be greater than 0", styler.red("Error:"));
                        std::process::exit(1);
                    }
                    Ok(_) => {
                        eprintln!(
                            "{} Height exceeds maximum allowed limit of {} pixels",
                            styler.red("Error:"),
                            MAX_IMAGE_HEIGHT
                        );
                        std::process::exit(1);
                    }
                    Err(_) => {
                        eprintln!("{} Height must be a positive integer", styler.red("Error:"));
                        std::process::exit(1);
                    }
                }
            }
            "--filter" | "--resize-filter" => {
                i += 1;
                if i >= args.len() {
                    missing_value(arg, styler);
                }
                resize_filter = match args[i].to_lowercase().as_str() {
                    "lanczos3" | "lanczos" => Some(ResizeFilter::Lanczos3),
                    "catmullrom" | "catmull-rom" | "bicubic" => Some(ResizeFilter::CatmullRom),
                    "triangle" | "bilinear" => Some(ResizeFilter::Triangle),
                    "nearest" => Some(ResizeFilter::Nearest),
                    "gaussian" => Some(ResizeFilter::Gaussian),
                    _ => {
                        eprintln!(
                            "{} Resize filter must be one of: lanczos3, catmullrom, triangle, nearest, gaussian",
                            styler.red("Error:")
                        );
                        std::process::exit(1);
                    }
                };
            }
            option if option.starts_with('-') => {
                eprintln!("{} Unknown option '{}'", styler.red("Error:"), option);
                std::process::exit(1);
            }
            input => inputs.push(PathBuf::from(input)),
        }

        i += 1;
    }

    if inputs.is_empty() {
        eprintln!(
            "{} No input files or directories specified",
            styler.red("Error:")
        );
        eprintln!(
            "Run '{}' for usage.",
            styler.cyan(&format!("{cmd_name} --help"))
        );
        std::process::exit(1);
    }

    if resize_filter.is_some() && resize_width.is_none() && resize_height.is_none() {
        eprintln!(
            "{} Resizing requires at least one dimension (--width or --height)",
            styler.red("Error:")
        );
        std::process::exit(1);
    }

    if let (Some(w), Some(h)) = (resize_width, resize_height) {
        let pixels = w as u64 * h as u64;
        if pixels > MAX_IMAGE_PIXELS {
            eprintln!(
                "{} Requested resize bounding box ({}x{} = {} pixels) exceeds maximum allowed limit ({} pixels)",
                styler.red("Error:"),
                w, h, pixels, MAX_IMAGE_PIXELS
            );
            std::process::exit(1);
        }
    }

    if resize_width.is_some() || resize_height.is_some() {
        options.resize = Some(ResizeOptions {
            width: resize_width,
            height: resize_height,
            filter: resize_filter.unwrap_or_default(),
        });
    }

    Some(CliArgs {
        inputs,
        output,
        options,
        jobs,
        explicit_threads,
    })
}

fn collect_inputs(inputs: &[PathBuf], output: Option<&Path>, styler: &Styler) -> Vec<InputFile> {
    let mut files = Vec::new();

    for input in inputs {
        if input.is_dir() {
            let mut found = Vec::new();
            let mut visited = HashSet::new();
            collect_from_dir(input, output, &mut found, &mut visited);
            found.sort();
            for path in found {
                files.push(InputFile {
                    path,
                    base: Some(input.clone()),
                });
            }
        } else if input.is_file() {
            files.push(InputFile {
                path: input.clone(),
                base: None,
            });
        } else {
            eprintln!(
                "{} Input not found: {}",
                styler.red("Error:"),
                input.display()
            );
            std::process::exit(1);
        }
    }

    files
}

fn is_directory_target(path: &Path) -> bool {
    if path.is_dir() {
        return true;
    }
    if path.exists() {
        return false;
    }
    let path_str = path.to_string_lossy();
    if path_str.ends_with('/') || path_str.ends_with('\\') {
        return true;
    }
    match path.extension().and_then(|ext| ext.to_str()) {
        None => true,
        Some(ext) => !ImageFormat::from_extension(ext).is_supported(),
    }
}

fn default_filename(path: &Path, target_format: ImageFormat) -> PathBuf {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = target_format.extension();
    let input_format = path
        .extension()
        .and_then(|e| e.to_str())
        .map(ImageFormat::from_extension)
        .unwrap_or(ImageFormat::Unknown);

    let is_same_format = input_format == target_format;
    let new_name = if is_same_format {
        format!("{stem}-compressed.{extension}")
    } else {
        format!("{stem}.{extension}")
    };

    path.with_file_name(new_name)
}

fn output_path(
    input: &InputFile,
    output: Option<&Path>,
    format: ImageFormat,
    batch: bool,
) -> PathBuf {
    let extension = format.extension();

    if batch {
        if let Some(output_dir) = output {
            let relative = input
                .base
                .as_ref()
                .and_then(|base| input.path.strip_prefix(base).ok())
                .unwrap_or_else(|| Path::new(input.path.file_name().unwrap_or_default()));
            let candidate = output_dir.join(relative).with_extension(extension);
            if same_path(&input.path, &candidate) {
                let stem = input.path.file_stem().unwrap_or_default().to_string_lossy();
                candidate.with_file_name(format!("{stem}-compressed.{extension}"))
            } else {
                candidate
            }
        } else {
            default_filename(&input.path, format)
        }
    } else {
        match output {
            Some(path) if is_directory_target(path) => {
                let candidate = path
                    .join(input.path.file_name().unwrap_or_default())
                    .with_extension(extension);
                if same_path(&input.path, &candidate) {
                    let stem = input.path.file_stem().unwrap_or_default().to_string_lossy();
                    candidate.with_file_name(format!("{stem}-compressed.{extension}"))
                } else {
                    candidate
                }
            }
            Some(path) => path.to_path_buf(),
            None => default_filename(&input.path, format),
        }
    }
}

fn unique_output_path(path: PathBuf, used: &mut HashSet<PathBuf>) -> PathBuf {
    if used.insert(path.clone()) {
        return path;
    }

    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let extension = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let mut counter = 1;
    loop {
        let name = if extension.is_empty() {
            format!("{}-{}", stem, counter)
        } else {
            format!("{}-{}.{}", stem, counter, extension)
        };
        let candidate = parent.join(name);
        if used.insert(candidate.clone()) {
            return candidate;
        }
        counter += 1;
    }
}

fn is_system_symlink(_path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        if _path == Path::new("/tmp")
            || _path == Path::new("/var")
            || _path == Path::new("/etc")
            || _path == Path::new("/home")
        {
            return true;
        }
    }
    #[cfg(target_os = "linux")]
    {
        if _path == Path::new("/bin")
            || _path == Path::new("/lib")
            || _path == Path::new("/lib64")
            || _path == Path::new("/sbin")
        {
            return true;
        }
    }
    false
}

#[cfg(unix)]
fn resolve_system_prefix(path: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Ok(rest) = path.strip_prefix("/tmp") {
            return Path::new("/private/tmp").join(rest);
        }
        if let Ok(rest) = path.strip_prefix("/var") {
            return Path::new("/private/var").join(rest);
        }
        if let Ok(rest) = path.strip_prefix("/etc") {
            return Path::new("/private/etc").join(rest);
        }
        if let Ok(rest) = path.strip_prefix("/home") {
            return Path::new("/System/Volumes/Data/home").join(rest);
        }
    }
    path.to_path_buf()
}

enum WriteOutputError {
    SymlinkDestination,
    CreateDir(PathBuf, std::io::Error),
    WriteFile(std::io::Error),
}

fn check_parent_symlinks(path: &Path) -> Result<(), WriteOutputError> {
    let mut cur = path.parent();
    while let Some(parent) = cur {
        if parent.as_os_str().is_empty() || parent == Path::new("/") {
            break;
        }
        if !is_system_symlink(parent) {
            match fs::symlink_metadata(parent) {
                Ok(meta) => {
                    if meta.file_type().is_symlink() {
                        return Err(WriteOutputError::SymlinkDestination);
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(WriteOutputError::WriteFile(err)),
            }
        }
        cur = parent.parent();
    }
    Ok(())
}

#[cfg(unix)]
fn safe_write_output(path: &Path, data: &[u8]) -> Result<(), WriteOutputError> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::io::FromRawFd;

    // Reject destination upfront if already a symlink
    if fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(WriteOutputError::SymlinkDestination);
    }

    let normalized = resolve_system_prefix(path);

    // Find the deepest existing ancestor
    let mut existing_ancestor: Option<PathBuf> = None;
    let mut check_curr = normalized.parent();
    while let Some(curr) = check_curr {
        if curr.as_os_str().is_empty() || curr == Path::new("/") {
            break;
        }
        match fs::symlink_metadata(curr) {
            Ok(meta) => {
                if meta.file_type().is_symlink() && !is_system_symlink(curr) {
                    return Err(WriteOutputError::SymlinkDestination);
                }
                existing_ancestor = Some(curr.to_path_buf());
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(WriteOutputError::WriteFile(err)),
        }
        check_curr = curr.parent();
    }

    let base_path = match existing_ancestor {
        Some(b) => resolve_system_prefix(&b),
        None => {
            if normalized.is_absolute() {
                PathBuf::from("/")
            } else {
                PathBuf::from(".")
            }
        }
    };

    // Open the base directory with O_DIRECTORY | O_NOFOLLOW
    let c_base = CString::new(base_path.as_os_str().as_bytes()).map_err(|_| {
        WriteOutputError::WriteFile(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid base path",
        ))
    })?;

    let base_fd = unsafe {
        libc::open(
            c_base.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };

    if base_fd < 0 {
        let err = std::io::Error::last_os_error();
        let raw = err.raw_os_error().unwrap_or(0);
        if raw == libc::ELOOP || raw == libc::ENOTDIR {
            return Err(WriteOutputError::SymlinkDestination);
        }
        return Err(WriteOutputError::WriteFile(err));
    }

    struct FdGuard(libc::c_int);
    impl Drop for FdGuard {
        fn drop(&mut self) {
            if self.0 >= 0 {
                unsafe { libc::close(self.0) };
            }
        }
    }
    let mut guard = FdGuard(base_fd);

    let relative = if base_path == Path::new(".") {
        normalized.strip_prefix("./").unwrap_or(&normalized)
    } else if base_path == Path::new("/") {
        normalized.strip_prefix("/").unwrap_or(&normalized)
    } else {
        match normalized.strip_prefix(&base_path) {
            Ok(rel) => rel,
            Err(_) => &normalized,
        }
    };

    let mut components: Vec<&std::ffi::OsStr> = Vec::new();
    for comp in relative.components() {
        match comp {
            std::path::Component::Normal(c) => components.push(c),
            std::path::Component::CurDir => {}
            _ => {
                return Err(WriteOutputError::WriteFile(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Unsupported path component",
                )));
            }
        }
    }

    if components.is_empty() {
        return Err(WriteOutputError::WriteFile(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Output path is a directory",
        )));
    }

    let filename = components.pop().unwrap();

    // Traverse and create intermediate directory components safely
    for comp in components {
        let c_comp = CString::new(comp.as_bytes()).map_err(|_| {
            WriteOutputError::WriteFile(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid path component",
            ))
        })?;

        let next_fd = unsafe {
            libc::openat(
                guard.0,
                c_comp.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };

        if next_fd >= 0 {
            unsafe { libc::close(guard.0) };
            guard.0 = next_fd;
        } else {
            let err = std::io::Error::last_os_error();
            let raw = err.raw_os_error().unwrap_or(0);
            if raw == libc::ENOENT {
                let res = unsafe { libc::mkdirat(guard.0, c_comp.as_ptr(), 0o777) };
                if res != 0 {
                    let mkdir_err = std::io::Error::last_os_error();
                    if mkdir_err.raw_os_error() != Some(libc::EEXIST) {
                        return Err(WriteOutputError::CreateDir(comp.into(), mkdir_err));
                    }
                }
                let next_fd2 = unsafe {
                    libc::openat(
                        guard.0,
                        c_comp.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if next_fd2 < 0 {
                    let err2 = std::io::Error::last_os_error();
                    let raw2 = err2.raw_os_error().unwrap_or(0);
                    if raw2 == libc::ELOOP || raw2 == libc::ENOTDIR {
                        return Err(WriteOutputError::SymlinkDestination);
                    }
                    return Err(WriteOutputError::CreateDir(comp.into(), err2));
                }
                unsafe { libc::close(guard.0) };
                guard.0 = next_fd2;
            } else if raw == libc::ELOOP || raw == libc::ENOTDIR {
                return Err(WriteOutputError::SymlinkDestination);
            } else {
                return Err(WriteOutputError::WriteFile(err));
            }
        }
    }

    // Now guard.0 is the parent directory fd.
    // Write to a temporary file in the same directory and atomically replace destination,
    // ensuring we never truncate in-place (which protects hard-linked files).
    static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let temp_name = format!(
        ".tmp.{}.{}.{}.tmp",
        filename.to_string_lossy(),
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let c_temp = CString::new(temp_name.as_bytes()).map_err(|_| {
        WriteOutputError::WriteFile(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid temp filename",
        ))
    })?;

    let temp_fd = unsafe {
        libc::openat(
            guard.0,
            c_temp.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o666,
        )
    };

    if temp_fd < 0 {
        return Err(WriteOutputError::WriteFile(std::io::Error::last_os_error()));
    }

    let mut temp_file = unsafe { std::fs::File::from_raw_fd(temp_fd) };
    if let Err(e) = std::io::Write::write_all(&mut temp_file, data) {
        unsafe { libc::unlinkat(guard.0, c_temp.as_ptr(), 0) };
        return Err(WriteOutputError::WriteFile(e));
    }
    drop(temp_file);

    // If destination already exists as a symlink, refuse before renaming
    let c_file = CString::new(filename.as_bytes()).map_err(|_| {
        unsafe { libc::unlinkat(guard.0, c_temp.as_ptr(), 0) };
        WriteOutputError::WriteFile(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid filename",
        ))
    })?;

    let check_fd = unsafe {
        libc::openat(
            guard.0,
            c_file.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if check_fd >= 0 {
        unsafe { libc::close(check_fd) };
    } else {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ELOOP) {
            unsafe { libc::unlinkat(guard.0, c_temp.as_ptr(), 0) };
            return Err(WriteOutputError::SymlinkDestination);
        }
    }

    let rename_res = unsafe { libc::renameat(guard.0, c_temp.as_ptr(), guard.0, c_file.as_ptr()) };

    if rename_res != 0 {
        let err = std::io::Error::last_os_error();
        unsafe { libc::unlinkat(guard.0, c_temp.as_ptr(), 0) };
        return Err(WriteOutputError::WriteFile(err));
    }

    Ok(())
}

#[cfg(not(unix))]
fn safe_write_output(path: &Path, data: &[u8]) -> Result<(), WriteOutputError> {
    static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    check_parent_symlinks(path)?;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent)
            .map_err(|e| WriteOutputError::CreateDir(parent.to_path_buf(), e))?;
    }

    check_parent_symlinks(path)?;

    if fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(WriteOutputError::SymlinkDestination);
    }

    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let temp_name = format!(
        ".tmp.{}.{}.{}.tmp",
        filename,
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let temp_path = parent.join(&temp_name);

    #[cfg(windows)]
    {
        use std::io::Write;
        use std::os::windows::fs::OpenOptionsExt;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;

        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        opts.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);

        let mut temp_file = match opts.open(&temp_path) {
            Ok(f) => f,
            Err(e) => return Err(WriteOutputError::WriteFile(e)),
        };

        if let Err(e) = temp_file.write_all(data) {
            let _ = fs::remove_file(&temp_path);
            return Err(WriteOutputError::WriteFile(e));
        }
    }

    #[cfg(not(windows))]
    {
        use std::io::Write;
        let mut temp_file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(WriteOutputError::WriteFile)?;

        if let Err(e) = temp_file.write_all(data) {
            let _ = fs::remove_file(&temp_path);
            return Err(WriteOutputError::WriteFile(e));
        }
    }

    // Re-check destination is not a symlink/reparse point before atomic rename
    if fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        let _ = fs::remove_file(&temp_path);
        return Err(WriteOutputError::SymlinkDestination);
    }

    if let Err(e) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(WriteOutputError::WriteFile(e));
    }

    Ok(())
}

fn get_invoked_name(args: &[String]) -> &str {
    args.first()
        .and_then(|p| Path::new(p).file_name()?.to_str())
        .map(|s| if s == "mpix" { "mpix" } else { "morfpix" })
        .unwrap_or("morfpix")
}

fn run_version() {
    println!(
        "morfpix {} ({} {})",
        env!("CARGO_PKG_VERSION"),
        env::consts::ARCH,
        env::consts::OS
    );
}

pub fn run() {
    let styler = Styler::new();
    let args: Vec<String> = env::args().collect();
    let cmd_name = get_invoked_name(&args);

    if args.len() < 2 {
        print_help(&styler, cmd_name);
        return;
    }

    let cli = match parse_args(&args, &styler, cmd_name) {
        Some(cli) => cli,
        None => return,
    };

    let files = collect_inputs(&cli.inputs, cli.output.as_deref(), &styler);
    if files.is_empty() {
        println!("No supported image files found.");
        return;
    }

    let batch = files.len() > 1 || cli.inputs.iter().any(|path| path.is_dir());

    if batch {
        if let Some(output_dir) = &cli.output {
            if !is_directory_target(output_dir) {
                eprintln!(
                    "{} Output path '{}' must be a directory when processing multiple files or a directory",
                    styler.red("Error:"),
                    output_dir.display()
                );
                std::process::exit(1);
            }
            if fs::symlink_metadata(output_dir)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
                || check_parent_symlinks(output_dir).is_err()
            {
                eprintln!(
                    "{} Refusing to write to symlink destination '{}'",
                    styler.red("Error:"),
                    output_dir.display()
                );
                std::process::exit(1);
            }
            if let Err(error) = fs::create_dir_all(output_dir) {
                eprintln!(
                    "{} Failed to create output directory '{}': {}",
                    styler.red("Error:"),
                    output_dir.display(),
                    error
                );
                std::process::exit(1);
            }
            if check_parent_symlinks(output_dir).is_err() {
                eprintln!(
                    "{} Refusing to write to symlink destination '{}'",
                    styler.red("Error:"),
                    output_dir.display()
                );
                std::process::exit(1);
            }
        }
    }

    let start = Instant::now();
    let max_threads = std::thread::available_parallelism()
        .map(|t| t.get())
        .unwrap_or(1);
    let jobs = match cli.jobs {
        Some(0) => max_threads,
        Some(j) => j.min(max_threads).max(1),
        None => {
            if batch {
                let total_bytes: u64 = files
                    .iter()
                    .filter_map(|f| fs::metadata(&f.path).ok().map(|m| m.len()))
                    .sum();
                let avg_size = if !files.is_empty() {
                    total_bytes / files.len() as u64
                } else {
                    0
                };
                if avg_size > 50 * 1024 * 1024 {
                    max_threads.min(2)
                } else if avg_size > 20 * 1024 * 1024 {
                    max_threads.min(3)
                } else {
                    max_threads.min(4)
                }
            } else {
                1
            }
        }
    };

    let mut conversion_options = cli.options.clone();
    if jobs > 1 {
        // Hierarchical CPU budgeting: divide available cores across parallel jobs
        // to prevent nested thread explosion (jobs * encoder_threads)
        let per_job_share = (max_threads / jobs).max(1);
        if cli.explicit_threads {
            conversion_options.avif.threads = conversion_options.avif.threads.min(per_job_share);
            conversion_options.jxl.threads = conversion_options.jxl.threads.min(per_job_share);
        } else {
            conversion_options.avif.threads = per_job_share;
            conversion_options.jxl.threads = per_job_share;
        }
    }

    let memory_budget = MemoryBudgetCoordinator::new(DEFAULT_BATCH_MEMORY_BUDGET);

    let state = std::sync::Mutex::new((
        HashSet::<PathBuf>::new(),
        Vec::<BatchItemResult>::new(),
        0usize,
        0usize,
    ));

    let process_file = |input: &InputFile| {
        let metadata = match fs::metadata(&input.path) {
            Ok(meta) => meta,
            Err(error) => {
                let mut guard = state.lock().unwrap();
                guard.3 += 1;
                eprintln!(
                    "  [{}/{}] {}: {}",
                    guard.3,
                    files.len(),
                    input.path.display(),
                    styler.red(&error.to_string())
                );
                guard.2 += 1;
                return;
            }
        };

        if metadata.len() > MAX_INPUT_FILE_SIZE as u64 {
            let mut guard = state.lock().unwrap();
            guard.3 += 1;
            eprintln!(
                "  [{}/{}] {}: {} File size ({} MB) exceeds maximum allowed limit ({} MB)",
                guard.3,
                files.len(),
                input.path.display(),
                styler.red("Error:"),
                metadata.len() / (1024 * 1024),
                MAX_INPUT_FILE_SIZE / (1024 * 1024)
            );
            guard.2 += 1;
            return;
        }

        let file_size = metadata.len() as usize;

        // Parse dimensions from small header buffer (first 64 KiB) to reserve working set upfront
        let header_estimate = {
            let mut header_buf = [0u8; 64 * 1024];
            if let Ok(mut f) = fs::File::open(&input.path) {
                use std::io::Read;
                let n = f.read(&mut header_buf).unwrap_or(0);
                estimate_memory_from_header(&header_buf[..n], file_size)
            } else {
                None
            }
        };

        let initial_estimate = header_estimate.unwrap_or(file_size);
        if initial_estimate > MAX_REASONABLE_MEMORY_ESTIMATE {
            let mut guard = state.lock().unwrap();
            guard.3 += 1;
            eprintln!(
                "  [{}/{}] {}: {} Memory estimate ({} MB) exceeds maximum allowed limit ({} MB)",
                guard.3,
                files.len(),
                input.path.display(),
                styler.red("Error:"),
                initial_estimate / (1024 * 1024),
                MAX_REASONABLE_MEMORY_ESTIMATE / (1024 * 1024)
            );
            guard.2 += 1;
            return;
        }

        let mut permit = memory_budget.acquire(initial_estimate);

        let mut data = match fs::read(&input.path) {
            Ok(data) => data,
            Err(error) => {
                drop(permit);
                let mut guard = state.lock().unwrap();
                guard.3 += 1;
                eprintln!(
                    "  [{}/{}] {}: {}",
                    guard.3,
                    files.len(),
                    input.path.display(),
                    styler.red(&error.to_string())
                );
                guard.2 += 1;
                return;
            }
        };

        let memory_estimate = estimate_memory_usage_with_options(&data, &conversion_options);
        if memory_estimate > MAX_REASONABLE_MEMORY_ESTIMATE {
            drop(permit);
            let mut guard = state.lock().unwrap();
            guard.3 += 1;
            eprintln!(
                "  [{}/{}] {}: {} Memory estimate ({} MB) exceeds maximum allowed limit ({} MB)",
                guard.3,
                files.len(),
                input.path.display(),
                styler.red("Error:"),
                memory_estimate / (1024 * 1024),
                MAX_REASONABLE_MEMORY_ESTIMATE / (1024 * 1024)
            );
            guard.2 += 1;
            return;
        }

        if memory_estimate > permit.amount() && !permit.try_resize(memory_estimate) {
            // To avoid holding read permits while waiting to expand (which causes circular wait),
            // release read permit and data, wait for required budget, then reread.
            drop(data);
            drop(permit);

            let retry_permit = memory_budget.acquire(memory_estimate);
            let reread_data = match fs::read(&input.path) {
                Ok(d) => d,
                Err(error) => {
                    drop(retry_permit);
                    let mut guard = state.lock().unwrap();
                    guard.3 += 1;
                    eprintln!(
                        "  [{}/{}] {}: {}",
                        guard.3,
                        files.len(),
                        input.path.display(),
                        styler.red(&error.to_string())
                    );
                    guard.2 += 1;
                    return;
                }
            };
            permit = retry_permit;
            data = reread_data;
        }

        let (output, result) = match convert(&data, &conversion_options) {
            Ok(output) => output,
            Err(error) => {
                let mut guard = state.lock().unwrap();
                guard.3 += 1;
                eprintln!(
                    "  [{}/{}] {}: {}",
                    guard.3,
                    files.len(),
                    input.path.display(),
                    styler.red(&error)
                );
                guard.2 += 1;
                return;
            }
        };

        let out_path = output_path(input, cli.output.as_deref(), result.output_format, batch);

        let out_path = {
            let mut guard = state.lock().unwrap();
            if same_path(&input.path, &out_path) {
                guard.3 += 1;
                eprintln!(
                    "{} Refusing to overwrite input file '{}'",
                    styler.red("Error:"),
                    input.path.display()
                );
                guard.2 += 1;
                return;
            }
            unique_output_path(out_path, &mut guard.0)
        };

        if let Err(error) = safe_write_output(&out_path, &output) {
            let mut guard = state.lock().unwrap();
            guard.3 += 1;
            match error {
                WriteOutputError::SymlinkDestination => {
                    eprintln!(
                        "{} Refusing to write to symlink destination '{}'",
                        styler.red("Error:"),
                        out_path.display()
                    );
                }
                WriteOutputError::CreateDir(dir, err) => {
                    eprintln!(
                        "{} Failed to create directory '{}': {}",
                        styler.red("Error:"),
                        dir.display(),
                        err
                    );
                }
                WriteOutputError::WriteFile(err) => {
                    eprintln!(
                        "{} Failed to write '{}': {}",
                        styler.red("Error:"),
                        out_path.display(),
                        err
                    );
                }
            }
            guard.2 += 1;
            return;
        }

        let mut guard = state.lock().unwrap();
        guard.3 += 1;
        if batch {
            println!(
                "  [{}/{}] {} -> {} ({}, {})",
                guard.3,
                files.len(),
                input.path.display(),
                out_path.display(),
                format_bytes(result.output_size_bytes),
                ui::format_change(&result, &styler)
            );
        } else {
            print_result(&input.path, &out_path, &result, &styler);
        }

        guard.1.push(BatchItemResult {
            input_size: result.input_size_bytes,
            output_size: result.output_size_bytes,
        });

        // The memory permit is held through output writing and dropped here so each active job stays accounted for
        drop(permit);
    };

    if jobs > 1 && batch {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
            .build()
            .expect("Failed to create thread pool");
        pool.install(|| {
            use rayon::prelude::*;
            files.par_iter().for_each(process_file);
        });
    } else {
        for input in &files {
            process_file(input);
        }
    }

    let (_, results, failures, _) = state.into_inner().unwrap();

    if batch {
        print_batch_summary(
            &results,
            failures,
            start.elapsed().as_millis() as u64,
            &styler,
        );
    }

    if failures > 0 {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_memory_budget_bounds_concurrent_reads() {
        let coordinator = MemoryBudgetCoordinator::new(100);

        // Acquire 60 bytes for reading file 1
        let mut permit1 = coordinator.acquire(60);
        assert_eq!(*coordinator.state.lock().unwrap(), 60);

        // Try resize permit1 to 90 bytes (fits within 100)
        assert!(permit1.try_resize(90));
        assert_eq!(*coordinator.state.lock().unwrap(), 90);

        // A second thread tries to acquire 30 bytes for reading (total would be 120 > 100)
        let acquired = Arc::new(AtomicUsize::new(0));
        let acquired_clone = Arc::clone(&acquired);

        std::thread::scope(|s| {
            s.spawn(|| {
                let _permit2 = coordinator.acquire(30);
                acquired_clone.store(1, Ordering::SeqCst);
            });

            // Verify that thread 2 is blocked waiting for budget
            thread::sleep(Duration::from_millis(50));
            assert_eq!(acquired.load(Ordering::SeqCst), 0);

            // Drop permit1 to release the 90 bytes
            drop(permit1);
        });

        assert_eq!(acquired.load(Ordering::SeqCst), 1);
        assert_eq!(*coordinator.state.lock().unwrap(), 0);
    }

    #[test]
    fn test_single_job_exceeding_budget_proceeds_alone() {
        let coordinator = MemoryBudgetCoordinator::new(100);

        // A single file metadata size is 80
        let mut permit = coordinator.acquire(80);

        // Its decoded working memory estimate is 150 (> 100 budget)
        // Since no other jobs hold a permit (other_allocated == 0), it proceeds alone
        assert!(permit.try_resize(150));
        assert_eq!(*coordinator.state.lock().unwrap(), 150);

        drop(permit);
        assert_eq!(*coordinator.state.lock().unwrap(), 0);
    }

    #[test]
    fn test_try_resize_fails_and_releases_read_permit_to_avoid_deadlock() {
        // 100 budget, two jobs each acquire 50
        let coordinator = MemoryBudgetCoordinator::new(100);
        let mut permit1 = coordinator.acquire(50);
        let mut permit2 = coordinator.acquire(50);

        // permit1 wants to resize to 80, but other_allocated = 50 (80 + 50 = 130 > 100)
        // try_resize returns false!
        assert!(!permit1.try_resize(80));

        // To avoid circular wait deadlock, job 1 drops its read permit before waiting for 80
        drop(permit1);

        // Now other_allocated for job 2 is 0, so permit2.try_resize(80) succeeds!
        assert!(permit2.try_resize(80));
        assert_eq!(*coordinator.state.lock().unwrap(), 80);

        drop(permit2);
        assert_eq!(*coordinator.state.lock().unwrap(), 0);
    }

    #[test]
    fn test_header_estimate_reserves_upfront_preventing_overcommit() {
        let coordinator = MemoryBudgetCoordinator::new(500);

        // A file header indicates 400 bytes working memory
        let permit1 = coordinator.acquire(400);

        // A second job needing 200 bytes waits upfront before reading (400 + 200 > 500)
        let acquired2 = Arc::new(AtomicUsize::new(0));
        let a2 = Arc::clone(&acquired2);

        std::thread::scope(|s| {
            s.spawn(|| {
                let _permit2 = coordinator.acquire(200);
                a2.store(1, Ordering::SeqCst);
            });

            thread::sleep(Duration::from_millis(50));
            assert_eq!(acquired2.load(Ordering::SeqCst), 0);

            drop(permit1);
        });

        assert_eq!(acquired2.load(Ordering::SeqCst), 1);
        assert_eq!(*coordinator.state.lock().unwrap(), 0);
    }

    #[test]
    fn test_coordinator_overflow_resilience() {
        let coordinator = MemoryBudgetCoordinator::new(100);
        let permit1 = coordinator.acquire(50);
        assert_eq!(*coordinator.state.lock().unwrap(), 50);

        // A second thread tries to acquire usize::MAX while permit1 is held
        let acquired = Arc::new(AtomicUsize::new(0));
        let acquired_clone = Arc::clone(&acquired);

        std::thread::scope(|s| {
            s.spawn(|| {
                // Should safely wait without integer overflow panic
                let _permit2 = coordinator.acquire(usize::MAX);
                acquired_clone.store(1, Ordering::SeqCst);
            });

            thread::sleep(Duration::from_millis(50));
            assert_eq!(acquired.load(Ordering::SeqCst), 0);

            // Dropping permit1 unblocks the waiter
            drop(permit1);
        });

        assert_eq!(acquired.load(Ordering::SeqCst), 1);
        assert_eq!(*coordinator.state.lock().unwrap(), 0);
    }

    #[test]
    fn test_coordinator_try_resize_overflow_resilience() {
        let coordinator = MemoryBudgetCoordinator::new(100);
        let mut permit1 = coordinator.acquire(50);
        let permit2 = coordinator.acquire(50);

        // While permit2 is held, permit1 resizing to usize::MAX fails safely without overflow
        assert!(!permit1.try_resize(usize::MAX));

        // When permit2 is dropped, permit1 is the sole job and can resize alone
        drop(permit2);
        assert!(permit1.try_resize(usize::MAX));
        assert_eq!(*coordinator.state.lock().unwrap(), usize::MAX);

        drop(permit1);
        assert_eq!(*coordinator.state.lock().unwrap(), 0);
    }
}
