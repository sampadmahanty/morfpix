use std::path::{Path, PathBuf};
use std::process::Command;

fn get_bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_morfpix"))
}

fn get_short_bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mpix"))
}

fn get_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn create_test_animated_gif(
    width: u32,
    height: u32,
    num_frames: usize,
) -> Result<Vec<u8>, image::ImageError> {
    let mut buffer = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut buffer);
        encoder.set_repeat(image::codecs::gif::Repeat::Infinite)?;
        for _ in 0..num_frames {
            let frame = image::Frame::new(image::RgbaImage::new(width, height));
            encoder.encode_frame(frame)?;
        }
    }
    Ok(buffer)
}

#[test]
fn test_cli_short_cmd_mpix() {
    let output = Command::new(get_short_bin_path())
        .arg("version")
        .output()
        .expect("Failed to execute mpix version");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("morfpix 1.1.0"));

    let help_output = Command::new(get_short_bin_path())
        .arg("--help")
        .output()
        .expect("Failed to execute mpix --help");

    assert!(help_output.status.success());
    let help_stdout = String::from_utf8_lossy(&help_output.stdout);
    assert!(help_stdout.contains("morfpix"));
    assert!(help_stdout.contains("alias: mpix"));
}

#[test]
fn test_cli_help() {
    let output = Command::new(get_bin_path())
        .arg("--help")
        .output()
        .expect("Failed to execute morfpix --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("morfpix"));
    assert!(stdout.contains("USAGE:"));
    assert!(stdout.contains("OPTIONS:"));
    assert!(stdout.contains("avif"));
    assert!(stdout.contains("jxl"));
    assert!(stdout.contains("--speed"));
    assert!(stdout.contains("--alpha-quality"));
    assert!(stdout.contains("--bit-depth"));
    assert!(stdout.contains("--color"));
    assert!(stdout.contains("--threads"));
    assert!(stdout.contains("--alpha-mode"));
    assert!(stdout.contains("--optimize"));
    assert!(stdout.contains("--png-level"));
    assert!(stdout.contains("--interlace"));
    assert!(stdout.contains("--lossless"));
    assert!(stdout.contains("--method"));
    assert!(stdout.contains("--exact"));
    assert!(stdout.contains("--effort"));
    assert!(stdout.contains("--gif-repeat"));
    assert!(stdout.contains("--gif-speed"));
    assert!(stdout.contains("--bmp-bpp"));
    assert!(stdout.contains("--baseline"));
    assert!(stdout.contains("--subsample"));
    assert!(stdout.contains("--smoothing"));
    assert!(stdout.contains("--width"));
    assert!(stdout.contains("--height"));
    assert!(stdout.contains("--filter"));
    assert!(stdout.contains("--jobs"));
}

#[test]
fn test_cli_version() {
    let output = Command::new(get_bin_path())
        .arg("version")
        .output()
        .expect("Failed to execute morfpix version");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("morfpix 1.1.0"));
}

#[test]
fn test_cli_convert_single_file() {
    let fixture = get_fixture_path("sample-photo.png");
    assert!(fixture.exists(), "Fixture missing: {:?}", fixture);

    let out_dir = std::env::temp_dir().join("morfpix_test_single");
    let _ = std::fs::create_dir_all(&out_dir);
    let out_file = out_dir.join("sample.webp");
    let _ = std::fs::remove_file(&out_file);

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("webp")
        .arg("-q")
        .arg("80")
        .arg("-o")
        .arg(&out_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(output.status.success());
    assert!(out_file.exists(), "Output file was not generated");
    assert!(std::fs::metadata(&out_file).unwrap().len() > 0);

    let _ = std::fs::remove_file(out_file);
}

#[test]
fn test_cli_batch_directory() {
    let fixtures = get_fixture_path("");
    assert!(
        fixtures.exists(),
        "Fixture directory missing: {:?}",
        fixtures
    );

    let out_dir = std::env::temp_dir().join("morfpix_test_batch");
    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::create_dir_all(&out_dir);

    let output = Command::new(get_bin_path())
        .arg(&fixtures)
        .arg("-f")
        .arg("webp")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("Failed to execute morfpix batch");

    assert!(output.status.success());
    let count = std::fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "webp"))
        .count();
    assert!(
        count >= 2,
        "Expected at least 2 WebP outputs, found {}",
        count
    );

    let _ = std::fs::remove_dir_all(out_dir);
}

#[test]
fn test_cli_converts_to_modern_formats() {
    let fixture = get_fixture_path("sample-photo.png");
    let out_dir = std::env::temp_dir().join("morfpix_test_modern");
    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::create_dir_all(&out_dir);

    for format in ["avif", "jxl"] {
        let out_file = out_dir.join(format!("sample.{format}"));
        let output = Command::new(get_bin_path())
            .arg(&fixture)
            .arg("-f")
            .arg(format)
            .arg("-o")
            .arg(&out_file)
            .output()
            .expect("Failed to execute morfpix");

        assert!(
            output.status.success(),
            "morfpix -f {} failed: {}",
            format,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            out_file.exists(),
            "Output file was not generated for {}",
            format
        );
        assert!(std::fs::metadata(&out_file).unwrap().len() > 0);
    }

    let _ = std::fs::remove_dir_all(out_dir);
}

#[test]
fn test_cli_avif_options() {
    let fixture = get_fixture_path("sample-photo.png");
    let out_file = std::env::temp_dir().join("morfpix_test_avif_options.avif");
    let _ = std::fs::remove_file(&out_file);

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("avif")
        .arg("-q")
        .arg("60")
        .arg("-s")
        .arg("10")
        .arg("--alpha-quality")
        .arg("40")
        .arg("--bit-depth")
        .arg("8")
        .arg("--color")
        .arg("rgb")
        .arg("-t")
        .arg("1")
        .arg("--alpha-mode")
        .arg("dirty")
        .arg("-o")
        .arg(&out_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "morfpix AVIF options failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_file.exists(), "AVIF output file was not generated");
    assert!(std::fs::metadata(&out_file).unwrap().len() > 0);

    let _ = std::fs::remove_file(out_file);
}

#[test]
fn test_cli_rejects_unsupported_format() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("tiff")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unsupported format"));
}

#[test]
fn test_cli_refuses_to_overwrite_input_file() {
    let fixture = get_fixture_path("sample-photo.png");
    let temp_input = std::env::temp_dir().join("morfpix_test_no_overwrite.png");
    std::fs::copy(&fixture, &temp_input).expect("Failed to copy fixture");

    let output = Command::new(get_bin_path())
        .arg(&temp_input)
        .arg("-o")
        .arg(&temp_input)
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Refusing to overwrite input file"),
        "stderr was: {}",
        stderr
    );

    let _ = std::fs::remove_file(temp_input);
}

#[test]
fn test_cli_accepts_format_with_leading_dot() {
    let fixture = get_fixture_path("sample-photo.png");
    let out_file = std::env::temp_dir().join("morfpix_test_leading_dot.webp");
    let _ = std::fs::remove_file(&out_file);

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg(".webp")
        .arg("-o")
        .arg(&out_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "morfpix -f .webp failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_file.exists());
    let _ = std::fs::remove_file(out_file);
}

#[test]
fn test_cli_resolves_uncreated_directory_with_dots_or_trailing_slash() {
    let fixture = get_fixture_path("sample-photo.png");

    // Case 1: Target directory with trailing slash that does not exist yet (e.g. .v1/)
    let test_dir1 = std::env::temp_dir().join("morfpix_test_dir_resolution.v1");
    let _ = std::fs::remove_dir_all(&test_dir1);
    let target_arg1 = format!("{}/", test_dir1.display());

    let output1 = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("webp")
        .arg("-o")
        .arg(&target_arg1)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output1.status.success(),
        "morfpix failed on directory with trailing slash: {}",
        String::from_utf8_lossy(&output1.stderr)
    );
    let expected_file1 = test_dir1.join("sample-photo.webp");
    assert!(
        expected_file1.exists(),
        "Expected converted file inside target directory at {:?}",
        expected_file1
    );
    let _ = std::fs::remove_dir_all(&test_dir1);

    // Case 2: Target directory WITHOUT trailing slash but with non-image dot in name that does not exist yet (.out)
    let test_dir2 = std::env::temp_dir().join("morfpix_test_dir_resolution.out");
    let _ = std::fs::remove_dir_all(&test_dir2);

    let output2 = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("webp")
        .arg("-o")
        .arg(&test_dir2)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output2.status.success(),
        "morfpix failed on directory with dot extension: {}",
        String::from_utf8_lossy(&output2.stderr)
    );
    let expected_file2 = test_dir2.join("sample-photo.webp");
    assert!(
        expected_file2.exists(),
        "Expected converted file inside target directory at {:?}",
        expected_file2
    );
    let _ = std::fs::remove_dir_all(&test_dir2);
}

#[test]
fn test_cli_batch_rejects_file_output_path() {
    let fixture1 = get_fixture_path("sample-photo.png");
    let fixture2 = get_fixture_path("phone-photo.jpeg");

    let out_file = std::env::temp_dir().join("morfpix_test_batch_file.webp");
    let _ = std::fs::remove_file(&out_file);

    let output = Command::new(get_bin_path())
        .arg(&fixture1)
        .arg(&fixture2)
        .arg("-o")
        .arg(&out_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("must be a directory"),
        "stderr was: {}",
        stderr
    );
    assert!(!out_file.exists());
}

#[test]
fn test_cli_batch_rejects_existing_file_as_output() {
    let fixture1 = get_fixture_path("sample-photo.png");
    let fixture2 = get_fixture_path("phone-photo.jpeg");

    let existing_file = std::env::temp_dir().join("morfpix_test_existing_file.txt");
    std::fs::write(&existing_file, b"dummy content").expect("Failed to create file");

    let output = Command::new(get_bin_path())
        .arg(&fixture1)
        .arg(&fixture2)
        .arg("-o")
        .arg(&existing_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("must be a directory"),
        "stderr was: {}",
        stderr
    );

    let _ = std::fs::remove_file(existing_file);
}

#[test]
#[cfg(unix)]
fn test_cli_handles_circular_symlinks_without_infinite_recursion() {
    let fixture = get_fixture_path("sample-photo.png");
    let test_dir = std::env::temp_dir().join("morfpix_test_circular_symlink");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).expect("Failed to create test dir");

    // Copy an image into the directory
    std::fs::copy(&fixture, test_dir.join("sample.png")).expect("Failed to copy fixture");

    // Create a subdirectory with a circular symlink pointing back to test_dir
    let sub_dir = test_dir.join("sub");
    std::fs::create_dir_all(&sub_dir).expect("Failed to create sub dir");
    let circular_link = sub_dir.join("loop_to_parent");
    std::os::unix::fs::symlink(&test_dir, &circular_link)
        .expect("Failed to create circular symlink");

    let out_dir = std::env::temp_dir().join("morfpix_test_circular_out");
    let _ = std::fs::remove_dir_all(&out_dir);

    let output = Command::new(get_bin_path())
        .arg(&test_dir)
        .arg("-f")
        .arg("webp")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "morfpix failed on circular symlinks: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_dir.join("sample.webp").exists());

    let _ = std::fs::remove_dir_all(&test_dir);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_rejects_oversized_file() {
    let temp_file = std::env::temp_dir().join("morfpix_oversized_test.png");
    let file = std::fs::File::create(&temp_file).expect("Failed to create temp file");
    file.set_len(300 * 1024 * 1024)
        .expect("Failed to set file length");

    let output = Command::new(get_bin_path())
        .arg(&temp_file)
        .arg("-f")
        .arg("webp")
        .output()
        .expect("Failed to execute morfpix");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("exceeds maximum allowed limit"),
        "stderr was: {}",
        stderr
    );

    let _ = std::fs::remove_file(&temp_file);
}

#[test]
fn test_cli_png_optimize_flag() {
    let input = get_fixture_path("sample-photo.png");
    let out_dir = std::env::temp_dir().join("morfpix_test_png_optimize");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).unwrap();
    let out_path = out_dir.join("sample.png");

    let output = Command::new(get_bin_path())
        .arg(&input)
        .arg("-f")
        .arg("png")
        .arg("--optimize")
        .arg("-o")
        .arg(&out_path)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_path.exists());

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_batch_parallel_jobs() {
    let fixtures = get_fixture_path("");
    let out_dir = std::env::temp_dir().join("morfpix_test_batch_parallel");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).unwrap();

    let output = Command::new(get_bin_path())
        .arg(&fixtures)
        .arg("-f")
        .arg("webp")
        .arg("-j")
        .arg("2")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("Failed to execute morfpix batch parallel");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let count = std::fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "webp"))
        .count();
    assert!(
        count >= 2,
        "Expected at least 2 WebP outputs, found {}",
        count
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_batch_parallel_jobs_jxl() {
    let fixtures = get_fixture_path("");
    let out_dir = std::env::temp_dir().join("morfpix_test_batch_parallel_jxl");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).unwrap();

    let output = Command::new(get_bin_path())
        .arg(&fixtures)
        .arg("-f")
        .arg("jxl")
        .arg("-j")
        .arg("2")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("Failed to execute morfpix batch parallel JXL");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let count = std::fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jxl"))
        .count();
    assert!(
        count >= 2,
        "Expected at least 2 JXL outputs, found {}",
        count
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_rejects_unreasonable_memory_estimate() {
    let test_dir = std::env::temp_dir().join("morfpix_test_unreasonable_estimate");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    // Valid PNG structure with dimensions 15,000 x 15,000 (225 MPixels, requiring ~3.6 GB working memory)
    let png_bytes: [u8; 65] = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x3a, 0x98, 0x00, 0x00, 0x3a, 0x98, 0x08, 0x00, 0x00, 0x00, 0x00, 0xfb,
        0x7f, 0x96, 0x79, 0x00, 0x00, 0x00, 0x08, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x03, 0x00,
        0x00, 0x00, 0x00, 0x01, 0x48, 0x06, 0x89, 0xd2, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
        0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    let test_file = test_dir.join("huge_dims.png");
    std::fs::write(&test_file, png_bytes).expect("Failed to write crafted PNG");

    let output = Command::new(get_bin_path())
        .arg(&test_file)
        .arg("-f")
        .arg("webp")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("exceeds maximum allowed limit"),
        "stderr was: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cli_default_output_format_conversion() {
    let fixture = get_fixture_path("sample-photo.png");
    let test_dir = std::env::temp_dir().join("morfpix_test_default_convert");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let temp_input = test_dir.join("photo.png");
    std::fs::copy(&fixture, &temp_input).unwrap();

    let output = Command::new(get_bin_path())
        .arg(&temp_input)
        .arg("-f")
        .arg("webp")
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected_output = test_dir.join("photo.webp");
    assert!(expected_output.exists(), "Expected clean photo.webp output");

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cli_default_output_same_format_compression() {
    let fixture = get_fixture_path("sample-photo.png");
    let test_dir = std::env::temp_dir().join("morfpix_test_default_same_format");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let temp_input = test_dir.join("photo.png");
    std::fs::copy(&fixture, &temp_input).unwrap();

    let output = Command::new(get_bin_path())
        .arg(&temp_input)
        .arg("-f")
        .arg("png")
        .arg("--optimize")
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected_output = test_dir.join("photo-compressed.png");
    assert!(
        expected_output.exists(),
        "Expected safe photo-compressed.png output"
    );

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cli_webp_lossless_and_options() {
    let fixture = get_fixture_path("sample-photo.png");
    let test_dir = std::env::temp_dir().join("morfpix_test_webp_lossless");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let output_file = test_dir.join("photo.webp");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("webp")
        .arg("--lossless")
        .arg("-m")
        .arg("5")
        .arg("--exact")
        .arg("-o")
        .arg(&output_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_file.exists());
    let bytes = std::fs::read(&output_file).unwrap();
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WEBP");

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cli_webp_invalid_method() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("webp")
        .arg("-m")
        .arg("9")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("WebP compression method must be an integer between 0 and 6"));
}

#[test]
fn test_cli_webp_method_missing_value() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("webp")
        .arg("--method")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Option '--method' requires a value"));
}

#[test]
fn test_cli_jxl_effort_and_options() {
    let fixture = get_fixture_path("sample-photo.png");
    let test_dir = std::env::temp_dir().join("morfpix_test_jxl_effort");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let output_file = test_dir.join("photo.jxl");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("jxl")
        .arg("-e")
        .arg("7")
        .arg("-t")
        .arg("2")
        .arg("-o")
        .arg(&output_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_file.exists());
    let bytes = std::fs::read(&output_file).unwrap();
    assert_eq!(&bytes[0..2], &[0xFF, 0x0A]);

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cli_jxl_invalid_effort() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("jxl")
        .arg("-e")
        .arg("10")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("JXL encoding effort must be an integer between 1 and 9"));
}

#[test]
fn test_cli_jxl_effort_missing_value() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("jxl")
        .arg("--effort")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Option '--effort' requires a value"));
}

#[test]
fn test_cli_jpeg_baseline_and_options() {
    let fixture = get_fixture_path("sample-photo.png");
    let test_dir = std::env::temp_dir().join("morfpix_test_jpeg_options");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let output_file = test_dir.join("photo.jpg");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("jpeg")
        .arg("-q")
        .arg("85")
        .arg("--baseline")
        .arg("--subsample")
        .arg("444")
        .arg("--smoothing")
        .arg("10")
        .arg("-o")
        .arg(&output_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_file.exists());
    let bytes = std::fs::read(&output_file).unwrap();
    // SOI marker
    assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
    // SOF0 baseline marker
    assert!(bytes.windows(2).any(|w| w == [0xFF, 0xC0]));

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cli_jpeg_invalid_subsample() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("jpeg")
        .arg("--subsample")
        .arg("invalid")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Chroma subsampling must be one of: 444, 422, 420, 440, auto"));
}

#[test]
fn test_cli_jpeg_invalid_smoothing() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("jpeg")
        .arg("--smoothing")
        .arg("150")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Smoothing factor must be an integer between 0 and 100"));
}

#[test]
fn test_cli_png_level_and_interlace() {
    let fixture = get_fixture_path("sample-photo.png");
    let test_dir = std::env::temp_dir().join("morfpix_test_png_options");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let output_file = test_dir.join("photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("png")
        .arg("--png-level")
        .arg("3")
        .arg("--interlace")
        .arg("-o")
        .arg(&output_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_file.exists());
    let bytes = std::fs::read(&output_file).unwrap();
    // PNG signature
    assert_eq!(
        &bytes[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    );
    // Byte 28 is interlace method (1 = Adam7)
    assert_eq!(bytes[28], 1);

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cli_png_invalid_level() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("png")
        .arg("--png-level")
        .arg("8")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("PNG optimization level must be an integer between 0 and 6"));
}

#[test]
fn test_cli_png_level_missing_value() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("png")
        .arg("--png-level")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Option '--png-level' requires a value"));
}

#[test]
fn test_cli_gif_options() {
    let fixture = get_fixture_path("sample-photo.png");
    let test_dir = std::env::temp_dir().join("morfpix_test_gif_options");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let output_loop = test_dir.join("photo_loop.gif");
    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("gif")
        .arg("--repeat")
        .arg("infinite")
        .arg("--gif-speed")
        .arg("5")
        .arg("-o")
        .arg(&output_loop)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_loop.exists());
    let bytes_loop = std::fs::read(&output_loop).unwrap();
    assert_eq!(&bytes_loop[0..6], b"GIF89a");
    assert!(bytes_loop.windows(11).any(|w| w == b"NETSCAPE2.0"));

    let output_none = test_dir.join("photo_none.gif");
    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("gif")
        .arg("--gif-repeat")
        .arg("none")
        .arg("-o")
        .arg(&output_none)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_none.exists());
    let bytes_none = std::fs::read(&output_none).unwrap();
    assert_eq!(&bytes_none[0..6], b"GIF89a");
    assert!(!bytes_none.windows(11).any(|w| w == b"NETSCAPE2.0"));

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cli_gif_invalid_repeat() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("gif")
        .arg("--gif-repeat")
        .arg("invalid")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(
        "GIF repeat count must be 'infinite', 'none', or an integer between 1 and 65535"
    ));
}

#[test]
fn test_cli_gif_invalid_speed() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("gif")
        .arg("--gif-speed")
        .arg("0")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("GIF speed must be an integer between 1 and 30"));

    let output_high = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("gif")
        .arg("--gif-speed")
        .arg("40")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output_high.status.success());
    let stderr_high = String::from_utf8_lossy(&output_high.stderr);
    assert!(stderr_high.contains("GIF speed must be an integer between 1 and 30"));
}

#[test]
fn test_cli_gif_repeat_missing_value() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("gif")
        .arg("--gif-repeat")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Option '--gif-repeat' requires a value"));
}

#[test]
fn test_cli_bmp_options() {
    let fixture = get_fixture_path("sample-photo.png");
    let test_dir = std::env::temp_dir().join("morfpix_test_bmp_options");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let output_24 = test_dir.join("photo_24.bmp");
    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("bmp")
        .arg("--bmp-bpp")
        .arg("24")
        .arg("-o")
        .arg(&output_24)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_24.exists());
    let bytes_24 = std::fs::read(&output_24).unwrap();
    assert_eq!(&bytes_24[0..2], b"BM");
    assert_eq!(&bytes_24[28..30], &[24, 0]);

    let output_32 = test_dir.join("photo_32.bmp");
    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("bmp")
        .arg("--bpp")
        .arg("32")
        .arg("-o")
        .arg(&output_32)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_32.exists());
    let bytes_32 = std::fs::read(&output_32).unwrap();
    assert_eq!(&bytes_32[0..2], b"BM");
    assert_eq!(&bytes_32[28..30], &[32, 0]);

    let output_8 = test_dir.join("photo_8.bmp");
    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("bmp")
        .arg("--bmp-bpp")
        .arg("8")
        .arg("-o")
        .arg(&output_8)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_8.exists());
    let bytes_8 = std::fs::read(&output_8).unwrap();
    assert_eq!(&bytes_8[0..2], b"BM");
    assert_eq!(&bytes_8[28..30], &[8, 0]);

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cli_bmp_invalid_bpp() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("bmp")
        .arg("--bmp-bpp")
        .arg("16")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("BMP bits per pixel (bpp) must be one of: 8, 24, 32, auto"));
}

#[test]
fn test_cli_bmp_missing_value() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("bmp")
        .arg("--bmp-bpp")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Option '--bmp-bpp' requires a value"));
}

fn create_symlink_file(original: &Path, link: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(original, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(original, link)
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Symlinks not supported on this platform",
        ))
    }
}

fn create_symlink_dir(original: &Path, link: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(original, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(original, link)
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Symlinks not supported on this platform",
        ))
    }
}

#[test]
fn test_cli_does_not_recurse_into_symlink_directories() {
    let test_dir = std::env::temp_dir().join("morfpix_test_no_symlink_dir_traversal");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let fixture = get_fixture_path("sample-photo.png");
    let sample = test_dir.join("sample.png");
    std::fs::copy(&fixture, &sample).unwrap();

    let outside_dir = std::env::temp_dir().join("morfpix_test_outside_dir");
    let _ = std::fs::remove_dir_all(&outside_dir);
    std::fs::create_dir_all(&outside_dir).unwrap();
    let outside_sample = outside_dir.join("outside.png");
    std::fs::copy(&fixture, &outside_sample).unwrap();

    let symlinked_dir = test_dir.join("symlinked_dir");
    if let Err(e) = create_symlink_dir(&outside_dir, &symlinked_dir) {
        eprintln!("Skipping symlink test: symlink creation failed: {}", e);
        let _ = std::fs::remove_dir_all(&test_dir);
        let _ = std::fs::remove_dir_all(&outside_dir);
        return;
    }

    let out_dir = std::env::temp_dir().join("morfpix_test_no_symlink_out");
    let _ = std::fs::remove_dir_all(&out_dir);

    let output = Command::new(get_bin_path())
        .arg(&test_dir)
        .arg("-f")
        .arg("webp")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_dir.join("sample.webp").exists());
    // Must NOT have traversed into symlinked_dir and converted outside.png
    assert!(!out_dir.join("symlinked_dir").join("outside.webp").exists());
    assert!(!out_dir.join("outside.webp").exists());
    assert!(!outside_dir.join("outside-compressed.webp").exists());

    let _ = std::fs::remove_dir_all(&test_dir);
    let _ = std::fs::remove_dir_all(&outside_dir);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_does_not_traverse_symlinked_image_files() {
    let test_dir = std::env::temp_dir().join("morfpix_test_no_symlink_file_traversal");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let fixture = get_fixture_path("sample-photo.png");
    let real_sample = test_dir.join("real_sample.png");
    std::fs::copy(&fixture, &real_sample).unwrap();

    let outside_dir = std::env::temp_dir().join("morfpix_test_outside_file_dir");
    let _ = std::fs::remove_dir_all(&outside_dir);
    std::fs::create_dir_all(&outside_dir).unwrap();
    let outside_sample = outside_dir.join("outside_target.png");
    std::fs::copy(&fixture, &outside_sample).unwrap();

    let symlink_file = test_dir.join("symlinked_sample.png");
    if let Err(e) = create_symlink_file(&outside_sample, &symlink_file) {
        eprintln!("Skipping symlink test: symlink creation failed: {}", e);
        let _ = std::fs::remove_dir_all(&test_dir);
        let _ = std::fs::remove_dir_all(&outside_dir);
        return;
    }

    let out_dir = std::env::temp_dir().join("morfpix_test_no_symlink_file_out");
    let _ = std::fs::remove_dir_all(&out_dir);

    let output = Command::new(get_bin_path())
        .arg(&test_dir)
        .arg("-f")
        .arg("webp")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_dir.join("real_sample.webp").exists());
    // Symlinked image file MUST NOT have been traversed or converted
    assert!(!out_dir.join("symlinked_sample.webp").exists());
    assert!(!outside_dir.join("outside_target.webp").exists());
    assert!(!outside_dir.join("outside_target-compressed.webp").exists());

    let _ = std::fs::remove_dir_all(&test_dir);
    let _ = std::fs::remove_dir_all(&outside_dir);
    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_cli_refuses_to_write_to_symlink_destination() {
    let test_dir = std::env::temp_dir().join("morfpix_test_symlink_dest_refusal");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let fixture = get_fixture_path("sample-photo.png");
    let target_file = test_dir.join("sensitive_file.txt");
    std::fs::write(&target_file, b"SENSITIVE_DATA_DO_NOT_OVERWRITE").unwrap();

    let symlink_dest = test_dir.join("output_symlink.webp");
    if let Err(e) = create_symlink_file(&target_file, &symlink_dest) {
        eprintln!("Skipping symlink test: symlink creation failed: {}", e);
        let _ = std::fs::remove_dir_all(&test_dir);
        return;
    }

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("webp")
        .arg("-o")
        .arg(&symlink_dest)
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Refusing to write to symlink destination"),
        "unexpected stderr: {}",
        stderr
    );

    // Verify target file was NOT overwritten
    let content = std::fs::read(&target_file).unwrap();
    assert_eq!(content, b"SENSITIVE_DATA_DO_NOT_OVERWRITE");

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cli_refuses_to_write_to_symlink_parent_directory() {
    let test_dir = std::env::temp_dir().join("morfpix_test_symlink_parent_refusal");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let outside_dir = std::env::temp_dir().join("morfpix_test_sensitive_outside_parent");
    let _ = std::fs::remove_dir_all(&outside_dir);
    std::fs::create_dir_all(&outside_dir).unwrap();

    let fixture = get_fixture_path("sample-photo.png");

    let symlink_parent = test_dir.join("symlinked_parent_dir");
    if let Err(e) = create_symlink_dir(&outside_dir, &symlink_parent) {
        eprintln!("Skipping symlink test: symlink creation failed: {}", e);
        let _ = std::fs::remove_dir_all(&test_dir);
        let _ = std::fs::remove_dir_all(&outside_dir);
        return;
    }

    let dest_file = symlink_parent.join("output.webp");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("webp")
        .arg("-o")
        .arg(&dest_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Refusing to write to symlink destination"),
        "unexpected stderr: {}",
        stderr
    );

    // Verify output was NOT written through the symlink into the outside directory
    assert!(!outside_dir.join("output.webp").exists());

    let _ = std::fs::remove_dir_all(&test_dir);
    let _ = std::fs::remove_dir_all(&outside_dir);
}

#[test]
fn test_cli_refuses_to_overwrite_hard_link_of_input_file() {
    let test_dir = std::env::temp_dir().join("morfpix_test_hard_link_refusal");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let fixture = get_fixture_path("sample-photo.png");
    let sample = test_dir.join("sample.png");
    std::fs::copy(&fixture, &sample).unwrap();

    let hard_link = test_dir.join("hard_link.png");
    if let Err(e) = std::fs::hard_link(&sample, &hard_link) {
        eprintln!(
            "Skipping hard link test: not supported on filesystem: {}",
            e
        );
        let _ = std::fs::remove_dir_all(&test_dir);
        return;
    }

    let orig_content = std::fs::read(&sample).unwrap();

    let output = Command::new(get_bin_path())
        .arg(&sample)
        .arg("-f")
        .arg("png")
        .arg("-o")
        .arg(&hard_link)
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Refusing to overwrite input file"),
        "unexpected stderr: {}",
        stderr
    );

    // Verify original input file was NOT overwritten or truncated
    let content_after = std::fs::read(&sample).unwrap();
    assert_eq!(content_after, orig_content);

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cli_resize_width_only() {
    let fixture = get_fixture_path("sample-photo.png");
    let out_file = std::env::temp_dir().join("morfpix_test_resize_w_only.png");
    let _ = std::fs::remove_file(&out_file);

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("--width")
        .arg("32")
        .arg("-o")
        .arg(&out_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "morfpix --width failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_file.exists());
    let bytes = std::fs::read(&out_file).unwrap();
    let (w, h) = morfpix_engine::read_image_dimensions(&bytes).unwrap();
    assert_eq!((w, h), (32, 32));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("32 x 32 px"));

    let _ = std::fs::remove_file(out_file);
}

#[test]
fn test_cli_resize_height_only() {
    let fixture = get_fixture_path("phone-photo.jpeg");
    let out_file = std::env::temp_dir().join("morfpix_test_resize_h_only.jpg");
    let _ = std::fs::remove_file(&out_file);

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("--height")
        .arg("899")
        .arg("-o")
        .arg(&out_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "morfpix --height failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_file.exists());
    let bytes = std::fs::read(&out_file).unwrap();
    let (w, h) = morfpix_engine::read_image_dimensions(&bytes).unwrap();
    assert_eq!((w, h), (600, 899));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("600 x 899 px"));

    let _ = std::fs::remove_file(out_file);
}

#[test]
fn test_cli_resize_both_dimensions_fit_box() {
    let fixture = get_fixture_path("phone-photo.jpeg");
    let out_file = std::env::temp_dir().join("morfpix_test_resize_box.jpg");
    let _ = std::fs::remove_file(&out_file);

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("--width")
        .arg("300")
        .arg("--height")
        .arg("300")
        .arg("-o")
        .arg(&out_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "morfpix --width --height failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_file.exists());
    let bytes = std::fs::read(&out_file).unwrap();
    let (w, h) = morfpix_engine::read_image_dimensions(&bytes).unwrap();
    // 1200x1798 fits into 300x300 bounding box as 200x300
    assert_eq!((w, h), (200, 300));

    let _ = std::fs::remove_file(out_file);
}

#[test]
fn test_cli_resize_zero_dimension() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("--width")
        .arg("0")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Width must be greater than 0"));

    let output2 = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("--height")
        .arg("0")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output2.status.success());
    let stderr2 = String::from_utf8_lossy(&output2.stderr);
    assert!(stderr2.contains("Height must be greater than 0"));
}

#[test]
fn test_cli_resize_malformed_dimension() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("--width")
        .arg("invalid")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Width must be a positive integer"));

    let output2 = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("--height")
        .arg("-10")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output2.status.success());
    let stderr2 = String::from_utf8_lossy(&output2.stderr);
    assert!(stderr2.contains("Height must be a positive integer"));
}

#[test]
fn test_cli_resize_unreasonable_dimension() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("--width")
        .arg("99999")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Width exceeds maximum allowed limit"));

    let output2 = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("--height")
        .arg("99999")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output2.status.success());
    let stderr2 = String::from_utf8_lossy(&output2.stderr);
    assert!(stderr2.contains("Height exceeds maximum allowed limit"));
}

#[test]
fn test_cli_resize_filter_without_dimensions() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("--filter")
        .arg("lanczos3")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Resizing requires at least one dimension (--width or --height)"));
}

#[test]
fn test_cli_resize_invalid_filter() {
    let fixture = get_fixture_path("sample-photo.png");

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("--width")
        .arg("32")
        .arg("--filter")
        .arg("spline")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(
        "Resize filter must be one of: lanczos3, catmullrom, triangle, nearest, gaussian"
    ));
}

#[test]
fn test_cli_unchanged_without_resize_flags() {
    let fixture = get_fixture_path("sample-photo.png");
    let out_file = std::env::temp_dir().join("morfpix_test_unchanged_no_resize.png");
    let _ = std::fs::remove_file(&out_file);

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("-f")
        .arg("png")
        .arg("-o")
        .arg(&out_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(output.status.success());
    assert!(out_file.exists());
    let bytes = std::fs::read(&out_file).unwrap();
    let (w, h) = morfpix_engine::read_image_dimensions(&bytes).unwrap();
    assert_eq!((w, h), (64, 64));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("64 x 64 px"));

    let _ = std::fs::remove_file(out_file);
}

#[test]
fn test_cli_rejects_resizing_animated_image() {
    let test_dir = std::env::temp_dir().join("morfpix_test_animated_reject");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let anim_gif = test_dir.join("animated.gif");
    let gif_bytes =
        create_test_animated_gif(40, 40, 2).expect("Failed to create test animated GIF");
    std::fs::write(&anim_gif, &gif_bytes).unwrap();

    let output = Command::new(get_bin_path())
        .arg(&anim_gif)
        .arg("--width")
        .arg("10")
        .output()
        .expect("Failed to execute morfpix");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Resizing animated images is unsupported"),
        "stderr was: {}",
        stderr
    );

    // Verify converting without resize flags succeeds (existing behavior converts first frame)
    let out_file = test_dir.join("single_frame.png");
    let output2 = Command::new(get_bin_path())
        .arg(&anim_gif)
        .arg("-f")
        .arg("png")
        .arg("-o")
        .arg(&out_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output2.status.success(),
        "Converting animated gif without resize failed: {}",
        String::from_utf8_lossy(&output2.stderr)
    );
    assert!(out_file.exists());

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_cli_resize_custom_filter() {
    let fixture = get_fixture_path("sample-photo.png");
    let out_file = std::env::temp_dir().join("morfpix_test_resize_custom_filter.png");
    let _ = std::fs::remove_file(&out_file);

    let output = Command::new(get_bin_path())
        .arg(&fixture)
        .arg("--width")
        .arg("32")
        .arg("--filter")
        .arg("catmullrom")
        .arg("-o")
        .arg(&out_file)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "morfpix --filter catmullrom failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_file.exists());
    let bytes = std::fs::read(&out_file).unwrap();
    let (w, h) = morfpix_engine::read_image_dimensions(&bytes).unwrap();
    assert_eq!((w, h), (32, 32));

    let _ = std::fs::remove_file(out_file);
}

#[test]
fn test_cli_resize_batch_directory() {
    let test_dir = std::env::temp_dir().join("morfpix_test_resize_batch_in");
    let out_dir = std::env::temp_dir().join("morfpix_test_resize_batch_out");
    let _ = std::fs::remove_dir_all(&test_dir);
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    let fixture1 = get_fixture_path("sample-photo.png");
    let fixture2 = get_fixture_path("phone-photo.jpeg");
    std::fs::copy(&fixture1, test_dir.join("sample.png")).unwrap();
    std::fs::copy(&fixture2, test_dir.join("phone.jpeg")).unwrap();

    let output = Command::new(get_bin_path())
        .arg(&test_dir)
        .arg("--width")
        .arg("60")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("Failed to execute morfpix");

    assert!(
        output.status.success(),
        "Batch resize failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let out1 = out_dir.join("sample.png");
    let out2 = out_dir.join("phone.jpg");
    assert!(out1.exists());
    assert!(out2.exists());

    let b1 = std::fs::read(&out1).unwrap();
    let (w1, h1) = morfpix_engine::read_image_dimensions(&b1).unwrap();
    assert_eq!((w1, h1), (60, 60));

    let b2 = std::fs::read(&out2).unwrap();
    let (w2, h2) = morfpix_engine::read_image_dimensions(&b2).unwrap();
    // 1200x1798 scaled to width 60 -> height = round(1798 * 60 / 1200) = 90
    assert_eq!((w2, h2), (60, 90));

    let _ = std::fs::remove_dir_all(&test_dir);
    let _ = std::fs::remove_dir_all(&out_dir);
}
