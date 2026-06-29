use backhand::{
    compression::{CompressionAction, Compressor, DefaultCompressor},
    kind::Kind,
    BackhandError, FilesystemCompressor, InnerNode, Squashfs, SuperBlock,
};
use binwalk::Binwalk;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::fs;
use std::hash::{Hash, Hasher};
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
pub async fn deep_scan(path: String) -> Result<Vec<DeepScanItem>, String> {
    let file_path = PathBuf::from(&path);

    if !file_path.exists() {
        return Err(format!("文件不存在: {}", path));
    }

    let file_data = fs::read(&file_path)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    let mut results = Vec::new();
    let mut seen_hashes = HashSet::new();

    recursive_scan(&file_data, 0, None, None, "original".to_string(), &mut results, &mut seen_hashes);

    Ok(results)
}

fn hash_data(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

fn recursive_scan(
    data: &[u8],
    layer: u32,
    parent_offset: Option<u64>,
    parent_name: Option<String>,
    source: String,
    results: &mut Vec<DeepScanItem>,
    seen_hashes: &mut HashSet<u64>,
) {
    if data.is_empty() || layer > MAX_DEPTH {
        return;
    }

    // Deduplication: skip data we've already scanned
    let data_hash = hash_data(data);
    if !seen_hashes.insert(data_hash) {
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

        results.push(DeepScanItem {
            layer,
            offset: result.offset as u64,
            size: result.size as u64,
            name: result.name.to_string(),
            description: result.description.to_string(),
            confidence: result.confidence as i32,
            parent_offset,
            parent_name: parent_name.clone(),
            source: source.clone(),
        });

        let extracted = &data[start..end];
        let name_lower = result.name.to_lowercase();

        let child_source = format!("{}_0x{:x}", result.name, result.offset);
        let child_parent_offset = Some(result.offset as u64);
        let child_parent_name = Some(result.name.to_string());

        // Try to decompress gzip content before recursive scan
        if name_lower.contains("gzip") {
            if let Ok(decompressed) = decompress_gzip(extracted) {
                recursive_scan(
                    &decompressed,
                    layer + 1,
                    child_parent_offset,
                    child_parent_name,
                    child_source,
                    results,
                    seen_hashes,
                );
                continue;
            }
        }

        // For SquashFS: unpack the filesystem and scan each inner file
        if name_lower.contains("squashfs") {
            scan_squashfs_contents(
                extracted,
                layer + 1,
                child_parent_offset,
                child_parent_name,
                &child_source,
                results,
                seen_hashes,
            );
            continue;
        }

        // For non-gzip/non-squashfs: scan extracted data directly
        recursive_scan(
            extracted,
            layer + 1,
            child_parent_offset,
            child_parent_name,
            child_source,
            results,
            seen_hashes,
        );
    }
}

#[tauri::command]
pub async fn scan_file(path: String) -> Result<Vec<ScanResult>, String> {
    let file_path = PathBuf::from(&path);
    
    if !file_path.exists() {
        return Err(format!("文件不存在: {}", path));
    }

    let file_data = fs::read(&file_path)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    let binwalker = Binwalk::new();
    let results: Vec<ScanResult> = binwalker
        .scan(&file_data)
        .into_iter()
        .map(|result| ScanResult {
            offset: result.offset as u64,
            size: result.size as u64,
            name: result.name.to_string(),
            description: result.description.to_string(),
            confidence: result.confidence as i32,
        })
        .collect();

    Ok(results)
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

        let (final_name, final_data) = if name_lower.contains("gzip") {
            match decompress_gzip(extracted_data) {
                Ok(decompressed) => (
                    format!("{}_0x{:x}_decompressed.bin", result.name, result.offset),
                    decompressed,
                ),
                Err(_) => (
                    format!("{}_0x{:x}.bin", result.name, result.offset),
                    extracted_data.to_vec(),
                ),
            }
        } else {
            (
                format!("{}_0x{:x}.bin", result.name, result.offset),
                extracted_data.to_vec(),
            )
        };

        let output_file = output_path.join(&final_name);

        if fs::write(&output_file, &final_data).is_ok() {
            extracted_files.push(ExtractedFile {
                name: final_name,
                path: output_file.to_string_lossy().to_string(),
                size: final_data.len() as u64,
                original_offset: result.offset as u64,
                file_type: result.name.clone(),
            });
        }
    }

    Ok(extracted_files)
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
    seen_hashes: &mut HashSet<u64>,
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

        let (detected_name, confidence) = detect_file_type(file_data);
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

        // Recursively scan each file's contents for embedded signatures
        recursive_scan(
            file_data,
            layer + 1,
            parent_offset,
            Some(format!("squashfs/{}", file_path)),
            source.to_string(),
            results,
            seen_hashes,
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
