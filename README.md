<div align="center">

# morfpix

A small command-line image compression and format conversion tool written in Rust.

**Repository:** [https://github.com/sampadmahanty/morfpix](https://github.com/sampadmahanty/morfpix)

</div>

## Scope

- Compress and convert images between JPEG, PNG, WebP, GIF, BMP, AVIF, and JPEG XL (JXL).
- Process a single file or a directory of images (batch mode).
- Modular design with core image processing logic isolated in `crates/engine`.

## Installation

Pre-compiled standalone binaries are available for **macOS** and **Windows** on the [Releases](https://github.com/sampadmahanty/morfpix/releases) page. No Rust toolchain or external dependencies required.

### macOS (Apple Silicon)

1. Download `morfpix-macos-arm64.tar.gz` from [Releases](https://github.com/sampadmahanty/morfpix/releases).
2. Extract and move the binaries to your `PATH` (e.g. `/usr/local/bin`):
   ```bash
   tar -xzf morfpix-macos-arm64.tar.gz
   sudo mv morfpix mpix /usr/local/bin/
   ```

### Windows

1. Download `morfpix-windows-x86_64.zip` from [Releases](https://github.com/sampadmahanty/morfpix/releases).
2. Extract `morfpix.exe` and `mpix.exe`.
3. Add the extracted directory to your `PATH` (or run them directly from PowerShell / Command Prompt).

### Build from Source (Optional)

Requires Rust 1.89 or newer.

```bash
cargo build --release
```

The binaries are written to `target/release/morfpix` and `target/release/mpix`.

## Usage

```text
morfpix [OPTIONS] <FILE...>
morfpix [OPTIONS] <DIRECTORY>

# Short command alias
mpix [OPTIONS] <FILE...>
mpix [OPTIONS] <DIRECTORY>
```

### Options

| Option                      | Description                                                                               |
| --------------------------- | ----------------------------------------------------------------------------------------- |
| `-o, --output <PATH>`     | Output file for a single input, or output directory for batch mode                        |
| `-f, --format <FORMAT>`   | Target format: `jpeg`, `png`, `webp`, `gif`, `bmp`, `avif`, `jxl`           |
| `-q, --quality <1-100>`   | Quality for JPEG, WebP, and AVIF (default: 80)                                            |
| `--baseline`              | Encode baseline (sequential) JPEG instead of progressive (default: false)                  |
| `--subsample <MODE>`      | Chroma subsampling for JPEG: `444`, `422`, `420`, `440`, `auto` (default: `auto`)          |
| `--smoothing <0-100>`     | Smoothing factor for JPEG (default: 0)                                                    |
| `-s, --speed <1-10>`      | AVIF encoding speed (default: 4)                                                          |
| `--alpha-quality <1-100>` | AVIF alpha quality (default: same as `--quality`)                                       |
| `--bit-depth <MODE>`      | AVIF bit depth: `8`, `10`, or `auto` (default: `auto`)                            |
| `--color <MODEL>`         | AVIF color model: `ycbcr` or `rgb` (default: `ycbcr`)                               |
| `-t, --threads <N>`       | Encoding threads for AVIF/JXL, `0` = auto (capped at 4 cores) (default: 0)                 |
| `--alpha-mode <MODE>`     | AVIF alpha mode: `clean`, `dirty`, or `premultiplied` (default: `clean`)          |
| `--optimize`              | Optimize PNG output using oxipng preset 2 (default: false)                                 |
| `--png-level <0-6>`       | PNG oxipng optimization level: `0` (none) to `6` (max) (default: `2`)                      |
| `--interlace`             | Enable PNG Adam7 interlacing (default: false)                                              |
| `--lossless`              | WebP lossless encoding mode (default: false)                                              |
| `-m, --method <0-6>`      | WebP compression method/effort: `0` (fastest) to `6` (smallest file) (default: `4`)        |
| `--exact`                 | Preserve RGB values under transparent pixels in WebP (default: false)                      |
| `-e, --effort <1-9>`      | JXL encoding effort: `1` (fastest) to `9` (smallest file) (default: `4`)                   |
| `--gif-repeat <MODE>`     | GIF repeat: `infinite`, `none`, or loop count `1`-`65535` (default: `infinite`)            |
| `--gif-speed <1-30>`      | GIF NeuQuant color quantization speed: `1` (highest quality) to `30` (fastest) (default: `10`) |
| `--bmp-bpp <MODE>`        | BMP bits per pixel: `8` (grayscale), `24` (RGB), `32` (RGBA), or `auto` (default: `auto`)  |
| `-j, --jobs <N>`          | Concurrent jobs for batch mode, `0` = all cores (default: auto up to 4)                    |
| `-h, --help`              | Print help                                                                                |
| `-V, --version`           | Print version                                                                             |

When `--format` is omitted, the input format is kept. When `--output` is omitted, files are written next to their source as `<stem>.<new-extension>`, or `<stem>-compressed.<extension>` if the format is unchanged.

### Behavior notes

- EXIF orientation is applied before encoding, so rotated photos stay upright.
- Embedded ICC profiles are color-managed to sRGB during decoding, so the output uses standard sRGB color metadata.
- Images are re-encoded; EXIF and other metadata are not copied to the output.
- `--quality` affects JPEG, WebP, and AVIF only. PNG, GIF, BMP, and JXL ignore it.
- PNG-specific options (`--optimize`, `--png-level`, `--interlace`) optimize PNG output via oxipng and enable Adam7 interlacing.
- JPEG-specific options (`--baseline`, `--subsample`, `--smoothing`) apply only when encoding JPEG via MozJPEG. By default, MozJPEG creates progressive JPEGs with 4:2:0 subsampling.
- AVIF-specific options (`--speed`, `--alpha-quality`, `--bit-depth`, `--color`, `--threads`, `--alpha-mode`) apply only when encoding AVIF.
- WebP-specific options (`--lossless`, `-m, --method`, `--exact`) apply only when encoding WebP. In lossless mode, `--quality` controls compression effort rather than visual lossiness.
- JXL options (`-e, --effort <1-9>`, `-t, --threads`) control compression effort and threading. JXL output is lossless modular mode, so `--quality` has no effect on it.
- GIF options (`--gif-repeat`, `--gif-speed`) control repetition looping behavior and NeuQuant color quantization speed/quality.
- BMP options (`--bmp-bpp`) control bits per pixel: `8` (grayscale palette), `24` (RGB), `32` (RGBA with alpha), or `auto` (preserves alpha in 32-bit if present, otherwise 24-bit RGB or 8-bit grayscale).
- Animated GIF, WebP, AVIF, and JXL inputs are converted from their first frame only.
- Converting an image with transparency to JPEG drops the alpha channel. BMP preserves alpha by default in 32-bit mode unless 24-bit is forced via `--bmp-bpp 24`.
- In batch mode, prefer a separate output directory. Using the input directory as the output directory can cause previously generated files to be picked up on a later run.
- Directory scanning does not follow symlinked directories by default to prevent unexpected traversal outside selected folders.
- Output creation refuses to write to pre-existing symlinks to prevent accidental overwrites elsewhere.

### Codecs

- JPEG uses MozJPEG, PNG uses `image-rs` (with optional oxiPNG optimization and Adam7 interlacing), and WebP uses libwebp.
- AVIF encoding uses `ravif` (rav1e-based); AVIF decoding uses `oxideav-avif`.
- JXL uses `zune-jpegxl` for lossless encoding and `jxl-oxide` for decoding.
- The AVIF and JXL backends are pure Rust and do not require CMake, NASM, or Meson.

### Examples

```bash
# Convert one image to WebP
morfpix photo.png -f webp -q 80

# Convert one image to lossless WebP with maximum compression effort
morfpix photo.png -f webp --lossless -m 6

# Convert one image to AVIF
morfpix photo.png -f avif -q 60 -s 4 --bit-depth 10

# Convert one image to lossless JPEG XL with custom effort
morfpix photo.png -f jxl -e 7

# Convert one image to baseline JPEG with 4:4:4 chroma subsampling
morfpix photo.png -f jpeg -q 85 --baseline --subsample 444

# Convert a JPEG to PNG with high oxipng optimization and Adam7 interlacing
morfpix photo.jpg -f png --png-level 4 --interlace -o photo.png

# Convert an image to GIF with infinite loop and high-quality quantization
morfpix photo.png -f gif --repeat infinite --gif-speed 5

# Convert an image to 24-bit RGB BMP (stripping alpha for legacy tools)
morfpix photo.png -f bmp --bmp-bpp 24

# Compress every image in a directory into another directory
morfpix ./images -f webp -q 82 -o ./compressed
```

## Testing

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## License

Dual-licensed under either of:

- [Apache License, Version 2.0](LICENSE-APACHE) ([http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
- [MIT License](LICENSE-MIT) ([http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))

at your option.
