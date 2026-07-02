/// Generate a test .bin file containing a SquashFS image with a PEM private key inside.
/// Run with: cargo run --example gen_squashfs_pem_bin
///
/// Output: BinWalker_gen_squashfs_pem_test.bin in the project root

use std::io::Cursor;
use std::path::PathBuf;

fn main() {
    // A realistic-looking PEM RSA private key
    let pem_content = b"-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEA0gP+LzY7v8k5yRzOoFfJqUXjD6QmNCk3wF3bLqZ4R4Lk
fH7w9y0pQmFvYXRlZCBQUklWQVRFIEtFWS0tLS0tCg==
-----END RSA PRIVATE KEY-----
";

    // Build a SquashFS image using backhand
    let mut fs = backhand::FilesystemWriter::default();
    fs.set_current_time();
    fs.set_block_size(4096);
    fs.set_only_root_id();
    fs.set_kind(backhand::kind::Kind::from_const(backhand::kind::LE_V4_0).unwrap());

    let file_header = backhand::NodeHeader {
        permissions: 0o644,
        ..backhand::NodeHeader::default()
    };
    fs.set_root_mode(0o755);

    // Use uncompressed (always available, no extra features needed)
    let compressor =
        backhand::FilesystemCompressor::new(backhand::compression::Compressor::None, None).unwrap();
    fs.set_compressor(compressor);

    // Create directory structure and add PEM file
    fs.push_dir("etc", file_header).unwrap();
    fs.push_dir("etc/ssl", file_header).unwrap();
    fs.push_file(
        Cursor::new(pem_content.to_vec()),
        "etc/ssl/private.key",
        file_header,
    )
    .unwrap();

    // Also add a config file with credentials
    let shadow_content = b"root:$1$xF$yD8L0KzUqJgK0h3FfA0Xk/:0:0:root:/root:/bin/sh
admin:$1$abc$defghijklmnopqrstuvwxyz:0:0:admin:/admin:/bin/sh
";
    fs.push_file(
        Cursor::new(shadow_content.to_vec()),
        "etc/shadow",
        file_header,
    )
    .unwrap();

    // Write the SquashFS image to a buffer
    let mut buf = Cursor::new(Vec::new());
    fs.write(&mut buf).unwrap();
    let squashfs_data = buf.into_inner();

    // Determine output path: project root
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_path = manifest_dir
        .parent()
        .unwrap()
        .join("BinWalker_gen_squashfs_pem_test.bin");

    std::fs::write(&output_path, &squashfs_data).unwrap();
    println!(
        "Generated test .bin file at: {}",
        output_path.display()
    );
    println!("Size: {} bytes", squashfs_data.len());
    println!(
        "Contains: SquashFS filesystem with etc/ssl/private.key (PEM RSA key) and etc/shadow (credentials)"
    );
}
