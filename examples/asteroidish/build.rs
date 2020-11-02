use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use shaderc;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Tell the build script to only run again if we change our source shaders
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=res/shaders");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // locate executable path even if the project is in workspace

    let executable_path = locate_target_dir_from_output_dir(&out_dir)
        .expect("failed to find target dir")
        .join(env::var("PROFILE").unwrap());

    let shaders_path = Path::new(&executable_path).join("res/shaders/");
    let shaders_path_string = shaders_path.display().to_string();

    let mut compiler = shaderc::Compiler::new().unwrap();
    let options = shaderc::CompileOptions::new().unwrap();
    // options.add_macro_definition("EP", Some("main"));

    println!("Building Shaders to {}", &shaders_path_string);

    if !shaders_path.exists() {
        fs::create_dir_all(&shaders_path_string).unwrap();
    }

    for entry in std::fs::read_dir(Path::new( &manifest_dir).join("res/shaders"))? {
        let entry = entry?;

        if entry.file_type()?.is_file() {
            let in_path = entry.path();

            // Support only vertex and fragment shaders currently
            let shader_type = in_path.extension().and_then(|ext| {
                match ext.to_string_lossy().as_ref() {
                    "vert" => Some(shaderc::ShaderKind::Vertex),
                    "frag" => Some(shaderc::ShaderKind::Fragment),
                    _ => None,
                }
            });

            if let Some(shader_type) = shader_type {
                println!("Compiling {}", in_path.to_string_lossy());

                let source = std::fs::read_to_string(&in_path)?;

                let binary_result = compiler.compile_into_spirv(
                    source.as_str(), shader_type,
                    in_path.file_name().unwrap().to_str().unwrap(), "main", Some(&options)).unwrap();

                if binary_result.get_num_warnings() > 0 {
                    println!("Warning compiling {}", in_path.file_name().unwrap().to_string_lossy());
                    println!("{}", binary_result.get_warning_messages());
                }

                // Determine the output path based on the input name
                let out_path = format!(
                    "{}{}.spv",
                    &shaders_path_string,
                    in_path.file_name().unwrap().to_string_lossy()
                );

                std::fs::write(&out_path, &binary_result.as_binary_u8())?;
            }
        }

    }

    Ok(())
}


fn locate_target_dir_from_output_dir(mut target_dir_search: &Path) -> Option<&Path> {
    loop {
        // if path ends with "target", we assume this is correct dir
        if target_dir_search.ends_with("target") {
            return Some(target_dir_search);
        }

        // otherwise, keep going up in tree until we find "target" dir
        target_dir_search = match target_dir_search.parent() {
            Some(path) => path,
            None => break,
        }
    }

    None
}
