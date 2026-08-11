//! Command Line Interface for carbide.
//!
//! Provides arguments parsing for transpiling C-style Rust (.carbide) files
//! into standard Rust, with support for direct compilation and cargo subcommand modes.

pub mod lexer;
pub mod ast;
pub mod parser;
pub mod transform;
pub mod emitter;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// CLI structure for the Carbide transpiler.
#[derive(Parser, Debug)]
#[command(name = "carbide")]
#[command(author, version, about = "C-style Rust transpiler", long_about = None)]
pub struct Cli {
    /// The subcommand to execute (e.g., cargo subcommand routing).
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Positional input .carbide file.
    #[arg(value_name = "FILE")]
    pub input: Option<PathBuf>,

    /// Target output path for the generated Rust file.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Run rustc programmatically on the transpiled output.
    #[arg(short, long)]
    pub compile: bool,

    /// Target crate type passed to rustc (e.g. bin, cdylib, staticlib, rlib, dylib, lib).
    #[arg(long = "crate-type", value_name = "TYPE")]
    pub crate_type: Option<String>,

    /// Compile as a dynamic library / DLL (cdylib, implies -c).
    #[arg(long, aliases = ["cdylib", "dylib"])]
    pub dll: bool,

    /// Compile as a static library archive (staticlib, implies -c).
    #[arg(long = "static", aliases = ["staticlib"])]
    pub staticlib: bool,

    /// Compile as a standalone binary executable (bin, implies -c).
    #[arg(long, aliases = ["bin"])]
    pub exe: bool,

    /// Compile as a Rust library (rlib/lib, implies -c).
    #[arg(long = "lib", aliases = ["rlib"])]
    pub rlib: bool,

    /// Emit `#![no_std]` at the top of generated Rust files (default is standard library mode).
    #[arg(long = "no-std", conflicts_with = "std")]
    pub no_std: bool,

    /// Target standard library mode (default, omits `#![no_std]`).
    #[arg(long = "std", conflicts_with = "no_std")]
    pub std: bool,
}

/// Available subcommands for carbide.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Carbide cargo subcommand driver.
    Carbide {
        /// Arguments passed to the underlying cargo/build driver.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    
    if cli.command.is_none() {
        let input = match &cli.input {
            Some(path) => path,
            None => {
                eprintln!("Error: No input file specified.");
                std::process::exit(1);
            }
        };

        if input.extension().map_or(true, |ext| ext != "carbide") {
            eprintln!("Error: Input file must have a .carbide extension.");
            std::process::exit(1);
        }
        
        println!("Transpiling: {:?}", input);
        
        let content = match std::fs::read_to_string(input) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error: Failed to read input file: {}", e);
                std::process::exit(1);
            }
        };

        let tokens = match lexer::Lexer::new(&content).tokenize_with_positions() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Lexer Error: {}", e);
                std::process::exit(1);
            }
        };

        let mut program = match parser::Parser::new(&content, tokens).parse_program() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Parser Error: {}", e);
                std::process::exit(1);
            }
        };

        transform::transform_program(&mut program);

        let mut emitter = emitter::Emitter::with_no_std(cli.no_std);
        emitter.emit_program(&program);
        let generated_code = emitter.finish();

        let effective_crate_type = if let Some(ct) = &cli.crate_type {
            Some(ct.as_str())
        } else if cli.dll {
            Some("cdylib")
        } else if cli.staticlib {
            Some("staticlib")
        } else if cli.exe {
            Some("bin")
        } else if cli.rlib {
            Some("lib")
        } else {
            None
        };

        let should_compile = cli.compile || effective_crate_type.is_some();

        let (rs_output_path, bin_output_path) = match &cli.output {
            Some(out) => {
                let ext = out.extension().and_then(|e| e.to_str()).unwrap_or("");
                let is_binary_ext = matches!(ext.to_lowercase().as_str(), "dll" | "lib" | "a" | "so" | "dylib" | "exe");
                if should_compile && (is_binary_ext || (effective_crate_type.is_some() && ext != "rs")) {
                    let mut rs_path = out.clone();
                    rs_path.set_extension("rs");
                    (rs_path, Some(out.clone()))
                } else {
                    (out.clone(), None)
                }
            }
            None => {
                let mut out = input.clone();
                out.set_extension("rs");
                (out, None)
            }
        };

        if let Err(e) = std::fs::write(&rs_output_path, generated_code) {
            eprintln!("Error: Failed to write output file: {}", e);
            std::process::exit(1);
        }

        println!("Successfully transpiled: {:?} -> {:?}", input, rs_output_path);

        if should_compile {
            println!("Invoking rustc to compile: {:?}", rs_output_path);
            let mut cmd = std::process::Command::new("rustc");
            cmd.arg("--edition=2021");
            cmd.arg(&rs_output_path);

            let deps_dir = std::path::Path::new("target").join("debug").join("deps");
            if deps_dir.exists() {
                cmd.arg("-L").arg(format!("dependency={}", deps_dir.display()));
                if let Ok(entries) = std::fs::read_dir(&deps_dir) {
                    for entry in entries.flatten() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        if fname.starts_with("liblibc-") && fname.ends_with(".rlib") {
                            cmd.arg(format!("--extern=libc={}", entry.path().display()));
                            break;
                        }
                    }
                }
            }

            if let Some(crate_type) = effective_crate_type {
                cmd.arg(format!("--crate-type={}", crate_type));
            }

            if let Some(bin_out) = &bin_output_path {
                cmd.arg("-o");
                cmd.arg(bin_out);
            }

            let status = cmd.status();
            match status {
                Ok(s) if s.success() => {
                    if let Some(bin_out) = &bin_output_path {
                        println!("Compilation successful -> {:?}", bin_out);
                    } else {
                        println!("Compilation successful.");
                    }
                }
                Ok(s) => {
                    eprintln!("rustc exited with non-zero status: {:?}", s);
                    std::process::exit(s.code().unwrap_or(1));
                }
                Err(e) => {
                    eprintln!("Failed to execute rustc: {}", e);
                    std::process::exit(1);
                }
            }
        }
    } else {
        let args = match &cli.command {
            Some(Commands::Carbide { args }) => args,
            _ => {
                eprintln!("Error: Invalid cargo subcommand arguments.");
                std::process::exit(1);
            }
        };

        let no_std_flag = args.iter().any(|a| a == "--no-std");
        println!("Carbide Cargo subcommand starting. Args: {:?}", args);
        
        let current_dir = std::env::current_dir().expect("Failed to get current directory");
        let cargo_toml_path = current_dir.join("Cargo.toml");
        if !cargo_toml_path.exists() {
            eprintln!("Error: Could not find Cargo.toml in the current directory.");
            std::process::exit(1);
        }

        let target_dir = current_dir.join("target");
        let workspace_dir = target_dir.join("carbide_workspace");
        let workspace_src_dir = workspace_dir.join("src");

        std::fs::create_dir_all(&workspace_src_dir).expect("Failed to create workspace directory");

        let mut cargo_toml_content = std::fs::read_to_string(&cargo_toml_path)
            .expect("Failed to read Cargo.toml");
        
        // Ensure staticlib/cdylib compilation target configuration is present
        if !cargo_toml_content.contains("[lib]") {
            cargo_toml_content.push_str("\n[lib]\ncrate-type = [\"staticlib\", \"cdylib\"]\n");
        }

        std::fs::write(workspace_dir.join("Cargo.toml"), cargo_toml_content)
            .expect("Failed to write temporary Cargo.toml");

        let cargo_lock = current_dir.join("Cargo.lock");
        if cargo_lock.exists() {
            let _ = std::fs::copy(&cargo_lock, workspace_dir.join("Cargo.lock"));
        }

        let src_dir = current_dir.join("src");
        if src_dir.exists() {
            for entry in std::fs::read_dir(src_dir).expect("Failed to read src directory") {
                let entry = entry.expect("Failed to read directory entry");
                let path = entry.path();
                if path.is_file() {
                    let file_name = path.file_name().unwrap();
                    let dest_path = workspace_src_dir.join(file_name);
                    
                    if path.extension().map_or(false, |ext| ext == "carbide") {
                        let mut rs_dest = dest_path.clone();
                        rs_dest.set_extension("rs");
                        
                        println!("Transpiling: {:?} -> {:?}", path, rs_dest);
                        let content = std::fs::read_to_string(&path).expect("Failed to read carbide file");
                        let tokens = lexer::Lexer::new(&content).tokenize_with_positions().unwrap_or_else(|e| {
                            eprintln!("Lexer Error in {:?}: {}", path, e);
                            std::process::exit(1);
                        });
                        let mut program = parser::Parser::new(&content, tokens).parse_program().unwrap_or_else(|e| {
                            eprintln!("Parser Error in {:?}: {}", path, e);
                            std::process::exit(1);
                        });
                        transform::transform_program(&mut program);
                        let mut emitter = emitter::Emitter::with_no_std(no_std_flag);
                        emitter.emit_program(&program);
                        let generated = emitter.finish();
                        
                        std::fs::write(&rs_dest, generated).expect("Failed to write rs file");
                    } else {
                        std::fs::copy(&path, &dest_path).expect("Failed to copy file to workspace");
                    }
                }
            }
        }

        println!("Invoking real cargo build on the workspace...");
        
        let mut cargo_args = vec![
            "build".to_string(),
            "--manifest-path".to_string(),
            workspace_dir.join("Cargo.toml").to_str().unwrap().to_string()
        ];
        
        // Append optional flags from the subcommand arguments
        for arg in args {
            if arg != "build" && arg != "--no-std" && arg != "--std" {
                cargo_args.push(arg.clone());
            }
        }

        let status = std::process::Command::new("cargo")
            .args(&cargo_args)
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("Cargo command completed successfully.");
            }
            Ok(s) => {
                eprintln!("Cargo command failed with exit code: {:?}", s.code());
                std::process::exit(s.code().unwrap_or(1));
            }
            Err(e) => {
                eprintln!("Failed to execute cargo command: {}", e);
                std::process::exit(1);
            }
        }

        let workspace_target_debug = workspace_dir.join("target").join("debug");
        let original_target_debug = target_dir.join("debug");
        let _ = std::fs::create_dir_all(&original_target_debug);

        if workspace_target_debug.exists() {
            for entry in std::fs::read_dir(workspace_target_debug).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().map(|e| e.to_str().unwrap().to_lowercase());
                    let is_artifact = match ext.as_deref() {
                        Some("dll" | "lib" | "a" | "so" | "dylib" | "exe" | "rlib") => true,
                        None => {
                            // On Unix, executables often have no extension
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if let Ok(metadata) = path.metadata() {
                                    metadata.permissions().mode() & 0o111 != 0
                                } else {
                                    false
                                }
                            }
                            #[cfg(not(unix))]
                            false
                        }
                        _ => false,
                    };
                    if is_artifact {
                        let dest = original_target_debug.join(path.file_name().unwrap());
                        let _ = std::fs::copy(&path, &dest);
                    }
                }
            }
        }
    }
}
