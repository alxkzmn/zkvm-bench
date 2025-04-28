// Most of the code borrowed from powdr/src/lib.rs

use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use powdr::{
    number::{KnownField, Mersenne31Field},
    riscv::{self, CompilerOptions, RuntimeLibs},
    Pipeline,
};

fn pil_file_path(asm_name: &Path) -> PathBuf {
    let file_stem = asm_name.file_stem().unwrap().to_str().unwrap();
    let opt_file_stem = format!("{file_stem}_opt");
    asm_name.with_file_name(opt_file_stem).with_extension("pil")
}

pub fn prepare_pipeline() -> powdr::Pipeline<Mersenne31Field> {
    let out_path = Path::new("powdr-target");

    // Build the guest program and get the assembly code
    let (asm_file_path, asm_contents) = build_guest("./guest", out_path, 5, 18, RuntimeLibs::new());

    // Create a pipeline from the asm program
    let mut pipeline = Pipeline::<Mersenne31Field>::default()
        .from_asm_string(asm_contents.clone(), Some(asm_file_path.clone()))
        .with_backend(powdr::backend::BackendType::Stwo, None)
        .with_output(out_path.into(), true);

    let asm_name = pipeline.asm_string().unwrap().0.clone().unwrap();
    let pil_file = pil_file_path(&asm_name);

    let generate_artifacts = if let Ok(existing_pil) = fs::read_to_string(&pil_file) {
        let computed_pil = pipeline.compute_optimized_pil().unwrap().to_string();
        if existing_pil != computed_pil {
            log::info!("Compiled PIL changed, invalidating artifacts...");
            true
        } else {
            log::info!("Compiled PIL did not change, will try to reuse artifacts...");
            false
        }
    } else {
        log::info!("PIL file not found, will generate artifacts...");
        true
    };

    let out_path = Path::new("powdr-target");
    let pkey = out_path.join("pkey.bin");
    let vkey = out_path.join("vkey.bin");

    if generate_artifacts {
        println!("Creating program ZK setup. This has to be done only once per program.");
        pipeline.compute_fixed_cols().unwrap();
        pipeline.setup_backend().unwrap();
        export_setup(&mut pipeline);
        pipeline.set_pkey_file(pkey.clone());
        pipeline.set_vkey_file(vkey.clone());
    } else {
        println!("Loading program ZK setup.");
        if pipeline.read_constants_mut(out_path).is_ok() {
            println!("Read constants from file...");
        } else {
            pipeline.compute_fixed_cols().unwrap();
        }

        if pkey.exists() && vkey.exists() {
            println!("Re-using proving and verification keys...");
            pipeline.set_pkey_file(pkey.clone());
            pipeline.set_vkey_file(vkey.clone());
            pipeline.setup_backend().unwrap();
        } else {
            println!("Exporting setup...");
            export_setup(&mut pipeline);
            pipeline.set_pkey_file(pkey.clone());
            pipeline.set_vkey_file(vkey.clone());
        }
    }

    pipeline
}

pub fn build_guest(
    guest_path: &str,
    out_path: &Path,
    min_degree_log: u8,
    max_degree_log: u8,
    precompiles: RuntimeLibs,
) -> (PathBuf, String) {
    let options = CompilerOptions::new(KnownField::Mersenne31Field, precompiles, false)
        .with_min_degree_log(min_degree_log)
        .with_max_degree_log(max_degree_log);
    riscv::compile_rust(guest_path, options, out_path, true, None)
        .ok_or_else(|| vec!["could not compile rust".to_string()])
        .unwrap()
}

fn export_setup<F: powdr::FieldElement>(pipeline: &mut powdr::Pipeline<F>) {
    let mut path = PathBuf::from("powdr-target");
    path.push("pkey.bin");
    let file = File::create(path).unwrap();

    pipeline.export_proving_key(file).unwrap();

    let mut path = PathBuf::from("powdr-target");
    path.push("vkey.bin");
    let file = File::create(path).unwrap();

    pipeline.export_verification_key(file).unwrap();
}

pub fn prove<F: powdr::FieldElement>(pipeline: &mut powdr::Pipeline<F>) {
    let bootloader_inputs =
        riscv::continuations::rust_continuations_dry_run(&mut pipeline.clone(), None);

    let generate_proof = |pipeline: &mut Pipeline<F>| -> Result<(), Vec<String>> {
        pipeline.compute_witness()?;
        let proof = pipeline.compute_proof().unwrap();
        //println!("Proof size: {} MB", proof.len() as f64 / 1024.0 / 1024.0);
        Ok(())
    };

    pipeline.rollback_from_witness();

    riscv::continuations::rust_continuations(pipeline, generate_proof, bootloader_inputs).unwrap();
}

pub fn verify<F: powdr::FieldElement>(mut pipeline: powdr::Pipeline<F>) {
    let proof = pipeline.proof().unwrap().clone();
    let publics: Vec<F> = pipeline
        .publics()
        .unwrap()
        .iter()
        .map(|(_name, v)| v.expect("all publics should be known since we created a proof"))
        .collect();
    pipeline.verify(&proof, &[publics]).unwrap();
}
