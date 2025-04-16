//! An end-to-end example of using the SP1 SDK to generate a proof of a program that can be executed
//! or have a core proof generated.
//!
//! You can run this script using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release -- --execute
//! ```
//! or
//! ```shell
//! RUST_LOG=info cargo run --release -- --prove
//! ```
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

use clap::Parser;
use memory_stats::memory_stats;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};

use jemalloc_ctl::{
    epoch,
    stats::{self},
};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const SHA_ELF: &[u8] = include_elf!("sha-program");

/// The arguments for the command.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long)]
    execute: bool,

    #[clap(long)]
    prove: bool,

    #[clap(long, default_value = "20")]
    n: u32,
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();

    // Parse the command line arguments.
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // Setup the prover client.
    let client = ProverClient::new();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();
    stdin.write(&args.n);

    if args.execute {
        // Execute the program
        let (output, report) = client.execute(SHA_ELF, &stdin).run().unwrap();
        println!("Program executed successfully.");

        println!("Number of cycles: {}", report.total_instruction_count());
    } else {
        epoch::advance().unwrap();
        let allocated_before = stats::allocated::read().unwrap();
        let resident_before = stats::resident::read().unwrap();

        // Setup the program for proving.
        let usage_before = memory_stats().unwrap();
        let (pk, vk) = client.setup(SHA_ELF);
        let usage_after = memory_stats().unwrap();
        println!(
            "memory_stats: Setup memory usage: {} MB resident | {} MB virt",
            (usage_after.physical_mem - usage_before.physical_mem) as f32 / (1024.0 * 1024.0),
            (usage_after.virtual_mem - usage_before.virtual_mem) as f32 / (1024.0 * 1024.0)
        );
        epoch::advance().unwrap();
        let allocated_after = stats::allocated::read().unwrap();
        let resident_after = stats::resident::read().unwrap();
        println!(
            "jemalloc: Setup memory usage: {} MB resident | {} MB allocated",
            (resident_after - resident_before) as f32 / 1024.0 / 1024.0,
            (allocated_after - allocated_before) as f32 / 1024.0 / 1024.0,
        );

        println!("ELF size: {} KB", pk.elf.len() as f32 / 1024.0);
        let pk_bytes = bincode::serialize(&pk).unwrap();
        println!(
            "Proving key size: {} MB",
            pk_bytes.len() as f32 / (1024.0 * 1024.0)
        );

        epoch::advance().unwrap();
        let allocated_before = stats::allocated::read().unwrap();
        let resident_before = stats::resident::read().unwrap();

        let usage_before = memory_stats().unwrap();
        // Generate the proof
        let proof = client
            .prove(&pk, &stdin)
            .run()
            .expect("failed to generate proof");
        let usage_after = memory_stats().unwrap();
        println!(
            "memory_stats: Prover memory usage: {} MB resident | {} MB virt",
            (usage_after.physical_mem - usage_before.physical_mem) as f32 / (1024.0 * 1024.0),
            (usage_after.virtual_mem - usage_before.virtual_mem) as f32 / (1024.0 * 1024.0)
        );
        epoch::advance().unwrap();
        let allocated_after = stats::allocated::read().unwrap();
        let resident_after = stats::resident::read().unwrap();
        println!(
            "jemalloc: Prover memory usage: {} MB resident | {} MB allocated",
            (resident_after - resident_before) as f32 / 1024.0 / 1024.0,
            (allocated_after - allocated_before) as f32 / 1024.0 / 1024.0,
        );

        let proof_bytes = bincode::serialize(&proof).unwrap();
        println!(
            "Proof size: {} MB",
            proof_bytes.len() as f32 / (1024.0 * 1024.0)
        );

        println!("Successfully generated proof!");

        // Verify the proof.
        client.verify(&proof, &vk).expect("failed to verify proof");
        println!("Successfully verified proof!");
    }
}
