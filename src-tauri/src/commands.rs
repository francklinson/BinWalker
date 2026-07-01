use backhand::{
    compression::{CompressionAction, Compressor, DefaultCompressor},
    kind::Kind,
    BackhandError, FilesystemCompressor, InnerNode, Squashfs, SuperBlock,
};
use binwalk::Binwalk;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{BufReader, Cursor, Read};
use std::path::PathBuf;
use oxiarc_lzma;
use lzma_rs;

#[derive(Serialize, Deserialize, Clone)]
pub struct ScanResult {
    pub offset: u64,
    pub size: u64,
    pub name: String,
    pub description: String,
    pub confidence: i32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct EntropyPoint {
    pub offset: u64,
    pub entropy: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ExtractedFile {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub original_offset: u64,
    pub file_type: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DeepScanItem {
    pub layer: u32,
    pub offset: u64,
    pub size: u64,
    pub name: String,
    pub description: String,
    pub confidence: i32,
    pub parent_offset: Option<u64>,
    pub parent_name: Option<String>,
    pub source: String,
}

const MAX_DEPTH: u32 = 10;

#[tauri::command]
pub async fn open_file_location(path: String) -> Result<(), String> {
    let log_file = std::env::temp_dir().join("binwalker_debug.log");
    let mut log_content = format!("=== open_file_location 被调用 ===\n");
    log_content.push_str(&format!("原始路径: {}\n", path));
    
    let file_path = PathBuf::from(&path);
    
    if !file_path.exists() {
        log_content.push_str(&format!("路径不存在: {}\n", path));
        let _ = std::fs::write(&log_file, log_content);
        return Err(format!("路径不存在: {}", path));
    }
    
    log_content.push_str("路径存在\n");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        
        let file_abs = match std::fs::canonicalize(&file_path) {
            Ok(p) => p,
            Err(e) => {
                log_content.push_str(&format!("canonicalize 失败: {}\n", e));
                let _ = std::fs::write(&log_file, log_content);
                return Err(format!("无法获取绝对路径: {}", e));
            }
        };
        
        log_content.push_str(&format!("绝对路径: {:?}\n", file_abs));
        
        let file_str = file_abs.to_string_lossy().to_string();
        let clean_path = file_str.strip_prefix(r"\\?\").unwrap_or(&file_str).to_string();
        log_content.push_str(&format!("清理后路径: {}\n", clean_path));
        
        // explorer /select 需要文件路径，会打开所在目录并选中该文件
        let arg = format!(r#"/select,"{}""#, clean_path);
        log_content.push_str(&format!("执行命令: explorer {}\n", arg));
        
        match std::process::Command::new("explorer.exe")
            .raw_arg(&arg)
            .spawn()
        {
            Ok(_) => log_content.push_str("命令已启动\n"),
            Err(e) => log_content.push_str(&format!("命令启动失败: {}\n", e)),
        }
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("打开文件位置失败: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        let parent = file_path.parent().unwrap_or(&file_path);
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| format!("打开文件位置失败: {}", e))?;
    }

    log_content.push_str("=== 函数执行完成 ===\n");
    let _ = std::fs::write(&log_file, log_content);
    
    Ok(())
}

#[tauri::command]
pub async fn deep_scan(path: String) -> Result<Vec<DeepScanItem>, String> {
    let file_path = PathBuf::from(&path);

    if !file_path.exists() {
        return Err(format!("文件不存在: {}", path));
    }

    let file_data = fs::read(&file_path)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    let mut results = Vec::new();
    let mut seen_offsets = HashSet::new();

    recursive_scan(&file_data, 0, None, None, "original".to_string(), &mut results, &mut seen_offsets);

    Ok(results)
}

fn recursive_scan(
    data: &[u8],
    layer: u32,
    parent_offset: Option<u64>,
    parent_name: Option<String>,
    source: String,
    results: &mut Vec<DeepScanItem>,
    seen_offsets: &mut HashSet<(u32, u64)>,
) {
    if data.is_empty() || layer > MAX_DEPTH {
        return;
    }

    // 基于位置的智能去重：只跳过已经扫描过的 (layer, absolute_offset) 组合
    let base_offset = parent_offset.unwrap_or(0);
    if !seen_offsets.insert((layer, base_offset)) {
        return;
    }

    let binwalker = Binwalk::new();
    let scan_results = binwalker.scan(data);

    for result in scan_results {
        if result.size == 0 {
            continue;
        }

        let start = result.offset as usize;
        let end = start + result.size as usize;
        if end > data.len() {
            continue;
        }

        // 当前项在当前数据块中的相对偏移
        let relative_offset = result.offset as u64;
        // 当前项在原始文件中的绝对偏移（仅对未压缩数据准确，用于传递给下一层作为 parent_offset）
        let absolute_offset = parent_offset.unwrap_or(0) + relative_offset;

        results.push(DeepScanItem {
            layer,
            offset: relative_offset, // 保持相对偏移，表示在当前扫描数据中的位置
            size: result.size as u64,
            name: result.name.to_string(),
            description: result.description.to_string(),
            confidence: result.confidence as i32,
            parent_offset, // 父级数据块在原始文件中的起始偏移
            parent_name: parent_name.clone(),
            source: source.clone(),
        });

        let extracted = &data[start..end];
        let name_lower = result.name.to_lowercase();
        let child_source = format!("{}_0x{:x}", result.name, result.offset);
        let child_parent_offset = Some(absolute_offset);
        let child_parent_name = Some(result.name.to_string());

        // 尝试所有压缩格式的递归解压扫描
        let mut decompressed = false;

        // gzip
        if name_lower.contains("gzip") || (extracted.len() >= 2 && extracted[0] == 0x1F && extracted[1] == 0x8B) {
            if let Ok(dec) = decompress_gzip(extracted) {
                recursive_scan(
                    &dec,
                    layer + 1,
                    child_parent_offset,
                    child_parent_name.clone(),
                    child_source.clone(),
                    results,
                    seen_offsets,
                );
                decompressed = true;
            }
        }

        // bzip2
        if !decompressed && (name_lower.contains("bzip2") || (extracted.len() >= 3 && &extracted[0..3] == b"BZh")) {
            if let Ok(dec) = decompress_bzip2(extracted) {
                recursive_scan(
                    &dec,
                    layer + 1,
                    child_parent_offset,
                    child_parent_name.clone(),
                    child_source.clone(),
                    results,
                    seen_offsets,
                );
                decompressed = true;
            }
        }

        // xz
        if !decompressed && (name_lower.contains("xz") || (extracted.len() >= 6 && extracted[0..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00])) {
            if let Ok(dec) = decompress_xz(extracted) {
                recursive_scan(
                    &dec,
                    layer + 1,
                    child_parent_offset,
                    child_parent_name.clone(),
                    child_source.clone(),
                    results,
                    seen_offsets,
                );
                decompressed = true;
            }
        }

        // lzma
        if !decompressed && (name_lower.contains("lzma") || (extracted.len() >= 1 && extracted[0] == 0x5D)) {
            if let Ok(dec) = decompress_lzma(extracted) {
                recursive_scan(
                    &dec,
                    layer + 1,
                    child_parent_offset,
                    child_parent_name.clone(),
                    child_source.clone(),
                    results,
                    seen_offsets,
                );
                decompressed = true;
            }
        }

        // SquashFS: 解包文件系统并扫描每个内部文件
        if !decompressed && name_lower.contains("squashfs") {
            scan_squashfs_contents(
                extracted,
                layer + 1,
                child_parent_offset,
                child_parent_name.clone(),
                &child_source,
                results,
                seen_offsets,
            );
            decompressed = true;
        }

        // uImage: 提取内部压缩数据并递归扫描
        if !decompressed && name_lower.contains("uimage") && extracted.len() > 64 {
            // uImage header is 64 bytes, data starts after that
            let inner_data = &extracted[64..];

            // Try to detect and decompress the inner data based on magic bytes
            // gzip
            if inner_data.len() >= 2 && inner_data[0] == 0x1F && inner_data[1] == 0x8B {
                if let Ok(dec) = decompress_gzip(inner_data) {
                    recursive_scan(
                        &dec,
                        layer + 1,
                        child_parent_offset,
                        child_parent_name.clone(),
                        format!("{}_inner", child_source),
                        results,
                        seen_offsets,
                    );
                    decompressed = true;
                }
            }
            // lzma
            else if inner_data.len() >= 1 && inner_data[0] == 0x5D {
                if let Ok(dec) = decompress_lzma(inner_data) {
                    recursive_scan(
                        &dec,
                        layer + 1,
                        child_parent_offset,
                        child_parent_name.clone(),
                        format!("{}_inner", child_source),
                        results,
                        seen_offsets,
                    );
                    decompressed = true;
                }
            }
            // xz
            else if inner_data.len() >= 6 && inner_data[0..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00] {
                if let Ok(dec) = decompress_xz(inner_data) {
                    recursive_scan(
                        &dec,
                        layer + 1,
                        child_parent_offset,
                        child_parent_name.clone(),
                        format!("{}_inner", child_source),
                        results,
                        seen_offsets,
                    );
                    decompressed = true;
                }
            }
            // bzip2
            else if inner_data.len() >= 3 && &inner_data[0..3] == b"BZh" {
                if let Ok(dec) = decompress_bzip2(inner_data) {
                    recursive_scan(
                        &dec,
                        layer + 1,
                        child_parent_offset,
                        child_parent_name.clone(),
                        format!("{}_inner", child_source),
                        results,
                        seen_offsets,
                    );
                    decompressed = true;
                }
            }
            // If no compression detected, still scan the raw inner data
            else if !inner_data.is_empty() {
                recursive_scan(
                    inner_data,
                    layer + 1,
                    child_parent_offset,
                    child_parent_name.clone(),
                    format!("{}_inner", child_source),
                    results,
                    seen_offsets,
                );
                decompressed = true;
            }
        }

        // 对于非压缩格式（如 ELF、PE、RSA 等），不递归扫描
        // 只有成功解压/解包的压缩格式才需要递归扫描
    }
}

#[tauri::command]
pub async fn get_entropy(path: String, block_size: usize) -> Result<Vec<EntropyPoint>, String> {
    let file_path = PathBuf::from(&path);
    
    if !file_path.exists() {
        return Err(format!("文件不存在: {}", path));
    }

    let file_data = fs::read(&file_path)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    let mut points = Vec::new();
    let mut offset = 0;

    while offset < file_data.len() {
        let end = std::cmp::min(offset + block_size, file_data.len());
        let block = &file_data[offset..end];
        
        let entropy = calculate_entropy(block);
        points.push(EntropyPoint {
            offset: offset as u64,
            entropy,
        });
        
        offset = end;
    }

    Ok(points)
}

fn calculate_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mut counts = [0u32; 256];
    for &byte in data {
        counts[byte as usize] += 1;
    }

    let len = data.len() as f64;
    let mut entropy = 0.0;

    for &count in &counts {
        if count > 0 {
            let p = count as f64 / len;
            entropy -= p * p.log2();
        }
    }

    entropy
}

#[tauri::command]
pub async fn extract_file(path: String, output_dir: String) -> Result<Vec<ExtractedFile>, String> {
    let file_path = PathBuf::from(&path);

    if !file_path.exists() {
        return Err(format!("文件不存在: {}", path));
    }

    let output_path = PathBuf::from(&output_dir);
    fs::create_dir_all(&output_path)
        .map_err(|e| format!("创建输出目录失败: {}", e))?;

    let file_data = fs::read(&file_path)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    let binwalker = Binwalk::new();
    let scan_results = binwalker.scan(&file_data);
    let mut extracted_files = Vec::new();

    for result in scan_results {
        if result.size == 0 {
            continue;
        }

        let start = result.offset as usize;
        let end = start + result.size as usize;

        if end > file_data.len() {
            continue;
        }

        let extracted_data = &file_data[start..end];
        let name_lower = result.name.to_lowercase();
        let base_name = format!("{}_0x{:x}", result.name, result.offset);

        extract_component(
            extracted_data,
            &name_lower,
            &base_name,
            &output_path,
            result.offset as u64,
            &result.name,
            &mut extracted_files,
            0,
        );
    }

    Ok(extracted_files)
}

fn extract_component(
    data: &[u8],
    type_hint: &str,
    base_name: &str,
    output_dir: &PathBuf,
    original_offset: u64,
    original_type: &str,
    extracted_files: &mut Vec<ExtractedFile>,
    depth: u32,
) {
    if depth > MAX_DEPTH || data.is_empty() {
        return;
    }

    // Try gzip
    if type_hint.contains("gzip") || (data.len() >= 2 && data[0] == 0x1F && data[1] == 0x8B) {
        if let Ok(decompressed) = decompress_gzip(data) {
            let inner_name = format!("{}_decompressed", base_name);
            let inner_type = detect_type_from_data(&decompressed);
            if inner_type != "data" {
                extract_component(
                    &decompressed,
                    &inner_type,
                    &inner_name,
                    output_dir,
                    original_offset,
                    original_type,
                    extracted_files,
                    depth + 1,
                );
            } else {
                save_extracted_file(&decompressed, &inner_name, "bin", output_dir, original_offset, original_type, extracted_files);
            }
            return;
        }
    }

    // Try bzip2
    if type_hint.contains("bzip2") || (data.len() >= 3 && &data[0..3] == b"BZh") {
        if let Ok(decompressed) = decompress_bzip2(data) {
            let inner_name = format!("{}_decompressed", base_name);
            let inner_type = detect_type_from_data(&decompressed);
            if inner_type != "data" {
                extract_component(&decompressed, &inner_type, &inner_name, output_dir, original_offset, original_type, extracted_files, depth + 1);
            } else {
                save_extracted_file(&decompressed, &inner_name, "bin", output_dir, original_offset, original_type, extracted_files);
            }
            return;
        }
    }

    // Try xz
    if type_hint.contains("xz") || (data.len() >= 6 && data[0..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) {
        if let Ok(decompressed) = decompress_xz(data) {
            let inner_name = format!("{}_decompressed", base_name);
            let inner_type = detect_type_from_data(&decompressed);
            if inner_type != "data" {
                extract_component(&decompressed, &inner_type, &inner_name, output_dir, original_offset, original_type, extracted_files, depth + 1);
            } else {
                save_extracted_file(&decompressed, &inner_name, "bin", output_dir, original_offset, original_type, extracted_files);
            }
            return;
        }
    }

    // Try lzma
    if type_hint.contains("lzma") || (data.len() >= 1 && data[0] == 0x5D) {
        if let Ok(decompressed) = decompress_lzma(data) {
            let inner_name = format!("{}_decompressed", base_name);
            let inner_type = detect_type_from_data(&decompressed);
            if inner_type != "data" {
                extract_component(&decompressed, &inner_type, &inner_name, output_dir, original_offset, original_type, extracted_files, depth + 1);
            } else {
                save_extracted_file(&decompressed, &inner_name, "bin", output_dir, original_offset, original_type, extracted_files);
            }
            return;
        }
    }

    // Try squashfs
    if type_hint.contains("squashfs") || (data.len() >= 4 && &data[0..4] == b"hsqs") {
        if let Ok(files) = unpack_squashfs(data) {
            let squashfs_dir = output_dir.join(format!("{}_squashfs-root", base_name));
            let _ = fs::create_dir_all(&squashfs_dir);
            for (file_path, file_data) in &files {
                let file_path_obj = squashfs_dir.join(file_path);
                if let Some(parent) = file_path_obj.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if fs::write(&file_path_obj, file_data).is_ok() {
                    extracted_files.push(ExtractedFile {
                        name: file_path.clone(),
                        path: file_path_obj.to_string_lossy().to_string(),
                        size: file_data.len() as u64,
                        original_offset,
                        file_type: "squashfs-inner".to_string(),
                    });
                }
            }
            return;
        }
    }

    // Try tar
    if type_hint.contains("tar") || (data.len() >= 262 && &data[257..262] == b"ustar") {
        if let Ok(files) = extract_tar(data) {
            let tar_dir = output_dir.join(format!("{}_tar-root", base_name));
            let _ = fs::create_dir_all(&tar_dir);
            for (file_path, file_data) in &files {
                let file_path_obj = tar_dir.join(file_path);
                if let Some(parent) = file_path_obj.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if fs::write(&file_path_obj, file_data).is_ok() {
                    extracted_files.push(ExtractedFile {
                        name: file_path.clone(),
                        path: file_path_obj.to_string_lossy().to_string(),
                        size: file_data.len() as u64,
                        original_offset,
                        file_type: "tar-inner".to_string(),
                    });
                }
            }
            return;
        }
    }

    // Fallback: save raw data with appropriate extension
    let ext = get_extension_for_type(type_hint);
    save_extracted_file(data, base_name, ext, output_dir, original_offset, original_type, extracted_files);
}

fn save_extracted_file(
    data: &[u8],
    base_name: &str,
    ext: &str,
    output_dir: &PathBuf,
    original_offset: u64,
    original_type: &str,
    extracted_files: &mut Vec<ExtractedFile>,
) {
    let final_name = format!("{}.{}", base_name, ext);
    let output_file = output_dir.join(&final_name);
    if fs::write(&output_file, data).is_ok() {
        extracted_files.push(ExtractedFile {
            name: final_name,
            path: output_file.to_string_lossy().to_string(),
            size: data.len() as u64,
            original_offset,
            file_type: original_type.to_string(),
        });
    }
}

fn get_extension_for_type(type_hint: &str) -> &str {
    if type_hint.contains("gzip") { return "gz"; }
    if type_hint.contains("bzip2") { return "bz2"; }
    if type_hint.contains("xz") { return "xz"; }
    if type_hint.contains("lzma") { return "lzma"; }
    if type_hint.contains("squashfs") { return "squashfs"; }
    if type_hint.contains("tar") { return "tar"; }
    if type_hint.contains("elf") { return "elf"; }
    if type_hint.contains("pe") || type_hint.contains("executable") { return "exe"; }
    if type_hint.contains("zip") { return "zip"; }
    if type_hint.contains("jpeg") || type_hint.contains("jpg") { return "jpg"; }
    if type_hint.contains("png") { return "png"; }
    if type_hint.contains("gif") { return "gif"; }
    if type_hint.contains("pdf") { return "pdf"; }
    if type_hint.contains("rsa") || type_hint.contains("private key") { return "pem"; }
    if type_hint.contains("certificate") { return "crt"; }
    "bin"
}

fn detect_type_from_data(data: &[u8]) -> String {
    if data.len() < 4 { return "data".to_string(); }
    if data.len() >= 2 && data[0] == 0x1F && data[1] == 0x8B { return "gzip".to_string(); }
    if data.len() >= 3 && &data[0..3] == b"BZh" { return "bzip2".to_string(); }
    if data.len() >= 6 && data[0..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00] { return "xz".to_string(); }
    if data.len() >= 4 && &data[0..4] == b"hsqs" { return "squashfs".to_string(); }
    if data.len() >= 262 && &data[257..262] == b"ustar" { return "tar".to_string(); }
    if data.len() >= 4 && data[0] == 0x7F && &data[1..4] == b"ELF" { return "elf".to_string(); }
    if data.len() >= 2 && data[0] == 0x4D && data[1] == 0x5A { return "pe".to_string(); }
    if data.len() >= 4 && &data[0..4] == b"PK\x03\x04" { return "zip".to_string(); }
    if data.len() >= 8 && &data[0..8] == b"\x89PNG\r\n\x1a\n" { return "png".to_string(); }
    if data.len() >= 3 && &data[0..3] == b"\xFF\xD8\xFF" { return "jpeg".to_string(); }
    if data.len() >= 6 && (&data[0..6] == b"GIF87a" || &data[0..6] == b"GIF89a") { return "gif".to_string(); }
    if data.len() >= 4 && &data[0..4] == b"%PDF" { return "pdf".to_string(); }
    if data.starts_with(b"-----BEGIN") { return "pem".to_string(); }
    "data".to_string()
}

fn decompress_bzip2(data: &[u8]) -> Result<Vec<u8>, String> {
    use bzip2::read::BzDecoder;
    let mut decoder = BzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)
        .map_err(|e| format!("Bzip2 解压失败: {}", e))?;
    Ok(decompressed)
}

fn decompress_xz(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut cursor = std::io::Cursor::new(data);
    let mut decompressed = Vec::new();
    lzma_rs::xz_decompress(&mut cursor, &mut decompressed)
        .map_err(|e| format!("XZ 解压失败: {}", e))?;
    Ok(decompressed)
}

fn decompress_lzma(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut cursor = std::io::Cursor::new(data);
    let mut decompressed = Vec::new();
    lzma_rs::lzma_decompress(&mut cursor, &mut decompressed)
        .map_err(|e| format!("LZMA 解压失败: {}", e))?;
    Ok(decompressed)
}

fn extract_tar(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    use std::io::Read;
    let mut archive = tar::Archive::new(data);
    let mut files = Vec::new();
    for entry in archive.entries().map_err(|e| format!("Tar 解析失败: {}", e))? {
        let mut entry = entry.map_err(|e| format!("Tar 条目读取失败: {}", e))?;
        let path = entry.path().map_err(|e| format!("Tar 路径读取失败: {}", e))?.to_string_lossy().to_string();
        let mut file_data = Vec::new();
        entry.read_to_end(&mut file_data).map_err(|e| format!("Tar 数据读取失败: {}", e))?;
        if !file_data.is_empty() {
            files.push((path, file_data));
        }
    }
    Ok(files)
}

fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| format!("Gzip 解压失败: {}", e))?;
    Ok(decompressed)
}

/// Custom compressor that adds LZMA (compression type 2) support to backhand
#[derive(Copy, Clone)]
struct LzmaSupportCompressor;

impl CompressionAction for LzmaSupportCompressor {
    fn decompress(
        &self,
        bytes: &[u8],
        out: &mut Vec<u8>,
        compressor: Compressor,
    ) -> Result<(), BackhandError> {
        match compressor {
            Compressor::Lzma => {
                if bytes.len() < 4 {
                    return Err(BackhandError::UnsupportedCompression(compressor));
                }

                // SquashFS LZMA blocks can have many formats. Systematically try all possibilities.

                // Helper: try lzma-rs with a 13-byte LZMA Alone header prepended
                let try_lzma_rs = |data: &[u8], props_byte: u8, dict_size: u32| -> Result<Vec<u8>, ()> {
                    let mut header = vec![
                        props_byte,
                        (dict_size & 0xFF) as u8,
                        ((dict_size >> 8) & 0xFF) as u8,
                        ((dict_size >> 16) & 0xFF) as u8,
                        ((dict_size >> 24) & 0xFF) as u8,
                        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // unknown uncompressed size
                    ];
                    header.extend_from_slice(data);
                    let mut cursor = std::io::Cursor::new(&header);
                    let mut decoded = Vec::new();
                    lzma_rs::lzma_decompress(&mut cursor, &mut decoded).map(|_| decoded).map_err(|_| ())
                };

                // Helper: try oxiarc_lzma raw decompress
                let try_oxiarc_raw = |data: &[u8], props: oxiarc_lzma::LzmaProperties, ds: u32| -> Result<Vec<u8>, ()> {
                    oxiarc_lzma::decompress_raw(std::io::Cursor::new(data), props, ds, None).map_err(|_| ())
                };

                // Try with bytes[0..5] as [props + dict_size LE], data at bytes[5..]
                let try_5byte_header = |data: &[u8]| -> Result<Vec<u8>, ()> {
                    if data.len() <= 5 { return Err(()); }
                    let props_byte = data[0];
                    let dict_size = u32::from_le_bytes([data[1], data[2], data[3], data[4]]).max(4096);
                    for ds in &[dict_size, 0x80000, 0x40000, 0x20000, 0x100000, 0x10000] {
                        if let Ok(r) = try_lzma_rs(&data[5..], props_byte, *ds) {
                            return Ok(r);
                        }
                    }
                    // Try oxiarc as well
                    if let Some(px) = oxiarc_lzma::LzmaProperties::from_byte(props_byte) {
                        for ds in &[dict_size, 0x80000, 0x40000, 0x20000, 0x10000] {
                            if let Ok(r) = try_oxiarc_raw(&data[5..], px, *ds) {
                                return Ok(r);
                            }
                        }
                    }
                    Err(())
                };

                let result = 'formats: {
                    // Format 1: [props:1 + dict_size:4 LE] at offset 0, data at offset 5
                    if bytes.len() > 6 {
                        if let Ok(r) = try_5byte_header(bytes) {
                            break 'formats Ok(r);
                        }
                    }

                    // Format 2: skip 4 bytes, then same as Format 1 at offset 4
                    if bytes.len() > 10 {
                        if let Ok(r) = try_5byte_header(&bytes[4..]) {
                            break 'formats Ok(r);
                        }
                    }

                    // Format 3: skip 4 bytes, bytes[4..17] = full 13-byte LZMA Alone header, data at offset 17
                    if bytes.len() > 18 {
                        let header = &bytes[4..17];
                        let props_byte = header[0];
                        let dict_size = u32::from_le_bytes([header[1], header[2], header[3], header[4]]).max(4096);
                        for ds in &[dict_size, 0x80000, 0x40000, 0x20000, 0x100000] {
                            if let Ok(r) = try_lzma_rs(&bytes[17..], props_byte, *ds) {
                                break 'formats Ok(r);
                            }
                        }
                    }

                    // Format 4: bytes[0..13] = full 13-byte LZMA Alone header, data at offset 13
                    if bytes.len() > 14 {
                        let props_byte = bytes[0];
                        let dict_size = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]).max(4096);
                        for ds in &[dict_size, 0x80000, 0x40000, 0x20000, 0x100000] {
                            if let Ok(r) = try_lzma_rs(&bytes[13..], props_byte, *ds) {
                                break 'formats Ok(r);
                            }
                        }
                    }

                    // Format 5: Broadcom 3-byte dict_size in bytes[1..4], data at offset 4
                    if bytes.len() > 5 {
                        let prop_byte = bytes[0];
                        if let Some(_p) = oxiarc_lzma::LzmaProperties::from_byte(prop_byte) {
                            let mut ds_buf = [0u8; 4];
                            ds_buf[0] = bytes[1]; ds_buf[1] = bytes[2]; ds_buf[2] = bytes[3];
                            let dict_size = u32::from_le_bytes(ds_buf).max(4096);
                            for ds in &[dict_size, 0x80000, 0x40000, 0x20000, 0x100000, 0x10000] {
                                if let Ok(r) = try_lzma_rs(&bytes[4..], prop_byte, *ds) {
                                    break 'formats Ok(r);
                                }
                            }
                        }
                    }

                    // Format 6: default props + various dict sizes, entire input is raw LZMA stream
                    let default_props: &[(u8, u32)] = &[
                        (0x5D, 0x100000), (0x5D, 0x80000), (0x5D, 0x40000), (0x5D, 0x20000),
                        (0x6D, 0x800000), (0x6D, 0x80000), (0x6D, 0x40000),
                    ];
                    for &(props_byte, ds) in default_props {
                        if let Ok(r) = try_lzma_rs(bytes, props_byte, ds) {
                            break 'formats Ok(r);
                        }
                    }

                    break 'formats Err(());
                };

                match result {
                    Ok(decompressed) => {
                        out.extend_from_slice(&decompressed);
                        Ok(())
                    }
                    Err(()) => {
                        Err(BackhandError::UnsupportedCompression(compressor))
                    }
                }
            }
            _ => DefaultCompressor.decompress(bytes, out, compressor),
        }
    }

    fn compress(
        &self,
        bytes: &[u8],
        fc: FilesystemCompressor,
        block_size: u32,
    ) -> Result<Vec<u8>, BackhandError> {
        DefaultCompressor.compress(bytes, fc, block_size)
    }

    fn compression_options(
        &self,
        superblock: &mut SuperBlock,
        kind: &Kind,
        fs_compressor: FilesystemCompressor,
    ) -> Result<Vec<u8>, BackhandError> {
        DefaultCompressor.compression_options(superblock, kind, fs_compressor)
    }
}

/// Recursively unpack a SquashFS filesystem in memory
fn unpack_squashfs(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    static LZMA_COMPRESSOR: LzmaSupportCompressor = LzmaSupportCompressor;
    let custom_kind = Kind::new(&LZMA_COMPRESSOR);

    let cursor = Cursor::new(data);
    let reader = BufReader::new(cursor);
    let squashfs = Squashfs::from_reader_with_offset_and_kind(reader, 0, custom_kind)
        .map_err(|e| format!("SquashFS 解析失败: {:?}", e))?;
    let fs = squashfs
        .into_filesystem_reader()
        .map_err(|e| format!("SquashFS 文件系统构建失败: {:?}", e))?;

    let mut files = Vec::new();
    for node in fs.files() {
        let path = node.fullpath.to_string_lossy().to_string();
        if path == "/" {
            continue;
        }
        if let InnerNode::File(file_reader) = &node.inner {
            let mut file_handle = fs.file(&file_reader.basic).reader();
            let mut file_data = Vec::new();
            file_handle
                .read_to_end(&mut file_data)
                .map_err(|e| format!("读取文件数据失败 ({}): {}", path, e))?;
            files.push((path.trim_start_matches('/').to_string(), file_data));
        }
    }
    Ok(files)
}

/// Scan all files inside a SquashFS and add findings from recursive scan
fn scan_squashfs_contents(
    squashfs_data: &[u8],
    layer: u32,
    parent_offset: Option<u64>,
    _parent_name: Option<String>,
    source: &str,
    results: &mut Vec<DeepScanItem>,
    seen_offsets: &mut HashSet<(u32, u64)>,
) {
    let files = match unpack_squashfs(squashfs_data) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("WARN: squashfs unpack failed for layer {}: {}", layer, e);
            return;
        }
    };

    for (file_path, file_data) in &files {
        if file_data.is_empty() {
            continue;
        }

        // 使用 binwalk 完整扫描每个文件，而不是简陋的 detect_file_type
        let binwalker = Binwalk::new();
        let scan_results = binwalker.scan(file_data);
        
        if scan_results.is_empty() {
            // 如果 binwalk 没有检测到任何内容，使用 detect_file_type 作为后备
            let (detected_name, confidence) = detect_file_type(file_data);
            if detected_name != "data" {
                results.push(DeepScanItem {
                    layer: layer + 1,
                    offset: 0,
                    size: file_data.len() as u64,
                    name: format!("squashfs_inner/{}", file_path),
                    description: format!("Extracted from SquashFS: {} (detected: {})", file_path, detected_name),
                    confidence,
                    parent_offset,
                    parent_name: Some(format!("squashfs/{}", file_path)),
                    source: source.to_string(),
                });
            }
        } else {
            // 将 binwalk 扫描结果添加到结果中
            for result in scan_results {
                results.push(DeepScanItem {
                    layer: layer + 1,
                    offset: result.offset as u64,
                    size: result.size as u64,
                    name: format!("squashfs_inner/{}/{}", file_path, result.name),
                    description: result.description.to_string(),
                    confidence: result.confidence as i32,
                    parent_offset,
                    parent_name: Some(format!("squashfs/{}", file_path)),
                    source: source.to_string(),
                });
            }
        }

        // Recursively scan each file's contents for embedded signatures
        recursive_scan(
            file_data,
            layer + 1,
            parent_offset,
            Some(format!("squashfs/{}", file_path)),
            source.to_string(),
            results,
            seen_offsets,
        );
    }
}

/// Simple file type detection based on magic bytes
fn detect_file_type(data: &[u8]) -> (String, i32) {
    if data.len() < 4 {
        return ("data".to_string(), 0);
    }

    // ELF
    if data.len() >= 4 && data[0] == 0x7F && data[1] == b'E' && data[2] == b'L' && data[3] == b'F' {
        let elf_class = if data[4] == 1 { "32-bit" } else if data[4] == 2 { "64-bit" } else { "unknown" };
        return (format!("ELF {} executable", elf_class), 200);
    }

    // ASCII text
    if data.iter().take(256).all(|&b| b.is_ascii_graphic() || b.is_ascii_whitespace()) {
        return ("ASCII text".to_string(), 180);
    }

    // PEM/KEY (RSA private key)
    if data.starts_with(b"-----BEGIN") {
        if data.starts_with(b"-----BEGIN RSA PRIVATE KEY-----") {
            return ("RSA private key".to_string(), 250);
        }
        if data.starts_with(b"-----BEGIN PRIVATE KEY-----") {
            return ("Private key (PKCS#8)".to_string(), 250);
        }
        if data.starts_with(b"-----BEGIN CERTIFICATE-----") {
            return ("Certificate".to_string(), 200);
        }
        return ("PEM data".to_string(), 180);
    }

    // Shell script
    if data.starts_with(b"#!") {
        let end = data.iter().position(|&b| b == b'\n').unwrap_or(64);
        let shebang = String::from_utf8_lossy(&data[..end.min(64)]);
        return (format!("script: {}", shebang), 180);
    }

    // SquashFS
    if data.len() >= 4 && &data[0..4] == b"hsqs" {
        return ("SquashFS filesystem".to_string(), 250);
    }

    // gzip
    if data.len() >= 2 && data[0] == 0x1F && data[1] == 0x8B {
        return ("gzip compressed data".to_string(), 200);
    }

    // LZMA
    if data.len() >= 3 && data[0] == 0x5D {
        return ("LZMA compressed data".to_string(), 180);
    }

    ("data".to_string(), 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_squashfs_extraction() {
        let firmware_path = r"C:\Users\admin\PycharmProjects\BinWalker\firmware_samples\DLink_DIR815\DIR815A1_FW104b04_20191217_beta01.bin";
        let data = std::fs::read(firmware_path).unwrap();
        
        let binwalker = Binwalk::new();
        let scan_results = binwalker.scan(&data);
        
        for result in &scan_results {
            if result.name.to_lowercase().contains("squashfs") {
                let start = result.offset as usize;
                let end = start + result.size as usize;
                let squashfs_data = &data[start..end];
                eprintln!("Found squashfs at offset {}, size {}", result.offset, result.size);
                
                match unpack_squashfs(squashfs_data) {
                    Ok(files) => {
                        eprintln!("SUCCESS: extracted {} files", files.len());
                        for (path, fdata) in &files {
                            eprintln!("  File: {} ({} bytes)", path, fdata.len());
                        }
                    }
                    Err(e) => {
                        eprintln!("FAILED: {}", e);
                        panic!("unpack_squashfs failed: {}", e);
                    }
                }
                return;
            }
        }
        panic!("No squashfs found in firmware!");
    }
}
