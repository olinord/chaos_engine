use shaderc;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // Resolve the workspace `target/<profile>/` directory that the runtime
    // binary will live in.
    let executable_path = locate_target_dir_from_output_dir(&out_dir)
        .expect("failed to find target dir")
        .join(env::var("PROFILE").unwrap());

    // The engine's UI system looks up shaders under an alias registered by the
    // client. This example registers "internal_shaders" pointing here.
    let shaders_out = Path::new(&executable_path).join("res/internal_shaders/");
    let shaders_out_string = shaders_out.display().to_string();

    let engine_root = manifest_dir
        .parent() // examples/
        .and_then(|p| p.parent()) // repo root
        .expect("failed to locate engine root");
    let shaders_in = engine_root.join("res/internal/shaders");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", shaders_in.display());

    let compiler = shaderc::Compiler::new().unwrap();
    let options = shaderc::CompileOptions::new().unwrap();

    println!("Building UI shaders from {} to {}", shaders_in.display(), &shaders_out_string);

    if !shaders_out.exists() {
        fs::create_dir_all(&shaders_out_string).unwrap();
    }

    for entry in fs::read_dir(&shaders_in)? {
        let entry = entry?;

        if entry.file_type()?.is_file() {
            let in_path = entry.path();

            let shader_type =
                in_path
                    .extension()
                    .and_then(|ext| match ext.to_string_lossy().as_ref() {
                        "vert" => Some(shaderc::ShaderKind::Vertex),
                        "frag" => Some(shaderc::ShaderKind::Fragment),
                        _ => None,
                    });

            if let Some(shader_type) = shader_type {
                println!("Compiling {}", in_path.to_string_lossy());

                let source = fs::read_to_string(&in_path)?;

                let binary_result = compiler
                    .compile_into_spirv(
                        source.as_str(),
                        shader_type,
                        in_path.file_name().unwrap().to_str().unwrap(),
                        "main",
                        Some(&options),
                    )
                    .unwrap();

                if binary_result.get_num_warnings() > 0 {
                    println!(
                        "Warning compiling {}",
                        in_path.file_name().unwrap().to_string_lossy()
                    );
                    println!("{}", binary_result.get_warning_messages());
                }

                let out_path = format!(
                    "{}{}",
                    &shaders_out_string,
                    in_path.file_name().unwrap().to_string_lossy()
                );

                fs::write(&out_path, &binary_result.as_binary_u8())?;
            }
        }
    }

    Ok(())
}

fn locate_target_dir_from_output_dir(mut target_dir_search: &Path) -> Option<&Path> {
    loop {
        if target_dir_search.ends_with("target") {
            return Some(target_dir_search);
        }
        target_dir_search = match target_dir_search.parent() {
            Some(path) => path,
            None => break,
        }
    }
    None
}
